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
    /// One bounded history page per poll. No initial notification storm and no
    /// full resync when the server expires a history cursor.
    pub async fn poll(&mut self, client: &Gmail) -> Result<(Vec<Message>, bool)> {
        let Some(start) = &self.start else {
            self.start = Some(client.profile().await?.history_id);
            return Ok((vec![], false));
        };
        let mut q = vec![
            ("startHistoryId", start.clone()),
            ("maxResults", "25".into()),
        ];
        if let Some(page) = &self.page {
            q.push(("pageToken", page.clone()));
        }
        let response = match client.call(Method::GET, "/history", &q, None).await {
            Ok(r) => json::<Response>(r).await?,
            Err(Error::Api { status: 404, .. }) => {
                self.start = Some(client.profile().await?.history_id);
                self.page = None;
                return Ok((vec![], true));
            }
            Err(e) => return Err(e),
        };
        let changed = !response.history.is_empty();
        let mut out = Vec::new();
        let mut ids = HashSet::new();
        for record in &response.history {
            for added in &record.messages_added {
                if out.len() >= 25 {
                    break;
                }
                let id = &added.message.id;
                if !ids.insert(id.clone()) {
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
        }
        self.page = response.next_page_token;
        if self.page.is_none() {
            self.start = Some(response.history_id);
        }
        out.retain(|m| self.dedup.fresh(&m.id));
        Ok((out, changed))
    }
}
