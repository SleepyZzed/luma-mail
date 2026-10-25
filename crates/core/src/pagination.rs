use crate::{Result, model::*, provider::MailProvider};
use std::{
    collections::{HashSet, VecDeque},
    sync::Arc,
};

#[derive(Clone)]
struct Cursor {
    provider: Arc<dyn MailProvider>,
    buffer: VecDeque<Conversation>,
    next: Option<String>,
    started: bool,
    exhausted: bool,
}

/// A k-way merge. Refill an exhausted frontier before emitting another row.
/// Only one provider page is buffered per account; no historical crawl.
#[derive(Clone)]
pub struct UnifiedPager {
    cursors: Vec<Cursor>,
    folder: Folder,
    search: String,
    seen: HashSet<ThreadKey>,
}

impl UnifiedPager {
    pub fn new(providers: Vec<Arc<dyn MailProvider>>, folder: Folder, search: String) -> Self {
        Self {
            cursors: providers
                .into_iter()
                .map(|provider| Cursor {
                    provider,
                    buffer: VecDeque::new(),
                    next: None,
                    started: false,
                    exhausted: false,
                })
                .collect(),
            folder,
            search,
            seen: HashSet::new(),
        }
    }
    pub fn has_more(&self) -> bool {
        self.cursors
            .iter()
            .any(|c| !c.exhausted || !c.buffer.is_empty())
    }
    pub async fn next_page(&mut self) -> Result<Vec<Conversation>> {
        // Commit only after the page succeeds. Dropping/cancelling this future,
        // or a provider failure, cannot consume rows that never reached the UI.
        let mut staged = self.clone();
        let rows = staged.fetch_page().await?;
        *self = staged;
        Ok(rows)
    }
    async fn fetch_page(&mut self) -> Result<Vec<Conversation>> {
        let mut out = Vec::new();
        while out.len() < 25 {
            for cursor in &mut self.cursors {
                if cursor.buffer.is_empty() && !cursor.exhausted {
                    let page = cursor
                        .provider
                        .list(
                            &self.folder,
                            &self.search,
                            if cursor.started {
                                cursor.next.as_deref()
                            } else {
                                None
                            },
                        )
                        .await?;
                    if cursor.started && page.next.is_some() && page.next == cursor.next {
                        return Err(crate::Error::Invalid(
                            "Google repeated a pagination token; refresh this view".into(),
                        ));
                    }
                    cursor.started = true;
                    cursor.next = page.next;
                    cursor.exhausted = cursor.next.is_none();
                    cursor.buffer = page.items.into();
                    // Empty pages with continuation are returned to the user without
                    // looping through unlimited remote pages in one UI request.
                    if cursor.buffer.is_empty() && !cursor.exhausted {
                        return Ok(out);
                    }
                }
            }
            let best = self
                .cursors
                .iter()
                .enumerate()
                .filter_map(|(i, c)| c.buffer.front().map(|v| (i, v)))
                .max_by(|(_, a), (_, b)| a.date.cmp(&b.date).then(b.key.cmp(&a.key)))
                .map(|(i, _)| i);
            let Some(index) = best else { break };
            let item = self.cursors[index]
                .buffer
                .pop_front()
                .expect("nonempty frontier");
            if self.seen.insert(item.key.clone()) {
                out.push(item);
            }
            if self.seen.len() >= 1000 {
                break;
            }
        }
        Ok(out)
    }
}
