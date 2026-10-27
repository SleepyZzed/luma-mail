//! Single explicit preview and acknowledged media reads. No extraction, spool
//! or attachment cache is written. Unclaimed media offers expire after 30s.
use crate::{app::App, attachment_source, protocol::Command};
use base64::{Engine, engine::general_purpose::STANDARD};
use luma_core::{
    Error, Result,
    preview::{self, Kind, Preview},
};
use serde_json::json;
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicBool, AtomicU64, Ordering},
};
const CHUNK: usize = 256 * 1024;
struct Blob {
    token: String,
    bytes: Vec<u8>,
    offset: usize,
    epoch: u64,
}
#[derive(Default)]
struct State {
    active: String,
    blob: Option<Blob>,
    expiry: Option<tokio::task::AbortHandle>,
}
pub struct SaveCancel {
    flag: AtomicBool,
    wake: tokio::sync::Notify,
}
pub struct Service {
    epoch: AtomicU64,
    state: Mutex<State>,
    decoder: Arc<tokio::sync::Semaphore>,
    saves: Mutex<std::collections::HashMap<String, Arc<SaveCancel>>>,
    saves_done: tokio::sync::Notify,
}
impl Default for Service {
    fn default() -> Self {
        Self {
            epoch: AtomicU64::new(0),
            state: Mutex::new(State::default()),
            decoder: Arc::new(tokio::sync::Semaphore::new(1)),
            saves: Mutex::new(std::collections::HashMap::new()),
            saves_done: tokio::sync::Notify::new(),
        }
    }
}
impl Service {
    pub async fn wait_saves(&self) {
        loop {
            let done = self.saves_done.notified();
            tokio::pin!(done);
            done.as_mut().enable();
            if self.saves.lock().unwrap().is_empty() {
                break;
            }
            done.await;
        }
    }
    pub fn begin_save(&self, token: &str) -> Result<Arc<SaveCancel>> {
        let mut saves = self.saves.lock().unwrap();
        if !saves.is_empty() {
            return Err(Error::Invalid(
                "Finish or cancel the current attachment download first".into(),
            ));
        }
        let cancel = Arc::new(SaveCancel {
            flag: AtomicBool::new(false),
            wake: tokio::sync::Notify::new(),
        });
        saves.insert(token.into(), cancel.clone());
        Ok(cancel)
    }
    pub fn cancel_save(&self, token: &str) {
        if let Some(cancel) = self.saves.lock().unwrap().get(token) {
            cancel.flag.store(true, Ordering::SeqCst);
            cancel.wake.notify_one();
        }
    }
    pub fn cancel_saves(&self) {
        for cancel in self.saves.lock().unwrap().values() {
            cancel.flag.store(true, Ordering::SeqCst);
            cancel.wake.notify_one();
        }
    }
    pub fn begin(&self, token: &str) -> u64 {
        let mut state = self.state.lock().unwrap();
        if let Some(timer) = state.expiry.take() {
            timer.abort();
        }
        state.active = token.into();
        state.blob = None;
        self.epoch.fetch_add(1, Ordering::SeqCst) + 1
    }
    pub fn release(&self, token: &str) -> bool {
        let mut state = self.state.lock().unwrap();
        if !token.is_empty() && state.active == token {
            if let Some(timer) = state.expiry.take() {
                timer.abort();
            }
            state.blob = None;
            state.active.clear();
            self.epoch.fetch_add(1, Ordering::SeqCst);
            return true;
        }
        false
    }
    pub fn current(&self, epoch: u64) -> bool {
        self.epoch.load(Ordering::SeqCst) == epoch
    }
    pub async fn read(&self, app: &App, c: &Command) -> Result<()> {
        let (data, done, total) = {
            let mut state = self.state.lock().unwrap();
            let blob = state
                .blob
                .as_mut()
                .filter(|b| b.token == c.text("token") && self.current(b.epoch))
                .ok_or_else(|| {
                    Error::Invalid("Preview expired; open the attachment again".into())
                })?;
            let offset = c.data["offset"]
                .as_u64()
                .and_then(|n| usize::try_from(n).ok())
                .ok_or_else(|| Error::Invalid("Invalid preview offset".into()))?;
            if offset != blob.offset {
                return Err(Error::Invalid(
                    "Preview chunks must be acknowledged in order".into(),
                ));
            }
            let end = (offset + CHUNK).min(blob.bytes.len());
            let data = STANDARD.encode(&blob.bytes[offset..end]);
            blob.offset = end;
            (data, end, blob.bytes.len())
        };
        app.events
            .emit(
                c.id,
                "attachmentChunk",
                json!({"token":c.text("token"),"data":data,"done":done,"total":total}),
            )
            .await;
        Ok(())
    }
}
pub fn token(c: &Command) -> Result<&str> {
    let token = c.text("token");
    if token.is_empty()
        || token.len() > 64
        || !token
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'-')
    {
        return Err(Error::Invalid("Invalid attachment preview token".into()));
    }
    Ok(token)
}
pub async fn save(app: &Arc<App>, c: &Command) -> Result<()> {
    if c.text("directory").is_empty() {
        return Err(Error::Invalid("Choose a destination folder first".into()));
    }
    let dir = std::path::PathBuf::from(c.text("directory"));
    if !tokio::fs::metadata(&dir).await?.is_dir() {
        return Err(Error::Invalid("Choose a destination folder".into()));
    }
    let attachment = attachment_source::resolve(app, c).await?;
    app.status(
        c.id,
        "loading",
        "Saving the attachment to your chosen folder",
    )
    .await;
    let bytes = attachment.source.read(app, 26 * 1024 * 1024).await?;
    let path = luma_core::attachment_save::save_unique(
        &dir,
        &attachment.name,
        &bytes,
        &std::sync::atomic::AtomicBool::new(false),
    )
    .await?;
    app.events
        .emit(
            c.id,
            "attachmentSaved",
            json!({"name":attachment.name,"path":path.to_string_lossy()}),
        )
        .await;
    app.status(c.id, "idle", "Attachment saved").await;
    Ok(())
}
/// Explicit exports are sequential. Cancellation lets the current safe write
/// unwind and remove its partial output rather than aborting a filesystem task.
pub async fn save_all(app: &Arc<App>, c: &Command, cancel: Arc<SaveCancel>) -> Result<()> {
    struct Guard<'a>(&'a Service, String);
    impl Drop for Guard<'_> {
        fn drop(&mut self) {
            self.0.saves.lock().unwrap().remove(&self.1);
            self.0.saves_done.notify_one();
        }
    }
    let token = token(c)?;
    let _guard = Guard(&app.attachment_preview, token.into());
    let sources = c.data["attachments"]
        .as_array()
        .filter(|s| !s.is_empty() && s.len() <= 100)
        .ok_or_else(|| Error::Invalid("Choose between one and 100 attachments".into()))?;
    let dir = std::path::PathBuf::from(c.text("directory"));
    if c.text("directory").is_empty() || !tokio::fs::metadata(&dir).await?.is_dir() {
        return Err(Error::Invalid("Choose a destination folder first".into()));
    }
    let mut saved = 0usize;
    let mut failures = Vec::new();
    // Authorize and capture immutable sources once, before exporting. Navigating
    // away later does not invalidate a download the user already requested.
    // Only inline part data can be copied here; retained inputs use shared Arcs.
    let capture = async {
        let mut files = Vec::with_capacity(sources.len());
        let mut resident = 0usize;
        for source in sources {
            let command = Command {
                id: c.id,
                action: "attachmentSaveAll".into(),
                data: source.clone(),
            };
            let file = attachment_source::resolve(app, &command).await?;
            resident = resident.saturating_add(file.source.snapshot_bytes());
            if resident > 8 * 1024 * 1024 {
                return Err(Error::ResponseLimit);
            }
            files.push(file);
        }
        Result::<_>::Ok(files)
    };
    let files = tokio::select! {biased;
        _=cancel.wake.notified()=>Vec::new(),
        result=capture=>result?,
    };
    for (index, file) in files.into_iter().enumerate() {
        if cancel.flag.load(Ordering::SeqCst) {
            break;
        }
        let read = async {
            app.events
                .emit(
                    c.id,
                    "attachmentSaveProgress",
                    json!({"token":token,"name":file.name,"completed":index,"total":sources.len()}),
                )
                .await;
            let bytes = file.source.read(app, 26 * 1024 * 1024).await?;
            Result::<_>::Ok((file.name, bytes))
        };
        let result = tokio::select! {biased;
            _=cancel.wake.notified()=>Err(Error::Invalid("Saving cancelled".into())),
            result=read=>result,
        };
        let result = match result {
            Ok((name, bytes)) => {
                luma_core::attachment_save::save_unique(&dir, &name, &bytes, &cancel.flag)
                    .await
                    .map(|_| ())
            }
            Err(error) => Err(error),
        };
        match result {
            Ok(()) => saved += 1,
            Err(e) if !cancel.flag.load(Ordering::SeqCst) => failures.push(e.to_string()),
            Err(_) => {}
        }
        if cancel.flag.load(Ordering::SeqCst) {
            break;
        }
        app.events
            .emit(
                c.id,
                "attachmentSaveProgress",
                json!({"token":token,"completed":index+1,"total":sources.len()}),
            )
            .await;
    }
    app.events.emit(c.id,"attachmentSaveFinished",json!({"token":token,"saved":saved,"cancelled":cancel.flag.load(Ordering::SeqCst),"errors":failures})).await;
    Ok(())
}
pub async fn open(app: &Arc<App>, c: &Command, epoch: u64) -> Result<()> {
    let token = token(c)?.to_owned();
    let attachment = attachment_source::resolve(app, c).await?;
    let kind = Kind::of(&attachment.name, &attachment.mime);
    if kind == Kind::Other {
        return Err(Error::Invalid(
            "Save this file to open it in its associated application".into(),
        ));
    }
    if attachment.size > preview::MAX_PREVIEW_BYTES as u64 {
        return Err(Error::Invalid(
            "This file exceeds the 24 MiB RAM preview limit. Save it to open externally.".into(),
        ));
    }
    app.events
        .emit(
            c.id,
            "attachmentLoading",
            json!({"token":token,"name":attachment.name,"kind":kind.label()}),
        )
        .await;
    let bytes = attachment
        .source
        .read(app, preview::MAX_PREVIEW_BYTES)
        .await?;
    if !app.attachment_preview.current(epoch) {
        return Ok(());
    }
    let permit = app
        .attachment_preview
        .decoder
        .clone()
        .acquire_owned()
        .await
        .map_err(|_| Error::Invalid("Preview decoder closed".into()))?;
    if !app.attachment_preview.current(epoch) {
        return Ok(());
    }
    let output = tokio::task::spawn_blocking(move || {
        let _permit = permit;
        preview::create(kind, bytes)
    })
    .await
    .map_err(|_| Error::Invalid("Preview decoding failed".into()))??;
    if !app.attachment_preview.current(epoch) {
        return Ok(());
    }
    let mut data =
        json!({"token":token,"name":attachment.name,"mime":attachment.mime,"kind":kind.label()});
    match output {
        Preview::Image(image) => {
            data["rgba"] = json!(STANDARD.encode(image.rgba));
            data["width"] = json!(image.width);
            data["height"] = json!(image.height);
        }
        Preview::Text(text) => {
            data["text"] = json!(text);
        }
        Preview::Media(bytes) => {
            data["size"] = json!(bytes.len());
            let mut state = app.attachment_preview.state.lock().unwrap();
            if !app.attachment_preview.current(epoch) {
                return Ok(());
            }
            state.blob = Some(Blob {
                token: token.clone(),
                bytes,
                offset: 0,
                epoch,
            });
            drop(state);
            let service = app.attachment_preview.clone();
            let timer = tokio::spawn(async move {
                tokio::time::sleep(std::time::Duration::from_secs(30)).await;
                let mut state = service.state.lock().unwrap();
                if state.blob.as_ref().is_some_and(|b| b.epoch == epoch) {
                    state.blob = None;
                    state.expiry = None;
                }
            });
            let mut state = app.attachment_preview.state.lock().unwrap();
            if app.attachment_preview.current(epoch) {
                state.expiry = Some(timer.abort_handle());
            } else {
                timer.abort();
            }
        }
    }
    app.events.emit(c.id, "attachmentPreview", data).await;
    Ok(())
}
/// Visible cards only. Take the shared decoder permit before reading bytes so
/// cancelled/queued thumbnails cannot accumulate file bodies.
pub async fn thumbnail(app: &Arc<App>, c: &Command) -> Result<()> {
    let token = token(c)?.to_owned();
    let permit = app
        .attachment_preview
        .decoder
        .clone()
        .acquire_owned()
        .await
        .map_err(|_| Error::Invalid("Preview decoder closed".into()))?;
    let attachment = attachment_source::resolve(app, c).await?;
    if Kind::of(&attachment.name, &attachment.mime) != Kind::Image
        || attachment.size > preview::AUTO_IMAGE_BYTES
    {
        return Err(Error::Invalid(
            "This attachment requires an explicit preview".into(),
        ));
    }
    let bytes = attachment
        .source
        .read(app, preview::AUTO_IMAGE_BYTES as usize)
        .await?;
    let image = tokio::task::spawn_blocking(move || {
        let _permit = permit;
        preview::thumbnail(&bytes, 128, 96)
    })
    .await
    .map_err(|_| Error::Invalid("Thumbnail decoding failed".into()))??;
    app.events
        .emit(
            c.id,
            "attachmentThumbnail",
            json!({"token":token,
        "width":image.width,"height":image.height,"rgba":STANDARD.encode(image.rgba)}),
        )
        .await;
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn resetting_a_preview_releases_the_offer_and_invalidates_old_work() {
        let service = Service::default();
        let first = service.begin("a");
        service.state.lock().unwrap().blob = Some(Blob {
            token: "a".into(),
            bytes: vec![0; 1024],
            offset: 0,
            epoch: first,
        });
        assert!(service.current(first));
        let second = service.begin("b");
        assert!(!service.current(first));
        assert!(service.current(second));
        assert!(service.state.lock().unwrap().blob.is_none());
        service.release("a");
        assert!(service.current(second));
        service.release("b");
        assert!(!service.current(second));
    }
}
