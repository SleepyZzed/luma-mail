use crate::ui::State;
use adw::prelude::*;
use std::rc::Rc;

pub fn show(s: &State, token: Option<String>) {
    s.background_hold.borrow_mut().take();
    if let Some(token) = token {
        s.window.set_startup_id(&token);
    }
    s.window.present();
}
pub fn quit(s: &Rc<State>) {
    // Give every composer its normal save/close handling. If a save is still
    // pending, keep the app reachable and let the user quit again after Saved.
    for window in s.app.windows() {
        if !window.is::<adw::ApplicationWindow>() {
            window.close();
        }
    }
    if s.app
        .windows()
        .iter()
        .any(|w| !w.is::<adw::ApplicationWindow>())
    {
        show(s, None);
        s.info("A composer is saving or needs attention. Close it, then Quit again.");
        return;
    }
    s.quitting.set(true);
    s.save();
    if s.settings.borrow().clear_cache_on_exit
        && let Err(e) = s.cache.lock().expect("cache mutex").clear()
    {
        s.error(&e);
    }
    if let Some(task) = s.tray_task.borrow_mut().take() {
        task.abort();
    }
    s.background_hold.borrow_mut().take();
    s.app.quit();
}

#[cfg(not(target_os = "linux"))]
pub fn start(s: &Rc<State>) {
    s.tray_available.set(false);
}

#[cfg(any(target_os = "linux", test))]
#[cfg_attr(not(target_os = "linux"), allow(dead_code))]
mod linux {
    use super::*;
    use ksni::{Tray, TrayMethods, menu::StandardItem};
    use tokio::sync::mpsc;
    enum Event {
        Show(Option<String>),
        Quit,
        Available(bool),
    }
    struct MailTray {
        events: mpsc::Sender<Event>,
    }
    impl Tray for MailTray {
        fn id(&self) -> String {
            "luma-mail".into()
        }
        fn title(&self) -> String {
            "Luma Mail".into()
        }
        fn icon_name(&self) -> String {
            "io.github.lumamail.Luma".into()
        }
        fn icon_pixmap(&self) -> Vec<ksni::Icon> {
            vec![ksni::Icon {
                width: 32,
                height: 32,
                data: include_bytes!("../../../data/tray-icon.argb").to_vec(),
            }]
        }
        fn activate(&mut self, _: i32, _: i32) {
            let _ = self
                .events
                .try_send(Event::Show(ksni::current_xdg_activation_token()));
        }
        fn menu(&self) -> Vec<ksni::MenuItem<Self>> {
            vec![
                StandardItem {
                    label: "Open Luma Mail".into(),
                    icon_name: "mail-unread-symbolic".into(),
                    activate: Box::new(|t: &mut Self| {
                        let _ = t
                            .events
                            .try_send(Event::Show(ksni::current_xdg_activation_token()));
                    }),
                    ..Default::default()
                }
                .into(),
                ksni::MenuItem::Separator,
                StandardItem {
                    label: "Quit".into(),
                    icon_name: "application-exit-symbolic".into(),
                    activate: Box::new(|t: &mut Self| {
                        let _ = t.events.try_send(Event::Quit);
                    }),
                    ..Default::default()
                }
                .into(),
            ]
        }
        fn watcher_online(&self) {
            let _ = self.events.try_send(Event::Available(true));
        }
        fn watcher_offline(&self, _: ksni::OfflineReason) -> bool {
            let _ = self.events.try_send(Event::Available(false));
            true
        }
    }
    pub fn start(s: &Rc<State>) {
        let (tx, mut rx) = mpsc::channel(32);
        let failure = tx.clone();
        let task = s.runtime.spawn(async move {
            match (MailTray { events: tx }).spawn().await {
                Ok(handle) => {
                    let _ = failure.send(Event::Available(true)).await;
                    let _handle = handle;
                    std::future::pending::<()>().await;
                }
                Err(_) => {
                    let _ = failure.send(Event::Available(false)).await;
                }
            }
        });
        *s.tray_task.borrow_mut() = Some(task.abort_handle());
        let weak = Rc::downgrade(s);
        gtk::glib::MainContext::default().spawn_local(async move{
            while let Some(event)=rx.recv().await{let Some(s)=weak.upgrade()else{break};match event{
                Event::Show(token)=>show(&s,token),Event::Quit=>quit(&s),Event::Available(available)=>{
                    s.tray_available.set(available);
                    if !available{if !s.window.is_visible(){show(&s,None);}s.info("No system tray host is available. Closing exits Luma; enable the Noctalia tray widget for tray mode.");}
                }
            }}
        });
    }
    #[cfg(test)]
    mod tests {
        use super::*;
        #[tokio::test]
        async fn tray_menu_dispatches_open_and_quit() {
            let (tx, mut rx) = mpsc::channel(8);
            let mut tray = MailTray { events: tx };
            let mut menu = tray.menu();
            assert_eq!(menu.len(), 3);
            if let ksni::MenuItem::Standard(item) = &mut menu[0] {
                assert_eq!(item.label, "Open Luma Mail");
                (item.activate)(&mut tray);
            }
            assert!(matches!(rx.recv().await, Some(Event::Show(_))));
            if let ksni::MenuItem::Standard(item) = &mut menu[2] {
                assert_eq!(item.label, "Quit");
                (item.activate)(&mut tray);
            }
            assert!(matches!(rx.recv().await, Some(Event::Quit)));
        }
    }
}
#[cfg(target_os = "linux")]
pub use linux::start;
