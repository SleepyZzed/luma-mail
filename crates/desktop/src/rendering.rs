use crate::attachments::{PREVIEWS, Source};
use crate::ui::{self, State};
use adw::prelude::*;
use luma_core::{Result, mail_images, preview};
use luma_core::{html_mail, model::Message};
use std::{cell::RefCell, collections::HashMap, rc::Rc};

#[allow(deprecated)] // GTK's named CSS colours also include the active Noctalia provider.
fn appearance(s: &State) -> html_mail::Appearance {
    let dark = adw::StyleManager::default().is_dark();
    let context = s.window.style_context();
    let bytes = |c: gtk::gdk::RGBA| {
        [c.red(), c.green(), c.blue()].map(|v| (v.clamp(0., 1.) * 255.).round() as u8)
    };
    html_mail::Appearance {
        background: context
            .lookup_color("window_bg_color")
            .map(bytes)
            .unwrap_or(if dark { [30, 30, 30] } else { [250, 250, 250] }),
        foreground: bytes(s.window.color()),
        dark,
    }
}
pub fn refresh_appearance(s: &Rc<State>) {
    if s.html_refresh_pending.replace(true) {
        return;
    }
    let weak = Rc::downgrade(s);
    gtk::glib::idle_add_local_once(move || {
        if let Some(s) = weak.upgrade() {
            s.html_refresh_pending.set(false);
            let jobs: Vec<_> = s
                .html_refresh
                .borrow()
                .iter()
                .filter_map(|f| f.upgrade())
                .collect();
            s.html_refresh.borrow_mut().retain(|f| f.strong_count() > 0);
            for refresh in jobs {
                refresh();
            }
        }
    });
}
struct Document {
    html: String,
    images: HashMap<String, String>,
    original: bool,
    revision: u64,
    rendered: Option<(Option<html_mail::Appearance>, u64, u16)>,
}
impl Document {
    fn merge(&mut self, images: HashMap<String, String>) {
        let mut bytes: usize = self.images.values().map(String::len).sum();
        for (source, image) in images {
            if !self.images.contains_key(&source)
                && self.images.len() < 8
                && bytes + image.len() <= 12 * 1024 * 1024
            {
                bytes += image.len();
                self.images.insert(source, image);
                self.revision = self.revision.wrapping_add(1);
            }
        }
    }
}
fn html_controls(
    s: &Rc<State>,
    body: &gtk::Box,
    owner: &gtk::Widget,
    message: &Message,
    account: &str,
    html: String,
    apply: impl Fn(String, f64) + 'static,
) -> Rc<RefCell<Document>> {
    let document = Rc::new(RefCell::new(Document {
        html,
        images: HashMap::new(),
        original: false,
        revision: 0,
        rendered: None,
    }));
    let model = document.clone();
    let weak = Rc::downgrade(s);
    let refresh: Rc<dyn Fn()> = Rc::new(move || {
        if let Some(s) = weak.upgrade() {
            let mut model = model.borrow_mut();
            let colours = (!model.original).then(|| appearance(&s));
            let key = (
                colours,
                model.revision,
                s.settings.borrow().ui_scale_percent,
            );
            if model.rendered == Some(key) {
                return;
            }
            model.rendered = Some(key);
            apply(
                html_mail::styled_document(&model.html, &model.images, colours),
                crate::presentation::scale(&s),
            );
        }
    });
    s.html_refresh.borrow_mut().retain(|f| f.strong_count() > 0);
    s.html_refresh.borrow_mut().push(Rc::downgrade(&refresh));
    let mapped = refresh.clone();
    owner.connect_map(move |_| mapped());
    let bar = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    let notice = ui::label("", "caption");
    notice.set_wrap(true);
    notice.set_hexpand(true);
    bar.append(&notice);
    let original = gtk::ToggleButton::with_label("Original colours");
    original.add_css_class("flat");
    original.set_tooltip_text(Some("Use the sender’s colours instead of matching Luma’s theme. Images retain their own colours in either mode."));
    bar.append(&original);
    let model = document.clone();
    let redraw = refresh.clone();
    original.connect_toggled(move |b| {
        model.borrow_mut().original = b.is_active();
        redraw();
    });
    let sources = mail_images::sources(&message.payload);
    let client = s.clients.borrow().get(account).cloned();
    if sources.iter().any(|source| source.starts_with("https://")) {
        notice.set_text("External images are blocked for privacy.");
        let load = gtk::Button::with_label("Load images");
        load.set_tooltip_text(Some(
            "Contact this message’s image servers. This may reveal that you opened the email.",
        ));
        bar.append(&load);
        let weak = Rc::downgrade(s);
        let message = message.clone();
        let client = client.clone();
        let model = document.clone();
        let redraw = refresh.clone();
        let owner = owner.downgrade();
        let notice = notice.downgrade();
        load.connect_clicked(move |button| {
            let Some(s) = weak.upgrade() else { return };
            button.set_sensitive(false);
            button.set_label("Loading images…");
            let button = button.downgrade();
            let notice = notice.clone();
            let model = model.clone();
            let redraw = redraw.clone();
            let message = message.clone();
            let client = client.clone();
            let request = ui::async_work(
                &s,
                async move { Ok(image_map(message, client, true).await) },
                move |_, images| {
                    let loaded = images.keys().any(|url| url.starts_with("https://"));
                    model.borrow_mut().merge(images);
                    redraw();
                    if let Some(button) = button.upgrade() {
                        button.set_label(if loaded {
                            "Images loaded"
                        } else {
                            "Retry images"
                        });
                        button.set_sensitive(!loaded);
                    }
                    if let Some(notice) = notice.upgrade() {
                        notice.set_text(if loaded {
                            "External images loaded for this message."
                        } else {
                            "External images could not be loaded."
                        });
                    }
                },
            );
            if let Some(owner) = owner.upgrade() {
                owner.connect_unrealize(move |_| request.abort());
            }
        });
    }
    body.append(&bar);
    refresh();
    if sources.iter().any(|source| !source.starts_with("https://")) {
        let model = document.clone();
        let redraw = refresh;
        let message = message.clone();
        let request = ui::async_work(
            s,
            async move { Ok(image_map(message, client, false).await) },
            move |_, images| {
                model.borrow_mut().merge(images);
                redraw();
            },
        );
        owner.connect_unrealize(move |_| request.abort());
    }
    document
}

