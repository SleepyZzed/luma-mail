//! Visible avatars only; at most 128 decoded photos in RAM, no disk/contact sync.
use crate::ui::State;
use adw::prelude::*;
use gtk::{gdk, glib};
use std::{
    cell::{Cell, RefCell},
    collections::{HashMap, VecDeque},
    rc::Rc,
    time::{Duration, Instant},
};
type Key = (String, String);
struct Avatar {
    key: Key,
    widget: glib::WeakRef<adw::Avatar>,
}
struct Cached {
    key: Key,
    expires: Instant,
    image: Option<gdk::Texture>,
    retry: bool,
    note: String,
}
#[derive(Default)]
pub(crate) struct Photos {
    widgets: RefCell<Vec<Avatar>>,
    cache: RefCell<VecDeque<Cached>>,
    clients: RefCell<HashMap<String, luma_core::contact_photos::Photos>>,
    pending: RefCell<HashMap<Key, tokio::task::AbortHandle>>,
    recent: RefCell<VecDeque<Instant>>,
    scheduled: RefCell<Option<(Instant, glib::SourceId)>>,
    generation: Cell<u64>,
    failures: RefCell<HashMap<Key, String>>,
    statuses: RefCell<Vec<(String, glib::WeakRef<adw::ActionRow>)>>,
}
impl Photos {
    pub fn clear(&self) {
        self.generation.set(self.generation.get().wrapping_add(1));
        if let Some((_, timer)) = self.scheduled.borrow_mut().take() {
            timer.remove();
        }
        for (_, task) in self.pending.borrow_mut().drain() {
            task.abort();
        }
        for entry in self.widgets.borrow().iter() {
            if let Some(widget) = entry.widget.upgrade() {
                widget.set_custom_image(None::<&gdk::Paintable>);
            }
        }
        self.cache.borrow_mut().clear();
        self.clients.borrow_mut().clear();
        self.recent.borrow_mut().clear();
        self.failures.borrow_mut().clear();
    }
    pub fn status(&self, account: &str) -> Option<String> {
        self.failures
            .borrow()
            .iter()
            .find(|(key, _)| key.0 == account)
            .map(|(_, value)| value.clone())
    }
}
pub(crate) fn watch(s: &Rc<State>, account: &str, row: &adw::ActionRow) {
    s.photos
        .statuses
        .borrow_mut()
        .retain(|(_, row)| row.upgrade().is_some());
    s.photos
        .statuses
        .borrow_mut()
        .push((account.into(), row.downgrade()));
    publish(s);
}
fn publish(s: &State) {
    for (account, row) in s.photos.statuses.borrow().iter() {
        let Some(row) = row.upgrade() else {
            continue;
        };
        let available = s
            .photos
            .cache
            .borrow()
            .iter()
            .filter(|c| c.key.0 == *account && c.image.is_some())
            .count();
        let missing = s
            .photos
            .cache
            .borrow()
            .iter()
            .filter(|c| c.key.0 == *account && c.image.is_none())
            .count();
        let text = if !enabled(s, account) {
            "Contact photos are off for this account".into()
        } else if let Some(error) = s.photos.status(account) {
            format!("Photo lookup failed: {error}. Retry or reconnect this account.")
        } else if s
            .photos
            .pending
            .borrow()
            .keys()
            .any(|key| key.0 == *account)
        {
            format!("Loading pictures… {available} available")
        } else if available > 0 {
            format!("{available} pictures available • Unmatched senders keep initials")
        } else if missing > 0 {
            format!(
                "Google returned no available picture for {missing} visible senders. Gmail-only pictures are not exposed by its public API."
            )
        } else {
            "Photos load when senders are visible".into()
        };
        row.set_subtitle(&text);
    }
}
fn enabled(s: &State, account: &str) -> bool {
    s.settings
        .borrow()
        .accounts
        .iter()
        .any(|a| a.id == account && a.enabled && a.contact_photos)
}
fn clear_key(s: &State, key: &Key) {
    for entry in s
        .photos
        .widgets
        .borrow()
        .iter()
        .filter(|entry| &entry.key == key)
    {
        if let Some(widget) = entry.widget.upgrade() {
            widget.set_custom_image(None::<&gdk::Paintable>);
        }
    }
}
pub(crate) fn register(s: &Rc<State>, account: &str, sender: &str, widget: &adw::Avatar) {
    let Ok(email) = luma_core::contact_photos::sender_email(sender) else {
        return;
    };
    let key = (account.to_owned(), email);
    let mut widgets = s.photos.widgets.borrow_mut();
    widgets.retain(|entry| entry.widget.upgrade().is_some());
    widgets.push(Avatar {
        key,
        widget: widget.downgrade(),
    });
    drop(widgets);
    let weak = Rc::downgrade(s);
    widget.connect_map(move |_| {
        if let Some(s) = weak.upgrade() {
            queue(&s);
        }
    });
    widget.set_tooltip_text(Some(if enabled(s, account) {
        "Looking for an available Google contact picture"
    } else {
        "Contact photos are off for this account"
    }));
    queue(s);
}
pub(crate) fn queue(s: &Rc<State>) {
    queue_after(s, Duration::from_millis(120));
}
fn queue_after(s: &Rc<State>, delay: Duration) {
    let deadline = Instant::now() + delay;
    if let Some((at, _)) = s.photos.scheduled.borrow().as_ref()
        && *at <= deadline
    {
        return;
    }
    if let Some((_, timer)) = s.photos.scheduled.borrow_mut().take() {
        timer.remove();
    }
    let weak = Rc::downgrade(s);
    *s.photos.scheduled.borrow_mut() = Some((
        deadline,
        glib::timeout_add_local_once(delay, move || {
            if let Some(s) = weak.upgrade() {
                s.photos.scheduled.borrow_mut().take();
                visible(&s);
            }
        }),
    ));
}
fn intersects(a: &gtk::graphene::Rect, b: &gtk::graphene::Rect) -> bool {
    a.x() < b.x() + b.width()
        && a.y() < b.y() + b.height()
        && a.x() + a.width() > b.x()
        && a.y() + a.height() > b.y()
}
fn on_screen(widget: &adw::Avatar, s: &State) -> bool {
    if !widget.is_mapped() {
        return false;
    }
    let Some(rect) = widget.compute_bounds(&s.window) else {
        return false;
    };
    if !intersects(
        &rect,
        &gtk::graphene::Rect::new(0., 0., s.window.width() as f32, s.window.height() as f32),
    ) {
        return false;
    }
    let mut parent = widget.parent();
    while let Some(current) = parent {
        if current.is::<gtk::ScrolledWindow>()
            && current
                .compute_bounds(&s.window)
                .is_some_and(|clip| !intersects(&rect, &clip))
        {
            return false;
        }
        parent = current.parent();
    }
    true
}
fn visible(s: &Rc<State>) {
    let entries: Vec<_> = s
        .photos
        .widgets
        .borrow()
        .iter()
        .filter_map(|a| a.widget.upgrade().map(|w| (a.key.clone(), w)))
        .filter(|(key, widget)| enabled(s, &key.0) && on_screen(widget, s))
        .collect();
    for (key, widget) in entries {
        let mut cache = s.photos.cache.borrow_mut();
        if let Some(index) = cache.iter().position(|entry| entry.key == key) {
            let entry = cache.remove(index).unwrap();
            if entry.expires > Instant::now() {
                widget.set_custom_image(entry.image.as_ref());
                widget.set_tooltip_text(Some(&entry.note));
                if entry.retry {
                    queue_after(
                        s,
                        entry.expires.saturating_duration_since(Instant::now())
                            + Duration::from_millis(150),
                    );
                }
                cache.push_back(entry);
                continue;
            }
            clear_key(s, &key);
        }
        drop(cache);
        if s.photos.pending.borrow().contains_key(&key) {
            continue;
        }
        if s.photos.pending.borrow().len() >= 2 {
            continue;
        }
        let Some(client) = s.clients.borrow().get(&key.0).cloned() else {
            continue;
        };
        let mut recent = s.photos.recent.borrow_mut();
        recent.retain(|at| at.elapsed() < Duration::from_secs(60));
        if recent.len() >= 12 {
            let delay = Duration::from_secs(60).saturating_sub(recent.front().unwrap().elapsed())
                + Duration::from_millis(150);
            drop(recent);
            queue_after(s, delay);
            continue;
        }
        recent.push_back(Instant::now());
        drop(recent);
        let photos = s
            .photos
            .clients
            .borrow_mut()
            .entry(key.0.clone())
            .or_insert_with(|| luma_core::contact_photos::Photos::new(client))
            .clone();
        let email = key.1.clone();
        let owner = s
            .settings
            .borrow()
            .accounts
            .iter()
            .find(|a| {
                a.enabled
                    && a.contact_photos
                    && a.drive_enabled
                    && a.email.eq_ignore_ascii_case(&email)
            })
            .map(|a| a.id.clone());
        let own_photos = owner.and_then(|id| {
            let client = s.clients.borrow().get(&id).cloned()?;
            Some(
                s.photos
                    .clients
                    .borrow_mut()
                    .entry(id)
                    .or_insert_with(|| luma_core::contact_photos::Photos::new(client))
                    .clone(),
            )
        });
        let own = own_photos.is_some();
        let generation = s.photos.generation.get();
        let target = key.clone();
        let task = crate::ui::async_work(
            s,
            async move {
                Ok(if let Some(own_photos) = own_photos {
                    match own_photos.own(&email).await {
                        Ok(Some(photo)) => Ok(Some(photo)),
                        _ => photos.get(&email).await,
                    }
                } else {
                    photos.get(&email).await
                })
            },
            move |s, result| {
                if s.photos.generation.get() != generation {
                    return;
                }
                s.photos.pending.borrow_mut().remove(&target);
                if !enabled(&s, &target.0) {
                    return;
                }
                // A transient failure must not become a ten-minute "no photo".
                // Permission failures wait for explicit Retry/Reconnect.
                let retry = result.as_ref().err().is_some_and(|error| {
                    !matches!(
                        error,
                        luma_core::Error::Reauthenticate
                            | luma_core::Error::Api {
                                status: 401 | 403,
                                ..
                            }
                    )
                });
                let ttl = match &result {
                    Ok(_) => {
                        s.photos.failures.borrow_mut().remove(&target);
                        Duration::from_secs(600)
                    }
                    Err(error) => {
                        s.photos
                            .failures
                            .borrow_mut()
                            .insert(target.clone(), error.to_string());
                        if matches!(
                            error,
                            luma_core::Error::Reauthenticate
                                | luma_core::Error::Api {
                                    status: 401 | 403,
                                    ..
                                }
                        ) {
                            Duration::from_secs(600)
                        } else {
                            queue_after(&s, Duration::from_secs(60));
                            Duration::from_secs(59)
                        }
                    }
                };
                let note = match &result {
                    Ok(Some(_)) => {
                        if own {
                            "Connected Google account picture".into()
                        } else {
                            "Google contact picture".into()
                        }
                    }
                    Ok(None) => {
                        "Google has no available contact/profile picture for this sender".into()
                    }
                    Err(error) => format!("Could not load picture: {error}"),
                };
                let image = result.ok().flatten().map(|image| {
                    gdk::MemoryTexture::new(
                        image.width as i32,
                        image.height as i32,
                        gdk::MemoryFormat::R8g8b8a8,
                        &glib::Bytes::from_owned(image.rgba),
                        image.width as usize * 4,
                    )
                    .upcast::<gdk::Texture>()
                });
                for entry in s
                    .photos
                    .widgets
                    .borrow()
                    .iter()
                    .filter(|entry| entry.key == target)
                {
                    if let Some(widget) = entry.widget.upgrade() {
                        widget.set_custom_image(image.as_ref());
                        widget.set_tooltip_text(Some(&note));
                    }
                }
                let mut cache = s.photos.cache.borrow_mut();
                cache.retain(|entry| entry.key != target);
                cache.push_back(Cached {
                    key: target,
                    expires: Instant::now() + ttl,
                    image,
                    retry,
                    note,
                });
                let mut evicted = Vec::new();
                while cache.len() > 128 {
                    evicted.push(cache.pop_front().unwrap().key);
                }
                drop(cache);
                for key in evicted {
                    clear_key(&s, &key);
                }
                queue(&s);
                publish(&s);
            },
        );
        s.photos.pending.borrow_mut().insert(key, task);
    }
    publish(s);
}
