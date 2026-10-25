mod attachments;
mod compose;
mod compose_lists;
#[cfg(debug_assertions)]
mod design_preview;
mod desktop;
#[cfg(all(test, target_os = "linux"))]
mod desktop_test;
mod google;
mod media;
mod message_images;
mod notifications;
mod rendering;
mod search;
mod sender_controls;
mod settings;
mod signin;
mod tray;
mod ui;
use adw::prelude::*;
use gtk::{gio, glib};
fn main() -> glib::ExitCode {
    #[cfg(target_os = "linux")]
    if let Err(error) = media::configure() {
        eprintln!("Cannot prepare private media runtime: {error}");
        return glib::ExitCode::FAILURE;
    }
    let flags = if cfg!(debug_assertions) && std::env::args().any(|a| a == "--design-preview") {
        gio::ApplicationFlags::HANDLES_OPEN | gio::ApplicationFlags::NON_UNIQUE
    } else {
        gio::ApplicationFlags::HANDLES_OPEN
    };
    let app = adw::Application::builder()
        .application_id("io.github.lumamail.Luma")
        .flags(flags)
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
            if std::env::args().any(|a| a == "--desktop-diagnostics") {
                println!("GTK cursor: {}", crate::desktop::cursor_diagnostic());
                if let Some(source) = gio::SettingsSchemaSource::default()
                    && let Some(schema) = source.lookup("org.gnome.desktop.interface", true)
                {
                    let settings =
                        gio::Settings::new_full(&schema, None::<&gio::SettingsBackend>, None);
                    println!(
                        "Desktop settings backend: {}",
                        settings
                            .backend()
                            .map(|b| b.type_().name().to_string())
                            .unwrap_or_else(|| "Default backend".into())
                    );
                }
            }
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
    } else if std::env::args().any(|a| a == "--render-diagnostics")
        || (cfg!(debug_assertions) && std::env::args().any(|a| a == "--design-preview"))
    {
        app.run_with_args(&["luma-mail"])
    } else {
        app.run()
    }
}
