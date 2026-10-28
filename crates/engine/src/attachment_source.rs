//! Resolve only attachments belonging to the selected reader or live composer.
//! Preview reads stay in RAM; explicit user saves use a separately chosen path.
use crate::{app::App, compose::Phase, protocol::Command};
use luma_core::{Error, Result, composer, model::Part, provider::MailProvider};
use std::{
    path::PathBuf,
    sync::{Arc, atomic::Ordering},
};
use tokio::io::AsyncReadExt;
pub struct Attachment {
    pub name: String,
    pub mime: String,
    pub size: u64,
    pub source: Source,
}
pub enum Source {
    File(PathBuf),
    Bytes(Arc<Vec<u8>>),
    Gmail {
        account: String,
        message: String,
        part: Part,
    },
}
impl Source {
    pub fn snapshot_bytes(&self) -> usize {
        match self {
            Self::Gmail { part, .. } => part.body.data.as_ref().map_or(0, |data| data.len()),
            _ => 0,
        }
    }
    pub async fn read(self, app: &App, max: usize) -> Result<Vec<u8>> {
        let bytes = match self {
            Self::File(path) => {
                let file = tokio::fs::File::open(path).await?;
                let metadata = file.metadata().await?;
                if !metadata.is_file() || metadata.len() > max as u64 {
                    return Err(Error::ResponseLimit);
                }
                let mut bytes = Vec::with_capacity(metadata.len() as usize);
                file.take(max as u64 + 1).read_to_end(&mut bytes).await?;
                bytes
            }
            Self::Bytes(bytes) => {
                if bytes.len() > max {
                    return Err(Error::ResponseLimit);
                }
                Arc::try_unwrap(bytes).unwrap_or_else(|b| (*b).clone())
            }
            Self::Gmail {
                account,
                message,
                part,
            } => {
                if part.body.size > max as u64 {
                    return Err(Error::ResponseLimit);
                }
                if let Some(id) = &part.body.attachment_id {
                    app.client(&account)?
                        .attachment_limited(&message, id, max)
                        .await?
                } else {
                    if part
                        .body
                        .data
                        .as_ref()
                        .is_some_and(|d| d.len() > max.div_ceil(3) * 4 + 4)
                    {
                        return Err(Error::ResponseLimit);
                    }
                    composer::decode_part(&part)?
                }
            }
        };
        if bytes.len() > max {
            return Err(Error::ResponseLimit);
        }
        Ok(bytes)
    }
}
pub async fn resolve(app: &Arc<App>, c: &Command) -> Result<Attachment> {
    if !c.text("session").is_empty() {
        return composed(app, c);
    }
    let key = crate::app::key(&c.data)?;
    let reading = app.reading.load(Ordering::SeqCst);
    let current = || {
        let view = app.view.lock().unwrap();
        app.reading.load(Ordering::SeqCst) == reading
            && view
                .selected
                .as_ref()
                .is_some_and(|k| k.account == key.account)
            && view
                .thread
                .as_ref()
                .is_some_and(|t| t.messages.iter().any(|m| m.id == key.id))
    };
    if !current() {
        return Err(Error::Invalid(
            "Reopen this message before accessing its attachments".into(),
        ));
    }
    let resident = app.view.lock().unwrap().bodies.get(&key.id);
    let message = match resident {
        Some(m) => m,
        None => Arc::new(app.client(&key.account)?.message(&key.id).await?),
    };
    if !current() {
        return Err(Error::Invalid("This message was closed".into()));
    }
    let part = message
        .payload
        .attachments()
        .into_iter()
        .find(|p| p.part_id == c.text("part"))
        .ok_or_else(|| Error::Invalid("Attachment no longer exists".into()))?
        .clone();
    Ok(Attachment {
        name: if part.filename.is_empty() {
            "Attachment".into()
        } else {
            part.filename.clone()
        },
        mime: part.mime_type.clone(),
        size: part.body.size,
        source: Source::Gmail {
            account: key.account,
            message: message.id.clone(),
            part,
        },
    })
}
fn composed(app: &App, c: &Command) -> Result<Attachment> {
    let session = crate::compose_commands::session(app, c)?;
    let data = session.data.lock().unwrap();
    if matches!(data.phase, Phase::Sent | Phase::Closed)
        || c.data["generation"].as_u64() != Some(data.attachment_generation)
    {
        return Err(Error::Invalid(
            "This attachment list changed; choose the attachment again".into(),
        ));
    }
    let id = c.text("attachment");
    if let Some(path) = data
        .form
        .attachments
        .iter()
        .find(|p| format!("file:{}", p.to_string_lossy()) == id)
    {
        return Ok(Attachment {
            name: path
                .file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .into(),
            mime: "application/octet-stream".into(),
            size: data.file_sizes.get(path).copied().unwrap_or(0),
            source: Source::File(path.clone()),
        });
    }
    if let Some(index) = id
        .strip_prefix("retained:")
        .and_then(|s| s.parse::<usize>().ok())
        && let Some(file) = data.form.retained.get(index)
    {
        return Ok(Attachment {
            name: file.name.clone(),
            mime: file.mime_type.clone(),
            size: file.data.len() as u64,
            source: Source::Bytes(file.data.clone()),
        });
    }
    Err(Error::Invalid(
        "Preview the original file or open its verified Drive link".into(),
    ))
}
