//! Explicit user saves only. No spool/cache; never overwrite an existing file.
use crate::{Error, Result};
use std::{
    path::{Path, PathBuf},
    sync::atomic::{AtomicBool, Ordering},
};
use tokio::io::AsyncWriteExt;

fn filename(name: &str) -> String {
    let name = name.rsplit(['/', '\\']).next().unwrap_or("");
    let name: String = name
        .chars()
        .filter(|c| !c.is_control())
        .map(|c| if "<>:\"|?*".contains(c) { '_' } else { c })
        .scan(0usize, |bytes, c| {
            *bytes += c.len_utf8();
            (*bytes <= 180).then_some(c)
        })
        .collect();
    let name = name.trim_matches([' ', '.']);
    if name.is_empty() {
        return "Attachment".into();
    }
    let stem = name.split('.').next().unwrap_or("").to_ascii_uppercase();
    if [
        "CON", "PRN", "AUX", "NUL", "COM1", "COM2", "COM3", "COM4", "COM5", "COM6", "COM7", "COM8",
        "COM9", "LPT1", "LPT2", "LPT3", "LPT4", "LPT5", "LPT6", "LPT7", "LPT8", "LPT9",
    ]
    .contains(&stem.as_str())
    {
        format!("_{name}")
    } else {
        name.into()
    }
}
pub async fn save_unique(
    dir: &Path,
    name: &str,
    bytes: &[u8],
    cancel: &AtomicBool,
) -> Result<PathBuf> {
    let name = filename(name);
    let stem = Path::new(&name)
        .file_stem()
        .unwrap_or_default()
        .to_string_lossy();
    let extension = Path::new(&name)
        .extension()
        .map(|s| format!(".{}", s.to_string_lossy()))
        .unwrap_or_default();
    for i in 0..10_000 {
        if cancel.load(Ordering::Relaxed) {
            return Err(Error::Invalid("Saving cancelled".into()));
        }
        let path = dir.join(if i == 0 {
            name.clone()
        } else {
            format!("{stem} ({i}){extension}")
        });
        let mut file = match tokio::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&path)
            .await
        {
            Ok(file) => file,
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(error) => return Err(error.into()),
        };
        let result = async {
            for chunk in bytes.chunks(256 * 1024) {
                if cancel.load(Ordering::Relaxed) {
                    return Err(Error::Invalid("Saving cancelled".into()));
                }
                file.write_all(chunk).await?;
            }
            file.flush().await?;
            Ok(())
        }
        .await;
        drop(file);
        if let Err(error) = result {
            let _ = tokio::fs::remove_file(&path).await;
            return Err(error);
        }
        return Ok(path);
    }
    Err(Error::Invalid("Too many files with the same name".into()))
}
#[cfg(test)]
mod tests {
    use super::*;
    #[tokio::test]
    async fn saves_are_explicit_collision_safe_and_cancel_without_new_files() {
        let dir = tempfile::tempdir().unwrap();
        let cancel = AtomicBool::new(false);
        let first = save_unique(dir.path(), "../../report.txt", b"first", &cancel)
            .await
            .unwrap();
        let second = save_unique(dir.path(), "C:\\mail\\report.txt", b"second", &cancel)
            .await
            .unwrap();
        assert_eq!(first, dir.path().join("report.txt"));
        assert_eq!(second, dir.path().join("report (1).txt"));
        assert_eq!(std::fs::read(first).unwrap(), b"first");
        assert_eq!(std::fs::read(second).unwrap(), b"second");
        cancel.store(true, Ordering::Relaxed);
        assert!(
            save_unique(dir.path(), "cancel.txt", b"never", &cancel)
                .await
                .is_err()
        );
        assert!(!dir.path().join("cancel.txt").exists());
        assert_eq!(filename("../..."), "Attachment");
        assert_eq!(filename("CON.txt"), "_CON.txt");
        assert!(filename(&"图".repeat(200)).len() <= 180);
    }
}