pub fn open_link(s: &Rc<State>, uri: &str) {
    let Ok(url) = url::Url::parse(uri) else {
        return;
    };
    if !matches!(url.scheme(), "https" | "http" | "mailto") {
        return;
    }
    let state = s.clone();
    let uri = url.to_string();
    gtk::glib::MainContext::default().spawn_local(async move {
        if gtk::UriLauncher::new(&uri)
            .launch_future(Some(&state.window))
            .await
            .is_err()
        {
            state.info("No application could open this link.");
        }
    });
}
pub fn plain(s: &Rc<State>, text: &str) -> gtk::Label {
    let label = ui::label("", "reader-body");
    let mut markup = String::new();
    let mut offset = 0;
    for (start, end, target) in html_mail::links(text) {
        markup.push_str(&gtk::glib::markup_escape_text(&text[offset..start]));
        markup.push_str(&format!(
            "<a href=\"{}\"><span foreground=\"#4285f4\">{}</span></a>",
            gtk::glib::markup_escape_text(&target),
            gtk::glib::markup_escape_text(&text[start..end])
        ));
        offset = end;
    }
    markup.push_str(&gtk::glib::markup_escape_text(&text[offset..]));
    label.set_markup(&markup);
    label.set_wrap(true);
    label.set_wrap_mode(gtk::pango::WrapMode::WordChar);
    label.set_selectable(true);
    let state = s.clone();
    label.connect_activate_link(move |_, uri| {
        open_link(&state, uri);
        gtk::glib::Propagation::Stop
    });
    let gesture = gtk::GestureClick::new();
    gesture.set_button(1);
    gesture.set_propagation_phase(gtk::PropagationPhase::Capture);
    let targets = html_mail::links(text);
    let weak = label.downgrade();
    let state = s.clone();
    gesture.connect_pressed(move |gesture, _, x, y| {
        if !gesture
            .current_event_state()
            .contains(gtk::gdk::ModifierType::CONTROL_MASK)
        {
            return;
        }
        if let Some(label) = weak.upgrade() {
            let (dx, dy) = label.layout_offsets();
            let (inside, index, _) = label.layout().xy_to_index(
                (x as i32 - dx) * gtk::pango::SCALE,
                (y as i32 - dy) * gtk::pango::SCALE,
            );
            if inside
                && let Some((_, _, uri)) = targets
                    .iter()
                    .find(|(a, z, _)| *a <= index as usize && (index as usize) < *z)
            {
                open_link(&state, uri);
                gesture.set_state(gtk::EventSequenceState::Claimed);
            }
        }
    });
    label.add_controller(gesture);
    label
}
#[cfg(debug_assertions)]
pub(crate) fn preview_html(s: &Rc<State>, body: &gtk::Box, html: String) {
    #[cfg(target_os = "windows")]
    windows::append(s, body, &Message::default(), "design-fixture", html);
    #[cfg(target_os = "linux")]
    linux::append(s, body, &Message::default(), "design-fixture", html);
}
pub fn append(s: &Rc<State>, body: &gtk::Box, message: &Message, text: &str, account: &str) {
    if let Ok(Some(html)) = html_mail::body(&message.payload)
        && !html_mail::needs_layout(&html)
    {
        body.append(&plain(s, text));
        return;
    }
    #[cfg(target_os = "windows")]
    if let Ok(Some(html)) = html_mail::body(&message.payload) {
        windows::append(s, body, message, account, html);
        return;
    }
    #[cfg(target_os = "linux")]
    if let Ok(Some(html)) = html_mail::body(&message.payload) {
        if linux::sandbox_available() {
            linux::append(s, body, message, account, html);
            return;
        }
        let notice = ui::label(
            "HTML preview is unavailable because this system blocks its sandbox. Showing readable text. See Troubleshooting for native package/AppArmor setup.",
            "caption",
        );
        notice.set_wrap(true);
        body.append(&notice);
    }
    body.append(&plain(s, text));
    crate::message_images::append(s, body, message, account);
}
#[cfg(target_os = "windows")]
#[path = "rendering_windows.rs"]
mod windows;
#[cfg(target_os = "linux")]
pub fn diagnostic(s: &Rc<State>) {
    linux::diagnostic(s);
}
#[cfg(target_os = "linux")]
pub fn performance_fixture(s: &Rc<State>, count: usize) {
    ui::clear(&s.reader);
    for _ in 0..count.min(3) {
        linux::append(
            s,
            &s.reader,
            &Message::default(),
            "performance-fixture",
            include_str!("../tests/fixtures/layout-mail.html").into(),
        );
    }
}
#[cfg(target_os = "linux")]
pub(crate) fn signature_webview(s: &Rc<State>) -> Result<webkit6::WebView> {
    if !linux::sandbox_available() {
        return Err(luma_core::Error::Invalid("The HTML sandbox is unavailable. Enable the supported Linux sandbox before opening the rich signature editor.".into()));
    }
    Ok(linux::signature_view(s))
}

