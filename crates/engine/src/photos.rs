//! Visible-avatar requests only. Two workers, a bounded queue and the reference
//! twelve-lookups/minute limit. Decoded pixels belong to the native RAM cache.
use crate::{app::App, protocol::Command};
use base64::{Engine, engine::general_purpose::STANDARD};
use luma_core::{Error, Result, contact_photos::Photos, preview::Thumbnail};
use serde_json::json;
use std::{
    collections::{HashMap, VecDeque},
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};
const QUEUE: usize = 64;
#[derive(Clone)]
struct Lookup {
    source: String,
    email: String,
    own: bool,
    epoch: u64,
}
struct Pending {
    lookup: Lookup,
    tokens: Vec<String>,
}
#[derive(Default)]
struct State {
    queue: VecDeque<String>,
    pending: HashMap<String, Pending>,
    recent: VecDeque<Instant>,
    epoch: u64,
}
impl State {
    fn next(&mut self, now: Instant) -> (Option<(String, Lookup)>, Option<Duration>) {
        self.recent
            .retain(|at| now.duration_since(*at) < Duration::from_secs(60));
        if self.queue.is_empty() {
            return (None, None);
        }
        if self.recent.len() >= 12 {
            return (
                None,
                Some(
                    Duration::from_secs(60)
                        .saturating_sub(now.duration_since(*self.recent.front().unwrap())),
                ),
            );
        }
        let key = self.queue.pop_front().unwrap();
        let lookup = self.pending.get(&key).map(|p| p.lookup.clone());
        if lookup.is_some() {
            self.recent.push_back(now);
        }
        (lookup.map(|l| (key, l)), None)
    }
}
#[derive(Default)]
pub struct Service {
    state: Mutex<State>,
    clients: Mutex<HashMap<String, Photos>>,
    wake: tokio::sync::Notify,
    changed: tokio::sync::Notify,
    workers: Mutex<Vec<tokio::task::AbortHandle>>,
}
impl Service {
    pub fn start(app: &Arc<App>) {
        for _ in 0..2 {
            let service = app.photos.clone();
            let owner = Arc::downgrade(app);
            let worker = tokio::spawn(async move {
                loop {
                    let ready = service.wake.notified();
                    tokio::pin!(ready);
                    ready.as_mut().enable();
                    let (job, delay) = service.state.lock().unwrap().next(Instant::now());
                    if let Some((key, lookup)) = job {
                        let Some(app) = owner.upgrade() else { return };
                        service.fetch(&app, &key, lookup).await;
                    } else if let Some(delay) = delay {
                        tokio::select! {_=tokio::time::sleep(delay)=>(),_=ready=>()}
                    } else {
                        ready.await;
                    }
                }
            });
            app.photos
                .workers
                .lock()
                .unwrap()
                .push(worker.abort_handle());
        }
    }
    pub fn stop(&self) {
        for worker in self.workers.lock().unwrap().drain(..) {
            worker.abort();
        }
        self.reset();
    }
    pub fn reset(&self) {
        let mut state = self.state.lock().unwrap();
        state.epoch += 1;
        state.queue.clear();
        state.pending.clear();
        drop(state);
        self.clients.lock().unwrap().clear();
        self.changed.notify_waiters();
        self.wake.notify_waiters();
    }
    pub fn cancel(&self, token: &str) {
        let mut state = self.state.lock().unwrap();
        for pending in state.pending.values_mut() {
            pending.tokens.retain(|t| t != token);
        }
        state.pending.retain(|_, p| !p.tokens.is_empty());
        let live: std::collections::HashSet<_> = state.pending.keys().cloned().collect();
        state.queue.retain(|k| live.contains(k));
        drop(state);
        self.changed.notify_waiters();
        self.wake.notify_waiters();
    }
    fn live(&self, key: &str, epoch: u64) -> bool {
        let state = self.state.lock().unwrap();
        state.epoch == epoch && state.pending.contains_key(key)
    }
    async fn fetch(&self, app: &Arc<App>, key: &str, lookup: Lookup) {
        let fetch = async {
            if app.preview {
                if std::env::var("LUMA_PREVIEW_PHOTOS").is_ok_and(|v| v == "1") {
                    let mut rgba = vec![0; 96 * 96 * 4];
                    for p in rgba.as_chunks_mut::<4>().0 {
                        p.copy_from_slice(&[106, 81, 164, 255]);
                    }
                    return Ok(Some(Thumbnail {
                        rgba,
                        width: 96,
                        height: 96,
                    }));
                }
                return Ok(None);
            }
            let client = app.client(&lookup.source)?;
            let photos = self
                .clients
                .lock()
                .unwrap()
                .entry(lookup.source.clone())
                .or_insert_with(|| Photos::new(client))
                .clone();
            if lookup.own {
                photos.own(&lookup.email).await
            } else {
                photos.get(&lookup.email).await
            }
        };
        tokio::pin!(fetch);
        let result = loop {
            let changed = self.changed.notified();
            tokio::pin!(changed);
            changed.as_mut().enable();
            if !self.live(key, lookup.epoch) {
                return;
            }
            tokio::select! {result=&mut fetch=>break result,_=changed=>()}
        };
        let tokens = {
            let mut state = self.state.lock().unwrap();
            if state.epoch != lookup.epoch {
                return;
            }
            state
                .pending
                .remove(key)
                .map(|p| p.tokens)
                .unwrap_or_default()
        };
        if !tokens.is_empty() {
            respond(app, tokens, result).await;
        }
    }
    pub async fn request(&self, app: &Arc<App>, c: &Command) -> Result<()> {
        let token = c.text("token");
        if token.is_empty()
            || token.len() > 64
            || !token
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b == b'-')
        {
            return Err(Error::Invalid("Invalid photo request".into()));
        }
        let owner = c.text("account");
        let sender = c.text("sender");
        if owner.len() > 128 || sender.len() > 2048 {
            return Err(Error::ResponseLimit);
        }
        let email = match luma_core::contact_photos::sender_email(sender) {
            Ok(v) => v,
            Err(_) => {
                respond(app, vec![token.into()], Ok(None)).await;
                return Ok(());
            }
        };
        let source = {
            let settings = app.settings.lock().unwrap();
            let viewing = settings
                .accounts
                .iter()
                .find(|a| a.id == owner && a.enabled && a.contact_photos);
            viewing.map(|a| {
                settings
                    .accounts
                    .iter()
                    .find(|o| o.enabled && o.drive_enabled && o.email.eq_ignore_ascii_case(&email))
                    .map(|o| (o.id.clone(), true))
                    .unwrap_or_else(|| (a.id.clone(), false))
            })
        };
        let source = if app.preview && std::env::var("LUMA_PREVIEW_PHOTOS").is_ok_and(|v| v == "1")
        {
            Some((owner.to_string(), false))
        } else {
            source
        };
        let Some((source, own)) = source else {
            respond(app, vec![token.into()], Ok(None)).await;
            return Ok(());
        };
        let key = format!("{source}|{own}|{email}");
        let queued = {
            let mut state = self.state.lock().unwrap();
            if let Some(p) = state.pending.get_mut(&key) {
                if p.tokens.iter().any(|t| t == token) {
                    true
                } else if p.tokens.len() < 64 {
                    p.tokens.push(token.into());
                    true
                } else {
                    false
                }
            } else if state.pending.len() < QUEUE {
                let epoch = state.epoch;
                state.pending.insert(
                    key.clone(),
                    Pending {
                        lookup: Lookup {
                            source,
                            email,
                            own,
                            epoch,
                        },
                        tokens: vec![token.into()],
                    },
                );
                state.queue.push_back(key);
                true
            } else {
                false
            }
        };
        if queued {
            self.wake.notify_one();
        } else {
            respond(
                app,
                vec![token.into()],
                Err(Error::Invalid(
                    "Photo queue is full; retry when scrolling settles".into(),
                )),
            )
            .await;
        }
        Ok(())
    }
}
async fn respond(app: &App, tokens: Vec<String>, result: Result<Option<Thumbnail>>) {
    let ttl = match &result {
        Err(
            Error::Reauthenticate
            | Error::Api {
                status: 401 | 403, ..
            },
        ) => 600,
        Err(_) => 60,
        _ => 600,
    };
    let (state, note, image) = match result {
        Ok(Some(image)) => (
            "available",
            "Supported Google profile/contact photo".to_string(),
            Some(image),
        ),
        Ok(None) => (
            "missing",
            "No picture is exposed by Google's supported APIs for this sender".into(),
            None,
        ),
        Err(e) => ("error", e.to_string(), None),
    };
    let mut data = json!({"requests":tokens,"state":state,"note":note,"ttlSeconds":ttl});
    if let Some(image) = image {
        data["source"] = json!(luma_core::composer::new_id());
        data["width"] = json!(image.width);
        data["height"] = json!(image.height);
        data["rgba"] = json!(STANDARD.encode(image.rgba));
    }
    app.events.emit(0, "photo", data).await;
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn rate_window_and_cancellation_bound_work_without_an_idle_timer() {
        let service = Service::default();
        let now = Instant::now();
        {
            let mut state = service.state.lock().unwrap();
            for i in 0..13 {
                let key = i.to_string();
                state.queue.push_back(key.clone());
                state.pending.insert(
                    key,
                    Pending {
                        lookup: Lookup {
                            source: "a".into(),
                            email: "x@example.com".into(),
                            own: false,
                            epoch: 0,
                        },
                        tokens: vec![format!("token{i}")],
                    },
                );
            }
            for _ in 0..12 {
                assert!(state.next(now).0.is_some());
            }
            let (job, delay) = state.next(now);
            assert!(job.is_none());
            assert_eq!(delay, Some(Duration::from_secs(60)));
            assert!(state.next(now + Duration::from_secs(60)).0.is_some());
            assert_eq!(state.next(now + Duration::from_secs(60)).1, None);
        }
        service.cancel("token12");
        assert!(!service.state.lock().unwrap().pending.contains_key("12"));
        service.reset();
        assert!(service.state.lock().unwrap().pending.is_empty());
        assert!(
            !service.state.lock().unwrap().recent.is_empty(),
            "retry cannot reset the quota window"
        );
    }
}
