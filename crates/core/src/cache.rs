use crate::{
    Result,
    config::{private_dir, private_file},
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    fs,
    io::{Read, Write},
    path::{Path, PathBuf},
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

#[derive(Serialize, Deserialize)]
struct Entry {
    account: String,
    key: String,
    created: u64,
    length: usize,
}
const MAGIC: &[u8; 8] = b"LUMAC02\n";
const MAX_ENTRIES: usize = 1024;
const MAX_BODY: usize = 8 * 1024 * 1024;
// Only this small header is read during expiry/account scans. Mail bytes stay
// opaque and are read once, for the message the user actually opened.
fn header(path: &Path) -> Result<(fs::File, Entry)> {
    let mut file = fs::File::open(path)?;
    let mut prefix = [0; 12];
    file.read_exact(&mut prefix)?;
    let length = u32::from_le_bytes(prefix[8..12].try_into().unwrap()) as usize;
    if &prefix[..8] != MAGIC || length > 8192 {
        return Err(crate::Error::ResponseLimit);
    }
    let mut bytes = vec![0; length];
    file.read_exact(&mut bytes)?;
    let entry: Entry = serde_json::from_slice(&bytes)?;
    if entry.length > MAX_BODY || file.metadata()?.len() != (12 + length + entry.length) as u64 {
        return Err(crate::Error::ResponseLimit);
    }
    Ok((file, entry))
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
    swept: Option<Instant>,
}
impl Cache {
    pub fn new(root: PathBuf, budget: u64, ttl: Duration) -> Result<Self> {
        let mut cache = Self {
            root,
            budget,
            ttl,
            swept: None,
        };
        cache.enforce(0)?;
        Ok(cache)
    }
    pub fn configure(&mut self, budget: u64, ttl: Duration) -> Result<()> {
        self.budget = budget;
        self.ttl = ttl;
        self.swept = None;
        self.enforce(0)
    }
    fn path(&self, account: &str, key: &str) -> PathBuf {
        let hash = Sha256::digest(format!("{account}\0{key}").as_bytes());
        self.root.join(format!("{hash:x}.body"))
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
        if self
            .swept
            .is_none_or(|last| last.elapsed() >= Duration::from_secs(60))
            || self.ttl.is_zero()
        {
            files.retain(|file| {
                let expired = header(&file.path)
                    .ok()
                    .is_none_or(|(_, e)| now.saturating_sub(e.created) >= self.ttl.as_secs());
                if expired {
                    let _ = fs::remove_file(&file.path);
                }
                !expired
            });
            self.swept = Some(Instant::now());
        }
        let mut used = files.iter().map(|f| f.bytes).sum::<u64>() + self.overhead();
        let mut count = files.len();
        for file in files {
            if used.saturating_add(reserve) <= self.budget
                && count + usize::from(reserve > 0) <= MAX_ENTRIES
            {
                break;
            }
            fs::remove_file(file.path)?;
            used = used.saturating_sub(file.bytes);
            count -= 1;
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
        if self.budget < 16384 || self.ttl.is_zero() || data.len() > MAX_BODY {
            return Ok(false);
        }
        let entry = serde_json::to_vec(&Entry {
            account: account.into(),
            key: key.into(),
            created: epoch(),
            length: data.len(),
        })?;
        if entry.len() > 8192 {
            return Ok(false);
        }
        let size = ((entry.len() + data.len() + 12) as u64).div_ceil(4096) * 4096
            + if cfg!(unix) { 512 } else { 4096 };
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
        file.write_all(MAGIC)?;
        file.write_all(&(entry.len() as u32).to_le_bytes())?;
        file.write_all(&entry)?;
        file.write_all(data)?;
        drop(file);
        private_file(&path)?;
        self.enforce(0)?;
        Ok(path.exists())
    }
    pub fn get(&mut self, account: &str, key: &str) -> Result<Option<Vec<u8>>> {
        if self
            .swept
            .is_none_or(|last| last.elapsed() >= Duration::from_secs(60))
        {
            self.enforce(0)?;
        }
        let path = self.path(account, key);
        let Ok((mut file, entry)) = header(&path) else {
            let _ = fs::remove_file(&path);
            return Ok(None);
        };
        if entry.account != account
            || entry.key != key
            || epoch().saturating_sub(entry.created) >= self.ttl.as_secs()
        {
            drop(file);
            let _ = fs::remove_file(&path);
            return Ok(None);
        }
        let mut data = vec![0; entry.length];
        file.read_exact(&mut data)?;
        drop(file);
        fs::OpenOptions::new()
            .write(true)
            .open(&path)?
            .set_modified(SystemTime::now())?;
        Ok(Some(data))
    }
    pub fn clear_account(&mut self, account: &str) -> Result<()> {
        for f in self.files()? {
            if header(&f.path)
                .ok()
                .is_some_and(|(_, e)| e.account == account)
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
                header(&f.path)
                    .ok()
                    .is_some_and(|(_, e)| e.account == account)
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
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn binary_envelope_avoids_json_byte_array_inflation() {
        let root = tempfile::tempdir().unwrap();
        let mut cache = Cache::new(
            root.path().join("bodies"),
            4 * 1024 * 1024,
            Duration::from_secs(60),
        )
        .unwrap();
        let data = vec![255; 1024 * 1024];
        let started = Instant::now();
        assert!(cache.put("a", "message", &data).unwrap());
        let bytes = cache.path("a", "message").metadata().unwrap().len();
        assert!(bytes < data.len() as u64 + 8192);
        assert_eq!(cache.get("a", "message").unwrap().unwrap(), data);
        assert!(cache.account_usage("a").unwrap() <= cache.usage().unwrap());
        let elapsed = started.elapsed();
        println!(
            "Cache audit: payload={} file={} allocated={} legacy-json={} write/read/scan={:?}",
            data.len(),
            bytes,
            cache.usage().unwrap(),
            serde_json::to_vec(&data).unwrap().len(),
            elapsed
        );
    }
    #[test]
    fn truncated_or_oversized_cache_headers_are_disposable_misses() {
        let root = tempfile::tempdir().unwrap();
        let mut cache = Cache::new(
            root.path().join("bodies"),
            1024 * 1024,
            Duration::from_secs(60),
        )
        .unwrap();
        cache.put("a", "m", b"hello").unwrap();
        let path = cache.path("a", "m");
        fs::write(&path, [MAGIC.as_slice(), &u32::MAX.to_le_bytes()].concat()).unwrap();
        assert!(cache.get("a", "m").unwrap().is_none());
        assert!(!path.exists());
        cache.put("a", "m", b"hello").unwrap();
        fs::OpenOptions::new()
            .write(true)
            .open(&path)
            .unwrap()
            .set_len(16)
            .unwrap();
        assert!(cache.get("a", "m").unwrap().is_none());
        cache.configure(0, Duration::from_secs(60)).unwrap();
        assert!(!cache.root().exists());
    }
}
