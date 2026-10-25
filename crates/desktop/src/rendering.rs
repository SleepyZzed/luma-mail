use crate::ui::{self, State};
use adw::prelude::*;
use luma_core::{html_mail, model::Message};
use std::rc::Rc;

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
            "<a href=\"{}\">{}</a>",
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
    label
}
pub fn append(s: &Rc<State>, body: &gtk::Box, message: &Message, text: &str, account: &str) {
    #[cfg(target_os = "linux")]
    if let Ok(Some(html)) = html_mail::body(&message.payload) {
        linux::append(s, body, message, account, html);
        return;
    }
    body.append(&plain(s, text));
    crate::message_images::append(s, body, message, account);
}
#[cfg(target_os = "linux")]
pub fn diagnostic(s: &Rc<State>) {
    linux::diagnostic(s);
}

#[cfg(target_os = "linux")]
mod linux {
    use super::*;
    use crate::attachments::{PREVIEWS, Source};
    use gtk::{gio, glib};
    use luma_core::{Result, mail_images, preview};
    use std::{cell::Cell, collections::HashMap};
    use webkit6::prelude::*;

    fn view(s: &Rc<State>) -> webkit6::WebView {
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
        let session = webkit6::NetworkSession::new_ephemeral();
        session.connect_download_started(|_, download| download.cancel());
        let context = webkit6::WebContext::new();
        if let Some(root) = std::env::var_os("APPDIR") {
            let runtime = std::path::PathBuf::from(root).join("usr");
            if let Ok(runtime) = runtime.canonicalize() {
                context.add_path_to_sandbox(runtime, true);
            }
        }
        context.set_cache_model(webkit6::CacheModel::DocumentViewer);
        let view = webkit6::WebView::builder()
            .settings(&settings)
            .network_session(&session)
            .web_context(&context)
            .build();
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
        view.connect_load_changed(|view, event| {
            if event == webkit6::LoadEvent::Finished {
                measure(view);
            }
        });
        let width = Cell::new(0);
        view.add_tick_callback(move |view, _| {
            if width.replace(view.width()) != view.width() {
                measure(view);
            }
            glib::ControlFlow::Continue
        });
        view
    }
    fn measure(view: &webkit6::WebView) {
        let weak = view.downgrade();
        view.evaluate_javascript(
            "Math.min(6000, Math.max(48, document.body ? document.body.scrollHeight + 8 : 160))",
            Some("LumaLayout"),
            None,
            None::<&gio::Cancellable>,
            move |result| {
                if let (Some(view), Ok(value)) = (weak.upgrade(), result) {
                    let height = value.to_double();
                    if height.is_finite() {
                        view.set_height_request(height.clamp(48., 6000.) as i32);
                    }
                }
            },
        );
    }
    pub(super) fn diagnostic(s: &Rc<State>) {
        ui::clear(&s.reader);
        let view = view(s);
        s.reader.append(&view);
        let done = Rc::new(Cell::new(false));
        let tested = done.clone();
        view.connect_load_changed(move |view,event| {
            if event!=webkit6::LoadEvent::Finished||tested.replace(true) {return;}
            let weak=view.downgrade();
            view.evaluate_javascript("JSON.stringify({table:document.querySelectorAll('table').length,link:document.querySelector('a').href,colour:getComputedStyle(document.querySelector('td')).color,scripts:document.querySelectorAll('script,iframe,form').length,executed:document.body.dataset.executed||'',image:document.querySelector('img').naturalWidth})",Some("LumaLayout"),None,None::<&gio::Cancellable>,move |result| {
                let value=result.expect("HTML engine must evaluate native isolated-world layout");
                let value:serde_json::Value=serde_json::from_str(&value.to_str()).expect("renderer diagnostic JSON");
                assert_eq!(value["table"],1);assert_eq!(value["link"],"https://example.com/");assert_eq!(value["colour"],"rgb(145, 70, 255)");assert_eq!(value["scripts"],0);assert_eq!(value["executed"],"");assert_eq!(value["image"],1);
                if let Some(view)=weak.upgrade(){view.set_widget_name("html-diagnostic-passed");}
                println!("PASS actual WebKit HTML layout, hyperlink, prepared image and inert sender content");
            });
        });
        let html = r##"<table style="width:500px"><tr><td style="color:#9146ff;padding:24px"><h1>Newsletter preview</h1><a href="https://example.com">Watch now</a><img src="cid:photo"><script>document.body.dataset.executed='yes'</script><iframe src="https://example.invalid"></iframe><form><input></form></td></tr></table>"##;
        let png = "data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mP8/x8AAwMCAO+jRZkAAAAASUVORK5CYII=";
        view.load_html(
            &html_mail::document(html, HashMap::from([("cid:photo".into(), png.into())])),
            Some("about:blank"),
        );
        let root = std::path::PathBuf::from(
            std::env::var_os("LUMA_RENDER_MEDIA").expect("media fixtures required"),
        );
        let mut streams = Vec::new();
        for name in ["audio.wav", "video.webm"] {
            let input = gio::MemoryInputStream::from_bytes(&glib::Bytes::from_owned(
                std::fs::read(root.join(name)).expect("synthetic media"),
            ));
            let media = gtk::MediaFile::for_input_stream(&input);
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
                for media in &streams {
                    media.pause();
                    media.clear();
                }
                println!(
                    "PASS actual in-memory WAV and WebM decoding/playback; no attachment temp files"
                );
                app.quit();
                return glib::ControlFlow::Break;
            }
            glib::ControlFlow::Continue
        });
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
                let result =
                    tokio::task::spawn_blocking(move || html_mail::image_data(&bytes)).await;
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
    pub(super) fn append(
        s: &Rc<State>,
        body: &gtk::Box,
        message: &Message,
        account: &str,
        html: String,
    ) {
        let view = view(s);
        let external = mail_images::sources(&message.payload)
            .iter()
            .any(|s| s.starts_with("https://"));
        if external {
            let bar = gtk::Box::new(gtk::Orientation::Horizontal, 8);
            let notice = ui::label("External images are blocked for privacy.", "caption");
            notice.set_wrap(true);
            notice.set_hexpand(true);
            bar.append(&notice);
            let load = gtk::Button::with_label("Load images");
            load.set_tooltip_text(Some(
                "Contact this message’s image servers. This may reveal that you opened the email.",
            ));
            bar.append(&load);
            body.append(&bar);
            let state = s.clone();
            let weak = view.downgrade();
            let message = message.clone();
            let client = s.clients.borrow().get(account).cloned();
            let source = html.clone();
            load.connect_clicked(move |button| {
                button.set_sensitive(false);
                button.set_label("Loading images…");
                let button = button.downgrade();
                let weak = weak.clone();
                let message = message.clone();
                let client = client.clone();
                let html = source.clone();
                let callback_view = weak.clone();
                let request = ui::async_work(
                    &state,
                    async move { Ok(image_map(message, client, true).await) },
                    move |_, images| {
                        let empty = images.is_empty();
                        if let Some(view) = callback_view.upgrade() {
                            view.load_html(
                                &html_mail::document(&html, images),
                                Some("about:blank"),
                            );
                        }
                        if let Some(button) = button.upgrade() {
                            button.set_label(if empty {
                                "Images unavailable"
                            } else {
                                "Images loaded"
                            });
                        }
                    },
                );
                if let Some(view) = weak.upgrade() {
                    view.connect_unrealize(move |_| request.abort());
                }
            });
        }
        body.append(&view);
        view.load_html(
            &html_mail::document(&html, HashMap::new()),
            Some("about:blank"),
        );
        let client = s.clients.borrow().get(account).cloned();
        let message = message.clone();
        let weak = view.downgrade();
        let request = ui::async_work(
            s,
            async move { Ok(image_map(message, client, false).await) },
            move |_, images| {
                if !images.is_empty()
                    && let Some(view) = weak.upgrade()
                {
                    view.load_html(&html_mail::document(&html, images), Some("about:blank"));
                }
            },
        );
        view.connect_unrealize(move |_| request.abort());
    }
}
