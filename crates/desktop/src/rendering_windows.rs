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
        .height_request(560)
        .build();
    area.set_widget_name("html-mail-view");
    let view: Rc<RefCell<Option<WebView>>> = Default::default();
    let geometry = Rc::new(Cell::new(None::<[i32; 9]>));
    let document = Arc::new(Mutex::new(html_mail::document(&html, HashMap::new())));
    let (links, received) = std::sync::mpsc::channel::<String>();
    let state = s.clone();
    let weak_area = area.downgrade();
    glib::timeout_add_local(std::time::Duration::from_millis(60), move || {
        if weak_area.upgrade().is_none() {
            return glib::ControlFlow::Break;
        }
        while let Ok(uri) = received.try_recv() {
            open_link(&state, &uri);
        }
        glib::ControlFlow::Continue
    });
    let handle = view.clone();
    let source = document.clone();
    let state = s.clone();
    let layout = geometry.clone();
    area.connect_map(move |area| {
        layout.set(None);
        if handle.borrow().is_some() { return; }
        if wry::webview_version().ok().and_then(|v|v.split('.').next()?.parse::<u32>().ok()).is_none_or(|v|v<101) { state.info("HTML preview requires Microsoft Edge WebView2 Runtime 101 or newer for private rendering."); return; }
        let Some(surface)=state.window.surface().and_then(|s| s.downcast::<gdk_win32::Win32Surface>().ok()) else { return; };
        let parent=Parent(surface.handle().0 as isize);
        let root=state.paths.cache.join("webview2-runtime");
        if luma_core::config::private_dir(&root).is_err() { return; }
        let mut context=wry::WebContext::new(Some(root));
        let data=source.clone(); let navigation=links.clone(); let new_window=links.clone();
        let result=WebViewBuilder::new_with_web_context(&mut context)
            .with_incognito(true).with_javascript_disabled().with_devtools(false)
            .with_focused(false).with_visible(false).with_default_context_menus(false)
            .with_browser_accelerator_keys(false)
            .with_additional_browser_args("--disk-cache-size=1 --media-cache-size=1 --disable-gpu-shader-disk-cache --disable-background-networking --disable-component-update --no-first-run")
            .with_custom_protocol("luma".into(),move |_,_| {
                wry::http::Response::builder().header("Content-Type","text/html; charset=utf-8").header("Cache-Control","no-store").header("Referrer-Policy","no-referrer")
                    .body(Cow::Owned(data.lock().expect("HTML document").as_bytes().to_vec())).unwrap()
            })
            .with_navigation_handler(move |uri| {
                if uri=="http://luma.mail/document" { return true; }
                if uri.starts_with("https://") || uri.starts_with("http://") || uri.starts_with("mailto:") { let _=navigation.send(uri); }
                false
            })
            .with_new_window_req_handler(move |uri,_| { let _=new_window.send(uri); wry::NewWindowResponse::Deny })
            .with_download_started_handler(|_,_| false)
            .with_url("luma://mail/document").build_as_child(&parent);
        match result {
            Ok(webview) => { *handle.borrow_mut()=Some(webview); area.queue_draw(); },
            Err(error) => state.info(&format!("HTML preview unavailable: {error}. Install or update Microsoft Edge WebView2 Runtime.")),
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
            position: wry::dpi::LogicalPosition::new(bounds.x() as f64, bounds.y() as f64).into(),
            size: wry::dpi::LogicalSize::new(bounds.width() as f64, bounds.height() as f64).into(),
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
    if mail_images::sources(&message.payload)
        .iter()
        .any(|s| s.starts_with("https://"))
    {
        let bar = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        let label = ui::label("External images are blocked for privacy.", "caption");
        label.set_hexpand(true);
        bar.append(&label);
        let load = gtk::Button::with_label("Load images");
        load.set_tooltip_text(Some(
            "Contact this message's image servers. This may reveal that you opened it.",
        ));
        bar.append(&load);
        body.append(&bar);
        let state = s.clone();
        let message = message.clone();
        let client = s.clients.borrow().get(account).cloned();
        let source = html.clone();
        let data = document.clone();
        let handle = view.clone();
        let weak = area.downgrade();
        load.connect_clicked(move |button| {
            button.set_sensitive(false);
            button.set_label("Loading images…");
            let button = button.downgrade();
            let data = data.clone();
            let handle = handle.clone();
            let message = message.clone();
            let client = client.clone();
            let html = source.clone();
            let request = ui::async_work(
                &state,
                async move { Ok(image_map(message, client, true).await) },
                move |_, images| {
                    *data.lock().unwrap() = html_mail::document(&html, images);
                    if let Some(view) = handle.borrow().as_ref() {
                        let _ = view.load_url("http://luma.mail/document");
                    }
                    if let Some(button) = button.upgrade() {
                        button.set_label("Images loaded");
                    }
                },
            );
            if let Some(area) = weak.upgrade() {
                area.connect_unmap(move |_| request.abort());
            }
        });
    }
    body.append(&area);
    let client = s.clients.borrow().get(account).cloned();
    let message = message.clone();
    let handle = view;
    let weak = area.downgrade();
    let request = ui::async_work(
        s,
        async move { Ok(image_map(message, client, false).await) },
        move |_, images| {
            if !images.is_empty() {
                *document.lock().unwrap() = html_mail::document(&html, images);
                if let Some(view) = handle.borrow().as_ref() {
                    let _ = view.load_url("http://luma.mail/document");
                }
            }
        },
    );
    if let Some(area) = weak.upgrade() {
        area.connect_unmap(move |_| request.abort());
    }
}
