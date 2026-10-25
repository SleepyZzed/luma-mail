use crate::{
    Result,
    config::{private_dir, private_file},
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    fs,
    path::{Path, PathBuf},
    time::{Duration, SystemTime, UNIX_EPOCH},
};

#[derive(Serialize, Deserialize)]
struct Entry {
    account: String,
    key: String,
    created: u64,
    data: Vec<u8>,
}
struct FileInfo {
    path: PathBuf,
    bytes: u64,
    modified: SystemTime,
}

/// Disposable body cache: no database, indexes, attachment archive or sidecars.
/// Callers serialize all access through a mutex. Files are private and names hashed.
pub struct Cache {
    root: PathBuf,
    budget: u64,
    ttl: Duration,
}
impl Cache {
    pub fn new(root: PathBuf, budget: u64, ttl: Duration) -> Result<Self> {
        let mut cache = Self { root, budget, ttl };
        cache.enforce(0)?;
        Ok(cache)
    }
    pub fn configure(&mut self, budget: u64, ttl: Duration) -> Result<()> {
        self.budget = budget;
        self.ttl = ttl;
        self.enforce(0)
    }
    fn path(&self, account: &str, key: &str) -> PathBuf {
        let hash = Sha256::digest(format!("{account}\0{key}").as_bytes());
        self.root.join(format!("{hash:x}.json"))
    }
    fn files(&self) -> Result<Vec<FileInfo>> {
        if !self.root.exists() {
            return Ok(vec![]);
        }
        if self.root.symlink_metadata()?.file_type().is_symlink() {
            return Err(crate::Error::Invalid(
                "Cache directory must not be a symlink".into(),
            ));
        }
        let mut files = Vec::new();
        for entry in fs::read_dir(&self.root)? {
            let entry = entry?;
            let meta = entry.path().symlink_metadata()?;
            if !meta.is_file() {
                continue;
            }
            files.push(FileInfo {
                path: entry.path(),
                bytes: allocated(&meta),
                modified: meta.modified()?,
            });
        }
        files.sort_by_key(|f| f.modified);
        Ok(files)
    }
    pub fn usage(&self) -> Result<u64> {
        Ok(self.files()?.iter().map(|f| f.bytes).sum::<u64>() + self.overhead())
    }
    fn overhead(&self) -> u64 {
        if self.root.exists() { 8192 } else { 0 }
    }
    fn enforce(&mut self, reserve: u64) -> Result<()> {
        let mut files = self.files()?;
        let now = epoch();
        // Creation time is inside the envelope; modification time records LRU use.
        files.retain(|file| {
            let expired = fs::read(&file.path)
                .ok()
                .and_then(|d| serde_json::from_slice::<Entry>(&d).ok())
                .is_none_or(|e| now.saturating_sub(e.created) >= self.ttl.as_secs());
            if expired {
                let _ = fs::remove_file(&file.path);
            }
            !expired
        });
        let mut used = files.iter().map(|f| f.bytes).sum::<u64>() + self.overhead();
        for file in files {
            if used.saturating_add(reserve) <= self.budget {
                break;
            }
            fs::remove_file(file.path)?;
            used = used.saturating_sub(file.bytes);
        }
        if self.budget < self.overhead() && self.root.exists() {
            // Only our flat cache directory; never recursively follow unknown paths.
            for f in self.files()? {
                fs::remove_file(f.path)?;
            }
            fs::remove_dir(&self.root)?;
        }
        Ok(())
    }
    pub fn put(&mut self, account: &str, key: &str, data: &[u8]) -> Result<bool> {
        if self.budget < 16384 || self.ttl.is_zero() || data.len() > 8 * 1024 * 1024 {
            return Ok(false);
        }
        let entry = serde_json::to_vec(&Entry {
            account: account.into(),
            key: key.into(),
            created: epoch(),
            data: data.into(),
        })?;
        let size = (entry.len() as u64).div_ceil(4096) * 4096 + if cfg!(unix) { 512 } else { 4096 };
        if size + 8192 > self.budget {
            return Ok(false);
        }
        let path = self.path(account, key);
        if path.exists() {
            fs::remove_file(&path)?;
        }
        private_dir(&self.root)?;
        self.enforce(size)?;
        // No temp-file duplication: entries are disposable and invalid files are evicted.
        let mut options = fs::OpenOptions::new();
        options.write(true).create_new(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        let mut file = options.open(&path)?;
        use std::io::Write;
        file.write_all(&entry)?;
        private_file(&path)?;
        self.enforce(0)?;
        Ok(path.exists())
    }
    pub fn get(&mut self, account: &str, key: &str) -> Result<Option<Vec<u8>>> {
        self.enforce(0)?;
        let path = self.path(account, key);
        let Ok(data) = fs::read(&path) else {
            return Ok(None);
        };
        let entry: Entry = serde_json::from_slice(&data)?;
        if entry.account != account || entry.key != key {
            return Ok(None);
        }
        fs::OpenOptions::new()
            .write(true)
            .open(&path)?
            .set_modified(SystemTime::now())?;
        Ok(Some(entry.data))
    }
    pub fn clear_account(&mut self, account: &str) -> Result<()> {
        for f in self.files()? {
            if fs::read(&f.path)
                .ok()
                .and_then(|d| serde_json::from_slice::<Entry>(&d).ok())
                .is_some_and(|e| e.account == account)
            {
                fs::remove_file(f.path)?;
            }
        }
        Ok(())
    }
    pub fn account_usage(&self, account: &str) -> Result<u64> {
        Ok(self
            .files()?
            .into_iter()
            .filter(|f| {
                fs::read(&f.path)
                    .ok()
                    .and_then(|d| serde_json::from_slice::<Entry>(&d).ok())
                    .is_some_and(|e| e.account == account)
            })
            .map(|f| f.bytes)
            .sum())
    }
    pub fn clear(&mut self) -> Result<()> {
        for f in self.files()? {
            fs::remove_file(f.path)?;
        }
        if self.root.exists() {
            fs::remove_dir(&self.root)?;
        }
        Ok(())
    }
    pub fn root(&self) -> &Path {
        &self.root
    }
}
fn epoch() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}
fn allocated(meta: &fs::Metadata) -> u64 {
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        meta.blocks() * 512 + 512
    }
    #[cfg(not(unix))]
    {
        meta.len().div_ceil(4096) * 4096 + 4096
    }
}
