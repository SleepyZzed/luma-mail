use raw_window_handle::{
    HandleError, HasWindowHandle, RawWindowHandle, Win32WindowHandle, WindowHandle,
};
use serde_json::json;
use std::{
    borrow::Cow,
    cell::Cell,
    ffi::c_void,
    num::NonZeroIsize,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
};
use wry::{WebView, WebViewBuilder, WebViewBuilderExtWindows, WebViewExtWindows};
const URL: &str = "http://luma.mail/document";
const LIMIT: usize = 12 * 1024 * 1024;
type Callback = extern "C" fn(*mut c_void, *const u8, usize);
struct Parent(isize);
impl HasWindowHandle for Parent {
    fn window_handle(&self) -> Result<WindowHandle<'_>, HandleError> {
        let handle =
            Win32WindowHandle::new(NonZeroIsize::new(self.0).ok_or(HandleError::Unavailable)?);
        // The host retains its native window until all child surfaces are dropped.
        Ok(unsafe { WindowHandle::borrow_raw(RawWindowHandle::Win32(handle)) })
    }
}
struct Events {
    live: AtomicBool,
    callback: Callback,
    context: usize,
}
impl Events {
    fn emit(&self, value: serde_json::Value) {
        if self.live.load(Ordering::SeqCst) {
            let bytes = value.to_string();
            (self.callback)(self.context as *mut c_void, bytes.as_ptr(), bytes.len());
        }
    }
}
pub struct Surface {
    view: WebView,
    document: Arc<Mutex<String>>,
    events: Arc<Events>,
    scroll: Cell<f64>,
}
impl Drop for Surface {
    fn drop(&mut self) {
        self.events.live.store(false, Ordering::SeqCst);
    }
}
// All raw pointers in this module belong to the private native host ABI. They
// must refer to live objects/buffers on the same GUI thread, never mail input.
unsafe fn text<'a>(bytes: *const u8, size: usize) -> Option<&'a str> {
    if bytes.is_null() || size > LIMIT {
        return None;
    }
    std::str::from_utf8(unsafe { std::slice::from_raw_parts(bytes, size) }).ok()
}
#[unsafe(no_mangle)]
pub unsafe extern "C" fn luma_webview_create(
    parent: isize,
    bytes: *const u8,
    size: usize,
    runtime: *const u8,
    runtime_size: usize,
    callback: Callback,
    context: *mut c_void,
) -> *mut Surface {
    let (Some(page), Some(runtime)) = (unsafe { text(bytes, size) }, unsafe {
        text(runtime, runtime_size)
    }) else {
        return std::ptr::null_mut();
    };
    let events = Arc::new(Events {
        live: AtomicBool::new(true),
        callback,
        context: context as usize,
    });
    let document = Arc::new(Mutex::new(page.to_owned()));
    let data = document.clone();
    let navigation = events.clone();
    let new = events.clone();
    let load = events.clone();
    let ipc = events.clone();
    let mut web_context = wry::WebContext::new(Some(std::path::PathBuf::from(runtime)));
    let view=WebViewBuilder::new_with_web_context(&mut web_context)
        .with_transparent(true).with_incognito(true).with_javascript_disabled().with_devtools(false)
        .with_focused(false).with_visible(false).with_default_context_menus(false).with_browser_accelerator_keys(false)
        .with_additional_browser_args("--disk-cache-size=1 --media-cache-size=1 --disable-gpu-shader-disk-cache --disable-background-networking --disable-component-update --no-first-run")
        .with_custom_protocol("luma".into(),move |_,_|wry::http::Response::builder().header("Content-Type","text/html; charset=utf-8").header("Cache-Control","no-store").header("Referrer-Policy","no-referrer").body(Cow::Owned(data.lock().unwrap_or_else(|p|p.into_inner()).as_bytes().to_vec())).expect("static headers"))
        .with_navigation_handler(move |uri|{if uri==URL{return true}else if safe_link(&uri){navigation.emit(json!({"kind":"link","url":uri}));}false})
        .with_new_window_req_handler(move |uri,_|{if safe_link(&uri){new.emit(json!({"kind":"link","url":uri}));}wry::NewWindowResponse::Deny})
        .with_on_page_load_handler(move |event,_|{if matches!(event,wry::PageLoadEvent::Finished){load.emit(json!({"kind":"loaded"}));}})
        .with_ipc_handler(move |request|{if let Ok(delta)=request.body().parse::<f64>()&&delta.is_finite(){ipc.emit(json!({"kind":"wheel","delta":delta.clamp(-3000.,3000.)}));}})
        .with_download_started_handler(|_,_|false).with_url("luma://mail/document").build_as_child(&Parent(parent));
    match view {
        Ok(view) => Box::into_raw(Box::new(Surface {
            view,
            document,
            events,
            scroll: Cell::new(-1.),
        })),
        Err(error) => {
            events.emit(json!({"kind":"error","message":format!("HTML view: {error}. Microsoft Edge WebView2 Runtime is required.")}));
            std::ptr::null_mut()
        }
    }
}
fn safe_link(uri: &str) -> bool {
    uri.starts_with("https://") || uri.starts_with("http://") || uri.starts_with("mailto:")
}
#[unsafe(no_mangle)]
pub unsafe extern "C" fn luma_webview_destroy(surface: *mut Surface) {
    if !surface.is_null() {
        drop(unsafe { Box::from_raw(surface) });
    }
}
#[unsafe(no_mangle)]
pub unsafe extern "C" fn luma_webview_document(
    surface: *mut Surface,
    bytes: *const u8,
    size: usize,
) {
    if let (Some(s), Some(page)) = (unsafe { surface.as_ref() }, unsafe { text(bytes, size) }) {
        *s.document.lock().unwrap_or_else(|p| p.into_inner()) = page.into();
        let _ = s.view.load_url(URL);
    }
}
#[unsafe(no_mangle)]
pub unsafe extern "C" fn luma_webview_visible(surface: *mut Surface, visible: bool) {
    if let Some(s) = unsafe { surface.as_ref() } {
        let _ = s.view.set_visible(visible);
    }
}
#[unsafe(no_mangle)]
pub unsafe extern "C" fn luma_webview_geometry(
    surface: *mut Surface,
    rects: *const i32,
    count: usize,
    zoom: f64,
    density: f64,
) {
    let Some(s) = (unsafe { surface.as_ref() }) else {
        return;
    };
    if rects.is_null()
        || !(2..=10).contains(&count)
        || !zoom.is_finite()
        || !density.is_finite()
        || density <= 0.
    {
        return;
    }
    let rects = unsafe { std::slice::from_raw_parts(rects, count * 4) };
    let full = &rects[..4];
    let c = &rects[4..8];
    // Only the visible vertical slice receives a native compositor surface.
    // The DOM keeps its full layout height; the host controls its scroll offset.
    let b = &[full[0], c[1], full[2], c[3]];
    let _ = s.view.set_bounds(wry::Rect {
        position: wry::dpi::PhysicalPosition::new(b[0], b[1]).into(),
        size: wry::dpi::PhysicalSize::new(b[2].max(1) as u32, b[3].max(1) as u32).into(),
    });
    let _ = s.view.zoom(zoom.clamp(0.5, 3.));
    let offset = f64::from(c[1] - full[1]) / density / zoom;
    if s.scroll.replace(offset) != offset {
        let _ = s
            .view
            .evaluate_script(&format!("window.scrollTo(0,{offset})"));
    }
    unsafe {
        let mut hwnd = Default::default();
        if s.view.controller().ParentWindow(&mut hwnd).is_ok() {
            use windows_sys::Win32::Graphics::Gdi::*;
            let region = CreateRectRgn(
                c[0] - b[0],
                c[1] - b[1],
                c[0] + c[2] - b[0],
                c[1] + c[3] - b[1],
            );
            for r in rects[8..].as_chunks::<4>().0 {
                let excluded = CreateRectRgn(
                    r[0] - b[0],
                    r[1] - b[1],
                    r[0] + r[2] - b[0],
                    r[1] + r[3] - b[1],
                );
                CombineRgn(region, region, excluded, RGN_DIFF);
                DeleteObject(excluded);
            }
            if SetWindowRgn(hwnd.0, region, 1) == 0 {
                DeleteObject(region);
            }
        }
    }
}
#[unsafe(no_mangle)]
pub unsafe extern "C" fn luma_webview_measure(surface: *mut Surface, width: i32, generation: u64) {
    let Some(s) = (unsafe { surface.as_ref() }) else {
        return;
    };
    let events = s.events.clone();
    // Reload discards scroll state even when outer geometry is unchanged.
    let offset = s.scroll.get().max(0.);
    let _ = s
        .view
        .evaluate_script(&format!("window.scrollTo(0,{offset})"));
    // Host-only measurement. Sender JavaScript stays disabled and stripped.
    let _=s.view.evaluate_script_with_callback("String(Math.ceil(document.body?document.body.getBoundingClientRect().height:48))",move |value|{
        if let Ok(value)=serde_json::from_str::<String>(&value)&&let Ok(height)=value.parse::<f64>()&&height.is_finite(){events.emit(json!({"kind":"height","height":height.clamp(24.,60000.),"width":width,"generation":generation}));}
    });
    let _=s.view.evaluate_script("if(!window.lumaWheel){window.lumaWheel=true;addEventListener('wheel',e=>{if(!e.ctrlKey){e.preventDefault();window.ipc.postMessage(String(e.deltaY*(e.deltaMode===1?24:e.deltaMode===2?innerHeight:1)));}},{passive:false});}");
}