async fn image_map(
    message: Message,
    client: Option<std::sync::Arc<luma_core::gmail::Gmail>>,
    remote: bool,
) -> HashMap<String, String> {
    let mut images = HashMap::new();
    let mut budget = 0;
    let mut embedded_count = 0;
    for url in mail_images::sources(&message.payload) {
        if images.len() >= 8 || budget >= 12 * 1024 * 1024 {
            break;
        }
        let Ok(_permit) = PREVIEWS.acquire().await else {
            break;
        };
        let bytes: Result<Vec<u8>> = if url.starts_with("https://") {
            if !remote {
                continue;
            }
            mail_images::fetch_external(&url).await
        } else if let Some(cid) = url.strip_prefix("cid:") {
            if embedded_count >= 4 {
                continue;
            }
            embedded_count += 1;
            let part = message
                .payload
                .attachments()
                .into_iter()
                .find(|p| p.header("Content-ID").trim_matches(['<', '>']) == cid);
            if let (Some(part), Some(client)) = (part, client.clone()) {
                Source::Gmail {
                    client,
                    message: message.id.clone(),
                    part: part.clone(),
                }
                .read(preview::AUTO_IMAGE_BYTES as usize)
                .await
            } else {
                continue;
            }
        } else if url.starts_with("data:image/") {
            mail_images::embedded(&url)
        } else {
            continue;
        };
        if let Ok(bytes) = bytes {
            let result = tokio::task::spawn_blocking(move || html_mail::image_data(&bytes)).await;
            if let Ok(Ok(data)) = result
                && data.len() <= 3 * 1024 * 1024
                && budget + data.len() <= 12 * 1024 * 1024
            {
                budget += data.len();
                images.insert(url, data);
            }
        }
    }
    images
}
#[cfg(target_os = "linux")]
mod linux {
    use super::*;
    use gtk::{gio, glib};
    use std::cell::Cell;
    use webkit6::prelude::*;

