//! Native HTML in Windows test builds. GTK owns layout; the child WebView2 is
//! clipped to its GTK scroll viewport and destroyed when the message unmaps.
//! Mail is served from memory, in InPrivate mode, with scripting disabled.
use super::*;
use gtk::glib;
use raw_window_handle::{
    HandleError, HasWindowHandle, RawWindowHandle, Win32WindowHandle, WindowHandle,
};
use std::{
    borrow::Cow,
    cell::{Cell, RefCell},
    num::NonZeroIsize,
    sync::{Arc, Mutex},
};
use wry::{WebView, WebViewBuilder, WebViewBuilderExtWindows, WebViewExtWindows};

struct Parent(isize);
enum Event {
    Link(String),
    Loaded,
    Height(f64, i32),
    Wheel(f64),
}
fn measure(view: &WebView, area: &gtk::DrawingArea, events: std::sync::mpsc::Sender<Event>) {
    let scale = area.scale_factor() as f64;
    let width = area.width();
    let _ = view.evaluate_script_with_callback(
        "document.body ? Math.ceil(document.body.getBoundingClientRect().height * devicePixelRatio) : 160",
        move |value| {
            if let Ok(height) = serde_json::from_str::<f64>(&value) && height.is_finite() {
                let _ = events.send(Event::Height(height / scale, width));
            }
        },
    );
}
impl HasWindowHandle for Parent {
    fn window_handle(&self) -> std::result::Result<WindowHandle<'_>, HandleError> {
        let handle =
            Win32WindowHandle::new(NonZeroIsize::new(self.0).ok_or(HandleError::Unavailable)?);
        // GTK retains this native surface until the child is dropped on unmap.
        Ok(unsafe { WindowHandle::borrow_raw(RawWindowHandle::Win32(handle)) })
    }
}
pub(super) fn append(
    s: &Rc<State>,
    body: &gtk::Box,
    message: &Message,
    account: &str,
    html: String,
) {
    let area = gtk::DrawingArea::builder()
        .hexpand(true)
        .height_request(160)
        .build();
    area.set_widget_name("html-mail-view");
    let view: Rc<RefCell<Option<WebView>>> = Default::default();
    let geometry = Rc::new(Cell::new(None::<[i32; 9]>));
    let document = Arc::new(Mutex::new(String::new()));
    let data = document.clone();
    let handle = view.clone();
    html_controls(
        s,
        body,
        area.upcast_ref(),
        message,
        account,
        html,
        move |page, zoom| {
            *data.lock().expect("HTML document") = page;
            if let Some(view) = handle.borrow().as_ref() {
                let _ = view.zoom(zoom);
                let _ = view.load_url("http://luma.mail/document");
            }
        },
    );
    let (links, received) = std::sync::mpsc::channel::<Event>();
    let state = s.clone();
    let weak_area = area.downgrade();
    let handle = view.clone();
    let events = links.clone();
    glib::timeout_add_local(std::time::Duration::from_millis(60), move || {
        let Some(area) = weak_area.upgrade() else {
            return glib::ControlFlow::Break;
        };
        while let Ok(event) = received.try_recv() {
            match event {
                Event::Link(uri) => open_link(&state, &uri),
                Event::Height(height, width) if width == area.width() => {
                    area.set_height_request(height.ceil().clamp(48., 60000.) as i32);
                }
                Event::Loaded => {
                    if let Some(view) = handle.borrow().as_ref() {
                        measure(view, &area, events.clone());
                        // Privileged host code only. Sender scripts remain disabled,
                        // sanitized away and forbidden by CSP. A full-height document
                        // forwards wheel motion to the GTK conversation scroll view.
                        let _ = view.evaluate_script("if(!window.lumaWheel){window.lumaWheel=true;addEventListener('wheel',e=>{if(!e.ctrlKey){e.preventDefault();window.ipc.postMessage(String(e.deltaY*(e.deltaMode===1?24:e.deltaMode===2?innerHeight:1)*devicePixelRatio));}},{passive:false});}");
                    }
                }
                Event::Wheel(delta) if delta.is_finite() => {
                    let mut parent = area.parent();
                    while let Some(widget) = parent {
                        if let Some(scroll) = widget.downcast_ref::<gtk::ScrolledWindow>() {
                            let adjustment = scroll.vadjustment();
                            adjustment.set_value(
                                (adjustment.value()
                                    + delta.clamp(-3000., 3000.) / area.scale_factor() as f64)
                                    .clamp(
                                        adjustment.lower(),
                                        (adjustment.upper() - adjustment.page_size())
                                            .max(adjustment.lower()),
                                    ),
                            );
                            break;
                        }
                        parent = widget.parent();
                    }
                }
                _ => (),
            }
        }
        glib::ControlFlow::Continue
    });
    let handle = view.clone();
    let source = document.clone();
    let state = s.clone();
    let layout = geometry.clone();
    let width = Rc::new(Cell::new(0));
    let mapped_width = width.clone();
    let map_events = links.clone();
    area.connect_map(move |area| {
        layout.set(None);
        mapped_width.set(0);
        if handle.borrow().is_some() { return; }
        if wry::webview_version().ok().and_then(|v|v.split('.').next()?.parse::<u32>().ok()).is_none_or(|v|v<101) { state.info("HTML preview requires Microsoft Edge WebView2 Runtime 101 or newer for private rendering."); return; }
        let Some(surface)=state.window.surface().and_then(|s| s.downcast::<gdk_win32::Win32Surface>().ok()) else { return; };
        let parent=Parent(surface.handle().0 as isize);
        let root=state.paths.cache.join("webview2-runtime");
        if luma_core::config::private_dir(&root).is_err() { return; }
        let mut context=wry::WebContext::new(Some(root));
        let data=source.clone(); let navigation=map_events.clone(); let new_window=map_events.clone();
        let loaded=map_events.clone(); let wheel=map_events.clone();
        let result=WebViewBuilder::new_with_web_context(&mut context)
            .with_transparent(true).with_incognito(true).with_javascript_disabled().with_devtools(false)
            .with_focused(false).with_visible(false).with_default_context_menus(false)
            .with_browser_accelerator_keys(false)
            .with_additional_browser_args("--disk-cache-size=1 --media-cache-size=1 --disable-gpu-shader-disk-cache --disable-background-networking --disable-component-update --no-first-run")
            .with_custom_protocol("luma".into(),move |_,_| {
                wry::http::Response::builder().header("Content-Type","text/html; charset=utf-8").header("Cache-Control","no-store").header("Referrer-Policy","no-referrer")
                    .body(Cow::Owned(data.lock().expect("HTML document").as_bytes().to_vec())).unwrap()
            })
            .with_navigation_handler(move |uri| {
                if uri=="http://luma.mail/document" { return true; }
                if uri.starts_with("https://") || uri.starts_with("http://") || uri.starts_with("mailto:") { let _=navigation.send(Event::Link(uri)); }
                false
            })
            .with_new_window_req_handler(move |uri,_| { let _=new_window.send(Event::Link(uri)); wry::NewWindowResponse::Deny })
            .with_on_page_load_handler(move |event,_| { if matches!(event,wry::PageLoadEvent::Finished) { let _=loaded.send(Event::Loaded); } })
            .with_ipc_handler(move |request| { if let Ok(delta)=request.body().parse::<f64>() { let _=wheel.send(Event::Wheel(delta)); } })
            .with_download_started_handler(|_,_| false)
            .with_url("luma://mail/document").build_as_child(&parent);
        match result {
            Ok(webview) => { let _ = webview.zoom(crate::presentation::scale(&state)); *handle.borrow_mut()=Some(webview); area.queue_draw(); },
            Err(error) => state.info(&format!("HTML preview unavailable: {error}. Install or update Microsoft Edge WebView2 Runtime.")),
        }
    });
    // WebView2 is a native child window, outside GTK's dialog stacking. Hide it
    // immediately when libadwaita presents Settings or another modal sheet.
    let handle = Rc::downgrade(&view);
    let layout = geometry.clone();
    let dialog_signal = s.window.connect_visible_dialog_notify(move |window| {
        layout.set(None);
        if let Some(handle) = handle.upgrade()
            && let Some(view) = handle.borrow().as_ref()
        {
            let _ = view.set_visible(window.visible_dialog().is_none());
        }
    });
    let owner = s.window.downgrade();
    let dialog_signal = RefCell::new(Some(dialog_signal));
    area.connect_destroy(move |_| {
        if let Some(owner) = owner.upgrade()
            && let Some(signal) = dialog_signal.borrow_mut().take()
        {
            owner.disconnect(signal);
        }
    });
    let handle = view.clone();
    let window = s.window.downgrade();
    area.add_tick_callback(move |area, _| {
        let Some(window) = window.upgrade() else {
            return glib::ControlFlow::Break;
        };
        let handle = handle.borrow();
        let Some(view) = handle.as_ref() else {
            return glib::ControlFlow::Continue;
        };
        if window.visible_dialog().is_some() {
            geometry.set(None);
            let _ = view.set_visible(false);
            return glib::ControlFlow::Continue;
        }
        if width.replace(area.width()) != area.width() {
            measure(view, area, links.clone());
        }
        let Some(bounds) = area.compute_bounds(&window) else {
            let _ = view.set_visible(false);
            return glib::ControlFlow::Continue;
        };
        let mut clip = bounds;
        let mut parent = area.parent();
        while let Some(widget) = parent {
            if widget.is::<gtk::ScrolledWindow>()
                && let Some(rect) = widget.compute_bounds(&window)
            {
                let Some(intersection) = clip.intersection(&rect) else {
                    let _ = view.set_visible(false);
                    return glib::ControlFlow::Continue;
                };
                clip = intersection;
            }
            parent = widget.parent();
        }
        let next = [
            bounds.x() as i32,
            bounds.y() as i32,
            bounds.width() as i32,
            bounds.height() as i32,
            clip.x() as i32,
            clip.y() as i32,
            clip.width() as i32,
            clip.height() as i32,
            area.scale_factor(),
        ];
        if geometry.replace(Some(next)) == Some(next) {
            return glib::ControlFlow::Continue;
        }
        let _ = view.set_bounds(wry::Rect {
            position: wry::dpi::PhysicalPosition::new(
                (bounds.x() * area.scale_factor() as f32).round() as i32,
                (bounds.y() * area.scale_factor() as f32).round() as i32,
            )
            .into(),
            size: wry::dpi::PhysicalSize::new(
                (bounds.width() * area.scale_factor() as f32).round() as u32,
                (bounds.height() * area.scale_factor() as f32).round() as u32,
            )
            .into(),
        });
        // Native children do not participate in GTK snapshot clipping. Restrict
        // their window region to the visible message viewport in physical pixels.
        let scale = area.scale_factor() as f32;
        unsafe {
            let mut hwnd = Default::default();
            if view.controller().ParentWindow(&mut hwnd).is_ok() {
                let region = windows_sys::Win32::Graphics::Gdi::CreateRectRgn(
                    ((clip.x() - bounds.x()) * scale).round() as i32,
                    ((clip.y() - bounds.y()) * scale).round() as i32,
                    ((clip.x() + clip.width() - bounds.x()) * scale).round() as i32,
                    ((clip.y() + clip.height() - bounds.y()) * scale).round() as i32,
                );
                if windows_sys::Win32::Graphics::Gdi::SetWindowRgn(hwnd.0, region, 1) == 0 {
                    windows_sys::Win32::Graphics::Gdi::DeleteObject(region);
                }
            }
        }
        let _ = view.set_visible(area.is_mapped() && window.is_visible());
        glib::ControlFlow::Continue
    });
    let handle = view.clone();
    area.connect_unmap(move |_| {
        handle.borrow_mut().take();
    });
    body.append(&area);
    #[cfg(debug_assertions)]
    if std::env::args().any(|a| a == "--html-diagnostics") {
        let state = s.clone();
        let weak = area.downgrade();
        let handle = view.clone();
        let minimum = std::env::var("LUMA_EXPECT_HTML_MIN_HEIGHT")
            .ok()
            .and_then(|s| s.parse::<i32>().ok())
            .unwrap_or(200);
        let mut stage = 0;
        let mut ticks = 0;
        glib::timeout_add_local(std::time::Duration::from_millis(100), move || {
            ticks += 1;
            assert!(ticks < 150, "native HTML diagnostics timed out");
            let area = weak.upgrade().expect("HTML surface stays mapped");
            let handle = handle.borrow();
            let Some(view) = handle.as_ref() else {
                return glib::ControlFlow::Continue;
            };
            match stage {
                0 if area.height_request() > minimum => {
                    assert!(
                        area.height_request() < 60000,
                        "synthetic newsletter fits within the bounded full-content view"
                    );
                    println!(
                        "PASS WebView2 content height {} GTK units",
                        area.height_request()
                    );
                    crate::settings::open(&state);
                    stage = 1;
                }
                1 => {
                    assert!(state.window.visible_dialog().is_some());
                    let mut visible = Default::default();
                    unsafe {
                        view.controller().IsVisible(&mut visible).unwrap();
                    }
                    assert!(!visible.as_bool(), "native HTML must not overlap Settings");
                    state.window.visible_dialog().unwrap().force_close();
                    println!("PASS native newsletter is hidden under Settings");
                    stage = 2;
                }
                2 if state.window.visible_dialog().is_none() => {
                    let mut visible = Default::default();
                    unsafe {
                        view.controller().IsVisible(&mut visible).unwrap();
                    }
                    assert!(
                        visible.as_bool(),
                        "newsletter restores after Settings closes"
                    );
                    println!("PASS native newsletter restores after Settings");
                    state.app.quit();
                    return glib::ControlFlow::Break;
                }
                _ => (),
            }
            glib::ControlFlow::Continue
        });
    }
}
