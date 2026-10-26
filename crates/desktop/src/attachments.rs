use crate::ui::{self, State, label, margins, vbox};
use adw::prelude::*;
use gtk::{gdk, glib};
use luma_core::{
    Error, Result,
    gmail::Gmail,
    model::{Message, Part},
    preview::{self, Kind, Preview},
};
use std::{path::PathBuf, rc::Rc, sync::Arc};
use tokio::io::AsyncReadExt;

// Shared across readers and composers: at most two preview fetch/decode jobs.
pub(crate) static PREVIEWS: tokio::sync::Semaphore = tokio::sync::Semaphore::const_new(2);

#[derive(Clone)]
pub enum Source {
    Local(PathBuf),
    Bytes(Arc<Vec<u8>>),
    Gmail {
        client: Arc<Gmail>,
        message: String,
        part: Part,
    },
}
impl Source {
    pub(crate) async fn read(self, max: usize) -> Result<Vec<u8>> {
        let data = match self {
            Self::Local(path) => {
                let file = tokio::fs::File::open(path).await?;
                if file.metadata().await?.len() > max as u64 {
                    return Err(Error::ResponseLimit);
                }
                let mut data = Vec::new();
                file.take(max as u64 + 1).read_to_end(&mut data).await?;
                data
            }
            Self::Bytes(bytes) => {
                if bytes.len() > max {
                    return Err(Error::ResponseLimit);
                }
                Arc::try_unwrap(bytes).unwrap_or_else(|b| (*b).clone())
            }
            Self::Gmail {
                client,
                message,
                part,
            } => {
                if part.body.size > max as u64 {
                    return Err(Error::ResponseLimit);
                }
                if let Some(id) = part.body.attachment_id {
                    client.attachment_limited(&message, &id, max).await?
                } else {
                    if part
                        .body
                        .data
                        .as_ref()
                        .is_some_and(|s| s.len() > max.div_ceil(3) * 4 + 4)
                    {
                        return Err(Error::ResponseLimit);
                    }
                    luma_core::composer::decode_part(&part)?
                }
            }
        };
        if data.len() > max {
            return Err(Error::ResponseLimit);
        }
        Ok(data)
    }
}

