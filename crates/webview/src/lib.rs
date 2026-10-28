//! GTK-free WebView2 child for Qt Quick on Windows. The private ABI is called
//! only on the Qt GUI thread; no credentials, HTTP client or mailbox lives here.
#[cfg(target_os = "windows")]
mod toast;
#[cfg(any(test, target_os = "windows"))]
mod toast_xml;
#[cfg(target_os = "windows")]
mod windows;