    thread_local! {
        // Weak owners: share engines while views coexist, release the session
        // and processes when the last message view is destroyed.
        static ENGINE: RefCell<(glib::WeakRef<webkit6::WebContext>, glib::WeakRef<webkit6::NetworkSession>)> = Default::default();
        static PEERS: RefCell<Vec<glib::WeakRef<webkit6::WebView>>> = const { RefCell::new(Vec::new()) };
    }
    fn engine() -> (webkit6::WebContext, webkit6::NetworkSession) {
        ENGINE.with(|saved| {
            let mut saved = saved.borrow_mut();
            if let (Some(context), Some(session)) = (saved.0.upgrade(), saved.1.upgrade()) {
                return (context, session);
            }
            let session = webkit6::NetworkSession::new_ephemeral();
            session.connect_download_started(|_, download| download.cancel());
            let mut pressure = webkit6::MemoryPressureSettings::new();
            pressure.set_memory_limit(256);
            pressure.set_kill_threshold(0.);
            // Each setter validates against the current neighbouring thresholds.
            // Raise the default strict threshold before raising conservative.
            pressure.set_strict_threshold(0.75);
            pressure.set_conservative_threshold(0.5);
            pressure.set_poll_interval(30.);
            let context = webkit6::WebContext::builder()
                .memory_pressure_settings(&pressure)
                .build();
            let graphics = gtk::glib::user_runtime_dir().join("luma-mail/graphics");
            if graphics.is_dir() {
                context.add_path_to_sandbox(graphics, false);
            }
            if let Some(root) = std::env::var_os("APPDIR") {
                let runtime = std::path::PathBuf::from(root).join("usr");
                if let Ok(runtime) = runtime.canonicalize() {
                    context.add_path_to_sandbox(runtime, true);
                }
            }
            context.set_cache_model(webkit6::CacheModel::DocumentViewer);
            *saved = (context.downgrade(), session.downgrade());
            (context, session)
        })
    }

