use crate::{Error, Result, gmail::Gmail, http::json, model::Message};
use reqwest::Method;
use serde::Deserialize;
use std::collections::{HashSet, VecDeque};

/// Only inspect already-fetched metadata; never fetch a body just for a toast.
/// Ambiguous numbers and non-code contexts deliberately produce no Copy action.
pub fn activation_code(subject: &str, snippet: &str) -> Option<String> {
    let text: String = format!("{subject} {snippet}").chars().take(800).collect();
    let text = text.to_ascii_lowercase();
    let bytes = text.as_bytes();
    let mut result = None;
    let mut at = 0;
    while at < bytes.len() {
        if !bytes[at].is_ascii_digit() {
            at += 1;
            continue;
        }
        let start = at;
        while at < bytes.len() && bytes[at].is_ascii_digit() {
            at += 1;
        }
        if !(4..=8).contains(&(at - start))
            || start > 0 && bytes[start - 1].is_ascii_alphanumeric()
            || at < bytes.len() && bytes[at].is_ascii_alphanumeric()
        {
            continue;
        }
        // Byte slicing is safe at these ASCII boundaries; clamp the context's
        // leading edge to a UTF-8 boundary for international sender text.
        let mut from = start.saturating_sub(64);
        while !text.is_char_boundary(from) {
            from += 1;
        }
        let context = &text[from..start];
        let labelled = context
            .split(|c: char| !c.is_ascii_alphanumeric())
            .any(|word| ["code", "otp", "passcode", "verification", "activation"].contains(&word));
        if !labelled {
            continue;
        }
        let value = text[start..at].to_string();
        if result.as_ref().is_some_and(|old| old != &value) {
            return None;
        }
        result = Some(value);
    }
    result
}

#[cfg(test)]
mod code_tests {
    use super::activation_code;
    #[test]
    fn code_context_and_ambiguity() {
        assert_eq!(
            activation_code("Your verification code", "Use 123456 to sign in"),
            Some("123456".into())
        );
        assert_eq!(
            activation_code("Activation code: 007341", ""),
            Some("007341".into())
        );
        assert_eq!(activation_code("Invoice 123456", "Total 2500"), None);
        assert_eq!(activation_code("Your postcode 123456", ""), None);
        assert_eq!(activation_code("Your code", "123456 or 654321"), None);
        assert_eq!(activation_code("Your code", "ABC123456DEF"), None);
        assert_eq!(activation_code("Your code", "123456789"), None);
        assert_eq!(
            activation_code("Your code", "é🙂🙂🙂🙂🙂🙂🙂🙂🙂🙂🙂🙂🙂🙂🙂🙂 code 123456"),
            Some("123456".into())
        );
    }
}

#[derive(Default)]
pub struct Dedup {
    seen: HashSet<String>,
    order: VecDeque<String>,
}
impl Dedup {
    pub fn fresh(&mut self, id: &str) -> bool {
        if !self.seen.insert(id.into()) {
            return false;
        }
        self.order.push_back(id.into());
        while self.order.len() > 512 {
            if let Some(old) = self.order.pop_front() {
                self.seen.remove(&old);
            }
        }
        true
    }
}
#[derive(Default)]
pub struct Monitor {
    start: Option<String>,
    page: Option<String>,
    offset: usize,
    dedup: Dedup,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Response {
    #[serde(default)]
    history: Vec<Record>,
    next_page_token: Option<String>,
    history_id: String,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Record {
    #[serde(default)]
    messages_added: Vec<Added>,
}
#[derive(Deserialize)]
struct Added {
    message: Message,
}

impl Monitor {
    /// At most 25 message candidates per poll. A history record can contain many
    /// messages: resume within its page instead of silently dropping the tail.
    pub async fn poll(&mut self, client: &Gmail) -> Result<(Vec<Message>, bool)> {
        let Some(start) = &self.start else {
            self.start = Some(client.profile().await?.history_id);
            return Ok((vec![], false));
        };
        let mut q = vec![
            ("startHistoryId", start.clone()),
            ("maxResults", "25".into()),
            (
                "fields",
                "history(messagesAdded(message(id))),nextPageToken,historyId".into(),
            ),
        ];
        if let Some(page) = &self.page {
            q.push(("pageToken", page.clone()));
        }
        let response = match client.call(Method::GET, "/history", &q, None).await {
            Ok(r) => json::<Response>(r).await?,
            Err(Error::Api { status: 404, .. }) => {
                self.start = Some(client.profile().await?.history_id);
                self.page = None;
                self.offset = 0;
                return Ok((vec![], true));
            }
            Err(e) => return Err(e),
        };
        let changed = !response.history.is_empty();
        let mut out = Vec::new();
        let mut ids = HashSet::new();
        let total = response
            .history
            .iter()
            .map(|r| r.messages_added.len())
            .sum::<usize>();
        let mut offset = self.offset;
        for added in response
            .history
            .iter()
            .flat_map(|r| &r.messages_added)
            .skip(self.offset)
            .take(25)
        {
            offset += 1;
            let id = &added.message.id;
            if self.dedup.seen.contains(id) || !ids.insert(id.clone()) {
                continue;
            }
            let message = match client.message_metadata(id).await {
                Ok(m) => m,
                Err(Error::Api { status: 404, .. }) => continue,
                Err(e) => return Err(e),
            };
            if message.has("INBOX")
                && message.has("UNREAD")
                && !message.has("TRASH")
                && !message.has("SPAM")
            {
                out.push(message);
            }
        }
        // Commit the offset only after every metadata request in this batch
        // succeeds; transient errors cannot consume unnotified messages.
        if offset >= total {
            self.offset = 0;
            self.page = response.next_page_token;
            if self.page.is_none() {
                self.start = Some(response.history_id);
            }
        } else {
            self.offset = offset;
        }
        out.retain(|m| self.dedup.fresh(&m.id));
        Ok((out, changed))
    }
}
