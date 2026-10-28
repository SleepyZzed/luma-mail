//! Provider-neutral online signature editor. Only its current server baseline is
//! retained in RAM. Typing never writes a preference, draft or signature file.
use crate::{app::App, protocol::Command};
use luma_core::{
    Error, Result,
    signatures::{MAX_HTML, Signature, SignatureStore},
};
use serde_json::json;
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicBool, Ordering},
};
#[derive(Default)]
pub struct Service {
    current: Mutex<Option<Arc<Editor>>>,
    open: tokio::sync::Mutex<()>,
}
struct Editor {
    id: String,
    account: String,
    baseline: Mutex<Signature>,
    operation: tokio::sync::Mutex<()>,
    busy: AtomicBool,
    dirty: AtomicBool,
    closed: AtomicBool,
    cancel: Arc<AtomicBool>,
    cancelled: tokio::sync::Notify,
}
impl Service {
    fn editor(&self, c: &Command) -> Result<Arc<Editor>> {
        self.current
            .lock()
            .unwrap()
            .as_ref()
            .filter(|e| e.id == c.text("session") && !e.closed.load(Ordering::SeqCst))
            .cloned()
            .ok_or_else(|| Error::Invalid("This signature editor has closed".into()))
    }
    pub fn account_active(&self, account: &str) -> bool {
        self.current
            .lock()
            .unwrap()
            .as_ref()
            .is_some_and(|e| e.account == account && !e.closed.load(Ordering::SeqCst))
    }
    pub fn input(&self, c: &Command) -> Result<()> {
        let e = self.editor(c)?;
        if c.action == "signatureCancelUpload" {
            e.cancel.store(true, Ordering::SeqCst);
            e.cancelled.notify_waiters();
        } else {
            e.dirty.store(c.flag("dirty"), Ordering::SeqCst);
        }
        Ok(())
    }
}
async fn close(app: &Arc<App>, id: u64, e: &Arc<Editor>) {
    e.closed.store(true, Ordering::SeqCst);
    e.cancel.store(true, Ordering::SeqCst);
    e.cancelled.notify_waiters();
    {
        let mut current = app.signatures.current.lock().unwrap();
        if current.as_ref().is_some_and(|v| Arc::ptr_eq(v, e)) {
            *current = None;
        }
    }
    app.events
        .emit(
            id,
            "signatureClosed",
            json!({"session":e.id,"continueQuit":app.quit_pending.load(Ordering::SeqCst)}),
        )
        .await;
}
async fn question(app: &Arc<App>, id: u64, e: &Arc<Editor>) {
    app.events
        .emit(
            id,
            "signatureCloseQuestion",
            json!({"session":e.id,"quit":app.quit_pending.load(Ordering::SeqCst)}),
        )
        .await;
}
pub async fn prepare_quit(app: &Arc<App>, id: u64) -> Result<bool> {
    let current = app.signatures.current.lock().unwrap().clone();
    let Some(e) = current else { return Ok(false) };
    if e.busy.load(Ordering::SeqCst) {
        app.events.emit(id,"signatureStatus",json!({"session":e.id,"busy":true,"status":"Waiting for the current signature operation before quitting"})).await;
    } else if e.dirty.load(Ordering::SeqCst) {
        question(app, id, &e).await;
    } else {
        close(app, id, &e).await;
    }
    Ok(true)
}
pub async fn run(app: &Arc<App>, c: &Command) -> Result<()> {
    if c.action == "signatureOpen" {
        let _open = app.signatures.open.lock().await;
        let existing = app.signatures.current.lock().unwrap().clone();
        if let Some(existing) = existing {
            app.events
                .emit(c.id, "signatureFocus", json!({"session":existing.id}))
                .await;
            return Ok(());
        }
        let account = app
            .settings
            .lock()
            .unwrap()
            .accounts
            .iter()
            .find(|a| a.id == c.text("account") && a.enabled)
            .cloned()
            .ok_or_else(|| Error::Invalid("Choose an enabled account".into()))?;
        if !app.preview && !account.sender_controls {
            return Err(Error::Invalid(
                "Connect Gmail sender controls in Settings before editing an online signature"
                    .into(),
            ));
        }
        let baseline = if app.preview {
            Signature {
                identity: account.email.clone(),
                html: "<p><strong>Alex Morgan</strong><br>Design team</p>".into(),
                can_update: true,
            }
        } else {
            app.client(&account.id)?.signature(&account.email).await?
        };
        let editor = Arc::new(Editor {
            id: luma_core::composer::new_id(),
            account: account.id,
            baseline: Mutex::new(baseline.clone()),
            operation: Default::default(),
            busy: AtomicBool::new(false),
            dirty: AtomicBool::new(false),
            closed: AtomicBool::new(false),
            cancel: Default::default(),
            cancelled: Default::default(),
        });
        let view = json!({"session":editor.id,"account":editor.account,"identity":baseline.identity,"html":baseline.html,"editable":baseline.can_update});
        *app.signatures.current.lock().unwrap() = Some(editor);
        app.events.emit(c.id, "signatureOpened", view).await;
        return Ok(());
    }
    let e = app.signatures.editor(c)?;
    if c.action == "signatureKeepEditing" {
        if app.quit_pending.swap(false, Ordering::SeqCst) {
            app.events.emit(c.id, "quitCancelled", json!({})).await;
        }
        return Ok(());
    }
    if c.action == "signatureClose" {
        if e.busy.load(Ordering::SeqCst) {
            return Err(Error::Invalid(
                "Wait for the signature request, or cancel its picture upload first".into(),
            ));
        }
        if c.flag("dirty") && !c.flag("discardConfirmed") {
            e.dirty.store(true, Ordering::SeqCst);
            question(app, c.id, &e).await;
        } else {
            close(app, c.id, &e).await;
        }
        return Ok(());
    }
    let _operation = e
        .operation
        .try_lock()
        .map_err(|_| Error::Invalid("Wait for the current signature operation".into()))?;
    if e.closed.load(Ordering::SeqCst) {
        return Ok(());
    }
    e.busy.store(true, Ordering::SeqCst);
    e.cancel.store(false, Ordering::SeqCst);
    app.events.emit(c.id,"signatureStatus",json!({"session":e.id,"busy":true,"operation":c.action,"status":if c.action=="signatureSave"{"Saving the signature to Gmail"}else{"Preparing signature pictures"}})).await;
    let result = operation(app, c, &e).await;
    e.busy.store(false, Ordering::SeqCst);
    app.events.emit(c.id,"signatureStatus",json!({"session":e.id,"busy":false,"status":result.as_ref().err().map(|err|err.to_string()).unwrap_or_default()})).await;
    if result.is_ok() && app.quit_pending.load(Ordering::SeqCst) && !e.closed.load(Ordering::SeqCst)
    {
        if e.dirty.load(Ordering::SeqCst) {
            question(app, c.id, &e).await;
        } else {
            close(app, c.id, &e).await;
        }
    }
    result
}
async fn operation(app: &Arc<App>, c: &Command, e: &Arc<Editor>) -> Result<()> {
    match c.action.as_str() {
        "signatureSave" => {
            let html = luma_core::signatures::clean(c.text("html"))?;
            let expected = e.baseline.lock().unwrap().clone();
            let saved = if app.preview {
                Signature { html, ..expected }
            } else {
                app.client(&e.account)?
                    .save_signature(&expected, &html)
                    .await?
            };
            *e.baseline.lock().unwrap() = saved.clone();
            e.dirty.store(false, Ordering::SeqCst);
            app.events
                .emit(
                    c.id,
                    "signatureSaved",
                    json!({"session":e.id,"html":saved.html,"preview":app.preview}),
                )
                .await;
            if !app.preview
                && app
                    .settings
                    .lock()
                    .unwrap()
                    .accounts
                    .iter()
                    .any(|a| a.id == e.account && !a.signature.is_empty())
            {
                let _preferences = app.preferences.lock().await;
                let mut settings = app.settings.lock().unwrap().clone();
                if let Some(a) = settings.accounts.iter_mut().find(|a| a.id == e.account) {
                    a.signature.clear();
                }
                app.paths.save(&settings)?;
                *app.settings.lock().unwrap() = settings;
            }
            if c.flag("close") {
                close(app, c.id, e).await;
            }
            Ok(())
        }
        "signaturePictures" => {
            let html = c.text("html");
            if html.len() > MAX_HTML {
                return Err(Error::ResponseLimit);
            }
            let urls = sources(html);
            let mut bytes = 0usize;
            let mut loaded = 0;
            for url in urls.into_iter().take(8) {
                if e.cancel.load(Ordering::SeqCst) || e.closed.load(Ordering::SeqCst) {
                    break;
                }
                let data = if app.preview {
                    use base64::Engine;
                    base64::engine::general_purpose::STANDARD.decode("iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR4nGMwrX38HwAFFQKVhpDJdgAAAABJRU5ErkJggg==").map_err(|_|Error::Invalid("Invalid synthetic picture".into()))?
                } else {
                    let cancel = e.cancelled.notified();
                    tokio::pin!(cancel);
                    if e.cancel.load(Ordering::SeqCst) {
                        break;
                    }
                    match tokio::select! {result=luma_core::mail_images::fetch_external(&url)=>result,_=&mut cancel=>break}
                    {
                        Ok(data) => data,
                        Err(_) => continue,
                    }
                };
                let encoded = tokio::task::spawn_blocking(move || {
                    let png = luma_core::signature_images::prepare(&data)?;
                    Ok::<_, Error>(base64::Engine::encode(
                        &base64::engine::general_purpose::STANDARD,
                        png,
                    ))
                })
                .await
                .map_err(|_| Error::Invalid("Cannot prepare signature image".into()))??;
                if bytes + encoded.len() > 8 * 1024 * 1024 {
                    break;
                }
                bytes += encoded.len();
                if e.closed.load(Ordering::SeqCst) {
                    break;
                }
                app.events
                    .emit(
                        c.id,
                        "signaturePicture",
                        json!({"session":e.id,"url":url,"bytes":encoded}),
                    )
                    .await;
                loaded += 1;
            }
            app.events
                .emit(
                    c.id,
                    "signaturePicturesLoaded",
                    json!({"session":e.id,"count":loaded}),
                )
                .await;
            Ok(())
        }
        "signatureUploadPicture" => {
            if !c.flag("publicConfirmed") {
                return Err(Error::Invalid(
                    "Confirm public link visibility before uploading a signature picture".into(),
                ));
            }
            let account = app
                .settings
                .lock()
                .unwrap()
                .accounts
                .iter()
                .find(|a| a.id == e.account)
                .cloned()
                .ok_or_else(|| Error::Invalid("Account no longer exists".into()))?;
            if !account.drive_enabled && !app.preview {
                return Err(Error::Invalid(
                    "Connect Drive before uploading a signature picture".into(),
                ));
            }
            let path = std::path::PathBuf::from(c.text("path"));
            let bytes = tokio::task::spawn_blocking(move || {
                use std::io::Read;
                let mut file = std::fs::File::open(path)?;
                let mut bytes = Vec::new();
                file.by_ref()
                    .take(luma_core::preview::MAX_PREVIEW_BYTES as u64 + 1)
                    .read_to_end(&mut bytes)?;
                luma_core::signature_images::prepare(&bytes)
            })
            .await
            .map_err(|_| Error::Invalid("Cannot prepare signature picture".into()))??;
            let (progress, mut updates) = tokio::sync::watch::channel((0u64, 0u64));
            let hosted = if app.preview {
                luma_core::signature_images::HostedImage {
                    id: "preview-picture".into(),
                    url: "https://example.com/luma-signature.png".into(),
                }
            } else {
                let client = app.client(&e.account)?;
                let future = luma_core::signature_images::upload_public(
                    &client,
                    bytes.clone(),
                    progress,
                    e.cancel.clone(),
                );
                tokio::pin!(future);
                loop {
                    tokio::select! {result=&mut future=>break result?,changed=updates.changed()=>{if changed.is_err(){continue;}let (done,total)=*updates.borrow();app.events.emit(c.id,"signatureProgress",json!({"session":e.id,"done":done,"total":total})).await;}}
                }
            };
            if !e.closed.load(Ordering::SeqCst) {
                e.dirty.store(true, Ordering::SeqCst);
                let encoded =
                    base64::Engine::encode(&base64::engine::general_purpose::STANDARD, bytes);
                app.events.emit(c.id,"signatureUploaded",json!({"session":e.id,"url":hosted.url,"bytes":encoded,"preview":app.preview})).await;
            }
            Ok(())
        }
        _ => Err(Error::Invalid("Unknown signature operation".into())),
    }
}
fn sources(html: &str) -> Vec<String> {
    use base64::Engine;
    let p = luma_core::model::Part {
        mime_type: "text/html".into(),
        body: luma_core::model::PartBody {
            data: Some(
                base64::engine::general_purpose::URL_SAFE_NO_PAD
                    .encode(html)
                    .into(),
            ),
            ..Default::default()
        },
        ..Default::default()
    };
    luma_core::mail_images::sources(&p)
        .into_iter()
        .filter_map(|url| luma_core::signatures::image_url(&url).ok())
        .collect()
}
