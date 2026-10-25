mod compose;
mod notifications;
mod settings;
mod tray;
mod ui;
use adw::prelude::*;
use gtk::{gio, glib};
fn main() -> glib::ExitCode {
    let app = adw::Application::builder()
        .application_id("io.github.lumamail.Luma")
        .flags(gio::ApplicationFlags::HANDLES_OPEN)
        .build();
    app.connect_activate(|app| {
        if let Some(window) = app.active_window() {
            window.present();
            return;
        }
        ui::create(app, None);
    });
    app.connect_open(|app, files, _| ui::create(app, files.first().map(|f| f.uri().to_string())));
    if std::env::args().any(|a| a == "--smoke-test") {
        app.connect_activate(|app| {
            let app = app.clone();
            glib::timeout_add_local_once(std::time::Duration::from_secs(3), move || {
                if let Some(path) = std::env::var_os("LUMA_SMOKE_SCREENSHOT")
                    && let Some(window) = app.active_window()
                {
                    let paintable = gtk::WidgetPaintable::new(Some(&window));
                    let snapshot = gtk::Snapshot::new();
                    paintable.snapshot(&snapshot, window.width() as f64, window.height() as f64);
                    if let Some(node) = snapshot.to_node()
                        && let Some(renderer) = window.renderer()
                    {
                        let texture = renderer.render_texture(&node, None);
                        if let Err(e) = texture.save_to_png(path) {
                            eprintln!("Launch screenshot failed: {e}");
                        }
                    }
                }
                app.quit();
            });
        });
        app.run_with_args(&["luma-mail"])
    } else {
        app.run()
    }
}
