//! On-demand saved-contact photos. Never synchronizes a contact/address-book database.
use crate::{
    Error, Result,
    gmail::Gmail,
    http::{bounded_bytes, json},
    preview::Thumbnail,
};
use serde::Deserialize;
use std::{
    sync::Arc,
    time::{Duration, Instant},
};
use tokio::sync::Mutex;

#[derive(Clone)]
pub struct Photos {
    client: Arc<Gmail>,
    warm: Arc<Mutex<Option<Instant>>>,
    endpoint: String,
}
#[derive(Deserialize, Default)]
struct Search {
    #[serde(default)]
    results: Vec<SearchResult>,
}
#[derive(Deserialize)]
struct SearchResult {
    person: Person,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Person {
    #[serde(default)]
    email_addresses: Vec<Address>,
    #[serde(default)]
    photos: Vec<Photo>,
}
#[derive(Deserialize)]
struct Address {
    value: String,
}
#[derive(Deserialize)]
struct Photo {
    url: String,
    #[serde(default, rename = "default")]
    fallback: bool,
}

/// A From header is a mailbox, not necessarily a bare email address.
pub fn sender_email(sender: &str) -> Result<String> {
    if sender.len() > 2048 {
        return Err(Error::Invalid("Sender header is too long".into()));
    }
    crate::sender_rules::address(sender)
}
fn photo_url(search: Search, email: &str) -> Option<url::Url> {
    search
        .results
        .into_iter()
        .filter(|r| {
            r.person
                .email_addresses
                .iter()
                .any(|a| a.value.eq_ignore_ascii_case(email))
        })
        .find_map(|r| person_photo(r.person))
}
fn person_photo(person: Person) -> Option<url::Url> {
    person
        .photos
        .into_iter()
        .filter(|p| !p.fallback)
        .find_map(|p| {
            if p.url.len() > 8192 {
                return None;
            }
            let url = url::Url::parse(&p.url).ok()?;
            let host = url.host_str()?;
            (url.scheme() == "https"
                && url.username().is_empty()
                && url.password().is_none()
                && url.port_or_known_default() == Some(443)
                && host.ends_with(".googleusercontent.com"))
            .then_some(url)
        })
}
impl Photos {
    pub fn new(client: Arc<Gmail>) -> Self {
        Self {
            client,
            warm: Arc::new(Mutex::new(None)),
            endpoint: "https://people.googleapis.com/v1/people:searchContacts".into(),
        }
    }
    #[cfg(test)]
    pub(crate) fn test_endpoint(&mut self, endpoint: String) {
        self.endpoint = endpoint;
    }
    async fn search(&self, query: &str) -> Result<Search> {
        json(
            self.client
                .http
                .request(
                    reqwest::Method::GET,
                    &self.endpoint,
                    &self.client.tokens.access_token().await?,
                    &[
                        ("query", query.into()),
                        ("pageSize", "10".into()),
                        ("readMask", "emailAddresses,photos".into()),
                        ("sources", "READ_SOURCE_TYPE_CONTACT".into()),
                        ("sources", "READ_SOURCE_TYPE_PROFILE".into()),
                        (
                            "fields",
                            "results(person(emailAddresses(value),photos(url,default)))".into(),
                        ),
                    ],
                    None,
                )
                .await?,
        )
        .await
    }
    pub async fn get(&self, email: &str) -> Result<Option<Thumbnail>> {
        let email = sender_email(email)?;
        {
            let mut warm = self.warm.lock().await;
            if warm.is_none_or(|at| at.elapsed() > Duration::from_secs(300)) {
                self.search("").await?;
                // Google's search cache requires a warmup. This never blocks GTK.
                tokio::time::sleep(Duration::from_secs(2)).await;
                *warm = Some(Instant::now());
            }
        }
        let Some(url) = photo_url(self.search(&email).await?, &email) else {
            return Ok(None);
        };
        self.download(url).await.map(Some)
    }
    async fn download(&self, url: url::Url) -> Result<Thumbnail> {
        // Never send a Gmail/People bearer token, cookies or a referrer to the CDN.
        let response = self.client.http.public_get(url.as_str()).await?;
        let bytes = bounded_bytes(response, 2 * 1024 * 1024).await?;
        tokio::task::spawn_blocking(move || crate::preview::thumbnail(&bytes, 96, 96))
            .await
            .map_err(|_| Error::Invalid("Contact photo decode failed".into()))?
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn from_headers_resolve_to_the_same_address_as_google_contacts() {
        for sender in [
            "Person <SENDER@example.com>",
            "\"Last, First\" <sender@example.com>",
            "Zéd <sender@example.com>",
            "sender@example.com",
        ] {
            assert_eq!(sender_email(sender).unwrap(), "sender@example.com");
        }
        assert!(sender_email("a@example.com, b@example.com").is_err());
        assert!(sender_email("not a mailbox").is_err());
        assert!(sender_email("a@example.com\r\nBcc: b@example.com").is_err());
    }
    #[test]
    fn matching_is_exact_and_photos_cannot_send_tokens_to_arbitrary_or_local_hosts() {
        let search = |email: &str, url: &str, fallback: bool| {
            serde_json::from_value(serde_json::json!({"results":[{"person":{"emailAddresses":[{"value":email}],"photos":[{"url":url,"default":fallback}]}}]})).unwrap()
        };
        assert!(
            photo_url(
                search(
                    "A@example.com",
                    "https://lh3.googleusercontent.com/photo",
                    false
                ),
                "a@example.com"
            )
            .is_some()
        );
        assert!(
            photo_url(
                search(
                    "other@example.com",
                    "https://lh3.googleusercontent.com/photo",
                    false
                ),
                "a@example.com"
            )
            .is_none()
        );
        for url in [
            "https://127.0.0.1/photo",
            "http://lh3.googleusercontent.com/p",
            "https://lh3.googleusercontent.com.evil.test/p",
            "https://user:pass@lh3.googleusercontent.com/p",
            "https://lh3.googleusercontent.com:444/p",
            "file:///tmp/photo",
        ] {
            assert!(photo_url(search("a@example.com", url, false), "a@example.com").is_none());
        }
        assert!(
            photo_url(
                search(
                    "a@example.com",
                    "https://lh3.googleusercontent.com/photo",
                    true
                ),
                "a@example.com"
            )
            .is_none()
        );
    }
}
