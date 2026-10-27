//! Set bundled runtime paths before GTK/GIO create threads. Mail remains in RAM;
//! disposable WebView2/font/codec metadata lives in a private session directory.
pub fn configure() -> luma_core::Result<tempfile::TempDir> {
    let session = tempfile::Builder::new()
        .prefix("luma-mail-session-")
        .tempdir()?;
    let executable = std::env::current_exe()?;
    let bundle = executable.parent().ok_or_else(|| {
        luma_core::Error::Invalid("Cannot find Windows application directory".into())
    })?;
    // SAFETY: main calls this before GTK/GIO/Tokio or the tray thread exist.
    unsafe {
        std::env::set_var("LUMA_SESSION_RUNTIME", session.path());
        std::env::set_var("XDG_CACHE_HOME", session.path().join("cache"));
        std::env::set_var("GST_REGISTRY", session.path().join("codecs.bin"));
        std::env::set_var("GST_REGISTRY_1_0", session.path().join("codecs.bin"));
        std::env::set_var("GIO_USE_VFS", "local");
        std::env::set_var("GSETTINGS_BACKEND", "memory");
        if bundle
            .join("share/glib-2.0/schemas/gschemas.compiled")
            .is_file()
        {
            let existing = std::env::var_os("PATH").unwrap_or_default();
            let path = std::env::join_paths(
                std::iter::once(bundle.to_path_buf()).chain(std::env::split_paths(&existing)),
            )
            .map_err(|error| luma_core::Error::Invalid(error.to_string()))?;
            std::env::set_var("PATH", path);
            let template = bundle.join("lib/gdk-pixbuf-2.0/2.10.0/loaders.cache.in");
            if template.is_file() {
                let cache = std::fs::read_to_string(template)?.replace(
                    "@LUMA_BUNDLE@",
                    &bundle.to_string_lossy().replace('\\', "\\\\"),
                );
                let file = session.path().join("pixbuf-loaders.cache");
                std::fs::write(&file, cache)?;
                std::env::set_var("GDK_PIXBUF_MODULE_FILE", file);
                std::env::set_var(
                    "GDK_PIXBUF_MODULEDIR",
                    bundle.join("lib/gdk-pixbuf-2.0/2.10.0/loaders"),
                );
            }
            std::env::set_var(
                "GSETTINGS_SCHEMA_DIR",
                bundle.join("share/glib-2.0/schemas"),
            );
            std::env::set_var("XDG_DATA_DIRS", bundle.join("share"));
            std::env::set_var("GTK_DATA_PREFIX", bundle);
            std::env::set_var("GTK_EXE_PREFIX", bundle);
            std::env::set_var("GIO_MODULE_DIR", bundle.join("lib/gio/modules"));
            std::env::set_var("GST_PLUGIN_PATH_1_0", bundle.join("lib/gstreamer-1.0"));
            std::env::set_var(
                "GST_PLUGIN_SYSTEM_PATH_1_0",
                bundle.join("lib/gstreamer-1.0"),
            );
            std::env::set_var(
                "GST_PLUGIN_SCANNER_1_0",
                bundle.join("libexec/gstreamer-1.0/gst-plugin-scanner.exe"),
            );
            std::env::set_var("FONTCONFIG_FILE", bundle.join("etc/fonts/fonts.conf"));
            std::env::set_var("FONTCONFIG_PATH", bundle.join("etc/fonts"));
        }
    }
    gtk::glib::set_application_name("Luma Mail");
    Ok(session)
}
pub fn webview_runtime() -> std::path::PathBuf {
    std::env::var_os("LUMA_SESSION_RUNTIME")
        .map(std::path::PathBuf::from)
        .expect("Windows runtime configured before GTK")
        .join("webview2")
}
