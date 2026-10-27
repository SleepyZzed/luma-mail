//! One event-driven polling task. Automatic checks never replace the loaded
//! view, fetch bodies or drive the manual loading indicator.
use crate::{app::App, mail};
use futures_util::{StreamExt, stream};
use luma_core::{
    model::{Conversation, Message, ThreadKey},
    notifications::Monitor,
};
use serde_json::json;
use std::{
    collections::{HashMap, HashSet, VecDeque},
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, AtomicU8, Ordering},
    },
    time::Duration,
};
const CHECK: u8 = 1;
const RECONCILE: u8 = 2;
const CONFIG: u8 = 4;
#[derive(Default)]
pub struct Service {
    wake: tokio::sync::Notify,
    flags: AtomicU8,
    worker: Mutex<Option<tokio::task::AbortHandle>>,
    updates: AtomicBool,
}
#[derive(Default)]
struct Pending {
    keys: VecDeque<ThreadKey>,
    seen: HashSet<ThreadKey>,
}
impl Pending {
    fn insert(&mut self, key: ThreadKey) -> bool {
        if self.seen.contains(&key) {
            return true;
        }
        if self.keys.len() >= 128 {
            return false;
        }
        self.seen.insert(key.clone());
        self.keys.push_back(key);
        true
    }
    fn remove(&mut self, key: &ThreadKey) {
        self.seen.remove(key);
        self.keys.retain(|k| k != key);
    }
}
impl Service {
    pub fn start(app: &Arc<App>) {
        if app.preview {
            return;
        }
        let service = app.sync.clone();
        let owner = Arc::downgrade(app);
        let task = tokio::spawn(async move {
            let mut monitors = HashMap::<String, Monitor>::new();
            let mut pending = Pending::default();
            let mut unread = HashMap::new();
            let mut next = tokio::time::Instant::now();
            let mut last_checked = 0;
            let mut errors = Vec::new();
            loop {
                let ready = service.wake.notified();
                tokio::pin!(ready);
                ready.as_mut().enable();
                tokio::select! {_=tokio::time::sleep_until(next)=>(),_=ready=>()}
                let flags = service.flags.swap(0, Ordering::SeqCst);
                let Some(app) = owner.upgrade() else { return };
                let interval =
                    Duration::from_secs(app.settings.lock().unwrap().poll_seconds.clamp(30, 3600));
                if flags & CONFIG != 0 {
                    next = next.min(tokio::time::Instant::now() + interval);
                }
                if flags & CHECK != 0 || tokio::time::Instant::now() >= next {
                    errors.clear();
                    let clients: Vec<_> = app.clients.lock().unwrap().values().cloned().collect();
                    let live: HashSet<_> = clients.iter().map(|c| c.account.clone()).collect();
                    monitors.retain(|id, _| live.contains(id));
                    unread.retain(|id, _| live.contains(id));
                    let jobs: Vec<_> = clients
                        .into_iter()
                        .map(|client| {
                            let monitor = monitors.remove(&client.account).unwrap_or_default();
                            (client, monitor)
                        })
                        .collect();
                    let output: Vec<_> = stream::iter(jobs)
                        .map(|(client, mut monitor)| async move {
                            let (updates, count) =
                                tokio::join!(monitor.poll(&client), client.unread());
                            (client.account.clone(), monitor, updates, count)
                        })
                        .buffer_unordered(4)
                        .collect()
                        .await;
                    for (account, monitor, updates, count) in output {
                        let prefs = app
                            .settings
                            .lock()
                            .unwrap()
                            .accounts
                            .iter()
                            .find(|a| a.id == account && a.enabled)
                            .cloned();
                        let Some(prefs) = prefs else { continue };
                        monitors.insert(account.clone(), monitor);
                        if let Ok(count) = count {
                            unread.insert(account.clone(), count);
                        }
                        match updates {
                            Ok((messages, changed)) => {
                                if changed && messages.is_empty() {
                                    service.updates.store(true, Ordering::SeqCst);
                                }
                                for message in messages {
                                    if !pending.insert(ThreadKey {
                                        account: account.clone(),
                                        id: message.thread_id.clone(),
                                    }) {
                                        service.updates.store(true, Ordering::SeqCst);
                                    }
                                    if prefs.notifications {
                                        let privacy =
                                            app.settings.lock().unwrap().notification_privacy;
                                        let (title, body) =
                                            notification(&prefs.email, &message, privacy);
                                        app.events.emit(0,"notification",json!({"account":account,"thread":message.thread_id,"title":title,"body":body})).await;
                                    }
                                }
                            }
                            Err(error) => {
                                errors.push(json!({"account":account,"message":error.to_string()}))
                            }
                        }
                    }
                    last_checked = chrono::Utc::now().timestamp();
                    next = tokio::time::Instant::now() + interval;
                }
                if !pending.keys.is_empty() {
                    reconcile(&app, &mut pending).await;
                }
                app.events.emit(0,"sync",json!({"silent":true,"lastChecked":last_checked,"unread":unread,"updatesAvailable":service.updates.load(Ordering::SeqCst),"errors":errors})).await;
            }
        });
        *app.sync.worker.lock().unwrap() = Some(task.abort_handle());
    }
    fn signal(&self, flag: u8) {
        if self.worker.lock().unwrap().is_some() {
            self.flags.fetch_or(flag, Ordering::SeqCst);
            self.wake.notify_one();
        }
    }
    pub fn check(&self) {
        self.signal(CHECK);
    }
    pub fn reconfigure(&self) {
        self.signal(CONFIG | CHECK);
    }
    pub fn settings_changed(&self) {
        self.signal(CONFIG);
    }
    pub fn reconcile(&self) {
        self.signal(RECONCILE);
    }
    pub fn acknowledge(&self) {
        self.updates.store(false, Ordering::SeqCst);
    }
    pub fn stop(&self) {
        if let Some(task) = self.worker.lock().unwrap().take() {
            task.abort();
        }
    }
}
fn notification(account: &str, message: &Message, privacy: bool) -> (String, String) {
    if privacy {
        ("New email".into(), account.into())
    } else {
        (
            message.payload.header("From").chars().take(200).collect(),
            format!(
                "{} · {}",
                account,
                message
                    .payload
                    .header("Subject")
                    .chars()
                    .take(250)
                    .collect::<String>()
            ),
        )
    }
}
async fn reconcile(app: &Arc<App>, pending: &mut Pending) {
    let generation = app.generation.load(Ordering::SeqCst);
    let revision = app.mutation_serial.load(Ordering::SeqCst);
    let read_revision = app.read_revision.load(Ordering::SeqCst);
    let (folder, query, scope) = {
        let v = app.view.lock().unwrap();
        (v.folder.clone(), v.search.clone(), v.accounts.clone())
    };
    if !query.is_empty() {
        app.sync.updates.store(true, Ordering::SeqCst);
        pending.keys.clear();
        pending.seen.clear();
        return;
    }
    let jobs: Vec<_> = pending
        .keys
        .iter()
        .cloned()
        .filter_map(|key| {
            if scope.as_ref().is_some_and(|s| !s.contains(&key.account)) {
                return None;
            }
            app.client(&key.account).ok().map(|c| (key, c))
        })
        .collect();
    let output: Vec<_> = stream::iter(jobs)
        .map(|(key, client)| async move {
            let thread = client.thread_metadata(&key.id).await;
            (key, thread)
        })
        .buffer_unordered(4)
        .collect()
        .await;
    let _write = app.mutation.lock().await;
    if !app.current(generation)
        || app.mutation_serial.load(Ordering::SeqCst) != revision
        || app.read_revision.load(Ordering::SeqCst) != read_revision
    {
        return;
    }
    for (key, result) in output {
        let thread = match result {
            Ok(thread) => thread,
            Err(luma_core::Error::Api { status: 404, .. }) => {
                pending.remove(&key);
                continue;
            }
            Err(_) => {
                app.sync.updates.store(true, Ordering::SeqCst);
                continue;
            }
        };
        if !app.clients.lock().unwrap().contains_key(&key.account) {
            pending.remove(&key);
            continue;
        }
        let Some(row) = Conversation::from_thread(&key.account, &thread) else {
            pending.remove(&key);
            continue;
        };
        let visible = folder.matches(&thread);
        let mut selected = false;
        {
            let mut view = app.view.lock().unwrap();
            apply(&mut view.rows, row.clone(), visible);
            if view.selected.as_ref() == Some(&key) {
                view.thread = Some(thread.clone());
                selected = true;
            }
        }
        if visible {
            app.events
                .emit(
                    0,
                    "rowChanged",
                    json!({"row":row,"insert":true,"generation":generation}),
                )
                .await;
        } else {
            app.events
                .emit(
                    0,
                    "syncRemoved",
                    json!({"keys":[key],"generation":generation}),
                )
                .await;
        }
        if selected {
            let messages: Vec<_> = thread.messages.iter().map(mail::message_header).collect();
            app.events.emit(0,"threadUpdated",json!({"key":key,"reading":app.reading.load(Ordering::SeqCst),"messages":messages})).await;
        }
        pending.remove(&key);
    }
    let live: HashSet<_> = app.clients.lock().unwrap().keys().cloned().collect();
    pending.keys.retain(|k| {
        live.contains(&k.account) && scope.as_ref().is_none_or(|s| s.contains(&k.account))
    });
    pending.seen = pending.keys.iter().cloned().collect();
}
fn apply(rows: &mut Vec<Conversation>, row: Conversation, visible: bool) {
    rows.retain(|r| r.key != row.key);
    if visible {
        rows.push(row);
        rows.sort_by_key(|r| std::cmp::Reverse(r.date));
        rows.truncate(1000);
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use luma_core::model::Thread;
    fn thread(id: &str, date: &str, labels: &[&str]) -> Thread {
        Thread {
            id: id.into(),
            messages: vec![Message {
                id: format!("{id}-reply"),
                thread_id: id.into(),
                internal_date: date.into(),
                label_ids: labels.iter().map(|s| s.to_string()).collect(),
                ..Default::default()
            }],
            ..Default::default()
        }
    }
    #[test]
    fn new_reply_moves_one_row_and_preserves_unread() {
        let a = thread("a", "10", &["INBOX"]);
        let b = thread("b", "20", &["INBOX"]);
        let mut rows = vec![
            Conversation::from_thread("x", &b).unwrap(),
            Conversation::from_thread("x", &a).unwrap(),
        ];
        let reply = thread("a", "30", &["INBOX", "UNREAD"]);
        apply(
            &mut rows,
            Conversation::from_thread("x", &reply).unwrap(),
            true,
        );
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].key.id, "a");
        assert!(rows[0].unread);
        assert_eq!(rows[1].key.id, "b");
    }
    #[test]
    fn queue_is_bounded_and_deduplicates_threads() {
        let mut p = Pending::default();
        for i in 0..128 {
            assert!(p.insert(ThreadKey {
                account: "x".into(),
                id: i.to_string()
            }));
        }
        assert!(p.insert(ThreadKey {
            account: "x".into(),
            id: "0".into()
        }));
        assert!(!p.insert(ThreadKey {
            account: "x".into(),
            id: "overflow".into()
        }));
        p.remove(&ThreadKey {
            account: "x".into(),
            id: "0".into(),
        });
        assert_eq!(p.keys.len(), 127);
    }
    #[test]
    fn private_notifications_contain_no_sender_or_subject() {
        let mut message = thread("a", "10", &[]).messages.remove(0);
        message.payload.headers = vec![
            luma_core::model::Header {
                name: "From".into(),
                value: "Private Sender".into(),
            },
            luma_core::model::Header {
                name: "Subject".into(),
                value: "Private Subject".into(),
            },
        ];
        assert_eq!(
            notification("a@example.com", &message, true),
            ("New email".into(), "a@example.com".into())
        );
        assert_eq!(
            notification("a@example.com", &message, false),
            (
                "Private Sender".into(),
                "a@example.com · Private Subject".into()
            )
        );
    }
}
