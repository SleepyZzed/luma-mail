//! One bounded, RAM-only inverse batch. Successful actions are consumed before
//! retry, so a transient failure never loses the remaining provider writes.
use luma_core::{
    Result,
    model::{MailAction, ThreadKey},
};
use std::{
    collections::{HashSet, VecDeque},
    future::Future,
    mem::size_of,
};
pub const MAX_BYTES: usize = 2 * 1024 * 1024;
pub const MAX_MESSAGES: usize = 8192;
#[derive(Debug)]
pub struct Entry {
    pub account: String,
    pub message: String,
    pub key: ThreadKey,
    pub actions: Vec<MailAction>,
}
impl Entry {
    fn bytes(&self) -> usize {
        size_of::<Self>()
            + self.account.capacity()
            + self.message.capacity()
            + self.key.account.capacity()
            + self.key.id.capacity()
            + self.actions.capacity() * size_of::<MailAction>()
            + self
                .actions
                .iter()
                .map(|action| match action {
                    MailAction::AddLabel(id) | MailAction::RemoveLabel(id) => id.capacity(),
                    _ => 0,
                })
                .sum::<usize>()
    }
}
#[derive(Default)]
pub struct Journal {
    entries: VecDeque<Entry>,
    bytes: usize,
    overflow: bool,
}
impl Journal {
    pub fn clear(&mut self) {
        *self = Self::default();
    }
    pub fn available(&self) -> bool {
        !self.entries.is_empty()
    }
    pub fn overflowed(&self) -> bool {
        self.overflow
    }
    pub fn push(&mut self, entry: Entry) {
        if self.overflow || entry.actions.is_empty() {
            return;
        }
        let bytes = entry.bytes();
        // Account for queue growth as well as owned strings/action allocations.
        if self.entries.len() >= MAX_MESSAGES || self.bytes.saturating_add(bytes) > MAX_BYTES / 2 {
            self.clear();
            self.overflow = true;
            return;
        }
        self.bytes += bytes;
        self.entries.push_back(entry);
    }
    pub async fn replay<F, Fut>(&mut self, mut write: F) -> (HashSet<ThreadKey>, Vec<String>)
    where
        F: FnMut(String, String, MailAction) -> Fut,
        Fut: Future<Output = Result<()>>,
    {
        let mut pending = Self::default();
        let mut keys = HashSet::new();
        let mut errors = Vec::new();
        while let Some(mut item) = self.entries.pop_front() {
            while let Some(action) = item.actions.first().cloned() {
                match write(item.account.clone(), item.message.clone(), action).await {
                    Ok(()) => {
                        item.actions.remove(0);
                        keys.insert(item.key.clone());
                    }
                    Err(error) => {
                        if errors.len() < 8 {
                            errors.push(error.to_string().chars().take(2048).collect());
                        }
                        break;
                    }
                }
            }
            pending.push(item);
        }
        *self = pending;
        (keys, errors)
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use luma_core::Error;
    fn entry(id: &str, actions: Vec<MailAction>) -> Entry {
        Entry {
            account: "a".into(),
            message: id.into(),
            key: ThreadKey {
                account: "a".into(),
                id: "thread".into(),
            },
            actions,
        }
    }
    #[tokio::test]
    async fn retry_keeps_failed_and_unattempted_actions_without_replaying_successes() {
        let mut journal = Journal::default();
        journal.push(entry("first", vec![MailAction::Restore, MailAction::Inbox]));
        journal.push(entry("second", vec![MailAction::Unstar]));
        let mut calls = Vec::new();
        let (keys, errors) = journal
            .replay(|_, id, action| {
                let fail = id == "first" && action == MailAction::Inbox;
                calls.push((id, action));
                std::future::ready(if fail {
                    Err(Error::Invalid("temporary failure".into()))
                } else {
                    Ok(())
                })
            })
            .await;
        assert_eq!(calls.len(), 3);
        assert_eq!(keys.len(), 1);
        assert_eq!(errors.len(), 1);
        assert!(journal.available());
        calls.clear();
        let (_, errors) = journal
            .replay(|_, id, action| {
                calls.push((id, action));
                std::future::ready(Ok(()))
            })
            .await;
        assert_eq!(calls, vec![("first".into(), MailAction::Inbox)]);
        assert!(errors.is_empty());
        assert!(!journal.available());
        assert_eq!(journal.bytes, 0);
    }
    #[test]
    fn oversized_batch_discards_the_whole_inverse_instead_of_offering_partial_undo() {
        let mut journal = Journal::default();
        for i in 0..=MAX_MESSAGES {
            journal.push(entry(&i.to_string(), vec![MailAction::Unstar]));
        }
        assert!(journal.overflowed());
        assert!(!journal.available());
        assert_eq!(journal.bytes, 0);
        journal.push(entry("later", vec![MailAction::Unstar]));
        assert!(!journal.available());
        journal.clear();
        journal.push(entry("new batch", vec![MailAction::Unstar]));
        assert!(journal.available());
    }
    #[test]
    fn byte_bound_counts_owned_capacity_and_label_ids() {
        let mut item = entry(
            "message",
            vec![MailAction::RemoveLabel("x".repeat(MAX_BYTES))],
        );
        item.account.reserve(MAX_BYTES);
        let mut journal = Journal::default();
        journal.push(item);
        assert!(journal.overflowed());
        assert_eq!(journal.bytes, 0);
    }
}
