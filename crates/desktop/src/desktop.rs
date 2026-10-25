use gtk::gio;
#[cfg(target_os = "linux")]
use gtk::prelude::*;

/// Follow the session cursor without shipping a competing cursor preference.
/// Keep this Settings object alive so dconf changes propagate while running.
#[cfg(target_os = "linux")]
pub fn follow_cursor() -> Option<gio::Settings> {
    let gtk = gtk::Settings::default()?;
    let theme = std::env::var("XCURSOR_THEME")
        .ok()
        .filter(|v| !v.trim().is_empty());
    let size = std::env::var("XCURSOR_SIZE")
        .ok()
        .and_then(|v| v.parse::<i32>().ok())
        .filter(|v| (8..=256).contains(v));
    let source = gio::SettingsSchemaSource::default()?;
    let schema = source.lookup("org.gnome.desktop.interface", true)?;
    if !schema.has_key("cursor-theme") || !schema.has_key("cursor-size") {
        return None;
    }
    let settings = gio::Settings::new_full(&schema, None::<&gio::SettingsBackend>, None);
    let apply = move |settings: &gio::Settings| {
        let configured = settings
            .user_value("cursor-theme")
            .and_then(|v| v.get::<String>());
        if let Some(name) = theme.as_ref().or(configured.as_ref()) {
            gtk.set_gtk_cursor_theme_name(Some(name));
        } else {
            gtk.reset_property("gtk-cursor-theme-name");
        }
        if let Some(value) = size.or_else(|| {
            settings
                .user_value("cursor-size")
                .and_then(|v| v.get::<i32>())
        }) {
            if (8..=256).contains(&value) {
                gtk.set_gtk_cursor_theme_size(value);
            }
        } else {
            gtk.reset_property("gtk-cursor-theme-size");
        }
    };
    apply(&settings);
    settings.connect_changed(None, move |settings, key| {
        if matches!(key, "cursor-theme" | "cursor-size") {
            apply(settings);
        }
    });
    Some(settings)
}

#[cfg(not(target_os = "linux"))]
pub fn follow_cursor() -> Option<gio::Settings> {
    None
}

pub fn cursor_diagnostic() -> String {
    gtk::Settings::default()
        .map(|s| {
            format!(
                "{} · {} px",
                s.gtk_cursor_theme_name()
                    .as_deref()
                    .unwrap_or("System default"),
                s.gtk_cursor_theme_size()
            )
        })
        .unwrap_or_else(|| "GTK settings unavailable".into())
}
