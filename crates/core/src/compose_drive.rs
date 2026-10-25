//! Automatic oversized attachments: private resumable upload, Gmail draft links,
//! then recipient-specific sharing only when sending.
use crate::{
    Error, Result,
    composer::{Compose, MAX_ATTACHMENT_BYTES, recipients},
    drive::{self, DriveFile},
    gmail::Gmail,
};
use std::{
    collections::HashMap,
    path::PathBuf,
    sync::{Arc, Mutex, atomic::AtomicBool},
    time::SystemTime,
};
use tokio::sync::watch;

#[derive(Clone, Debug, Hash, PartialEq, Eq)]
pub struct Fingerprint {
    account: String,
    path: PathBuf,
    size: u64,
    modified: Option<SystemTime>,
}
pub type UploadCache = Arc<Mutex<HashMap<Fingerprint, DriveFile>>>;
pub fn cached(cache: &UploadCache, account: &str, path: &std::path::Path) -> Option<DriveFile> {
    let key = fingerprint(account, path).ok()?;
    cache.lock().ok()?.get(&key).cloned()
}
#[derive(Clone, Default)]
pub struct Progress {
    pub name: String,
    pub done: u64,
    pub total: u64,
    pub sharing: bool,
}
pub fn fingerprint(account: &str, path: &std::path::Path) -> Result<Fingerprint> {
    let metadata = std::fs::metadata(path)?;
    if !metadata.is_file() {
        return Err(Error::Invalid("Attachments must be regular files".into()));
    }
    Ok(Fingerprint {
        account: account.into(),
        path: path.into(),
        size: metadata.len(),
        modified: metadata.modified().ok(),
    })
}
/// Move the largest local files to Drive until the ordinary MIME budget fits.
pub fn plan(c: &Compose) -> Result<Vec<PathBuf>> {
    let retained = c.retained.iter().map(|a| a.data.len() as u64).sum::<u64>();
    if retained > MAX_ATTACHMENT_BYTES {
        return Err(Error::Invalid(
            "Existing MIME attachments exceed the editing limit".into(),
        ));
    }
    let mut sizes = c
        .attachments
        .iter()
        .map(|path| fingerprint("", path).map(|f| (path.clone(), f.size)))
        .collect::<Result<Vec<_>>>()?;
    let mut total = sizes
        .iter()
        .fold(retained, |sum, (_, size)| sum.saturating_add(*size));
    sizes.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
    let mut large = Vec::new();
    for (path, size) in sizes {
        if total <= MAX_ATTACHMENT_BYTES {
            break;
        }
        large.push(path);
        total = total.saturating_sub(size);
    }
    Ok(large)
}
pub fn draft_references(header: &str) -> Result<Vec<DriveFile>> {
    use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
    if header.is_empty() {
        return Ok(vec![]);
    }
    if header.len() > 16384 {
        return Err(Error::ResponseLimit);
    }
    let compact: String = header
        .chars()
        .filter(|c| !c.is_ascii_whitespace())
        .collect();
    let files: Vec<DriveFile> = serde_json::from_slice(
        &URL_SAFE_NO_PAD
            .decode(compact)
            .map_err(|_| Error::Invalid("Invalid Drive attachment metadata".into()))?,
    )?;
    if files.len() > 32 {
        return Err(Error::ResponseLimit);
    }
    for file in &files {
        crate::gmail::check_id(&file.id)?;
        drive::verified_link(file)?;
        if file.name.len() > 1024 {
            return Err(Error::ResponseLimit);
        }
    }
    Ok(files)
}
pub fn append_link(c: &mut Compose, file: &DriveFile) -> Result<()> {
    let link = drive::verified_link(file)?;
    if contains_link(c, &link) {
        return Ok(());
    }
    if !c.html {
        c.text = format!(
            "<div style=\"white-space:pre-wrap\">{}</div>",
            escape(&c.text)
        );
        c.html = true;
    }
    c.text.push_str(&format!(
        "<p><a href=\"{}\">{}</a></p>",
        escape(&link),
        escape(&file.name)
    ));
    Ok(())
}
fn escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}
/// Attribute values must be decoded as HTML: Drive query strings often contain
/// '&amp;' after a visual composer save. Raw substring checks duplicate links.
pub fn contains_link(c: &Compose, link: &str) -> bool {
    if !c.html {
        return c.text.contains(link);
    }
    use html5ever::tokenizer::{
        BufferQueue, StartTag, Token, TokenSink, TokenSinkResult, Tokenizer,
    };
    struct Links<'a> {
        target: &'a str,
        found: std::cell::Cell<bool>,
    }
    impl TokenSink for Links<'_> {
        type Handle = ();
        fn process_token(&self, token: Token, _: u64) -> TokenSinkResult<()> {
            if let Token::TagToken(tag) = token
                && tag.kind == StartTag
            {
                if tag.name == html5ever::local_name!("script") {
                    return TokenSinkResult::RawData(
                        html5ever::tokenizer::states::RawKind::ScriptData,
                    );
                }
                if matches!(tag.name.as_ref(), "style" | "textarea" | "title") {
                    return TokenSinkResult::RawData(
                        html5ever::tokenizer::states::RawKind::Rawtext,
                    );
                }
                if tag.name == html5ever::local_name!("a")
                    && tag.attrs.iter().any(|a| {
                        a.name.local == html5ever::local_name!("href")
                            && a.value.as_ref() == self.target
                    })
                {
                    self.found.set(true);
                }
            }
            TokenSinkResult::Continue
        }
    }
    let input = BufferQueue::default();
    input.push_back(c.text.clone().into());
    let tokenizer = Tokenizer::new(
        Links {
            target: link,
            found: std::cell::Cell::new(false),
        },
        Default::default(),
    );
    let _ = tokenizer.feed(&input);
    tokenizer.end();
    tokenizer.sink.found.get()
}
pub async fn prepare(
    mut c: Compose,
    client: &Gmail,
    enabled: bool,
    cache: UploadCache,
    progress: watch::Sender<Progress>,
    cancel: Arc<AtomicBool>,
) -> Result<Compose> {
    let large = plan(&c)?;
    if c.drive_files.len() + large.len() > 32 {
        return Err(Error::Invalid(
            "Use at most 32 Drive attachments in one message".into(),
        ));
    }
    if !large.is_empty() && !enabled {
        return Err(Error::Invalid("Large attachments need Drive permission. Enable Drive for this sending account in Settings; your files are kept in the composer.".into()));
    }
    for path in large {
        let key = fingerprint(&client.account, &path)?;
        let cached = cache.lock().expect("upload cache").get(&key).cloned();
        let file = if let Some(file) = cached {
            file
        } else {
            let name = path
                .file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .into_owned();
            let (tx, mut rx) = watch::channel((0, key.size));
            let upload = drive::upload(client, &path, tx, cancel.clone());
            tokio::pin!(upload);
            let _ = progress.send(Progress {
                name: name.clone(),
                done: 0,
                total: key.size,
                sharing: false,
            });
            let file = loop {
                tokio::select! {
                    result=&mut upload=>break result?,
                    result=rx.changed()=>{if result.is_ok(){let(done,total)=*rx.borrow();let _=progress.send(Progress{name:name.clone(),done,total,sharing:false});}}
                }
            };
            if fingerprint(&client.account, &path)? != key {
                return Err(Error::Invalid("Attachment changed while uploading. The private upload may remain in Drive; choose the file again.".into()));
            }
            cache
                .lock()
                .expect("upload cache")
                .insert(key, file.clone());
            file
        };
        append_link(&mut c, &file)?;
        if !c.drive_files.iter().any(|f| f.id == file.id) {
            c.drive_files.push(file);
        }
        c.attachments.retain(|p| p != &path);
    }
    let _ = progress.send(Progress::default());
    Ok(c)
}
pub async fn share_for_send(
    client: &Gmail,
    c: &Compose,
    progress: &watch::Sender<Progress>,
) -> Result<()> {
    let owner = crate::sender_rules::address(&c.from)?;
    let mut targets = Vec::new();
    for recipient in recipients(&format!("{},{},{}", c.to, c.cc, c.bcc))? {
        let address = crate::sender_rules::address(&recipient)?;
        if address != owner && !targets.contains(&address) {
            targets.push(address);
        }
    }
    for reference in &c.drive_files {
        let link = drive::verified_link(reference)?;
        if !contains_link(c, &link) {
            continue;
        }
        // Gmail draft headers are untrusted. Verify the actual file and private
        // app marker before granting anybody access on a reopened draft.
        let file = drive::existing(client, &reference.id).await?;
        if !file.owned_by_me
            || file
                .app_properties
                .get("lumaMailAttachment")
                .map(String::as_str)
                != Some("1")
        {
            return Err(Error::Invalid("This Drive link is not an app-created attachment owned by this account. Share it explicitly in Drive before sending.".into()));
        }
        let _ = progress.send(Progress {
            name: file.name.clone(),
            sharing: true,
            ..Default::default()
        });
        if !targets.is_empty() {
            drive::share(client, &file, &targets.join(","))
                .await
                .map_err(|e| {
                    Error::Invalid(format!(
                        "{e}. Mail was not sent. Some recipient grants may already exist in Drive."
                    ))
                })?;
        }
    }
    let _ = progress.send(Progress::default());
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn named_drive_links_survive_repeated_saves_without_duplicates() {
        let file = DriveFile {
            id: "file123".into(),
            name: "Project & notes.docx".into(),
            web_view_link: Some("https://drive.google.com/file/d/file123/view?a=1&b=2".into()),
            owned_by_me: true,
            app_properties: Default::default(),
        };
        let mut c = Compose {
            text: "Hello <team>\nNext line".into(),
            ..Default::default()
        };
        append_link(&mut c, &file).unwrap();
        assert!(c.html);
        assert!(c.text.contains("Hello &lt;team&gt;\nNext line"));
        assert!(c.text.contains(">Project &amp; notes.docx</a>"));
        assert!(contains_link(&c, file.web_view_link.as_deref().unwrap()));
        let once = c.text.clone();
        append_link(&mut c, &file).unwrap();
        append_link(&mut c, &file).unwrap();
        assert_eq!(c.text, once);
        assert_eq!(c.text.matches("<a ").count(), 1);
        c.text = format!(
            "<script>let x='<a href=\"{}\">'</script>",
            file.web_view_link.as_deref().unwrap()
        );
        assert!(!contains_link(&c, file.web_view_link.as_deref().unwrap()));
    }
    #[test]
    fn largest_files_move_to_drive_without_reading_or_spooling_contents() {
        let dir = tempfile::tempdir().unwrap();
        let small = dir.path().join("small");
        let large = dir.path().join("large");
        std::fs::File::create(&small)
            .unwrap()
            .set_len(2 * 1024 * 1024)
            .unwrap();
        std::fs::File::create(&large)
            .unwrap()
            .set_len(17 * 1024 * 1024)
            .unwrap();
        let mut c = Compose::new("a@example.com".into());
        c.attachments = vec![small.clone(), large.clone()];
        assert_eq!(plan(&c).unwrap(), vec![large]);
        c.attachments = vec![small];
        assert!(plan(&c).unwrap().is_empty());
    }
}
