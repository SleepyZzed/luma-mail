//! Explicitly approved, public signature-image copies in the user's own Drive.
//! Callers must confirm link visibility before calling upload_public.
use crate::{
    Error, Result,
    gmail::Gmail,
    http::{bounded_bytes, json},
};
use serde::Deserialize;
use std::{
    io::Cursor,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
};
use tokio::sync::watch;
pub const MAX_PNG: usize = 2 * 1024 * 1024;
#[derive(Clone, Debug)]
pub struct HostedImage {
    pub id: String,
    pub url: String,
}
pub fn prepare(bytes: &[u8]) -> Result<Vec<u8>> {
    if bytes.len() > crate::preview::MAX_PREVIEW_BYTES {
        return Err(Error::ResponseLimit);
    }
    let image = crate::preview::thumbnail(bytes, 768, 576)?;
    let image = image::RgbaImage::from_raw(image.width, image.height, image.rgba)
        .ok_or(Error::ResponseLimit)?;
    let mut png = Cursor::new(Vec::new());
    image
        .write_to(&mut png, image::ImageFormat::Png)
        .map_err(|_| Error::Invalid("Cannot prepare signature picture".into()))?;
    let png = png.into_inner();
    if png.len() > MAX_PNG {
        return Err(Error::ResponseLimit);
    }
    Ok(png)
}
pub async fn upload_public(
    client: &Gmail,
    png: Vec<u8>,
    progress: watch::Sender<(u64, u64)>,
    cancel: Arc<AtomicBool>,
) -> Result<HostedImage> {
    upload_at(client, png, progress, cancel, "https://www.googleapis.com").await
}
async fn upload_at(
    client: &Gmail,
    png: Vec<u8>,
    progress: watch::Sender<(u64, u64)>,
    cancel: Arc<AtomicBool>,
    root: &str,
) -> Result<HostedImage> {
    #[derive(Deserialize)]
    struct FileId {
        id: String,
    }
    #[derive(Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct Link {
        web_content_link: Option<String>,
    }
    if png.len() > MAX_PNG {
        return Err(Error::ResponseLimit);
    }
    if !png.starts_with(b"\x89PNG\r\n\x1a\n") {
        return Err(Error::Invalid(
            "Signature upload requires a prepared PNG picture".into(),
        ));
    }
    let check = || {
        if cancel.load(Ordering::Relaxed) {
            Err(Error::Invalid(
                "Picture upload cancelled. A private uploaded copy may remain in Drive.".into(),
            ))
        } else {
            Ok(())
        }
    };
    check()?;
    let token = client.tokens.access_token().await?;
    let init=client.http.client.post(format!("{root}/upload/drive/v3/files")).query(&[("uploadType","resumable"),("fields","id")]).bearer_auth(&token)
        .header("X-Upload-Content-Type","image/png").header("X-Upload-Content-Length",png.len())
        .json(&serde_json::json!({"name":format!("luma-signature-{}.png",uuid::Uuid::new_v4()),"mimeType":"image/png","appProperties":{"lumaSignatureImage":"1"}})).send().await?;
    if !init.status().is_success() {
        return Err(Error::Api {
            status: init.status().as_u16(),
            reason: "Signature picture upload could not start".into(),
        });
    }
    let location = init
        .headers()
        .get("location")
        .and_then(|h| h.to_str().ok())
        .ok_or_else(|| Error::Invalid("Drive omitted the picture upload location".into()))?;
    let url =
        url::Url::parse(location).map_err(|_| Error::Invalid("Invalid upload location".into()))?;
    let origin =
        url::Url::parse(root).map_err(|_| Error::Invalid("Invalid upload origin".into()))?;
    if url.origin() != origin.origin() {
        return Err(Error::Invalid(
            "Drive returned an untrusted upload location".into(),
        ));
    }
    let size = png.len();
    let mut id = None;
    for (i, chunk) in png.chunks(256 * 1024).enumerate() {
        check()?;
        let offset = i * 256 * 1024;
        let response = client
            .http
            .client
            .put(url.clone())
            .bearer_auth(client.tokens.access_token().await?)
            .header("Content-Length", chunk.len())
            .header(
                "Content-Range",
                format!("bytes {}-{}/{size}", offset, offset + chunk.len() - 1),
            )
            .body(chunk.to_vec())
            .send()
            .await?;
        if response.status().is_success() {
            if offset + chunk.len() != size {
                return Err(Error::Invalid(
                    "Drive confirmed an incomplete picture; nothing was shared".into(),
                ));
            }
            let file: FileId = json(response).await?;
            id = Some(file.id);
        } else if response.status().as_u16() == 308 {
            let expected = format!("bytes=0-{}", offset + chunk.len() - 1);
            if response
                .headers()
                .get("range")
                .and_then(|h| h.to_str().ok())
                != Some(expected.as_str())
            {
                return Err(Error::Invalid(
                    "Drive did not confirm the complete picture chunk; nothing was shared".into(),
                ));
            }
        } else {
            return Err(Error::Api {
                status: response.status().as_u16(),
                reason: "Picture upload interrupted; no picture was inserted".into(),
            });
        }
        let _ = progress.send(((offset + chunk.len()) as u64, size as u64));
    }
    let id = id
        .filter(|id| {
            !id.is_empty()
                && id.len() <= 256
                && id
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-')
        })
        .ok_or_else(|| Error::Invalid("Drive did not confirm the picture upload".into()))?;
    check()?;
    let permission = client
        .http
        .client
        .post(format!("{root}/drive/v3/files/{id}/permissions"))
        .query(&[("fields", "id")])
        .bearer_auth(client.tokens.access_token().await?)
        .json(&serde_json::json!({"type":"anyone","role":"reader","allowFileDiscovery":false}))
        .send()
        .await?;
    if !permission.status().is_success() {
        return Err(Error::Api{status:permission.status().as_u16(),reason:"The image was uploaded privately, but link sharing failed. No picture was inserted. Manage the copy in Google Drive.".into()});
    }
    // Link access is already granted. Do not call cancellation a rollback.
    if cancel.load(Ordering::Relaxed) {
        return Err(Error::Invalid("The picture copy is now public in Drive, but was not inserted. Remove it in Drive if unwanted.".into()));
    }
    let response = client
        .http
        .client
        .get(format!("{root}/drive/v3/files/{id}"))
        .query(&[("fields", "webContentLink")])
        .bearer_auth(client.tokens.access_token().await?)
        .send()
        .await?;
    if !response.status().is_success() {
        return Err(Error::Api{status:response.status().as_u16(),reason:"The public picture was uploaded but its link could not be read. Manage the copy in Drive.".into()});
    }
    let link: Link = serde_json::from_slice(&bounded_bytes(response, 32 * 1024).await?)?;
    let url = crate::signatures::image_url(
        &link
            .web_content_link
            .ok_or_else(|| Error::Invalid("Drive omitted the public picture link".into()))?,
    )?;
    Ok(HostedImage { id, url })
}
#[cfg(test)]
mod tests {
    use super::*;
    #[tokio::test]
    async fn creates_only_an_app_owned_png_then_grants_read_only_link_access() {
        use wiremock::{
            Mock, MockServer, ResponseTemplate,
            matchers::{body_partial_json, method, path},
        };
        let server = MockServer::start().await;
        let client = crate::tests::gmail(&server);
        Mock::given(method("POST")).and(path("/upload/drive/v3/files")).and(body_partial_json(serde_json::json!({"mimeType":"image/png","appProperties":{"lumaSignatureImage":"1"}}))).respond_with(ResponseTemplate::new(200).insert_header("Location",format!("{}/session",server.uri()))).expect(1).mount(&server).await;
        Mock::given(method("PUT"))
            .and(path("/session"))
            .respond_with(
                ResponseTemplate::new(200).set_body_json(serde_json::json!({"id":"picture-1"})),
            )
            .expect(1)
            .mount(&server)
            .await;
        Mock::given(method("POST"))
            .and(path("/drive/v3/files/picture-1/permissions"))
            .and(body_partial_json(
                serde_json::json!({"type":"anyone","role":"reader","allowFileDiscovery":false}),
            ))
            .respond_with(
                ResponseTemplate::new(200).set_body_json(serde_json::json!({"id":"permission-1"})),
            )
            .expect(1)
            .mount(&server)
            .await;
        Mock::given(method("GET"))
            .and(path("/drive/v3/files/picture-1"))
            .respond_with(ResponseTemplate::new(200).set_body_json(
                serde_json::json!({"webContentLink":"https://example.com/picture.png"}),
            ))
            .expect(1)
            .mount(&server)
            .await;
        let (tx, _) = watch::channel((0, 0));
        let file = upload_at(
            &client,
            picture(),
            tx,
            Arc::new(AtomicBool::new(false)),
            &server.uri(),
        )
        .await
        .unwrap();
        assert_eq!(file.id, "picture-1");
        assert_eq!(file.url, "https://example.com/picture.png");
    }
    fn picture() -> Vec<u8> {
        let image = image::RgbaImage::from_pixel(2, 2, image::Rgba([10, 50, 90, 255]));
        let mut bytes = Cursor::new(Vec::new());
        image.write_to(&mut bytes, image::ImageFormat::Png).unwrap();
        prepare(&bytes.into_inner()).unwrap()
    }
    #[tokio::test]
    async fn cancel_and_invalid_images_do_not_start_uploads() {
        let server = wiremock::MockServer::start().await;
        let client = crate::tests::gmail(&server);
        for (bytes, cancelled) in [(picture(), true), (vec![1, 2, 3], false)] {
            let (tx, _) = watch::channel((0, 0));
            assert!(
                upload_at(
                    &client,
                    bytes,
                    tx,
                    Arc::new(AtomicBool::new(cancelled)),
                    &server.uri()
                )
                .await
                .is_err()
            );
        }
        assert!(server.received_requests().await.unwrap().is_empty());
    }
    #[tokio::test]
    async fn untrusted_upload_origin_never_receives_credentials_or_link_permissions() {
        use wiremock::{
            Mock, MockServer, ResponseTemplate,
            matchers::{method, path},
        };
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/upload/drive/v3/files"))
            .respond_with(
                ResponseTemplate::new(200)
                    .insert_header("Location", "https://example.invalid/upload"),
            )
            .expect(1)
            .mount(&server)
            .await;
        let (tx, _) = watch::channel((0, 0));
        let result = upload_at(
            &crate::tests::gmail(&server),
            picture(),
            tx,
            Arc::new(AtomicBool::new(false)),
            &server.uri(),
        )
        .await;
        assert!(result.unwrap_err().to_string().contains("untrusted"));
        assert_eq!(server.received_requests().await.unwrap().len(), 1);
    }
}
