//! GTK's Linux GStreamer backend aborts on MemoryInputStream. A sealed anonymous
//! RAM file gives it a seekable URI, with no path in the disk cache or /tmp.
#[cfg(target_os = "linux")]
pub fn configure() -> luma_core::Result<()> {
    let runtime = gtk::glib::user_runtime_dir().join("luma-mail");
    luma_core::config::private_dir(&runtime)?;
    let graphics = runtime.join("graphics");
    luma_core::config::private_dir(&graphics)?;
    let registry = runtime.join("gstreamer");
    luma_core::config::private_dir(&registry)?;
    // Remove only our disposable codec index from an earlier development build.
    let old = registry.join("disabled-registry.bin");
    if old.symlink_metadata().is_ok_and(|m| m.is_file()) {
        std::fs::remove_file(old)?;
    }
    let index = registry.join("codecs.bin");
    // WebKit exposes GST_REGISTRY's parent directory to its sandbox. /dev/null
    // would cause it to rebind all of /dev, breaking its private device mounts.
    // Keep the small plugin inventory in the private runtime directory instead.
    // It contains library paths/capabilities, never mail or attachment bytes.
    // Allow updates so installed codecs are detected without a full rescan on
    // every launch. GST_REGISTRY_UPDATE=no still writes an initial inventory.
    // SAFETY: called at the very start of main, before GTK/GIO/Tokio threads.
    unsafe {
        // Mesa otherwise permits a 1 GiB shader cache in ~/.cache. Keep this
        // process and its renderer children in private session storage instead.
        // The single-file Fossilize mode ignores Mesa's size limit.
        std::env::set_var("MESA_SHADER_CACHE_DIR", &graphics);
        std::env::set_var("MESA_SHADER_CACHE_MAX_SIZE", "16M");
        std::env::set_var("MESA_DISK_CACHE_SINGLE_FILE", "0");
        std::env::set_var("GST_REGISTRY", &index);
        std::env::set_var("GST_REGISTRY_1_0", &index);
        std::env::remove_var("GST_REGISTRY_UPDATE");
    }
    Ok(())
}
#[cfg(target_os = "linux")]
pub fn from_bytes(bytes: Vec<u8>) -> luma_core::Result<gtk::MediaFile> {
    use adw::prelude::*;
    use std::{
        io::Write,
        os::fd::{AsRawFd, FromRawFd},
    };
    if bytes.len() > luma_core::preview::MAX_PREVIEW_BYTES {
        return Err(luma_core::Error::ResponseLimit);
    }
    // SAFETY: constant NUL-terminated name, checked return, one owning File.
    let fd = unsafe {
        libc::memfd_create(
            c"Luma attachment".as_ptr(),
            libc::MFD_CLOEXEC | libc::MFD_ALLOW_SEALING,
        )
    };
    if fd < 0 {
        return Err(std::io::Error::last_os_error().into());
    }
    let mut file = unsafe { std::fs::File::from_raw_fd(fd) };
    file.write_all(&bytes)?;
    drop(bytes);
    let seals = libc::F_SEAL_SHRINK | libc::F_SEAL_GROW | libc::F_SEAL_WRITE | libc::F_SEAL_SEAL;
    if unsafe { libc::fcntl(file.as_raw_fd(), libc::F_ADD_SEALS, seals) } < 0 {
        return Err(std::io::Error::last_os_error().into());
    }
    let media = gtk::MediaFile::for_filename(format!("/proc/self/fd/{}", file.as_raw_fd()));
    // GStreamer may reopen the URI while seeking. Retain the descriptor exactly
    // as long as this media object, then let the kernel release its RAM.
    media.add_weak_ref_notify_local(move || drop(file));
    Ok(media)
}
