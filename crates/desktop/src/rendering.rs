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
