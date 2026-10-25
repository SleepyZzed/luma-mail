//! Small, explicit composer state; never a mailbox or address-book crawl.
use crate::{
    Error, Result,
    composer::{Compose, recipients},
    config::{private_dir, private_file},
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    fs,
    io::{Read, Write},
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

pub const MAX_DRAFTS: usize = 8;
pub const MAX_DRAFT_BYTES: usize = 512 * 1024;
pub const DRAFT_BUDGET: u64 = 8 * 1024 * 1024;
const RECIPIENT_BYTES: usize = 64 * 1024;
const MAX_RECIPIENTS: usize = 100;
#[derive(Clone, Serialize, Deserialize)]
pub struct Span {
    pub start: i32,
    pub end: i32,
    pub tag: String,
}
#[derive(Clone, Serialize, Deserialize)]
pub struct ServerDraft {
    pub id: String,
    pub message: String,
    pub thread: String,
}
#[derive(Clone, Serialize, Deserialize)]
pub struct LocalDraft {
    pub id: String,
    pub account: String,
    pub compose: Compose,
    pub spans: Vec<Span>,
    pub server: Option<ServerDraft>,
    pub retained_on_server: bool,
    pub uncertain: bool,
    pub updated: u64,
    pub revision: u64,
    pub sequence: u64,
}
#[derive(Clone, Serialize, Deserialize)]
pub struct Recipient {
    pub account: String,
    pub address: String,
    pub last_sent: u64,
}
#[derive(Clone)]
pub struct Store {
    root: PathBuf,
    writes: std::sync::Arc<std::sync::Mutex<std::collections::HashMap<String, u64>>>,
    drafts_enabled: std::sync::Arc<std::sync::atomic::AtomicBool>,
    recent_enabled: std::sync::Arc<std::sync::atomic::AtomicBool>,
}
fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}
fn bounded<T: serde::de::DeserializeOwned>(path: &Path, cap: usize) -> Result<T> {
    let file = fs::File::open(path)?;
    if file.metadata()?.len() > cap as u64 {
        return Err(Error::ResponseLimit);
    }
    let mut bytes = Vec::new();
    file.take(cap as u64 + 1).read_to_end(&mut bytes)?;
    if bytes.len() > cap {
        return Err(Error::ResponseLimit);
    }
    Ok(serde_json::from_slice(&bytes)?)
}
fn atomic<T: Serialize>(path: &Path, value: &T, cap: usize) -> Result<()> {
    let bytes = serde_json::to_vec(value)?;
    if bytes.len() > cap {
        return Err(Error::Invalid(
            "Local recovery limit reached. Save this draft to Gmail before closing.".into(),
        ));
    }
    private_dir(path.parent().unwrap())?;
    let temp = path.with_extension("tmp");
    let _ = fs::remove_file(&temp); // Our fixed interrupted write, never a user file.
    let mut options = fs::OpenOptions::new();
    options.create_new(true).write(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut file = options.open(&temp)?;
    file.write_all(&bytes)?;
    file.sync_all()?;
    drop(file);
    private_file(&temp)?;
    fs::rename(&temp, path)?;
    Ok(())
}
impl Store {
    pub fn new(config: &Path) -> Self {
        Self {
            root: config.join("composer"),
            writes: Default::default(),
            drafts_enabled: std::sync::Arc::new(std::sync::atomic::AtomicBool::new(true)),
            recent_enabled: std::sync::Arc::new(std::sync::atomic::AtomicBool::new(true)),
        }
    }
    pub fn configure(&self, drafts: bool, recent: bool) {
        self.drafts_enabled
            .store(drafts, std::sync::atomic::Ordering::SeqCst);
        self.recent_enabled
            .store(recent, std::sync::atomic::Ordering::SeqCst);
    }
    fn draft_path(&self, id: &str) -> PathBuf {
        self.root
            .join(format!("{:x}.draft", Sha256::digest(id.as_bytes())))
    }
    fn files(&self) -> Result<Vec<PathBuf>> {
        if !self.root.exists() {
            return Ok(vec![]);
        }
        if self.root.symlink_metadata()?.file_type().is_symlink() {
            return Err(Error::Invalid(
                "Composer directory must not be a symlink".into(),
            ));
        }
        Ok(fs::read_dir(&self.root)?
            .filter_map(|f| f.ok())
            .map(|f| f.path())
            .filter(|p| p.symlink_metadata().is_ok_and(|m| m.is_file()))
            .collect())
    }
    pub fn usage(&self) -> Result<u64> {
        let mut bytes = if self.root.exists() { 8192 } else { 0 };
        for path in self.files()? {
            let m = path.metadata()?;
            #[cfg(unix)]
            {
                use std::os::unix::fs::MetadataExt;
                bytes += m.blocks() * 512 + 512;
            }
            #[cfg(not(unix))]
            {
                bytes += m.len().div_ceil(4096) * 4096 + 4096;
            }
        }
        Ok(bytes)
    }
    pub fn drafts(&self) -> Result<Vec<LocalDraft>> {
        let _guard = self.writes.lock().expect("composer recovery writes");
        self.drafts_unlocked()
    }
    fn drafts_unlocked(&self) -> Result<Vec<LocalDraft>> {
        let mut drafts = Vec::new();
        for path in self.files()? {
            if path.extension().is_some_and(|e| e == "tmp") {
                let _ = fs::remove_file(path);
                continue;
            }
            if path.extension().is_some_and(|e| e == "draft") {
                let d: LocalDraft = bounded(&path, MAX_DRAFT_BYTES)?;
                if drafts.len() >= MAX_DRAFTS {
                    return Err(Error::ResponseLimit);
                }
                drafts.push(d);
            }
        }
        drafts.sort_by_key(|d| std::cmp::Reverse(d.updated));
        Ok(drafts)
    }
    pub fn save(&self, draft: LocalDraft) -> Result<()> {
        self.save_if_open(draft, &std::sync::atomic::AtomicBool::new(false))
    }
    pub fn save_if_open(
        &self,
        mut draft: LocalDraft,
        closed: &std::sync::atomic::AtomicBool,
    ) -> Result<()> {
        let mut writes = self.writes.lock().expect("composer recovery writes");
        if closed.load(std::sync::atomic::Ordering::SeqCst) {
            return Ok(());
        }
        if !self
            .drafts_enabled
            .load(std::sync::atomic::Ordering::SeqCst)
        {
            return Err(Error::Invalid("Local draft recovery is disabled".into()));
        }
        if writes.get(&draft.id).is_some_and(|r| *r > draft.sequence) {
            return Ok(());
        }
        let path = self.draft_path(&draft.id);
        if draft.spans.len() > 4096
            || draft.compose.attachments.len() > 64
            || draft.compose.drive_files.len() > 32
        {
            return Err(Error::ResponseLimit);
        }
        if !path.exists() && self.drafts_unlocked()?.len() >= MAX_DRAFTS {
            return Err(Error::Invalid("Eight local drafts are already saved. Finish or discard one to enable recovery for this message.".into()));
        }
        // Reserve a maximum-sized atomic replacement plus allocation overhead.
        if self.usage()? + MAX_DRAFT_BYTES as u64 + 16384 > DRAFT_BUDGET {
            return Err(Error::ResponseLimit);
        }
        draft.updated = now();
        atomic(&path, &draft, MAX_DRAFT_BYTES)?;
        writes.insert(draft.id.clone(), draft.sequence);
        Ok(())
    }
    pub fn remove(&self, id: &str) -> Result<()> {
        let mut writes = self.writes.lock().expect("composer recovery writes");
        writes.remove(id);
        match fs::remove_file(self.draft_path(id)) {
            Ok(()) => Ok(()),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(e) => Err(e.into()),
        }
    }
    pub fn clear_drafts(&self) -> Result<()> {
        let mut writes = self.writes.lock().expect("composer recovery writes");
        writes.clear();
        for path in self.files()? {
            if path.extension().is_some_and(|s| s == "draft" || s == "tmp") {
                fs::remove_file(path)?;
            }
        }
        Ok(())
    }
    pub fn recent(&self, account: &str, prefix: &str) -> Vec<String> {
        let prefix = prefix.trim().to_lowercase();
        if prefix.is_empty() {
            return vec![];
        }
        let rows: Vec<Recipient> =
            bounded(&self.root.join("recipients.json"), RECIPIENT_BYTES).unwrap_or_default();
        rows.into_iter()
            .filter(|r| {
                r.account == account
                    && now().saturating_sub(r.last_sent) < 90 * 86400
                    && (r.address.to_lowercase().starts_with(&prefix)
                        || r.address
                            .split('<')
                            .nth(1)
                            .is_some_and(|a| a.to_lowercase().starts_with(&prefix)))
            })
            .take(5)
            .map(|r| r.address)
            .collect()
    }
    pub fn recipients(&self) -> Vec<Recipient> {
        bounded(&self.root.join("recipients.json"), RECIPIENT_BYTES).unwrap_or_default()
    }
    pub fn remember(&self, account: &str, c: &Compose) -> Result<()> {
        let _guard = self.writes.lock().expect("composer recovery writes");
        if !self
            .recent_enabled
            .load(std::sync::atomic::Ordering::SeqCst)
        {
            return Ok(());
        }
        let mut rows: Vec<Recipient> =
            bounded(&self.root.join("recipients.json"), RECIPIENT_BYTES).unwrap_or_default();
        rows.retain(|r| now().saturating_sub(r.last_sent) < 90 * 86400);
        for field in [&c.to, &c.cc, &c.bcc] {
            for address in recipients(field)? {
                if address.len() > 320 {
                    continue;
                }
                rows.retain(|r| r.account != account || !r.address.eq_ignore_ascii_case(&address));
                rows.insert(
                    0,
                    Recipient {
                        account: account.into(),
                        address,
                        last_sent: now(),
                    },
                );
            }
        }
        rows.truncate(MAX_RECIPIENTS);
        // Account IDs are bounded too; never let configuration inflate this file.
        while serde_json::to_vec(&rows)?.len() > RECIPIENT_BYTES && !rows.is_empty() {
            rows.pop();
        }
        atomic(&self.root.join("recipients.json"), &rows, RECIPIENT_BYTES)
    }
    pub fn clear_recent(&self) -> Result<()> {
        let _guard = self.writes.lock().expect("composer recovery writes");
        let path = self.root.join("recipients.json");
        if path.exists() {
            fs::remove_file(path)?;
        }
        Ok(())
    }
    pub fn clear_account(&self, account: &str) -> Result<()> {
        for draft in self.drafts()?.into_iter().filter(|d| d.account == account) {
            self.remove(&draft.id)?;
        }
        let mut rows: Vec<Recipient> =
            bounded(&self.root.join("recipients.json"), RECIPIENT_BYTES).unwrap_or_default();
        rows.retain(|r| r.account != account);
        if self.root.join("recipients.json").exists() {
            atomic(&self.root.join("recipients.json"), &rows, RECIPIENT_BYTES)?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn recovery_is_bounded_and_never_serializes_attachment_contents() {
        let root = tempfile::tempdir().unwrap();
        let store = Store::new(root.path());
        let mut c = Compose::new("me@example.com".into());
        c.text = "unsent".into();
        c.retained.push(crate::composer::RetainedAttachment {
            data: b"PRIVATE_ATTACHMENT_BYTES".to_vec(),
            ..Default::default()
        });
        for n in 0..8 {
            store
                .save(LocalDraft {
                    id: n.to_string(),
                    account: "a".into(),
                    compose: c.clone(),
                    spans: vec![],
                    server: None,
                    retained_on_server: true,
                    uncertain: false,
                    updated: 0,
                    revision: 1,
                    sequence: 1,
                })
                .unwrap();
        }
        assert!(
            store
                .save(LocalDraft {
                    id: "extra".into(),
                    account: "a".into(),
                    compose: c.clone(),
                    spans: vec![],
                    server: None,
                    retained_on_server: false,
                    uncertain: false,
                    updated: 0,
                    revision: 1,
                    sequence: 1
                })
                .is_err()
        );
        assert!(
            !fs::read_to_string(store.draft_path("0"))
                .unwrap()
                .contains("PRIVATE_ATTACHMENT")
        );
        assert!(store.usage().unwrap() <= DRAFT_BUDGET);
        let d = store.drafts().unwrap().pop().unwrap();
        assert_eq!(d.compose.text, "unsent");
        assert!(d.compose.retained.is_empty());
        store.clear_account("a").unwrap();
        assert!(store.drafts().unwrap().is_empty());
    }
    #[test]
    fn recent_is_account_scoped_capped_and_prefix_filtered() {
        let root = tempfile::tempdir().unwrap();
        let store = Store::new(root.path());
        let mut c = Compose::default();
        for n in 0..120 {
            c.to = format!("person{n}@example.com");
            store.remember("a", &c).unwrap();
        }
        assert_eq!(
            bounded::<Vec<Recipient>>(&store.root.join("recipients.json"), RECIPIENT_BYTES)
                .unwrap()
                .len(),
            100
        );
        assert!(store.recent("b", "person").is_empty());
        assert!(store.recent("a", "").is_empty());
        assert_eq!(
            store.recent("a", "person119"),
            vec!["person119@example.com"]
        );
        store.clear_recent().unwrap();
        assert!(store.recent("a", "person").is_empty());
    }
}
