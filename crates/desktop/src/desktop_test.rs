//! Linux-only acceptance test driven by packaging/test-desktop.py on an isolated
//! session bus. This module is absent from the application binary.
use crate::ui::State;
use adw::prelude::*;
use gtk::{gio, glib};
use std::{path::PathBuf, rc::Rc, time::Duration};

pub fn start(s: &Rc<State>) {
    let root = PathBuf::from(std::env::var_os("LUMA_DESKTOP_TEST_ROOT").unwrap());
    let s = s.clone();
    let mut stage = 0;
    let mut ticks = 0;
    glib::timeout_add_local(Duration::from_millis(100), move || {
        ticks += 1;
        assert!(ticks < 250, "desktop D-Bus integration test timed out");
        if stage == 0 && s.tray_available.get() {
            s.window.close();
            assert!(!s.window.is_visible(), "close must hide the main window");
            assert!(
                s.background_hold.borrow().is_some(),
                "hidden app must stay alive"
            );
            std::fs::write(root.join("hidden"), b"ok").unwrap();
            let n = gio::Notification::new("Luma desktop integration test");
            n.set_body(Some("Test delivery while the main window is hidden"));
            s.app
                .send_notification(Some("desktop-integration-test"), &n);
            stage = 1;
        } else if stage == 1 && s.window.is_visible() {
            assert!(
                s.background_hold.borrow().is_none(),
                "restore releases the hold"
            );
            std::fs::write(root.join("restored"), b"ok").unwrap();
            stage = 2;
        }
        glib::ControlFlow::Continue
    });
}

#[test]
#[ignore = "requires packaging/test-desktop.py, a display and isolated D-Bus"]
fn desktop_dbus_end_to_end() {
    let root =
        PathBuf::from(std::env::var_os("LUMA_DESKTOP_TEST_ROOT").expect("test driver required"));
    assert!(
        std::env::var_os("LUMA_TEST_ROOT").is_some(),
        "test config must be isolated"
    );
    let app = adw::Application::builder()
        .application_id("io.github.lumamail.Luma")
        .build();
    app.connect_activate(|app| crate::ui::create(app, None));
    assert_eq!(
        app.run_with_args(&["luma-desktop-test"]),
        glib::ExitCode::SUCCESS
    );
    assert!(root.join("hidden").is_file());
    assert!(root.join("restored").is_file());
}
