//! GTK's Linux GStreamer backend aborts on MemoryInputStream. A sealed anonymous
//! RAM file gives it a seekable URI, with no path in the disk cache or /tmp.
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
