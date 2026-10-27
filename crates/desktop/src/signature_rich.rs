//! A short-lived editing surface; never a persistent browser profile.
use crate::ui::State;
use adw::prelude::*;
#[cfg(not(any(target_os = "linux", target_os = "windows")))]
use gtk::glib;
use luma_core::{Error, Result};
use std::{cell::Cell, rc::Rc};
#[cfg(target_os = "linux")]
use webkit6::prelude::*;

#[derive(Clone)]
pub(crate) struct RichEditor {
    pub widget: gtk::Widget,
    ready: Rc<Cell<bool>>,
    #[cfg(target_os = "linux")]
    view: webkit6::WebView,
    #[cfg(target_os = "windows")]
    view: crate::windows_surface::Surface,
    #[cfg(not(any(target_os = "linux", target_os = "windows")))]
    source: gtk::TextView,
    #[cfg(not(any(target_os = "linux", target_os = "windows")))]
    dirty: Rc<Cell<bool>>,
}
impl RichEditor {
    pub fn new(s: &Rc<State>, html: &str, editable: bool) -> Result<Self> {
        let safe = luma_core::signatures::clean(html)?;
        #[cfg(target_os = "linux")]
        {
            let view = crate::rendering::signature_webview(s)?;
            view.set_vexpand(true);
            view.set_height_request(280);
            view.set_zoom_level(crate::presentation::scale(s));
            // Keep this isolated script world registered for the editor lifetime.
            // evaluate_javascript alone creates a transient world on WebKitGTK.
            let script = webkit6::UserScript::for_world(
                include_str!("signature_editor.js"),
                webkit6::UserContentInjectedFrames::TopFrame,
                webkit6::UserScriptInjectionTime::End,
                "LumaSignature",
                &[],
                &[],
            );
            view.user_content_manager()
                .expect("signature content manager")
                .add_script(&script);
            let ready = Rc::new(Cell::new(false));
            let loaded = ready.clone();
            view.connect_load_changed(move |view, event| {
                if event == webkit6::LoadEvent::Finished {
                    let ready = loaded.clone();
                    view.evaluate_javascript(
                        "Boolean(window.lumaSignature)",
                        Some("LumaSignature"),
                        None,
                        None::<&gtk::gio::Cancellable>,
                        move |result| ready.set(result.is_ok_and(|value| value.to_boolean())),
                    );
                    if view.is_mapped() {
                        view.grab_focus();
                    }
                }
            });
            let colour = s.window.color().to_string();
            let page = format!(
                "<!doctype html><html><head><meta charset=\"utf-8\"><meta http-equiv=\"Content-Security-Policy\" content=\"default-src 'none'; style-src 'unsafe-inline'; img-src data:; base-uri 'none'; form-action 'none'\"><style>body{{margin:0;padding:16px;background:transparent;color:{colour};font:16px/1.4 system-ui,sans-serif}}#luma-signature{{min-height:240px;outline:none;overflow-wrap:anywhere}}img{{max-width:100%;height:auto}}table{{max-width:100%}}a{{color:#4285f4}}</style></head><body><div id=\"luma-signature\" contenteditable=\"{editable}\" spellcheck=\"true\">{safe}</div></body></html>"
            );
            view.load_html(&page, Some("about:blank"));
            let weak = Rc::downgrade(s);
            let weak_view = view.downgrade();
            let refresh: Rc<dyn Fn()> = Rc::new(move || {
                if let (Some(s), Some(view)) = (weak.upgrade(), weak_view.upgrade()) {
                    view.set_zoom_level(crate::presentation::scale(&s));
                    let code = format!(
                        "document.body.style.color={}",
                        serde_json::to_string(&s.window.color().to_string()).unwrap()
                    );
                    view.evaluate_javascript(
                        &code,
                        Some("LumaSignature"),
                        None,
                        None::<&gtk::gio::Cancellable>,
                        |_| {},
                    );
                }
            });
            s.html_refresh.borrow_mut().retain(|f| f.strong_count() > 0);
            s.html_refresh.borrow_mut().push(Rc::downgrade(&refresh));
            view.connect_map(move |_| refresh());
            Ok(Self {
                widget: view.clone().upcast(),
                view,
                ready,
            })
        }
        #[cfg(target_os = "windows")]
        {
            let colour = s.window.color().to_string();
            let page = format!(
                "<!doctype html><html><head><meta charset=\"utf-8\"><meta http-equiv=\"Content-Security-Policy\" content=\"default-src 'none'; style-src 'unsafe-inline'; img-src data:; base-uri 'none'; form-action 'none'\"><style>body{{margin:0;padding:16px;background:transparent;color:{colour};font:16px/1.4 system-ui,sans-serif}}#luma-signature{{min-height:240px;outline:none;overflow-wrap:anywhere}}img{{max-width:100%;height:auto}}table{{max-width:100%}}a{{color:#4285f4}}</style></head><body><div id=\"luma-signature\" contenteditable=\"{editable}\" spellcheck=\"true\">{safe}</div></body></html>"
            );
            let view = crate::windows_surface::Surface::new(
                crate::windows_platform::webview_runtime(),
                page,
                false,
                |_| {},
            );
            view.zoom(crate::presentation::scale(s));
            let ready = Rc::new(Cell::new(false));
            let loaded = ready.clone();
            let weak = Rc::downgrade(s);
            let weak_view = view.weak();
            let refresh: Rc<dyn Fn()> = Rc::new(move || {
                if let (Some(s), Some(view)) = (weak.upgrade(), weak_view()) {
                    view.zoom(crate::presentation::scale(&s));
                    view.eval(
                        format!(
                            "document.body && (document.body.style.color={})",
                            serde_json::to_string(&s.window.color().to_string()).unwrap()
                        ),
                        |_| {},
                    );
                }
            });
            s.html_refresh.borrow_mut().retain(|f| f.strong_count() > 0);
            s.html_refresh.borrow_mut().push(Rc::downgrade(&refresh));
            let on_map = refresh.clone();
            view.area.connect_map(move |_| on_map());
            view.set_on_load(move |view| {
                refresh();
                let ready = loaded.clone();
                view.eval(
                    format!(
                        "(()=>{{ {};return Boolean(window.lumaSignature);}})()",
                        include_str!("signature_editor.js")
                    ),
                    move |v| ready.set(v.is_ok_and(|v| v == "true")),
                );
            });
            Ok(Self {
                widget: view.area.clone().upcast(),
                view,
                ready,
            })
        }
        #[cfg(not(any(target_os = "linux", target_os = "windows")))]
        {
            let _ = s;
            let source = gtk::TextView::builder()
                .wrap_mode(gtk::WrapMode::WordChar)
                .vexpand(true)
                .build();
            source.buffer().set_text(&safe);
            source.set_editable(editable);
            let dirty = Rc::new(Cell::new(false));
            let changed = dirty.clone();
            source.buffer().connect_changed(move |_| changed.set(true));
            let scroll = gtk::ScrolledWindow::builder()
                .child(&source)
                .vexpand(true)
                .min_content_height(280)
                .build();
            Ok(Self {
                widget: scroll.upcast(),
                source,
                dirty,
                ready: Rc::new(Cell::new(true)),
            })
        }
    }
    pub fn is_ready(&self) -> bool {
        self.ready.get()
    }
    #[cfg(target_os = "linux")]
    pub(super) fn eval(&self, code: String, done: impl FnOnce(Result<String>) + 'static) {
        self.view.evaluate_javascript(
            &code,
            Some("LumaSignature"),
            None,
            None::<&gtk::gio::Cancellable>,
            move |result| {
                done(
                    result
                        .map(|v| v.to_str().to_string())
                        .map_err(|error| Error::Invalid(format!("Signature editor: {error}"))),
                )
            },
        );
    }
    #[cfg(target_os = "windows")]
    pub(super) fn eval(&self, code: String, done: impl FnOnce(Result<String>) + 'static) {
        self.view.eval(code, done);
    }
    pub fn command(
        &self,
        command: &str,
        value: &str,
        done: impl FnOnce(Result<()>) + 'static,
    ) -> Result<()> {
        if !self.is_ready() {
            return Err(Error::Invalid(
                "The signature editor is still loading".into(),
            ));
        }
        #[cfg(any(target_os = "linux", target_os = "windows"))]
        self.eval(
            format!(
                "window.lumaSignature.command({},{})",
                serde_json::to_string(command).unwrap(),
                serde_json::to_string(value).unwrap()
            ),
            move |result| {
                done(result.and_then(|value| {
                    if value == "true" {
                        Ok(())
                    } else {
                        Err(Error::Invalid(
                            "Select the text or picture to format first".into(),
                        ))
                    }
                }))
            },
        );
        #[cfg(not(any(target_os = "linux", target_os = "windows")))]
        {
            let b = self.source.buffer();
            let (mut a, mut z) = b.selection_bounds().unwrap_or_else(|| {
                let i = b.iter_at_offset(b.cursor_position());
                (i, i)
            });
            let selected = b.text(&a, &z, false).to_string();
            let escape = |s: &str| glib::markup_escape_text(s).to_string();
            let replacement=match command {
                "bold"=>format!("<strong>{selected}</strong>"), "italic"=>format!("<em>{selected}</em>"), "underline"=>format!("<u>{selected}</u>"), "strikeThrough"=>format!("<s>{selected}</s>"),
                "insertUnorderedList"=>format!("<ul><li>{selected}</li></ul>"), "insertOrderedList"=>format!("<ol><li>{selected}</li></ol>"),
                "createLink"=>format!("<a href=\"{}\">{selected}</a>",escape(value)),
                "fontName"=>format!("<span style=\"font-family:{}\">{selected}</span>",escape(value)), "fontSizePt"=>format!("<span style=\"font-size:{}pt\">{selected}</span>",escape(value)),
                "foreColor"=>format!("<span style=\"color:{}\">{selected}</span>",escape(value)), "hiliteColor"=>format!("<span style=\"background-color:{}\">{selected}</span>",escape(value)),
                "justifyLeft"|"justifyCenter"|"justifyRight"|"justifyFull"=>format!("<div style=\"text-align:{}\">{selected}</div>",match command {"justifyCenter"=>"center","justifyRight"=>"right","justifyFull"=>"justify",_=>"left"}),
                "insertImage"=>{let v:serde_json::Value=serde_json::from_str(value)?;format!("<img src=\"{}\" width=\"160\">",escape(v["url"].as_str().unwrap_or("")))},
                _=>return Err(Error::Invalid("This action uses the visual editor in the Linux build; this Windows development preview edits HTML source".into())),
            };
            b.begin_user_action();
            b.delete(&mut a, &mut z);
            b.insert(&mut a, &replacement);
            b.end_user_action();
            done(Ok(()));
        }
        Ok(())
    }
    pub fn snapshot(&self, done: impl FnOnce(Result<String>) + 'static) {
        #[cfg(any(target_os = "linux", target_os = "windows"))]
        self.eval("window.lumaSignature.snapshot()".into(), move |result| {
            done(result.and_then(|html| luma_core::signatures::clean(&html)))
        });
        #[cfg(not(any(target_os = "linux", target_os = "windows")))]
        {
            let b = self.source.buffer();
            done(luma_core::signatures::clean(&b.text(
                &b.start_iter(),
                &b.end_iter(),
                false,
            )));
        }
    }
    pub fn dirty(&self, done: impl FnOnce(bool) + 'static) {
        #[cfg(any(target_os = "linux", target_os = "windows"))]
        self.eval("window.lumaSignature.dirty()".into(), move |v| {
            done(v.map(|s| s == "true").unwrap_or(true))
        });
        #[cfg(not(any(target_os = "linux", target_os = "windows")))]
        done(self.dirty.get());
    }
    pub fn mark_saved(&self) {
        #[cfg(any(target_os = "linux", target_os = "windows"))]
        self.eval("window.lumaSignature.saved()".into(), |_| {});
        #[cfg(not(any(target_os = "linux", target_os = "windows")))]
        self.dirty.set(false);
    }
    pub fn image(&self, url: &str, data: &str) {
        #[cfg(any(target_os = "linux", target_os = "windows"))]
        self.eval(
            format!(
                "window.lumaSignature.image({},{})",
                serde_json::to_string(url).unwrap(),
                serde_json::to_string(data).unwrap()
            ),
            |_| {},
        );
        #[cfg(not(any(target_os = "linux", target_os = "windows")))]
        let _ = (url, data);
    }
    pub fn replace(&self, html: &str) -> Result<()> {
        let html = luma_core::signatures::clean(html)?;
        #[cfg(any(target_os = "linux", target_os = "windows"))]
        self.eval(
            format!(
                "window.lumaSignature.replace({})",
                serde_json::to_string(&html).unwrap()
            ),
            |_| {},
        );
        #[cfg(not(any(target_os = "linux", target_os = "windows")))]
        {
            self.source.buffer().set_text(&html);
            self.dirty.set(false);
        }
        Ok(())
    }
    pub fn images(&self, done: impl FnOnce(Vec<String>) + 'static) {
        #[cfg(any(target_os = "linux", target_os = "windows"))]
        self.eval(
            "JSON.stringify(window.lumaSignature.images())".into(),
            move |v| {
                done(
                    v.ok()
                        .and_then(|s| serde_json::from_str(&s).ok())
                        .unwrap_or_default(),
                )
            },
        );
        #[cfg(not(any(target_os = "linux", target_os = "windows")))]
        done(Vec::new());
    }
}
