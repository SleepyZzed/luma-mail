//! Native Windows toasts. The Qt owner retains at most 64 handles in RAM.
use std::ffi::c_void;
use windows::{
    Data::Xml::Dom::XmlDocument,
    Foundation::TypedEventHandler,
    UI::Notifications::{
        ToastActivatedEventArgs, ToastFailedEventArgs, ToastNotification, ToastNotificationManager,
        ToastNotifier,
    },
    Win32::System::Com::{COINIT_APARTMENTTHREADED, CoInitializeEx, CoUninitialize},
    core::{HSTRING, IInspectable, Interface},
};
type Callback = extern "C" fn(usize, *const u8, usize);
struct Apartment(bool);
impl Drop for Apartment {
    fn drop(&mut self) {
        if self.0 {
            unsafe { CoUninitialize() }
        }
    }
}
struct Toast {
    notifier: ToastNotifier,
    notification: ToastNotification,
    activated: i64,
    failed: i64,
    _apartment: Apartment,
}
impl Drop for Toast {
    fn drop(&mut self) {
        let _ = self.notification.RemoveActivated(self.activated);
        let _ = self.notification.RemoveFailed(self.failed);
        let _ = self.notifier.Hide(&self.notification);
    }
}
fn create(input: &[u8], callback: Callback, token: usize) -> windows::core::Result<Box<Toast>> {
    let data: serde_json::Value = serde_json::from_slice(input).map_err(|_| {
        windows::core::Error::from_hresult(windows::Win32::Foundation::E_INVALIDARG)
    })?;
    let apartment = Apartment(unsafe { CoInitializeEx(None, COINIT_APARTMENTTHREADED) }.is_ok());
    let document = XmlDocument::new()?;
    document.LoadXml(&HSTRING::from(
        crate::toast_xml::document(
            data["title"].as_str().unwrap_or("New email"),
            data["body"].as_str().unwrap_or(""),
            data["copyCode"].as_bool().unwrap_or(false),
            data["activation"].as_str().unwrap_or(""),
        )
        .ok_or_else(|| {
            windows::core::Error::from_hresult(windows::Win32::Foundation::E_INVALIDARG)
        })?,
    ))?;
    let notification = ToastNotification::CreateToastNotification(&document)?;
    let handler = TypedEventHandler::<ToastNotification, IInspectable>::new(move |_, args| {
        let action = args
            .as_ref()
            .and_then(|args| args.cast::<ToastActivatedEventArgs>().ok())
            .and_then(|args| args.Arguments().ok())
            .map(|a| a.to_string_lossy())
            .unwrap_or_else(|| "open".into());
        let action = action.split(':').next().unwrap_or("");
        if action == "open" || action == "copy" {
            callback(token, action.as_ptr(), action.len());
        }
        Ok(())
    });
    let activated = notification.Activated(&handler)?;
    let failed = notification.Failed(&TypedEventHandler::<
        ToastNotification,
        ToastFailedEventArgs,
    >::new(move |_, _| {
        callback(token, b"failed".as_ptr(), 6);
        Ok(())
    }))?;
    let notifier = ToastNotificationManager::CreateToastNotifierWithId(&HSTRING::from(
        "io.github.lumamail.Luma",
    ))?;
    let toast = Box::new(Toast {
        notifier,
        notification,
        activated,
        failed,
        _apartment: apartment,
    });
    toast.notifier.Show(&toast.notification)?;
    Ok(toast)
}
#[unsafe(no_mangle)]
pub unsafe extern "C" fn luma_notification_show(
    bytes: *const u8,
    len: usize,
    callback: Callback,
    token: usize,
) -> *mut c_void {
    if bytes.is_null() || len > 16384 {
        return std::ptr::null_mut();
    }
    match std::panic::catch_unwind(|| {
        create(
            unsafe { std::slice::from_raw_parts(bytes, len) },
            callback,
            token,
        )
    }) {
        Ok(Ok(toast)) => Box::into_raw(toast).cast(),
        _ => std::ptr::null_mut(),
    }
}
#[unsafe(no_mangle)]
pub unsafe extern "C" fn luma_notification_free(toast: *mut c_void) {
    if !toast.is_null() {
        drop(unsafe { Box::from_raw(toast.cast::<Toast>()) });
    }
}