pub struct Card {
    pub root: gtk::Box,
    pub open: gtk::Button,
    pub action: gtk::Button,
    picture: gtk::Picture,
    icon: gtk::Image,
}
pub fn card(name: &str, mime: &str, bytes: u64, remove: bool) -> Card {
    let kind = Kind::of(name, mime);
    let root = gtk::Box::new(gtk::Orientation::Horizontal, 2);
    root.add_css_class("attachment-card");
    let open = gtk::Button::new();
    open.add_css_class("flat");
    open.set_hexpand(true);
    open.set_tooltip_text(Some(&format!("Preview {name}")));
    let content = gtk::Box::new(gtk::Orientation::Horizontal, 10);
    let cover = gtk::Overlay::new();
    cover.set_size_request(48, 48);
    let icon = gtk::Image::from_icon_name(kind.icon());
    icon.set_pixel_size(30);
    cover.set_child(Some(&icon));
    let picture = gtk::Picture::new();
    picture.set_content_fit(gtk::ContentFit::Cover);
    picture.set_size_request(48, 48);
    picture.set_can_shrink(true);
    picture.set_visible(false);
    picture.add_css_class("attachment-thumbnail");
    cover.add_overlay(&picture);
    content.append(&cover);
    let text = vbox(4);
    text.set_hexpand(true);
    let title = label(name, "heading");
    title.set_ellipsize(gtk::pango::EllipsizeMode::Middle);
    title.set_max_width_chars(25);
    title.set_tooltip_text(Some(name));
    text.append(&title);
    text.append(&label(
        &format!("{} · {}", kind.label(), preview::size(bytes)),
        "caption",
    ));
    content.append(&text);
    open.set_child(Some(&content));
    root.append(&open);
    let action = ui::button(
        if remove {
            "luma-close-symbolic"
        } else {
            "luma-download-symbolic"
        },
        if remove {
            "Remove attachment"
        } else {
            "Save attachment…"
        },
    );
    action.add_css_class("flat");
    action.add_css_class("circular");
    action.set_valign(gtk::Align::Center);
    root.append(&action);
    Card {
        root,
        open,
        action,
        picture,
        icon,
    }
}
pub(crate) fn texture(image: preview::Thumbnail) -> gdk::MemoryTexture {
    gdk::MemoryTexture::new(
        image.width as i32,
        image.height as i32,
        gdk::MemoryFormat::R8g8b8a8,
        &glib::Bytes::from_owned(image.rgba),
        image.width as usize * 4,
    )
}
pub fn thumbnail(s: &Rc<State>, card: &Card, source: Source) {
    let picture = card.picture.downgrade();
    let icon = card.icon.downgrade();
    let request = ui::async_work(
        s,
        async move {
            let _permit = PREVIEWS
                .acquire()
                .await
                .map_err(|_| Error::Invalid("Preview cancelled".into()))?;
            let result = async {
                let bytes = source.read(preview::AUTO_IMAGE_BYTES as usize).await?;
                tokio::task::spawn_blocking(move || preview::thumbnail(&bytes, 160, 120))
                    .await
                    .map_err(|_| Error::Invalid("Image preview failed".into()))?
            }
            .await;
            Ok(result)
        },
        move |_, result| {
            if let Some(picture) = picture.upgrade()
                && let Ok(image) = result
            {
                picture.set_paintable(Some(&texture(image)));
                picture.set_visible(true);
                if let Some(icon) = icon.upgrade() {
                    icon.set_visible(false);
                }
            }
        },
    );
    card.root.connect_unrealize(move |_| request.abort());
}
pub fn remote_strip(s: &Rc<State>, message: &Message, account: &str) -> Option<gtk::Box> {
    let parts: Vec<_> = message
        .payload
        .attachments()
        .into_iter()
        .filter(|p| {
            p.header("Content-ID").is_empty()
                || p.header("Content-Disposition")
                    .to_ascii_lowercase()
                    .starts_with("attachment")
        })
        .collect();
    if parts.is_empty() {
        return None;
    }
    let client = s.clients.borrow().get(account).cloned()?;
    let strip = vbox(6);
    strip.add_css_class("attachment-strip");
    let heading = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    let count = label(
        &format!(
            "{} attachment{}",
            parts.len(),
            if parts.len() == 1 { "" } else { "s" }
        ),
        "caption",
    );
    count.set_hexpand(true);
    heading.append(&count);
    if parts.len() > 1 {
        let all = gtk::Button::with_label("Save all…");
        all.add_css_class("flat");
        let sources: Vec<_> = parts
            .iter()
            .take(100)
            .map(|part| {
                (
                    part.filename.clone(),
                    Source::Gmail {
                        client: client.clone(),
                        message: message.id.clone(),
                        part: (*part).clone(),
                    },
                )
            })
            .collect();
        let state = Rc::downgrade(s);
        all.set_tooltip_text(Some("Save up to 100 attachments to a chosen folder"));
        all.connect_clicked(move |_| {
            if let Some(s) = state.upgrade() {
                save_all(&s, sources.clone());
            }
        });
        heading.append(&all);
    }
    strip.append(&heading);
    let flow = gtk::FlowBox::builder()
        .selection_mode(gtk::SelectionMode::None)
        .column_spacing(8)
        .row_spacing(8)
        .max_children_per_line(3)
        .min_children_per_line(1)
        .build();
    strip.append(&flow);
    let mut auto_bytes = 0;
    let mut auto_count = 0;
    for part in parts.into_iter().take(100) {
        let name = if part.filename.is_empty() {
            "Inline image"
        } else {
            &part.filename
        };
        let card = card(name, &part.mime_type, part.body.size, false);
        let source = Source::Gmail {
            client: client.clone(),
            message: message.id.clone(),
            part: part.clone(),
        };
        let kind = Kind::of(name, &part.mime_type);
        let state = s.clone();
        let title = name.to_owned();
        let mime = part.mime_type.clone();
        let bytes = part.body.size;
        let open_source = source.clone();
        card.open
            .connect_clicked(move |_| open(&state, &title, &mime, bytes, open_source.clone()));
        let state = s.clone();
        let title = name.to_owned();
        let save_source = source.clone();
        card.action
            .connect_clicked(move |_| save(&state, &title, save_source.clone()));
        flow.insert(&card.root, -1);
        if kind == Kind::Image
            && part.body.size <= preview::AUTO_IMAGE_BYTES
            && auto_count < 4
            && auto_bytes + part.body.size <= 12 * 1024 * 1024
        {
            auto_count += 1;
            auto_bytes += part.body.size;
            thumbnail(s, &card, source);
        }
    }
    Some(strip)
}
pub fn open(s: &Rc<State>, name: &str, mime: &str, bytes: u64, source: Source) {
    if let Some(window) = s.attachment_window.borrow().upgrade() {
        window.close();
    }
    let window = adw::Window::builder()
        .transient_for(&s.window)
        .application(&s.app)
        .title(name)
        .default_width(780)
        .default_height(620)
        .destroy_with_parent(true)
        .build();
    s.attachment_window.borrow_mut().set(Some(&window));
    let root = vbox(0);
    let header = adw::HeaderBar::new();
    header.set_title_widget(Some(&adw::WindowTitle::new(name, &preview::size(bytes))));
    let save_button = ui::button("luma-download-symbolic", "Save attachment…");
    header.pack_end(&save_button);
    let state = s.clone();
    let title = name.to_owned();
    let save_source = source.clone();
    save_button.connect_clicked(move |_| save(&state, &title, save_source.clone()));
    root.append(&header);
    let body = vbox(12);
    body.set_vexpand(true);
    margins(&body, 20);
    root.append(&body);
    window.set_content(Some(&root));
    let spinner = gtk::Spinner::new();
    spinner.start();
    spinner.set_vexpand(true);
    body.append(&spinner);
    window.present();
    let kind = Kind::of(name, mime);
    if kind == Kind::Other || bytes > preview::MAX_PREVIEW_BYTES as u64 {
        ui::clear(&body);
        body.append(&label(if bytes > preview::MAX_PREVIEW_BYTES as u64 { "This file is larger than the 24 MB in-app preview limit. Save it to open externally." } else { "Preview is unavailable for this file type. Save it to open in your preferred application." }, "dim-label"));
        return;
    }
    let weak_body = body.downgrade();
    let weak_window = window.downgrade();
    let request = ui::async_work(
        s,
        async move {
            let _permit = PREVIEWS
                .acquire()
                .await
                .map_err(|_| Error::Invalid("Preview cancelled".into()))?;
            Ok(async {
                let bytes = source.read(preview::MAX_PREVIEW_BYTES).await?;
                tokio::task::spawn_blocking(move || preview::create(kind, bytes))
                    .await
                    .map_err(|_| Error::Invalid("Preview failed".into()))?
            }
            .await)
        },
        move |_, result| {
            let (Some(body), Some(window)) = (weak_body.upgrade(), weak_window.upgrade()) else {
                return;
            };
            if !window.is_visible() {
                return;
            }
            ui::clear(&body);
            match result {
                Ok(Preview::Image(image)) => {
                    let picture = gtk::Picture::for_paintable(&texture(image));
                    picture.set_content_fit(gtk::ContentFit::Contain);
                    picture.set_can_shrink(true);
                    picture.set_vexpand(true);
                    body.append(&picture);
                }
                Ok(Preview::Text(text)) => {
                    let content = gtk::TextView::builder()
                        .editable(false)
                        .cursor_visible(false)
                        .wrap_mode(gtk::WrapMode::WordChar)
                        .vexpand(true)
                        .build();
                    content.buffer().set_text(&text);
                    body.append(
                        &gtk::ScrolledWindow::builder()
                            .vexpand(true)
                            .child(&content)
                            .build(),
                    );
                    if kind == Kind::Document {
                        body.append(&label(
                            "Text preview · Save to view the original document layout",
                            "caption",
                        ));
                    }
                }
                Ok(Preview::Media(bytes)) => {
                    #[cfg(not(target_os = "linux"))]
                    {
                        drop(bytes);
                        body.append(&label("Media playback is available in the Linux build. Save the file to play it on this Windows test host.", "caption"));
                    }
                    #[cfg(target_os = "linux")]
                    {
                        let media = match crate::media::from_bytes(bytes) {
                            Ok(media) => media,
                            Err(error) => {
                                body.append(&label(&error.to_string(), "error"));
                                return;
                            }
                        };
                        if kind == Kind::Video {
                            let video = gtk::Video::for_media_stream(Some(&media));
                            video.set_autoplay(false);
                            video.set_vexpand(true);
                            body.append(&video);
                        } else {
                            let image = gtk::Image::from_icon_name("luma-audio-symbolic");
                            image.set_pixel_size(96);
                            image.set_vexpand(true);
                            body.append(&image);
                            body.append(&gtk::MediaControls::new(Some(&media)));
                        }
                        let notice = label(
                            "Press Play · Playback uses your system’s installed media codecs",
                            "caption",
                        );
                        notice.set_wrap(true);
                        body.append(&notice);
                        let weak = notice.downgrade();
                        media.connect_error_notify(move |media| {
                    if media.error().is_some() && let Some(notice) = weak.upgrade() { notice.set_text("Your media backend cannot play this format. Save it to open in another player."); }
                });
                        if media.error().is_some() {
                            notice.set_text("Your media backend cannot play this format. Save it to open in another player.");
                        }
                        window.connect_close_request(move |_| {
                            media.pause();
                            media.clear();
                            glib::Propagation::Proceed
                        });
                    }
                }
                Err(error) => {
                    let text = label(
                        &format!("{error}\nYou can save the original file instead."),
                        "dim-label",
                    );
                    text.set_wrap(true);
                    body.append(&text);
                }
            }
        },
    );
    window.connect_close_request(move |_| {
        request.abort();
        glib::Propagation::Proceed
    });
}
pub fn save(s: &Rc<State>, name: &str, source: Source) {
    let safe_name = std::path::Path::new(name)
        .file_name()
        .unwrap_or_default()
        .to_string_lossy();
    let dialog = gtk::FileDialog::builder()
        .title("Save attachment")
        .initial_name(safe_name.as_ref())
        .build();
    let state = s.clone();
    glib::MainContext::default().spawn_local(async move {
        let Ok(file) = dialog.save_future(Some(&state.window)).await else {
            return;
        };
        let Some(path) = file.path() else { return };
        ui::async_work(
            &state,
            async move {
                let bytes = source.read(26 * 1024 * 1024).await?;
                tokio::fs::write(path, bytes).await?;
                Ok(())
            },
            |s, ()| s.info("Attachment saved to your chosen location"),
        );
    });
}

