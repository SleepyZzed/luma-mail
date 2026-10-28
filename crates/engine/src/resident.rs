//! Selected-reader RAM only. Oversized bodies can still be displayed, but are
//! fetched again on demand rather than retained outside the resident budget.
use luma_core::model::{Message, Part};
use std::{
    collections::{HashMap, VecDeque},
    mem::size_of,
    sync::Arc,
};

const MAX_BYTES: usize = 8 * 1024 * 1024;
const MAX_MESSAGES: usize = 8;

#[derive(Default)]
pub struct Bodies {
    entries: HashMap<String, (Arc<Message>, usize)>,
    recent: VecDeque<String>,
    bytes: usize,
}
impl Bodies {
    pub fn clear(&mut self) {
        self.entries.clear();
        self.recent.clear();
        self.bytes = 0;
    }
    pub fn insert(&mut self, message: Arc<Message>) {
        let id = message.id.clone();
        self.remove(&id);
        let bytes = footprint(&message);
        if bytes > MAX_BYTES {
            return;
        }
        while self.bytes.saturating_add(bytes) > MAX_BYTES || self.entries.len() >= MAX_MESSAGES {
            let Some(old) = self.recent.pop_front() else {
                break;
            };
            self.remove(&old);
        }
        self.bytes += bytes;
        self.recent.push_back(id.clone());
        self.entries.insert(id, (message, bytes));
    }
    pub fn get(&mut self, id: &str) -> Option<Arc<Message>> {
        let message = self.entries.get(id)?.0.clone();
        self.recent.retain(|old| old != id);
        self.recent.push_back(id.into());
        Some(message)
    }
    fn remove(&mut self, id: &str) {
        if let Some((_, bytes)) = self.entries.remove(id) {
            self.bytes = self.bytes.saturating_sub(bytes);
        }
        self.recent.retain(|old| old != id);
    }
}
fn footprint(message: &Message) -> usize {
    fn part(p: &Part, depth: usize) -> usize {
        if depth > 32 {
            return MAX_BYTES + 1;
        }
        let mut bytes = p.part_id.capacity()
            + p.mime_type.capacity()
            + p.filename.capacity()
            + p.body
                .data
                .as_ref()
                .map_or(0, |s| s.len() + 2 * size_of::<usize>())
            + p.body.attachment_id.as_ref().map_or(0, String::capacity)
            + p.headers.capacity() * size_of::<luma_core::model::Header>()
            + p.parts.capacity() * size_of::<Part>();
        for h in &p.headers {
            bytes = bytes.saturating_add(h.name.capacity() + h.value.capacity());
        }
        for child in &p.parts {
            bytes = bytes.saturating_add(part(child, depth + 1));
        }
        bytes
    }
    size_of::<Message>()
        + 2 * size_of::<usize>()
        + message.id.capacity()
        + message.thread_id.capacity()
        + message.snippet.capacity()
        + message.internal_date.capacity()
        + message.history_id.capacity()
        + message.label_ids.capacity() * size_of::<String>()
        + message
            .label_ids
            .iter()
            .map(String::capacity)
            .sum::<usize>()
        + part(&message.payload, 0)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn body(id: usize, bytes: usize) -> Arc<Message> {
        let mut message = Message {
            id: id.to_string(),
            ..Default::default()
        };
        message.payload.body.data = Some("x".repeat(bytes).into());
        Arc::new(message)
    }
    #[test]
    fn byte_budget_evicts_the_least_recently_used_body() {
        let mut bodies = Bodies::default();
        for id in 0..3 {
            bodies.insert(body(id, 2 * 1024 * 1024));
        }
        assert!(bodies.get("0").is_some());
        bodies.insert(body(3, 2 * 1024 * 1024));
        assert!(bodies.get("1").is_none());
        assert!(bodies.get("0").is_some());
        assert!(bodies.bytes <= MAX_BYTES);
    }
    #[test]
    fn oversized_content_is_not_retained_and_clear_releases_every_reference() {
        let mut bodies = Bodies::default();
        let large = body(1, MAX_BYTES);
        bodies.insert(large.clone());
        assert!(bodies.get("1").is_none());
        assert_eq!(Arc::strong_count(&large), 1);
        let small = body(2, 100);
        bodies.insert(small.clone());
        bodies.clear();
        assert_eq!(Arc::strong_count(&small), 1);
        assert_eq!(bodies.bytes, 0);
    }
    #[test]
    fn metadata_count_is_bounded_even_for_empty_bodies() {
        let mut bodies = Bodies::default();
        for id in 0..40 {
            bodies.insert(body(id, 0));
        }
        assert_eq!(bodies.entries.len(), MAX_MESSAGES);
        assert!(bodies.get("0").is_none());
        assert!(bodies.get("39").is_some());
    }
}
