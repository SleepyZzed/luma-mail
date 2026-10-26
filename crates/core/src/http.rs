use crate::{Error, Result};
use futures_util::StreamExt;
use reqwest::{Client, Method, Response, StatusCode};
use serde::de::DeserializeOwned;
use std::{
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    },
    time::Duration,
};
use tokio::sync::{OwnedSemaphorePermit, Semaphore};

/// Keep the concurrency slot until the body is consumed or dropped, including
/// when a caller cancels decoding. Headers alone do not finish a transfer.
pub struct HttpResponse {
    response: Response,
    permit: Option<OwnedSemaphorePermit>,
}
impl std::ops::Deref for HttpResponse {
    type Target = Response;
    fn deref(&self) -> &Self::Target {
        &self.response
    }
}
impl From<Response> for HttpResponse {
    fn from(response: Response) -> Self {
        Self {
            response,
            permit: None,
        }
    }
}

#[derive(Clone)]
pub struct Http {
    pub client: Client,
    permits: Arc<Semaphore>,
    upload: Option<Arc<UploadProgress>>,
}

/// Bytes yielded to the HTTP transport, followed by a separate server wait.
/// This does not claim the server has committed an upload before its response.
#[derive(Default)]
pub struct UploadProgress {
    sent: AtomicU64,
    total: AtomicU64,
}
impl UploadProgress {
    pub fn snapshot(&self) -> (u64, u64) {
        (
            self.sent.load(Ordering::Relaxed),
            self.total.load(Ordering::Relaxed),
        )
    }
}