fn save_all(s: &Rc<State>, sources: Vec<(String, Source)>) {
    use std::{
        cell::Cell,
        sync::atomic::{AtomicBool, Ordering},
        time::Duration,
    };
    let state = s.clone();
    glib::MainContext::default().spawn_local(async move {
        let dialog = gtk::FileDialog::builder().title("Save all attachments to folder").build();
        let Ok(folder) = dialog.select_folder_future(Some(&state.window)).await else { return; };
        let Some(folder) = folder.path() else { return; };
        let cancel = Arc::new(AtomicBool::new(false));
        let window = adw::Window::builder().title("Save attachments").transient_for(&state.window)
            .default_width(420).resizable(false).modal(true).build();
        let content = vbox(16);
        margins(&content, 24);
        let status = label("Preparing downloads…", "heading");
        status.set_ellipsize(gtk::pango::EllipsizeMode::Middle);
        let bar = gtk::ProgressBar::new();
        let stop = gtk::Button::with_label("Cancel");
        content.append(&status); content.append(&bar); content.append(&stop);
        window.set_content(Some(&content));
        let cancelling = cancel.clone();
        stop.connect_clicked(move |button| { cancelling.store(true, Ordering::Relaxed); button.set_sensitive(false); });
        let cancelling = cancel.clone();
        window.connect_close_request(move |_| { cancelling.store(true, Ordering::Relaxed); glib::Propagation::Proceed });
        let total = sources.len();
        let (progress, rx) = tokio::sync::watch::channel((0usize, String::new()));
        let running = Rc::new(Cell::new(true));
        let active = running.clone();
        let weak = window.downgrade();
        glib::timeout_add_local(Duration::from_millis(100), move || {
            if !active.get() || weak.upgrade().is_none() { return glib::ControlFlow::Break; }
            let (done, name) = rx.borrow().clone();
            status.set_text(&format!("{} / {total} · {name}", done));
            bar.set_fraction(done as f64 / total.max(1) as f64);
            glib::ControlFlow::Continue
        });
        let weak = window.downgrade();
        ui::async_work(&state, async move {
            let mut done = 0;
            for (name, source) in sources {
                if cancel.load(Ordering::Relaxed) { return Ok((done, Some("Cancelled".to_owned()))); }
                progress.send_replace((done, name.clone()));
                let result = async {
                    let bytes = source.read(26 * 1024 * 1024).await?;
                    luma_core::attachment_save::save_unique(&folder, &name, &bytes, &cancel).await?;
                    Ok::<_, Error>(())
                }.await;
                if let Err(error) = result { return Ok((done, Some(error.to_string()))); }
                done += 1;
                progress.send_replace((done, name));
            }
            Ok((done, None))
        }, move |s, (done, error)| {
            running.set(false);
            if let Some(window) = weak.upgrade() { window.close(); }
            s.info(&match error {
                Some(error) => format!("Saved {done} of {total} attachments · {error}. Completed files are in your chosen folder."),
                None => format!("Saved {done} attachments to your chosen folder"),
            });
        });
        window.present();
    });
}
