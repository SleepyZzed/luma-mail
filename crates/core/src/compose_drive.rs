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
    if c.text.contains(&link) {
        return Ok(());
    }
    if c.html {
        let escape = |s: &str| {
            s.replace('&', "&amp;")
                .replace('<', "&lt;")
                .replace('>', "&gt;")
                .replace('"', "&quot;")
        };
        c.text.push_str(&format!(
            "<p><a href=\"{}\">{} (Google Drive)</a></p>",
            escape(&link),
            escape(&file.name)
        ));
    } else {
        c.text
            .push_str(&format!("\n\n{} (Google Drive): {link}\n", file.name));
    }
    Ok(())
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
        if !c.text.contains(&link) {
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
