use crate::{Error, Result, gmail::Gmail, http::json, model::Message};
use reqwest::Method;
use serde::Deserialize;
use std::collections::{HashSet, VecDeque};

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