    pub(super) fn sandbox_available() -> bool {
        static AVAILABLE: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
        *AVAILABLE.get_or_init(|| {
            let executable = std::env::var_os("APPDIR")
                .map(|root| std::path::PathBuf::from(root).join("usr/bin/bwrap"))
                .unwrap_or_else(|| "/usr/bin/bwrap".into());
            // Probe the same namespace capability before WebKit's fatal helper
            // launch. Never disable its sandbox to make HTML available.
            std::process::Command::new(executable)
                .args([
                    "--ro-bind",
                    "/",
                    "/",
                    "--unshare-user",
                    "--unshare-pid",
                    "--unshare-net",
                    "--die-with-parent",
                    "/usr/bin/true",
                ])
                .output()
                .is_ok_and(|r| r.status.success())
        })
    }

    fn view(s: &Rc<State>) -> webkit6::WebView {
        view_base(s, true)
    }
    pub(super) fn signature_view(s: &Rc<State>) -> webkit6::WebView {
        view_base(s, false)
    }
    fn view_base(s: &Rc<State>, auto_height: bool) -> webkit6::WebView {
        let settings = webkit6::Settings::new();
        // Native layout measurements are evaluated in an isolated world. Sender
        // scripts are disabled separately, removed by sanitization and blocked
        // by CSP. No mail text is interpolated into evaluated code.
        settings.set_enable_javascript_markup(false);
        settings.set_enable_html5_local_storage(false);
        settings.set_enable_html5_database(false);
        settings.set_allow_file_access_from_file_urls(false);
        settings.set_allow_universal_access_from_file_urls(false);
        settings.set_enable_media(false);
        settings.set_enable_media_stream(false);
        settings.set_enable_webgl(false);
        settings.set_javascript_can_access_clipboard(false);
        settings.set_enable_smooth_scrolling(true);
        settings.set_enable_page_cache(false);
        let (context, session) = engine();
        let view = PEERS.with(|peers| {
            let mut peers = peers.borrow_mut();
            peers.retain(|p| p.upgrade().is_some());
            // Every document is sanitized local about:blank with sender
            // scripting/network navigation disabled. Related views share the
            // renderer process while keeping separate page DOMs and controls.
            let view = if let Some(peer) = peers.iter().find_map(glib::WeakRef::upgrade) {
                webkit6::WebView::builder().related_view(&peer).build()
            } else {
                webkit6::WebView::builder()
                    .settings(&settings)
                    .network_session(&session)
                    .web_context(&context)
                    .build()
            };
            peers.push(view.downgrade());
            view
        });
        view.set_background_color(&gtk::gdk::RGBA::TRANSPARENT);
        view.set_hexpand(true);
        view.set_vexpand(false);
        view.set_height_request(160);
        let state = s.clone();
        view.connect_decide_policy(move |_, decision, kind| {
            if matches!(
                kind,
                webkit6::PolicyDecisionType::NavigationAction
                    | webkit6::PolicyDecisionType::NewWindowAction
            ) {
                if let Some(navigation) =
                    decision.downcast_ref::<webkit6::NavigationPolicyDecision>()
                    && let Some(action) = navigation.navigation_action()
                    && let Some(uri) = action.request().and_then(|r| r.uri())
                {
                    if uri == "about:blank" {
                        return false;
                    }
                    if action.is_user_gesture() {
                        open_link(&state, &uri);
                    }
                }
                decision.ignore();
                return true;
            }
            false
        });
        view.connect_permission_request(|_, request| {
            request.deny();
            true
        });
        if auto_height {
            let revision = Rc::new(Cell::new(0u64));
            view.connect_load_changed(move |view, event| {
                if event == webkit6::LoadEvent::Finished {
                    measure(view, &revision);
                }
            });
        }
        view
    }
    fn host(view: &webkit6::WebView) -> gtk::Overlay {
        let host = gtk::Overlay::new();
        host.set_child(Some(view));
        // Allocation events replace a permanent per-frame callback. This also
        // catches pane resizing, scaling and content-height changes.
        let probe = gtk::DrawingArea::new();
        probe.set_can_target(false);
        probe.set_hexpand(true);
        probe.set_vexpand(true);
        host.add_overlay(&probe);
        host.set_measure_overlay(&probe, false);
        let weak = view.downgrade();
        let width = Cell::new(0);
        let revision = Rc::new(Cell::new(0));
        probe.connect_resize(move |_, w, _| {
            if width.replace(w) != w
                && let Some(view) = weak.upgrade()
            {
                measure(&view, &revision);
            }
        });
        host
    }
    fn measure(view: &webkit6::WebView, revision: &Rc<Cell<u64>>) {
        let epoch = revision.get().wrapping_add(1);
        revision.set(epoch);
        let revision = revision.clone();
        let zoom = view.zoom_level();
        let width = view.width();
        let weak = view.downgrade();
        view.evaluate_javascript(
            "Math.min(60000, Math.max(48, document.body ? Math.ceil(document.body.getBoundingClientRect().height) : 160))",
            Some("LumaLayout"),
            None,
            None::<&gio::Cancellable>,
            move |result| {
                if let (Some(view), Ok(value)) = (weak.upgrade(), result) {
                    let height = value.to_double() * zoom;
                    if height.is_finite() && revision.get() == epoch && view.width() == width
                        && (view.zoom_level() - zoom).abs() < 0.001 {
                        view.set_height_request(height.clamp(48., 60000.) as i32);
                    }
                }
            },
        );
    }
    pub(super) fn diagnostic(s: &Rc<State>) {
        assert!(
            sandbox_available(),
            "WebKit namespace sandbox is unavailable on this runner"
        );
        ui::clear(&s.reader);
        let view = view(s);
        let root = std::path::PathBuf::from(
            std::env::var_os("LUMA_RENDER_MEDIA").expect("media fixtures required"),
        );
        s.settings.borrow_mut().theme = "dark".into();
        ui::apply_theme(s);
        let html = include_str!("../tests/fixtures/layout-mail.html");
        let weak = view.downgrade();
        let model = html_controls(
            s,
            &s.reader,
            view.upcast_ref(),
            &Message::default(),
            "",
            html.into(),
            move |page, zoom| {
                if let Some(view) = weak.upgrade() {
                    view.set_zoom_level(zoom);
                    view.load_html(&page, Some("about:blank"));
                }
            },
        );
        model.borrow_mut().merge(HashMap::from([(
            "cid:photo".into(),
            html_mail::image_data(
                &std::fs::read(root.join("portrait.png")).expect("portrait fixture"),
            )
            .unwrap(),
        )]));
        let bar = s
            .reader
            .last_child()
            .unwrap()
            .downcast::<gtk::Box>()
            .unwrap();
        let original = bar
            .last_child()
            .unwrap()
            .downcast::<gtk::ToggleButton>()
            .unwrap();
        s.reader.append(&host(&view));
        let stage = Rc::new(Cell::new(0));
        let busy = Rc::new(Cell::new(false));
        let state = Rc::downgrade(s);
        view.connect_load_changed(move |view, event| {
            if event != webkit6::LoadEvent::Finished || busy.replace(true) { return; }
            let stage = stage.clone(); let busy = busy.clone(); let state = state.clone();
            let original = original.clone(); let weak = view.downgrade();
            view.evaluate_javascript(include_str!("../tests/fixtures/layout-check.js"), Some("LumaLayout"), None, None::<&gio::Cancellable>, move |result| {
                let value = result.expect("native isolated-world layout");
                let value: serde_json::Value = serde_json::from_str(&value.to_str()).expect("layout JSON");
                assert_eq!(value["tables"], 2); assert_eq!(value["link"], "https://example.com/");
                assert_eq!(value["button"], "rgb(145, 71, 255)");
                assert_eq!(value["scripts"], 0); assert_eq!(value["executed"], "");
                assert_eq!(value["titleText"], false); assert_eq!(value["image"], 180);
                assert!(value["statusWidth"].as_f64().unwrap() > 40., "Status must fit a word: {value}");
                assert!(value["statusHeight"].as_f64().unwrap() < 70., "Status must not wrap vertically: {value}");
                assert!(value["imageHeight"].as_f64().unwrap() <= 480.);
                assert!(value["imageHeight"].as_f64().unwrap() >= 100.);
                let step = stage.get();
                if step == 1 {
                    assert_eq!(value["background"], "rgb(229, 227, 232)");
                } else {
                    assert_eq!(value["background"], "rgba(0, 0, 0, 0)");
                    assert_eq!(value["bodyBackground"], "rgba(0, 0, 0, 0)");
                    let colours = appearance(&state.upgrade().unwrap());
                    let [r,g,b] = colours.foreground;
                    assert_eq!(value["text"], format!("rgb({r}, {g}, {b})"));
                    let [r,g,b] = colours.background;
                    assert_eq!(value["pageBackground"], format!("rgb({r}, {g}, {b})"));
                }
                let Some(view) = weak.upgrade() else { return };
                if step >= 5 {
                    let height = (value["height"].as_f64().unwrap() * view.zoom_level()).clamp(48.,60000.) as i32;
                    assert!((view.height_request()-height).abs()<=1, "HTML height must match the final zoom/layout");
                }
                let next_view = view.clone();
                let advance = move || {
                    busy.set(false);
                    stage.set(step + 1);
                    let s = state.upgrade().unwrap();
                    match step {
                        0 => original.set_active(true),
                        1 => {
                            s.settings.borrow_mut().theme = "light".into();
                            ui::apply_theme(&s);
                            original.set_active(false);
                        }
                        2 => {
                            s.settings.borrow_mut().theme = "dark".into();
                            ui::apply_theme(&s);
                        }
                        3 => {
                            s.settings.borrow_mut().ui_scale_percent=125;
                            crate::presentation::apply(&s);
                        }
                        4 => {
                            assert!((next_view.zoom_level()-1.25).abs()<0.01);
                            s.settings.borrow_mut().ui_scale_percent=100;
                            crate::presentation::apply(&s);
                        }
                        _ => {
                            assert!((next_view.zoom_level()-1.).abs()<0.01);
                            next_view.set_widget_name("html-diagnostic-passed");
                            println!("PASS actual WebKit table sizing, title removal, inline image bounds, dark/light theme updates, Original colours and UI zoom; inert sender content");
                        }
                    }
                };
                if step < 3 && let Some(path) = std::env::var_os("LUMA_RENDER_SCREENSHOT") {
                    let path = std::path::PathBuf::from(path);
                    let stem = path.file_stem().unwrap().to_string_lossy();
                    let base = stem.strip_suffix("-newsletter").unwrap_or(&stem);
                    let label = ["dark", "original", "light"][step as usize];
                    let path = path.with_file_name(format!("{base}-{label}-newsletter.png"));
                    webkit6::prelude::WebViewExt::snapshot(&view, webkit6::SnapshotRegion::FullDocument, webkit6::SnapshotOptions::empty(), None::<&gio::Cancellable>, move |result| {
                        result.expect("theme snapshot").save_to_png(path).expect("theme PNG"); advance();
                    });
                } else { advance(); }
            });
        });
        for refresh in s.html_refresh.borrow().iter().filter_map(|f| f.upgrade()) {
            refresh();
        }
        let mut streams = Vec::new();
        for name in ["audio.wav", "video.webm"] {
            let media =
                crate::media::from_bytes(std::fs::read(root.join(name)).expect("synthetic media"))
                    .expect("anonymous RAM media source");
            media.set_muted(true);
            if name.ends_with("webm") {
                let video = gtk::Video::for_media_stream(Some(&media));
                video.set_autoplay(false);
                s.reader.append(&video);
            } else {
                s.reader.append(&gtk::MediaControls::new(Some(&media)));
            }
            streams.push(media);
        }
        let app = s.app.clone();
        let mut ticks = 0;
        let mut settled = None::<(i32, std::time::Instant)>;
        glib::timeout_add_local(std::time::Duration::from_millis(100), move || {
            ticks += 1;
            assert!(ticks < 300, "HTML/media diagnostics timed out");
            for media in &streams {
                assert!(
                    media.error().is_none(),
                    "Media decoding failed: {:?}",
                    media.error()
                );
                if media.is_prepared() && !media.is_playing() && media.timestamp() == 0 {
                    media.play();
                }
            }
            if view.widget_name() == "html-diagnostic-passed"
                && streams.iter().all(|m| m.timestamp() > 0 || m.is_ended())
            {
                let height = view.height_request();
                if settled
                    .as_ref()
                    .is_none_or(|(previous, _)| *previous != height)
                {
                    settled = Some((height, std::time::Instant::now()));
                    return glib::ControlFlow::Continue;
                }
                if settled.as_ref().unwrap().1.elapsed() < std::time::Duration::from_millis(200) {
                    return glib::ControlFlow::Continue;
                }
                for media in &streams {
                    media.pause();
                    media.clear();
                }
                println!(
                    "PASS actual in-memory WAV and WebM decoding/playback; no attachment temp files"
                );
                if let Some(path) = std::env::var_os("LUMA_RENDER_SCREENSHOT") {
                    let app = app.clone();
                    webkit6::prelude::WebViewExt::snapshot(
                        &view,
                        webkit6::SnapshotRegion::FullDocument,
                        webkit6::SnapshotOptions::empty(),
                        None::<&gio::Cancellable>,
                        move |result| {
                            let texture = result.expect("actual WebKit document snapshot");
                            assert!(texture.width() > 100 && texture.height() > 100);
                            texture.save_to_png(path).expect("renderer screenshot");
                            app.quit();
                        },
                    );
                } else {
                    app.quit();
                }
                return glib::ControlFlow::Break;
            }
            glib::ControlFlow::Continue
        });
    }
    pub(super) fn append(
        s: &Rc<State>,
        body: &gtk::Box,
        message: &Message,
        account: &str,
        html: String,
    ) {
        let view = view(s);
        let weak = view.downgrade();
        let document = html_controls(
            s,
            body,
            view.upcast_ref(),
            message,
            account,
            html,
            move |page, zoom| {
                if let Some(view) = weak.upgrade() {
                    view.set_zoom_level(zoom);
                    view.load_html(&page, Some("about:blank"));
                }
            },
        );
        let retry = gtk::Button::with_label("Reload message preview");
        let notice = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        let text = ui::label(
            "The HTML renderer stopped. Your message is still in Gmail.",
            "caption",
        );
        text.set_wrap(true);
        text.set_hexpand(true);
        notice.append(&text);
        notice.append(&retry);
        let failed = gtk::Revealer::builder()
            .child(&notice)
            .reveal_child(false)
            .build();
        body.append(&failed);
        let model = Rc::downgrade(&document);
        let failed_view = failed.downgrade();
        view.connect_web_process_terminated(move |_, _| {
            if let Some(model) = model.upgrade() {
                model.borrow_mut().rendered = None;
            }
            if let Some(failed) = failed_view.upgrade() {
                failed.set_reveal_child(true);
            }
        });
        let recovered = failed.downgrade();
        view.connect_load_changed(move |_, event| {
            if event == webkit6::LoadEvent::Finished
                && let Some(failed) = recovered.upgrade()
            {
                failed.set_reveal_child(false);
            }
        });
        let state = Rc::downgrade(s);
        retry.connect_clicked(move |_| {
            if let Some(s) = state.upgrade() {
                refresh_appearance(&s);
            }
        });
        body.append(&host(&view));
    }
}
