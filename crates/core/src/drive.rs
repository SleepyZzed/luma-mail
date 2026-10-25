use crate::{
    Error, Result,
    composer::recipients,
    gmail::Gmail,
    http::{bounded_bytes, json},
};
use reqwest::Method;
use serde::Deserialize;
use serde_json::json;
use std::{
    path::Path,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
};
use tokio::{
    io::{AsyncReadExt, AsyncSeekExt},
    sync::watch,
};

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DriveFile {
    pub id: String,
    pub name: String,
    pub web_view_link: Option<String>,
}

pub async fn upload(
    client: &Gmail,
    path: &Path,
    progress: watch::Sender<(u64, u64)>,
    cancel: Arc<AtomicBool>,
) -> Result<DriveFile> {
    let mut file = tokio::fs::File::open(path).await?;
    let meta = file.metadata().await?;
    if !meta.is_file() || meta.len() == 0 {
        return Err(Error::Invalid("Choose a nonempty regular file".into()));
    }
    let size = meta.len();
    let token = client.tokens.access_token().await?;
    let response = client
        .http
        .client
        .post("https://www.googleapis.com/upload/drive/v3/files")
        .query(&[
            ("uploadType", "resumable"),
            ("fields", "id,name,webViewLink"),
        ])
        .bearer_auth(token)
        .header("X-Upload-Content-Type", "application/octet-stream")
        .header("X-Upload-Content-Length", size)
        .json(&json!({"name":path.file_name().unwrap_or_default().to_string_lossy()}))
        .send()
        .await?;
    if !response.status().is_success() {
        return Err(Error::Api {
            status: response.status().as_u16(),
            reason: "Drive upload initialization failed".into(),
        });
    }
    let location = response
        .headers()
        .get("location")
        .and_then(|v| v.to_str().ok())
        .ok_or_else(|| Error::Invalid("Drive omitted the upload session URL".into()))?;
    let url = url::Url::parse(location)
        .map_err(|_| Error::Invalid("Invalid Drive session URL".into()))?;
    if url.scheme() != "https"
        || !url
            .host_str()
            .is_some_and(|h| h == "googleapis.com" || h.ends_with(".googleapis.com"))
    {
        return Err(Error::Invalid("Untrusted Drive upload session URL".into()));
    }
    let mut offset = 0u64;
    let mut failures = 0u8;
    let mut chunk = vec![0u8; 256 * 1024];
    loop {
        if cancel.load(Ordering::Relaxed) {
            return Err(Error::Invalid(
                "Upload cancelled; no link was inserted".into(),
            ));
        }
        file.seek(std::io::SeekFrom::Start(offset)).await?;
        let n = file.read(&mut chunk).await?;
        if n == 0 {
            return Err(Error::Invalid(
                "The attachment changed or upload was interrupted".into(),
            ));
        }
        let response = client
            .http
            .client
            .put(url.clone())
            .bearer_auth(client.tokens.access_token().await?)
            .header("Content-Length", n)
            .header(
                "Content-Range",
                format!("bytes {}-{}/{size}", offset, offset + n as u64 - 1),
            )
            .body(chunk[..n].to_vec())
            .send()
            .await;
        match response {
            Ok(response) if response.status().is_success() => {
                let result = json(response).await?;
                let _ = progress.send((size, size));
                return Ok(result);
            }
            Ok(response) if response.status().as_u16() == 308 => {
                let next = response
                    .headers()
                    .get("range")
                    .and_then(|h| h.to_str().ok())
                    .and_then(|r| r.rsplit('-').next())
                    .and_then(|r| r.parse::<u64>().ok())
                    .map(|r| r + 1)
                    .unwrap_or(0);
                if next <= offset || next > size {
                    return Err(Error::Invalid("Invalid Drive upload offset".into()));
                }
                offset = next;
                failures = 0;
                let _ = progress.send((offset, size));
            }
            _ => {
                failures += 1;
                if failures > 3 {
                    return Err(Error::Invalid(
                        "Drive upload interrupted. No link was inserted; retry the upload.".into(),
                    ));
                }
                tokio::time::sleep(std::time::Duration::from_secs(1 << failures)).await;
                // Query the committed offset before replaying an interrupted chunk.
                let status = client
                    .http
                    .client
                    .put(url.clone())
                    .bearer_auth(client.tokens.access_token().await?)
                    .header("Content-Length", 0)
                    .header("Content-Range", format!("bytes */{size}"))
                    .send()
                    .await?;
                if status.status().is_success() {
                    return json(status).await;
                }
                if status.status().as_u16() != 308 {
                    return Err(Error::Invalid("Drive upload session expired".into()));
                }
                offset = status
                    .headers()
                    .get("range")
                    .and_then(|h| h.to_str().ok())
                    .and_then(|r| r.rsplit('-').next())
                    .and_then(|r| r.parse::<u64>().ok())
                    .map(|r| r + 1)
                    .unwrap_or(0);
                if offset >= size {
                    return Err(Error::Invalid("Invalid Drive upload progress".into()));
                }
            }
        }
    }
}

pub async fn share(client: &Gmail, file: &DriveFile, emails: &str) -> Result<String> {
    crate::gmail::check_id(&file.id)?;
    for email in recipients(emails)? {
        // Drive expects the mailbox itself, without a display name.
        let email = if let Some(start) = email.find('<') {
            email[start + 1..].trim_end_matches('>').to_owned()
        } else {
            email
        };
        let response = client
            .http
            .request(
                Method::POST,
                &format!(
                    "https://www.googleapis.com/drive/v3/files/{}/permissions",
                    file.id
                ),
                &client.tokens.access_token().await?,
                &[("sendNotificationEmail", "false".into())],
                Some(&json!({"type":"user","role":"reader","emailAddress":email})),
            )
            .await?;
        let _ = bounded_bytes(response, 65536).await?;
    }
    file.web_view_link
        .clone()
        .filter(|link| {
            url::Url::parse(link)
                .ok()
                .is_some_and(|u| u.scheme() == "https" && u.host_str() == Some("drive.google.com"))
        })
        .ok_or_else(|| Error::Invalid("Drive did not return a verified file link".into()))
}

pub async fn existing(client: &Gmail, id: &str) -> Result<DriveFile> {
    crate::gmail::check_id(id)?;
    json(
        client
            .http
            .request(
                Method::GET,
                &format!("https://www.googleapis.com/drive/v3/files/{id}"),
                &client.tokens.access_token().await?,
                &[("fields", "id,name,webViewLink".into())],
                None,
            )
            .await?,
    )
    .await
}
