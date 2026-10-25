use crate::{Error, Result};
use futures_util::StreamExt;
use reqwest::{Client, Method, Response, StatusCode};
use serde::de::DeserializeOwned;
use std::{sync::Arc, time::Duration};
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
}

impl Http {
    pub fn new() -> Result<Self> {
        let _ = rustls::crypto::ring::default_provider().install_default();
        Ok(Self {
            client: Client::builder()
                .redirect(reqwest::redirect::Policy::none())
                .connect_timeout(Duration::from_secs(15))
                .timeout(Duration::from_secs(60))
                .user_agent("LumaMail/0.1")
                .build()?,
            permits: Arc::new(Semaphore::new(4)),
        })
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
                request = request.json(body);
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
            let quota = if status == StatusCode::FORBIDDEN {
                let data = bounded_bytes(response, 32 * 1024).await?;
                let value: serde_json::Value = serde_json::from_slice(&data).unwrap_or_default();
                value
                    .pointer("/error/errors")
                    .and_then(|v| v.as_array())
                    .is_some_and(|errors| {
                        errors.iter().any(|e| {
                            matches!(
                                e.get("reason").and_then(|v| v.as_str()),
                                Some("rateLimitExceeded" | "userRateLimitExceeded")
                            )
                        })
                    })
            } else {
                false
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
                reason: status.canonical_reason().unwrap_or("request failed").into(),
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
    Ok(serde_json::from_slice(
        &bounded_bytes(response, 8 * 1024 * 1024).await?,
    )?)
}
