//! Event-driven WebView2 child surface. Documents and scripts come from the host;
//! sender JavaScript, navigation, downloads and persistent browser storage are disabled.
use adw::prelude::*;
use gtk::glib;
use luma_core::{Error, Result};
use raw_window_handle::{
    HandleError, HasWindowHandle, RawWindowHandle, Win32WindowHandle, WindowHandle,
};
use std::{
    borrow::Cow,
    cell::{Cell, RefCell},
    num::NonZeroIsize,
    rc::Rc,
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
enum Event {
    Loaded,
    Link(String),
    Height(f64, i32, u64),
    Wheel(f64),
}
type Loaded = Box<dyn Fn(&Surface)>;
struct Inner {
    view: RefCell<Option<WebView>>,
    document: Arc<Mutex<String>>,
    geometry: Cell<Option<[i32; 9]>>,
    measured_width: Cell<i32>,
    generation: Cell<u64>,
    measure_height: bool,
    zoom: Cell<f64>,
    ready: Cell<bool>,
    on_load: RefCell<Option<Loaded>>,
    on_link: Box<dyn Fn(String)>,
    connections: RefCell<Vec<(glib::Object, glib::SignalHandlerId)>>,
    events: tokio::sync::mpsc::Sender<Event>,
    runtime: std::path::PathBuf,
}
#[derive(Clone)]
pub(crate) struct Surface {
    pub area: gtk::DrawingArea,
    inner: Rc<Inner>,
}
impl Surface {
    pub fn new(
        runtime: std::path::PathBuf,
        page: String,
        measure_height: bool,
        on_link: impl Fn(String) + 'static,
    ) -> Self {
        let area = gtk::DrawingArea::builder()
            .hexpand(true)
            .height_request(if measure_height { 160 } else { 280 })
            .vexpand(!measure_height)
            .build();
        let (tx, mut rx) = tokio::sync::mpsc::channel(64);
        let inner = Rc::new(Inner {
            view: Default::default(),
            document: Arc::new(Mutex::new(page)),
            geometry: Cell::new(None),
            measured_width: Cell::new(0),
            generation: Cell::new(0),
            measure_height,
            zoom: Cell::new(1.),
            ready: Cell::new(false),
            on_load: Default::default(),
            on_link: Box::new(on_link),
            connections: Default::default(),
            events: tx,
            runtime,
        });
        let surface = Self {
            area: area.clone(),
            inner: inner.clone(),
        };
        let weak = Rc::downgrade(&inner);
        let widget = area.downgrade();
        glib::MainContext::default().spawn_local(async move {
            while let Some(event)=rx.recv().await {
                let (Some(inner),Some(area))=(weak.upgrade(),widget.upgrade()) else {break};
                let s=Self{inner,area};
                match event {
                    Event::Link(uri)=>(s.inner.on_link)(uri),
                    Event::Height(height,width,generation) if generation==s.inner.generation.get()&&width==s.area.width() && height.is_finite()=>s.area.set_height_request(height.ceil().clamp(48.,60000.) as i32),
                    Event::Loaded=>{
                        s.inner.ready.set(true);s.inner.measured_width.set(0);s.measure();s.update();
                        if s.inner.measure_height {s.eval("if(!window.lumaWheel){window.lumaWheel=true;addEventListener('wheel',e=>{if(!e.ctrlKey){e.preventDefault();window.ipc.postMessage(String(e.deltaY*(e.deltaMode===1?24:e.deltaMode===2?innerHeight:1)*devicePixelRatio));}},{passive:false});}})()".into(), |_|{});}
                        if let Some(loaded)=s.inner.on_load.borrow().as_ref(){loaded(&s);}
                    },
                    Event::Wheel(delta) if delta.is_finite() && s.inner.measure_height=>{
                        let mut parent=s.area.parent();
                        while let Some(w)=parent {
                            if let Some(scroll)=w.downcast_ref::<gtk::ScrolledWindow>() {
                                let a=scroll.vadjustment();a.set_value((a.value()+delta.clamp(-3000.,3000.)/s.area.scale_factor() as f64).clamp(a.lower(),(a.upper()-a.page_size()).max(a.lower())));break;
                            }parent=w.parent();
                        }
                    },_=>(),
                }
            }
        });
        // The widget owns Inner; Inner never owns the widget. Unmapping drops the renderer.
        let hold = inner.clone();
        let widget = area.downgrade();
        area.connect_map(move |_| {
            if let Some(area) = widget.upgrade() {
                Self {
                    area,
                    inner: hold.clone(),
                }
                .map();
            }
        });
        let weak = Rc::downgrade(&inner);
        let widget = area.downgrade();
        area.connect_unmap(move |_| {
            if let (Some(i), Some(_)) = (weak.upgrade(), widget.upgrade()) {
                i.ready.set(false);
                i.view.borrow_mut().take();
                i.geometry.set(None);
                for (o, id) in i.connections.borrow_mut().drain(..) {
                    o.disconnect(id);
                }
            }
        });
        let weak = Rc::downgrade(&inner);
        area.connect_resize(move |area, _, _| {
            if let Some(inner) = weak.upgrade() {
                let s = Self {
                    area: area.clone(),
                    inner,
                };
                s.measure();
                s.update();
            }
        });
        let weak = Rc::downgrade(&inner);
        area.set_draw_func(move |area, _, _, _| {
            if let Some(inner) = weak.upgrade() {
                Self {
                    area: area.clone(),
                    inner,
                }
                .update();
            }
        });
        surface
    }
    pub fn weak(&self) -> impl Fn() -> Option<Self> + 'static {
        let area = self.area.downgrade();
        let inner = Rc::downgrade(&self.inner);
        move || {
            Some(Self {
                area: area.upgrade()?,
                inner: inner.upgrade()?,
            })
        }
    }
    pub fn set_on_load(&self, f: impl Fn(&Surface) + 'static) {
        *self.inner.on_load.borrow_mut() = Some(Box::new(f));
    }
    pub fn load(&self, page: String, zoom: f64) {
        *self.inner.document.lock().expect("HTML document") = page;
        self.inner.zoom.set(zoom);
        self.inner.ready.set(false);
        self.inner
            .generation
            .set(self.inner.generation.get().wrapping_add(1));
        self.inner.measured_width.set(0);
        if let Some(v) = self.inner.view.borrow().as_ref() {
            let _ = v.zoom(zoom);
            let _ = v.load_url("http://luma.mail/document");
        }
    }
    pub fn zoom(&self, zoom: f64) {
        self.inner.zoom.set(zoom);
        if let Some(v) = self.inner.view.borrow().as_ref() {
            let _ = v.zoom(zoom);
        }
        self.measure();
    }
    pub fn eval(&self, code: String, done: impl FnOnce(Result<String>) + 'static) {
        let handle = self.inner.view.borrow();
        let Some(v) = handle.as_ref() else {
            done(Err(Error::Invalid(
                "The Windows HTML surface is not ready".into(),
            )));
            return;
        };
        let (tx, rx) = tokio::sync::oneshot::channel();
        let callback = Arc::new(Mutex::new(Some(tx)));
        // WebView2 returns JSON-encoded results. Catch exceptions explicitly: its
        // default exception result is indistinguishable from a successful null.
        let code = format!(
            "(()=>{{try{{return {{ok:true,value:String(({}))}}}}catch(e){{return {{ok:false,error:String(e)}}}}}})()",
            code.trim().trim_end_matches(';')
        );
        if let Err(error) = v.evaluate_script_with_callback(&code, move |json| {
            let result = serde_json::from_str::<serde_json::Value>(&json)
                .map_err(Error::from)
                .and_then(|v| {
                    if v["ok"] == true {
                        Ok(v["value"].as_str().unwrap_or("").to_owned())
                    } else {
                        Err(Error::Invalid(format!(
                            "HTML editor: {}",
                            v["error"].as_str().unwrap_or("script returned no result")
                        )))
                    }
                });
            if let Some(tx) = callback.lock().expect("script result").take() {
                let _ = tx.send(result);
            }
        }) {
            done(Err(Error::Invalid(format!(
                "Windows HTML surface: {error}"
            ))));
            return;
        }
        glib::MainContext::default().spawn_local(async move {
            done(
                match glib::future_with_timeout(std::time::Duration::from_secs(10), rx).await {
                    Ok(Ok(value)) => value,
                    Ok(Err(_)) => Err(Error::Invalid("The Windows HTML surface closed".into())),
                    Err(_) => Err(Error::Timeout),
                },
            );
        });
    }

    fn measure(&self) {
        if !self.inner.measure_height || !self.inner.ready.get() {
            return;
        }
        let width = self.area.width();
        if self.inner.measured_width.replace(width) == width {
            return;
        }
        let tx = self.inner.events.clone();
        let scale = self.area.scale_factor() as f64;
        let generation = self.inner.generation.get();
        self.eval("document.body ? Math.ceil(document.body.getBoundingClientRect().height * devicePixelRatio) : 160".into(),move |value|{if let Ok(height)=value.and_then(|v|v.parse::<f64>().map_err(|e|Error::Invalid(e.to_string()))){let _=tx.try_send(Event::Height(height/scale,width,generation));}});
    }
    fn map(&self) {
        if self.inner.view.borrow().is_some() {
            return;
        }
        let Some(window) = self.area.root().and_downcast::<gtk::Window>() else {
            return;
        };
        let Some(surface) = window
            .surface()
            .and_then(|s| s.downcast::<gdk_win32::Win32Surface>().ok())
        else {
            return;
        };
        let parent = Parent(surface.handle().0 as isize);
        let data = self.inner.document.clone();
        let nav = self.inner.events.clone();
        let new = self.inner.events.clone();
        let load = self.inner.events.clone();
        let wheel = self.inner.events.clone();
        if luma_core::config::private_dir(&self.inner.runtime).is_err() {
            return;
        }
        let mut context = wry::WebContext::new(Some(self.inner.runtime.clone()));
        let result=WebViewBuilder::new_with_web_context(&mut context).with_transparent(true).with_incognito(true).with_javascript_disabled().with_devtools(false).with_focused(false).with_visible(false).with_default_context_menus(false).with_browser_accelerator_keys(false)
            .with_additional_browser_args("--disk-cache-size=1 --media-cache-size=1 --disable-gpu-shader-disk-cache --disable-background-networking --disable-component-update --no-first-run")
            .with_custom_protocol("luma".into(),move |_,_|wry::http::Response::builder().header("Content-Type","text/html; charset=utf-8").header("Cache-Control","no-store").header("Referrer-Policy","no-referrer").body(Cow::Owned(data.lock().expect("HTML document").as_bytes().to_vec())).unwrap())
            .with_navigation_handler(move |uri|{if uri=="http://luma.mail/document"{return true}
                if uri.starts_with("https://")||uri.starts_with("http://")||uri.starts_with("mailto:"){let _=nav.try_send(Event::Link(uri));}false})
            .with_new_window_req_handler(move |uri,_|{let _=new.try_send(Event::Link(uri));wry::NewWindowResponse::Deny})
            .with_on_page_load_handler(move |event,_|{if matches!(event,wry::PageLoadEvent::Finished){let _=load.try_send(Event::Loaded);}})
            .with_ipc_handler(move |r|{if let Ok(delta)=r.body().parse::<f64>(){let _=wheel.try_send(Event::Wheel(delta));}})
            .with_download_started_handler(|_,_|false).with_url("luma://mail/document").build_as_child(&parent);
        match result {
            Ok(v) => {
                let _ = v.zoom(self.inner.zoom.get());
                *self.inner.view.borrow_mut() = Some(v);
                self.inner.measured_width.set(0);
                self.inner.geometry.set(None);
                self.update();
            }
            Err(error) => {
                self.area.set_tooltip_text(Some(&format!(
                    "HTML preview unavailable: {error}. Install Microsoft Edge WebView2 Runtime."
                )));
                eprintln!("Windows HTML surface unavailable: {error}");
            }
        }
        let mut owner = self.area.parent();
        while let Some(w) = owner {
            if let Some(scroll) = w.downcast_ref::<gtk::ScrolledWindow>() {
                for a in [scroll.vadjustment(), scroll.hadjustment()] {
                    let weak = Rc::downgrade(&self.inner);
                    let area = self.area.downgrade();
                    let id = a.connect_value_changed(move |_| {
                        if let (Some(inner), Some(area)) = (weak.upgrade(), area.upgrade()) {
                            Self { inner, area }.update();
                        }
                    });
                    self.inner
                        .connections
                        .borrow_mut()
                        .push((a.clone().upcast(), id));
                    let weak = Rc::downgrade(&self.inner);
                    let area = self.area.downgrade();
                    let id = a.connect_changed(move |_| {
                        if let (Some(inner), Some(area)) = (weak.upgrade(), area.upgrade()) {
                            Self { inner, area }.update();
                        }
                    });
                    self.inner.connections.borrow_mut().push((a.upcast(), id));
                }
            }
            owner = w.parent();
        }
        if let Some(w) = window.downcast_ref::<adw::Window>() {
            let weak = Rc::downgrade(&self.inner);
            let area = self.area.downgrade();
            let id = w.connect_visible_dialog_notify(move |_| {
                if let (Some(inner), Some(area)) = (weak.upgrade(), area.upgrade()) {
                    inner.geometry.set(None);
                    Self { inner, area }.update();
                }
            });
            self.inner
                .connections
                .borrow_mut()
                .push((w.clone().upcast(), id));
        } else if let Some(w) = window.downcast_ref::<adw::ApplicationWindow>() {
            let weak = Rc::downgrade(&self.inner);
            let area = self.area.downgrade();
            let id = w.connect_visible_dialog_notify(move |_| {
                if let (Some(inner), Some(area)) = (weak.upgrade(), area.upgrade()) {
                    inner.geometry.set(None);
                    Self { inner, area }.update();
                }
            });
            self.inner
                .connections
                .borrow_mut()
                .push((w.clone().upcast(), id));
        }
    }
    fn update(&self) {
        let handle = self.inner.view.borrow();
        let Some(v) = handle.as_ref() else { return };
        let Some(window) = self.area.root().and_downcast::<gtk::Window>() else {
            return;
        };
        let modal = window
            .downcast_ref::<adw::Window>()
            .is_some_and(|w| w.visible_dialog().is_some())
            || window
                .downcast_ref::<adw::ApplicationWindow>()
                .is_some_and(|w| w.visible_dialog().is_some());
        if modal || !self.area.is_mapped() {
            self.inner.geometry.set(None);
            let _ = v.set_visible(false);
            return;
        }
        let Some(bounds) = self.area.compute_bounds(&window) else {
            return;
        };
        let mut clip = bounds;
        let mut parent = self.area.parent();
        while let Some(w) = parent {
            if w.is::<gtk::ScrolledWindow>()
                && let Some(rect) = w.compute_bounds(&window)
            {
                let Some(intersection) = clip.intersection(&rect) else {
                    self.inner.geometry.set(None);
                    let _ = v.set_visible(false);
                    return;
                };
                clip = intersection;
            }
            parent = w.parent();
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
            self.area.scale_factor(),
        ];
        if self.inner.geometry.replace(Some(next)) == Some(next) {
            return;
        }
        let scale = self.area.scale_factor() as f32;
        let _ = v.set_bounds(wry::Rect {
            position: wry::dpi::PhysicalPosition::new(
                (bounds.x() * scale).round() as i32,
                (bounds.y() * scale).round() as i32,
            )
            .into(),
            size: wry::dpi::PhysicalSize::new(
                (bounds.width() * scale).round().max(1.) as u32,
                (bounds.height() * scale).round().max(1.) as u32,
            )
            .into(),
        });
        unsafe {
            let mut hwnd = Default::default();
            if v.controller().ParentWindow(&mut hwnd).is_ok() {
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
        let _ = v.set_visible(true);
    }
    pub fn visible(&self) -> bool {
        let mut visible = Default::default();
        self.inner.view.borrow().as_ref().is_some_and(|v| unsafe {
            v.controller().IsVisible(&mut visible).is_ok() && visible.as_bool()
        })
    }
}
