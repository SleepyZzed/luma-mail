//! Visible avatars only; at most 128 decoded photos in RAM, no disk/contact sync.
use crate::ui::State;
use adw::prelude::*;
use gtk::{gdk, glib};
use std::{
    cell::RefCell,
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
    at: Instant,
    image: Option<gdk::Texture>,
}
#[derive(Default)]
pub(crate) struct Photos {
    widgets: RefCell<Vec<Avatar>>,
    cache: RefCell<VecDeque<Cached>>,
    clients: RefCell<HashMap<String, luma_core::contact_photos::Photos>>,
    pending: RefCell<HashMap<Key, tokio::task::AbortHandle>>,
    recent: RefCell<VecDeque<Instant>>,
    scheduled: RefCell<Option<glib::SourceId>>,
}
impl Photos {
    pub fn clear(&self) {
        if let Some(timer) = self.scheduled.borrow_mut().take() {
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
    let Ok(addresses) = luma_core::composer::recipients(sender) else {
        return;
    };
    let Some(email) = addresses.first().filter(|_| addresses.len() == 1) else {
        return;
    };
    let key = (account.to_owned(), email.to_ascii_lowercase());
    let mut widgets = s.photos.widgets.borrow_mut();
    widgets.retain(|entry| entry.widget.upgrade().is_some());
    widgets.push(Avatar {
        key,
        widget: widget.downgrade(),
    });
    drop(widgets);
    queue(s);
}
pub(crate) fn queue(s: &Rc<State>) {
    if s.photos.scheduled.borrow().is_some() {
        return;
    }
    let weak = Rc::downgrade(s);
    *s.photos.scheduled.borrow_mut() = Some(glib::timeout_add_local_once(
        Duration::from_millis(120),
        move || {
            if let Some(s) = weak.upgrade() {
                s.photos.scheduled.borrow_mut().take();
                visible(&s);
            }
        },
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
            if entry.at.elapsed() < Duration::from_secs(600) {
                widget.set_custom_image(entry.image.as_ref());
                cache.push_back(entry);
                continue;
            }
            clear_key(s, &key);
        }
        drop(cache);
        if s.photos.pending.borrow().len() >= 2 {
            break;
        }
        if s.photos.pending.borrow().contains_key(&key) {
            continue;
        }
        let mut recent = s.photos.recent.borrow_mut();
        recent.retain(|at| at.elapsed() < Duration::from_secs(60));
        if recent.len() >= 12 {
            continue;
        }
        recent.push_back(Instant::now());
        drop(recent);
        let Some(client) = s.clients.borrow().get(&key.0).cloned() else {
            continue;
        };
        let photos = s
            .photos
            .clients
            .borrow_mut()
            .entry(key.0.clone())
            .or_insert_with(|| luma_core::contact_photos::Photos::new(client))
            .clone();
        let email = key.1.clone();
        let target = key.clone();
        let task = crate::ui::async_work(
            s,
            async move { Ok(photos.get(&email).await) },
            move |s, result| {
                s.photos.pending.borrow_mut().remove(&target);
                if !enabled(&s, &target.0) {
                    return;
                }
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
                    }
                }
                let mut cache = s.photos.cache.borrow_mut();
                cache.retain(|entry| entry.key != target);
                cache.push_back(Cached {
                    key: target,
                    at: Instant::now(),
                    image,
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
            },
        );
        s.photos.pending.borrow_mut().insert(key, task);
    }
}
