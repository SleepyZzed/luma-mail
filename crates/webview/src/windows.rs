//! Asynchronous private WebView2 child, confined to Qt's GUI STA.
use serde_json::json;
use std::{
    cell::{Cell, RefCell},
    ffi::c_void,
    fs::{File, OpenOptions},
    os::windows::fs::OpenOptionsExt,
    rc::Rc,
};
use webview2_com::{Microsoft::Web::WebView2::Win32::*, *};
use windows::{
    Win32::{
        Foundation::{COLORREF, E_POINTER, HINSTANCE, HWND, LPARAM, LRESULT, RECT, WPARAM},
        Graphics::Gdi::*,
        System::{
            Com::{COINIT_APARTMENTTHREADED, CoInitializeEx, CoUninitialize},
            LibraryLoader::GetModuleHandleW,
        },
        UI::{Shell::SHCreateMemStream, WindowsAndMessaging::*},
    },
    core::{HSTRING, Interface, PCWSTR, PWSTR, Result, w},
};
const URL: &str = "http://luma.mail/document";
const LIMIT: usize = 12 * 1024 * 1024;
// A STATIC control paints the system's white background during resize, even
// when WebView2 itself is themed. Own the host paint so that no startup/resize
// frame can fall back to a caption or an OS-default white rectangle.
unsafe extern "system" fn host_paint(
    window: HWND,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    unsafe {
        if message == WM_ERASEBKGND || message == WM_PAINT {
            let mut paint = PAINTSTRUCT::default();
            let dc = if message == WM_PAINT {
                BeginPaint(window, &mut paint)
            } else {
                HDC(wparam.0 as *mut c_void)
            };
            let mut rect = RECT::default();
            let _ = GetClientRect(window, &mut rect);
            let brush = CreateSolidBrush(COLORREF(GetWindowLongPtrW(window, GWLP_USERDATA) as u32));
            FillRect(dc, &rect, brush);
            let _ = DeleteObject(brush.into());
            if message == WM_PAINT {
                let _ = EndPaint(window, &paint);
            }
            return LRESULT(1);
        }
        DefWindowProcW(window, message, wparam, lparam)
    }
}
fn host_class() -> Option<HINSTANCE> {
    static CLASS: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    let module = unsafe { GetModuleHandleW(None).ok()? };
    let instance = HINSTANCE(module.0);
    let valid = CLASS.get_or_init(|| unsafe {
        RegisterClassW(&WNDCLASSW {
            lpfnWndProc: Some(host_paint),
            hInstance: instance,
            lpszClassName: w!("LumaPrivateHtmlHost"),
            ..Default::default()
        }) != 0
    });
    valid.then_some(instance)
}
struct Apartment(bool);
impl Drop for Apartment {
    fn drop(&mut self) {
        if self.0 {
            unsafe { CoUninitialize() }
        }
    }
}
thread_local! {static APARTMENT:Apartment=Apartment(unsafe{CoInitializeEx(None,COINIT_APARTMENTTHREADED)}.is_ok());}
type Callback = extern "C" fn(*mut c_void, *const u8, usize);
#[derive(PartialEq)]
struct Geometry {
    rects: Vec<i32>,
    zoom: f64,
    density: f64,
}
struct State {
    child: HWND,
    callback: Callback,
    context: *mut c_void,
    live: Cell<bool>,
    document: RefCell<String>,
    controller: RefCell<Option<ICoreWebView2Controller>>,
    web: RefCell<Option<ICoreWebView2>>,
    geometry: RefCell<Option<Geometry>>,
    visible: Cell<bool>,
    ready: Cell<bool>,
    revision: Cell<u64>,
    navigation: Cell<u64>,
    background: Cell<u32>,
    reveal: Cell<f64>,
    scroll: Cell<f64>,
    initialization_lock: RefCell<Option<File>>,
}
impl State {
    fn colour(&self) -> COREWEBVIEW2_COLOR {
        let argb = self.background.get();
        COREWEBVIEW2_COLOR {
            A: 255,
            R: (argb >> 16) as u8,
            G: (argb >> 8) as u8,
            B: argb as u8,
        }
    }
    fn emit(&self, value: serde_json::Value) {
        if self.live.get() {
            let bytes = value.to_string();
            (self.callback)(self.context, bytes.as_ptr(), bytes.len());
        }
    }
    fn fail(&self, error: impl std::fmt::Display) {
        self.emit(json!({"kind":"error","message":format!("HTML view: {error}. Microsoft Edge WebView2 Runtime is required.")}));
    }
    fn script(&self, script: &str) {
        if let Some(web) = self.web.borrow().as_ref() {
            unsafe {
                let _ = web.ExecuteScript(
                    &HSTRING::from(script),
                    &ExecuteScriptCompletedHandler::create(Box::new(|_, _| Ok(()))),
                );
            }
        }
    }
    fn apply_geometry(&self) {
        let geometry = self.geometry.borrow();
        let Some(g) = geometry.as_ref() else { return };
        let full = &g.rects[..4];
        let clip = &g.rects[4..8];
        let width = full[2].max(1);
        let height = clip[3].max(1);
        unsafe {
            let _ = MoveWindow(self.child, full[0], clip[1], width, height, true);
            if let Some(controller) = self.controller.borrow().as_ref() {
                let _ = controller.SetBounds(RECT {
                    left: 0,
                    top: 0,
                    right: width,
                    bottom: height,
                });
                let _ = controller.SetZoomFactor(g.zoom);
            }
        }
        self.apply_clip();
        let offset = f64::from(clip[1] - full[1]) / g.density / g.zoom;
        if self.scroll.replace(offset) != offset {
            self.script(&format!("window.scrollTo(0,{offset})"));
        }
    }
    fn apply_clip(&self) {
        let geometry = self.geometry.borrow();
        let Some(g) = geometry.as_ref() else { return };
        let full = &g.rects[..4];
        let clip = &g.rects[4..8];
        unsafe {
            let progress = self.reveal.get();
            let unit = g.zoom * g.density;
            let width = (f64::from(clip[2]).min(260. * unit) * (1. - progress)
                + f64::from(clip[2]) * progress)
                .ceil() as i32;
            let height = (f64::from(clip[3]).min(38. * unit) * (1. - progress)
                + f64::from(clip[3]) * progress)
                .ceil() as i32;
            let corner = (38. * unit * (1. - progress)).round() as i32;
            let region = CreateRoundRectRgn(
                clip[0] - full[0],
                0,
                clip[0] - full[0] + width,
                height,
                corner,
                corner,
            );
            for r in g.rects[8..].as_chunks::<4>().0 {
                let excluded = CreateRectRgn(
                    r[0] - full[0],
                    r[1] - clip[1],
                    r[0] + r[2] - full[0],
                    r[1] + r[3] - clip[1],
                );
                CombineRgn(Some(region), Some(region), Some(excluded), RGN_DIFF);
                let _ = DeleteObject(excluded.into());
            }
            if SetWindowRgn(self.child, Some(region), true) == 0 {
                let _ = DeleteObject(region.into());
            }
        }
    }
    fn apply_visible(&self) {
        unsafe {
            let visible = self.live.get()
                && self.visible.get()
                && self.ready.get()
                && self.controller.borrow().is_some();
            if let Some(controller) = self.controller.borrow().as_ref() {
                let _ = controller.SetIsVisible(visible);
            }
            let _ = ShowWindow(self.child, if visible { SW_SHOWNA } else { SW_HIDE });
        }
    }
}
impl Drop for State {
    fn drop(&mut self) {
        unsafe {
            self.live.set(false);
            self.web.get_mut().take();
            if let Some(controller) = self.controller.get_mut().take() {
                let _ = controller.Close();
            }
            let _ = DestroyWindow(self.child);
            self.initialization_lock.get_mut().take();
        }
    }
}
pub struct Surface(Rc<State>);
impl Drop for Surface {
    fn drop(&mut self) {
        self.0.live.set(false);
        *self.0.document.borrow_mut() = String::new();
        self.0.geometry.borrow_mut().take();
        self.0.apply_visible();
        if let Some(controller) = self.0.controller.borrow_mut().take() {
            unsafe {
                let _ = controller.Close();
            }
        }
        self.0.web.borrow_mut().take();
    }
}
unsafe fn text<'a>(bytes: *const u8, size: usize) -> Option<&'a str> {
    if bytes.is_null() || size > LIMIT {
        return None;
    }
    std::str::from_utf8(unsafe { std::slice::from_raw_parts(bytes, size) }).ok()
}
fn take_string(value: PWSTR) -> String {
    CoTaskMemPWSTR::from(value).to_string()
}
fn safe_link(uri: &str) -> bool {
    uri.starts_with("https://") || uri.starts_with("http://") || uri.starts_with("mailto:")
}
fn initialize(state: Rc<State>, runtime: &str) -> Result<()> {
    unsafe {
        let options = CoreWebView2EnvironmentOptions::default();
        options.set_additional_browser_arguments("--disk-cache-size=1 --media-cache-size=1 --disable-gpu-shader-disk-cache --disable-background-networking --disable-component-update --no-first-run".into());
        options.set_are_browser_extensions_enabled(false);
        // Normal asynchronous COM callbacks keep Qt's own event loop, springs and
        // cancellation responsive. No wait_with_pump or nested dispatch loop.
        CreateCoreWebView2EnvironmentWithOptions(
            PCWSTR::null(),
            &HSTRING::from(runtime),
            &ICoreWebView2EnvironmentOptions::from(options),
            &CreateCoreWebView2EnvironmentCompletedHandler::create(Box::new(
                move |result, environment| {
                    if !state.live.get() {
                        return Ok(());
                    }
                    let result: Result<()> = (|| {
                        result?;
                        let environment =
                            environment.ok_or_else(|| windows::core::Error::from(E_POINTER))?;
                        // Require a private controller; never fall back to a persistent
                        // browser profile on an obsolete WebView2 runtime.
                        let env10 = environment.cast::<ICoreWebView2Environment10>()?;
                        let options = env10.CreateCoreWebView2ControllerOptions()?;
                        options.SetIsInPrivateModeEnabled(true)?;
                        if let Ok(colour) = options.cast::<ICoreWebView2ControllerOptions3>() {
                            colour.SetDefaultBackgroundColor(state.colour())?;
                        }
                        let pending = state.clone();
                        env10.CreateCoreWebView2ControllerWithOptions(
                            state.child,
                            &options,
                            &CreateCoreWebView2ControllerCompletedHandler::create(Box::new(
                                move |result, controller| {
                                    let result: Result<()> = (|| {
                                        result?;
                                        let controller = controller
                                            .ok_or_else(|| windows::core::Error::from(E_POINTER))?;
                                        if !pending.live.get() {
                                            controller.Close()?;
                                            return Ok(());
                                        }
                                        *pending.controller.borrow_mut() = Some(controller.clone());
                                        attach(&pending, &environment, &controller)
                                    })(
                                    );
                                    pending.initialization_lock.borrow_mut().take();
                                    if let Err(error) = result {
                                        pending.fail(error);
                                        if let Some(controller) =
                                            pending.controller.borrow_mut().take()
                                        {
                                            let _ = controller.Close();
                                        }
                                        pending.web.borrow_mut().take();
                                    }
                                    Ok(())
                                },
                            )),
                        )
                    })();
                    if let Err(error) = result {
                        state.initialization_lock.borrow_mut().take();
                        state.fail(error);
                    }
                    Ok(())
                },
            )),
        )
    }
}
fn attach(
    state: &Rc<State>,
    environment: &ICoreWebView2Environment,
    controller: &ICoreWebView2Controller,
) -> Result<()> {
    unsafe {
        controller.SetIsVisible(false)?;
        controller
            .cast::<ICoreWebView2Controller2>()?
            .SetDefaultBackgroundColor(state.colour())?;
        let web = controller.CoreWebView2()?;
        let settings = web.Settings()?;
        settings.SetIsScriptEnabled(false)?;
        settings.SetAreHostObjectsAllowed(false)?;
        settings.SetAreDefaultScriptDialogsEnabled(false)?;
        settings.SetIsWebMessageEnabled(true)?;
        settings.SetIsStatusBarEnabled(false)?;
        settings.SetAreDefaultContextMenusEnabled(false)?;
        settings.SetIsZoomControlEnabled(false)?;
        settings.SetAreDevToolsEnabled(false)?;
        settings
            .cast::<ICoreWebView2Settings3>()?
            .SetAreBrowserAcceleratorKeysEnabled(false)?;
        let settings4 = settings.cast::<ICoreWebView2Settings4>()?;
        settings4.SetIsGeneralAutofillEnabled(false)?;
        settings4.SetIsPasswordAutosaveEnabled(false)?;
        settings
            .cast::<ICoreWebView2Settings5>()?
            .SetIsPinchZoomEnabled(false)?;
        settings
            .cast::<ICoreWebView2Settings6>()?
            .SetIsSwipeNavigationEnabled(false)?;
        let mut token = 0i64;
        web.AddWebResourceRequestedFilter(w!("*"), COREWEBVIEW2_WEB_RESOURCE_CONTEXT_ALL)?;
        let weak = Rc::downgrade(state);
        let environment = environment.clone();
        web.add_WebResourceRequested(&WebResourceRequestedEventHandler::create(Box::new(move |_,args|{
        let (Some(state),Some(args))=(weak.upgrade(),args)else{return Ok(())};
        let request=args.Request()?;let mut uri=PWSTR::null();request.Uri(&mut uri)?;let uri=take_string(uri);
        // Only the engine's sanitized RAM document is served. External/file
        // subresources are denied even if upstream sanitization regresses.
        let page=state.document.borrow();let permitted=state.live.get()&&uri==URL;
        let body=if permitted{page.as_bytes()}else{b"Blocked"};
        let stream=SHCreateMemStream(Some(body)).ok_or_else(||windows::core::Error::from(E_POINTER))?;
        let response=environment.CreateWebResourceResponse(&stream,if permitted{200}else{403},if permitted{w!("OK")}else{w!("Blocked")},w!("Content-Type: text/html; charset=utf-8\r\nCache-Control: no-store\r\nReferrer-Policy: no-referrer\r\nX-Content-Type-Options: nosniff"))?;args.SetResponse(&response)
    })),&mut token)?;
        let weak = Rc::downgrade(state);
        web.add_NavigationStarting(
            &NavigationStartingEventHandler::create(Box::new(move |_, args| {
                let Some(args) = args else { return Ok(()) };
                let mut uri = PWSTR::null();
                args.Uri(&mut uri)?;
                let uri = take_string(uri);
                let permitted = weak.upgrade().is_some_and(|state| {
                    if uri == URL && state.live.get() {
                        let mut id = 0;
                        if args.NavigationId(&mut id).is_err() {
                            return false;
                        }
                        state.navigation.set(id);
                        state.ready.set(false);
                        state.apply_visible();
                        true
                    } else {
                        if safe_link(&uri) {
                            state.emit(json!({"kind":"link","url":uri}));
                        }
                        false
                    }
                });
                args.SetCancel(!permitted)
            })),
            &mut token,
        )?;
        web.add_FrameNavigationStarting(
            &NavigationStartingEventHandler::create(Box::new(|_, args| {
                if let Some(args) = args {
                    args.SetCancel(true)?;
                }
                Ok(())
            })),
            &mut token,
        )?;
        let weak = Rc::downgrade(state);
        web.add_NewWindowRequested(
            &NewWindowRequestedEventHandler::create(Box::new(move |_, args| {
                if let Some(args) = args {
                    args.SetHandled(true)?;
                    let mut uri = PWSTR::null();
                    args.Uri(&mut uri)?;
                    let uri = take_string(uri);
                    if let Some(state) = weak.upgrade()
                        && safe_link(&uri)
                    {
                        state.emit(json!({"kind":"link","url":uri}));
                    }
                }
                Ok(())
            })),
            &mut token,
        )?;
        web.add_PermissionRequested(
            &PermissionRequestedEventHandler::create(Box::new(|_, args| {
                if let Some(args) = args {
                    args.SetState(COREWEBVIEW2_PERMISSION_STATE_DENY)?;
                }
                Ok(())
            })),
            &mut token,
        )?;
        web.cast::<ICoreWebView2_4>()?.add_DownloadStarting(
            &DownloadStartingEventHandler::create(Box::new(|_, args| {
                if let Some(args) = args {
                    args.SetCancel(true)?;
                    args.SetHandled(true)?;
                }
                Ok(())
            })),
            &mut token,
        )?;
        let weak = Rc::downgrade(state);
        web.add_NavigationCompleted(
            &NavigationCompletedEventHandler::create(Box::new(move |_, args| {
                if let (Some(state), Some(args)) = (weak.upgrade(), args) {
                    let mut id = 0;
                    args.NavigationId(&mut id)?;
                    if id != state.navigation.get() {
                        return Ok(());
                    }
                    let mut success = Default::default();
                    args.IsSuccess(&mut success)?;
                    if success.as_bool() {
                        state.ready.set(true);
                        state.scroll.set(-1.);
                        state.apply_geometry();
                        state.apply_visible();
                        state.emit(json!({"kind":"loaded","revision":state.revision.get()}));
                    } else {
                        let mut status = COREWEBVIEW2_WEB_ERROR_STATUS::default();
                        args.WebErrorStatus(&mut status)?;
                        if status != COREWEBVIEW2_WEB_ERROR_STATUS_OPERATION_CANCELED {
                            state.fail(format!("document load failed ({})", status.0));
                        }
                    }
                }
                Ok(())
            })),
            &mut token,
        )?;
        let weak = Rc::downgrade(state);
        web.add_WebMessageReceived(
            &WebMessageReceivedEventHandler::create(Box::new(move |_, args| {
                if let (Some(state), Some(args)) = (weak.upgrade(), args) {
                    let mut source = PWSTR::null();
                    args.Source(&mut source)?;
                    if take_string(source) != URL {
                        return Ok(());
                    }
                    let mut value = PWSTR::null();
                    args.TryGetWebMessageAsString(&mut value)?;
                    if let Ok(delta) = take_string(value).parse::<f64>()
                        && delta.is_finite()
                    {
                        state.emit(json!({"kind":"wheel","delta":delta.clamp(-3000.,3000.)}));
                    }
                }
                Ok(())
            })),
            &mut token,
        )?;
        let weak = Rc::downgrade(state);
        web.add_ProcessFailed(
            &ProcessFailedEventHandler::create(Box::new(move |_, _| {
                if let Some(state) = weak.upgrade() {
                    state.fail("the private HTML renderer stopped; reopen the message to retry");
                }
                Ok(())
            })),
            &mut token,
        )?;
        *state.web.borrow_mut() = Some(web.clone());
        state.apply_geometry();
        state.apply_visible();
        web.Navigate(&HSTRING::from(URL))
    }
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
    if !APARTMENT.with(|apartment| apartment.0) {
        return std::ptr::null_mut();
    }
    let Some(instance) = host_class() else {
        return std::ptr::null_mut();
    };
    let child = unsafe {
        CreateWindowExW(
            WINDOW_EX_STYLE::default(),
            w!("LumaPrivateHtmlHost"),
            w!(""),
            WS_CHILD | WS_CLIPSIBLINGS | WS_CLIPCHILDREN,
            0,
            0,
            1,
            1,
            Some(HWND(parent as *mut c_void)),
            None,
            Some(instance),
            None,
        )
    };
    let Ok(child) = child else {
        return std::ptr::null_mut();
    };
    unsafe {
        SetWindowLongPtrW(child, GWLP_USERDATA, 0x001e1719);
    }
    // Delete-on-close initialization lease: runtime cleanup cannot race a late
    // COM completion. It contains no content and survives no process exit.
    let lock = OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .share_mode(0)
        .custom_flags(0x04000000)
        .open(
            std::path::Path::new(runtime).join(format!("initializing-{}.lock", child.0 as usize)),
        );
    let Ok(lock) = lock else {
        unsafe {
            let _ = DestroyWindow(child);
        };
        return std::ptr::null_mut();
    };
    let state = Rc::new(State {
        child,
        callback,
        context,
        live: Cell::new(true),
        document: RefCell::new(page.into()),
        controller: RefCell::new(None),
        web: RefCell::new(None),
        geometry: RefCell::new(None),
        visible: Cell::new(false),
        ready: Cell::new(false),
        revision: Cell::new(0),
        navigation: Cell::new(0),
        background: Cell::new(0xff19171e),
        reveal: Cell::new(1.),
        scroll: Cell::new(-1.),
        initialization_lock: RefCell::new(Some(lock)),
    });
    if let Err(error) = initialize(state.clone(), runtime) {
        state.fail(error);
        return std::ptr::null_mut();
    }
    Box::into_raw(Box::new(Surface(state)))
}
#[unsafe(no_mangle)]
pub unsafe extern "C" fn luma_webview_destroy(surface: *mut Surface) {
    if !surface.is_null() {
        drop(unsafe { Box::from_raw(surface) })
    }
}
#[unsafe(no_mangle)]
pub unsafe extern "C" fn luma_webview_document(
    surface: *mut Surface,
    bytes: *const u8,
    size: usize,
) {
    if let (Some(surface), Some(page)) = (unsafe { surface.as_ref() }, unsafe { text(bytes, size) })
    {
        *surface.0.document.borrow_mut() = page.into();
        surface
            .0
            .revision
            .set(surface.0.revision.get().wrapping_add(1));
        surface.0.navigation.set(0);
        surface.0.ready.set(false);
        surface.0.apply_visible();
        if let Some(web) = surface.0.web.borrow().as_ref() {
            unsafe {
                let _ = web.Navigate(&HSTRING::from(URL));
            }
        }
    }
}
#[unsafe(no_mangle)]
pub unsafe extern "C" fn luma_webview_visible(surface: *mut Surface, visible: bool) {
    if let Some(surface) = unsafe { surface.as_ref() }
        && surface.0.visible.replace(visible) != visible
    {
        surface.0.apply_visible();
    }
}
#[unsafe(no_mangle)]
pub unsafe extern "C" fn luma_webview_background(surface: *mut Surface, argb: u32) {
    if let Some(surface) = unsafe { surface.as_ref() }
        && surface.0.background.replace(argb) != argb
    {
        unsafe {
            SetWindowLongPtrW(
                surface.0.child,
                GWLP_USERDATA,
                ((argb & 255) << 16 | (argb & 0xff00) | ((argb >> 16) & 255)) as isize,
            );
            let _ = InvalidateRect(Some(surface.0.child), None, true);
        }
        if let Some(controller) = surface.0.controller.borrow().as_ref() {
            unsafe {
                if let Ok(controller) = controller.cast::<ICoreWebView2Controller2>() {
                    let _ = controller.SetDefaultBackgroundColor(surface.0.colour());
                }
            }
        }
    }
}
#[unsafe(no_mangle)]
pub unsafe extern "C" fn luma_webview_reveal(surface: *mut Surface, value: f64) {
    if value.is_finite()
        && let Some(surface) = unsafe { surface.as_ref() }
    {
        let value = value.clamp(0., 1.);
        if surface.0.reveal.replace(value) != value {
            surface.0.apply_clip();
        }
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
    let Some(surface) = (unsafe { surface.as_ref() }) else {
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
    let rects = unsafe { std::slice::from_raw_parts(rects, count * 4) }.to_vec();
    let geometry = Geometry {
        rects,
        zoom: zoom.clamp(0.5, 3.),
        density,
    };
    if surface.0.geometry.borrow().as_ref() == Some(&geometry) {
        return;
    }
    *surface.0.geometry.borrow_mut() = Some(geometry);
    surface.0.apply_geometry();
}
#[unsafe(no_mangle)]
pub unsafe extern "C" fn luma_webview_measure(surface: *mut Surface, width: i32, generation: u64) {
    let Some(surface) = (unsafe { surface.as_ref() }) else {
        return;
    };
    let weak = Rc::downgrade(&surface.0);
    if !surface.0.ready.get() {
        return;
    }
    let revision = surface.0.revision.get();
    if let Some(web) = surface.0.web.borrow().as_ref() {
        unsafe {
            let _=web.ExecuteScript(w!("String(Math.ceil(document.body?document.body.getBoundingClientRect().height:48))"),&ExecuteScriptCompletedHandler::create(Box::new(move |result,value|{
            if result.is_ok()&&let Some(state)=weak.upgrade()&&state.ready.get()&&state.revision.get()==revision&&let Ok(value)=serde_json::from_str::<String>(&value)&&let Ok(height)=value.parse::<f64>()&&height.is_finite(){state.emit(json!({"kind":"height","height":height.clamp(24.,60000.),"width":width,"generation":generation}));}Ok(())
        })));
        }
    }
    surface.0.script("if(!window.lumaWheel){window.lumaWheel=true;addEventListener('wheel',e=>{if(!e.ctrlKey){e.preventDefault();chrome.webview.postMessage(String(e.deltaY*(e.deltaMode===1?24:e.deltaMode===2?innerHeight:1)));}},{passive:false});}");
}

#[cfg(test)]
#[path = "tests.rs"]
mod tests;