impl Http {
    pub fn new() -> Result<Self> {
        let _ = rustls::crypto::ring::default_provider().install_default();
        Ok(Self {
            client: Client::builder()
                .redirect(reqwest::redirect::Policy::none())
                .connect_timeout(Duration::from_secs(15))
                .timeout(Duration::from_secs(60))
                .user_agent(format!("LumaMail/{}", env!("CARGO_PKG_VERSION")))
                .build()?,
            permits: Arc::new(Semaphore::new(4)),
            upload: None,
        })
    }
    pub fn with_upload_progress(&self, progress: Arc<UploadProgress>) -> Self {
        let mut http = self.clone();
        http.upload = Some(progress);
        http
    }
    /// This POST contains only independent GETs; retries cannot replay writes.
    pub(crate) async fn read_batch(
        &self,
        url: &str,
        token: &str,
        boundary: &str,
        body: &str,
    ) -> Result<HttpResponse> {
        for attempt in 0..4 {
            let permit = self
                .permits
                .clone()
                .acquire_owned()
                .await
                .map_err(|_| Error::Invalid("Client closed".into()))?;
            let response = self
                .client
                .post(url)
                .bearer_auth(token)
                .header(
                    reqwest::header::CONTENT_TYPE,
                    format!("multipart/mixed; boundary={boundary}"),
                )
                .body(body.to_owned())
                .send()
                .await?;
            let status = response.status();
            if status.is_success() {
                return Ok(HttpResponse {
                    response,
                    permit: Some(permit),
                });
            }
            if status == StatusCode::UNAUTHORIZED {
                return Err(Error::Reauthenticate);
            }
            if (status == StatusCode::TOO_MANY_REQUESTS || status.is_server_error()) && attempt < 3
            {
                drop(response);
                drop(permit);
                tokio::time::sleep(Duration::from_secs(1 << attempt)).await;
                continue;
            }
            return Err(Error::Api {
                status: status.as_u16(),
                reason: status
                    .canonical_reason()
                    .unwrap_or("Batch read failed")
                    .into(),
            });
        }
        unreachable!()
    }
    /// Only replay reads. A failed send/draft creation is never retried automatically.
    pub async fn request(
        &self,
        method: Method,
        url: &str,
        token: &str,
        query: &[(&str, String)],
        body: Option<&serde_json::Value>,
    ) -> Result<HttpResponse> {
        for attempt in 0..4 {
            let permit = self
                .permits
                .clone()
                .acquire_owned()
                .await
                .map_err(|_| Error::Invalid("Client closed".into()))?;
            let mut request = self
                .client
                .request(method.clone(), url)
                .bearer_auth(token)
                .query(query);
            if let Some(body) = body {
                if let Some(progress) = self.upload.clone() {
                    let bytes = serde_json::to_vec(body)?;
                    let total = bytes.len();
                    progress.sent.store(0, Ordering::Relaxed);
                    progress.total.store(total as u64, Ordering::Relaxed);
                    let stream = futures_util::stream::unfold(
                        (bytes, 0, progress),
                        |(bytes, offset, progress)| async move {
                            if offset >= bytes.len() {
                                return None;
                            }
                            let end = (offset + 16384).min(bytes.len());
                            let chunk = bytes[offset..end].to_vec();
                            progress.sent.store(end as u64, Ordering::Relaxed);
                            Some((Ok::<_, std::io::Error>(chunk), (bytes, end, progress)))
                        },
                    );
                    request = request
                        .header(reqwest::header::CONTENT_TYPE, "application/json")
                        .header(reqwest::header::CONTENT_LENGTH, total)
                        .body(reqwest::Body::wrap_stream(stream));
                } else {
                    request = request.json(body);
                }
            } else if matches!(method, Method::POST | Method::PUT | Method::PATCH) {
                // Gmail trash/untrash have no JSON body. Google's HTTP gateway
                // requires an explicit zero length for these mutation requests.
                request = request.header(reqwest::header::CONTENT_LENGTH, 0);
            }
            let response = request.send().await?;
            let status = response.status();
            if status.is_success() {
                return Ok(HttpResponse {
                    response,
                    permit: Some(permit),
                });
            }
            if status == StatusCode::UNAUTHORIZED {
                return Err(Error::Reauthenticate);
            }
            let wait = response
                .headers()
                .get("retry-after")
                .and_then(|v| v.to_str().ok())
                .and_then(|v| v.parse::<u64>().ok())
                .unwrap_or(1 << attempt)
                .min(30);
            let (quota, reason) = if status == StatusCode::FORBIDDEN {
                let data = bounded_bytes(response, 32 * 1024).await?;
                let value: serde_json::Value = serde_json::from_slice(&data).unwrap_or_default();
                let reasons: Vec<&str> = value
                    .pointer("/error/errors")
                    .and_then(|v| v.as_array())
                    .into_iter()
                    .flatten()
                    .filter_map(|e| e.get("reason").and_then(|v| v.as_str()))
                    .collect();
                let quota = reasons
                    .iter()
                    .any(|r| matches!(*r, "rateLimitExceeded" | "userRateLimitExceeded"));
                // Return only our fixed descriptions. Arbitrary Google error
                // bodies/messages can contain private data and are never shown.
                let reason = reasons
                    .iter()
                    .find_map(|r| match *r {
                        "rateLimitExceeded" | "userRateLimitExceeded" => {
                            Some("Request rate limit reached; wait before refreshing again")
                        }
                        "dailyLimitExceeded" | "quotaExceeded" => {
                            Some("Google API quota reached; retry after the quota resets")
                        }
                        "insufficientPermissions" => {
                            Some("Google permission missing; reconnect this account in Settings")
                        }
                        "accessNotConfigured" => Some(
                            "Google API is not enabled for Luma's project; contact the maintainer",
                        ),
                        "domainPolicy" => {
                            Some("Your Workspace administrator blocks this operation")
                        }
                        "forbidden" => {
                            Some("Google denied this operation for the connected account")
                        }
                        _ => None,
                    })
                    .unwrap_or("Forbidden");
                (quota, reason)
            } else {
                (false, status.canonical_reason().unwrap_or("request failed"))
            };
            let retry = method == Method::GET
                && (quota || status == StatusCode::TOO_MANY_REQUESTS || status.is_server_error());
            if retry && attempt < 3 {
                // Backoff must not occupy a transfer slot or starve other mail.
                drop(permit);
                tokio::time::sleep(Duration::from_secs(wait)).await;
                continue;
            }
            // Never expose arbitrary response bodies (which can contain private mail).
            return Err(Error::Api {
                status: status.as_u16(),
                reason: reason.into(),
            });
        }
        unreachable!()
    }
}

pub async fn bounded_bytes(response: impl Into<HttpResponse>, max: usize) -> Result<Vec<u8>> {
    let HttpResponse {
        response,
        permit: _permit,
    } = response.into();
    if response
        .content_length()
        .is_some_and(|len| len > max as u64)
    {
        return Err(Error::ResponseLimit);
    }
    let mut bytes = Vec::new();
    let mut stream = response.bytes_stream();
    while let Some(chunk) = stream.next().await {
        let chunk = chunk?;
        if bytes.len().saturating_add(chunk.len()) > max {
            return Err(Error::ResponseLimit);
        }
        bytes.extend_from_slice(&chunk);
    }
    Ok(bytes)
}
pub async fn json<T: DeserializeOwned>(response: impl Into<HttpResponse>) -> Result<T> {
    let response = response.into();
    let empty = response.status() == StatusCode::NO_CONTENT;
    let bytes = bounded_bytes(response, 8 * 1024 * 1024).await?;
    // Gmail may return 204 for an empty projected list (for example no matching
    // attachment flags). Required fields still fail deserialization for entities.
    Ok(serde_json::from_slice(if empty && bytes.is_empty() {
        b"{}"
    } else {
        &bytes
    })?)
}
