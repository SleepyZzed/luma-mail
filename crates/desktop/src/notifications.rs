use crate::ui::{self, State};
use adw::prelude::*;
use gtk::{gio, glib};
use luma_core::notifications::Monitor;
use std::{
    cell::{Cell, RefCell},
    collections::HashMap,
    rc::Rc,
    sync::Arc,
    time::Duration,
};

#[derive(Default)]
pub struct Poller {
    monitors: Arc<tokio::sync::Mutex<HashMap<String, Monitor>>>,
    running: Cell<bool>,
    next: RefCell<Option<glib::SourceId>>,
}

pub fn start(s: &Rc<State>) {
    let action = gio::SimpleAction::new(
        "open-thread",
        Some(&<(String, String) as glib::variant::StaticVariantType>::static_variant_type()),
    );
    let weak = Rc::downgrade(s);
    action.connect_activate(move |_, param| {
        if let (Some(s), Some((account, id))) = (
            weak.upgrade(),
            param.and_then(|v| v.get::<(String, String)>()),
        ) {
            crate::tray::show(&s, None);
            ui::open_thread(&s, luma_core::model::ThreadKey { account, id });
        }
    });
    s.app.add_action(&action);
    poll_now(s, false);
}
pub fn poll_now(s: &Rc<State>, refresh_list: bool) {
    if s.poller.running.replace(true) {
        return;
    }
    if let Some(timer) = s.poller.next.borrow_mut().take() {
        timer.remove();
    }
    let clients: Vec<_> = s.clients.borrow().values().cloned().collect();
    let prefs = s.settings.borrow().accounts.clone();
    let work = s.poller.monitors.clone();
    ui::async_work(
        s,
        async move {
            let mut monitors = work.lock().await;
            let mut result = Vec::new();
            for c in clients {
                let monitor = monitors.entry(c.account.clone()).or_default();
                let updates = monitor.poll(&c).await;
                let unread = c.unread().await;
                result.push((c.account.clone(), updates, unread));
            }
            monitors.retain(|id, _| prefs.iter().any(|a| a.id == *id && a.enabled));
            Ok(result)
        },
        move |s, results| {
            s.poller.running.set(false);
            let mut changed = false;
            for (account, updates, unread) in results {
                if let Ok(count) = unread {
                    s.unread.borrow_mut().insert(account.clone(), count);
                }
                match updates {
                    Ok((messages, did_change)) => {
                        changed |= did_change;
                        let a = s
                            .settings
                            .borrow()
                            .accounts
                            .iter()
                            .find(|a| a.id == account)
                            .cloned();
                        let Some(a) = a else { continue };
                        if !a.notifications {
                            continue;
                        }
                        for m in messages {
                            let privacy = s.settings.borrow().notification_privacy;
                            let n = gio::Notification::new(if privacy {
                                "New email"
                            } else {
                                m.payload.header("From")
                            });
                            n.set_body(Some(&if privacy {
                                a.email.clone()
                            } else {
                                format!("{} · {}", a.email, m.payload.header("Subject"))
                            }));
                            n.set_default_action_and_target_value(
                                "app.open-thread",
                                Some(&(account.clone(), m.thread_id.clone()).to_variant()),
                            );
                            s.app
                                .send_notification(Some(&format!("{}-{}", account, m.id)), &n);
                        }
                    }
                    Err(e) => s.error(&e),
                }
            }
            if changed && refresh_list {
                ui::refresh_background(&s);
            }
            ui::sidebar(&s);
            let weak = Rc::downgrade(&s);
            let interval = s.settings.borrow().poll_seconds.clamp(30, 3600);
            let timer = glib::timeout_add_local_once(Duration::from_secs(interval), move || {
                if let Some(s) = weak.upgrade() {
                    s.poller.next.borrow_mut().take();
                    poll_now(&s, true);
                }
            });
            *s.poller.next.borrow_mut() = Some(timer);
        },
    );
}
