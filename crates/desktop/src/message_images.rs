use crate::{
    attachments::{self, Source},
    ui::{self, State, label, vbox},
};
use adw::prelude::*;
use luma_core::{Error, Result, mail_images, model::Message, preview};
use std::rc::Rc;

enum Image {
    Attachment(Source),
    Data(String),
    External(String),
}
async fn pixels(image: Image) -> Result<preview::Thumbnail> {
    let _permit = attachments::PREVIEWS
        .acquire()
        .await
        .map_err(|_| Error::Invalid("Preview cancelled".into()))?;
    let bytes = match image {
        Image::Attachment(source) => source.read(preview::AUTO_IMAGE_BYTES as usize).await?,
        Image::Data(source) => mail_images::embedded(&source)?,
        Image::External(url) => mail_images::fetch_external(&url).await?,
    };
    tokio::task::spawn_blocking(move || preview::thumbnail(&bytes, 640, 480))
        .await
        .map_err(|_| Error::Invalid("Image preview failed".into()))?
}
fn show(s: &Rc<State>, gallery: &gtk::Box, image: Image) {
    let slot = vbox(6);
    let spinner = gtk::Spinner::new();
    spinner.start();
    slot.append(&spinner);
    gallery.append(&slot);
    let weak = slot.downgrade();
    let request = ui::async_work(
        s,
        async move { Ok(pixels(image).await) },
        move |_, result| {
            let Some(slot) = weak.upgrade() else { return };
            ui::clear(&slot);
            match result {
                Ok(pixels) => {
                    let picture = gtk::Picture::for_paintable(&attachments::texture(pixels));
                    picture.set_content_fit(gtk::ContentFit::Contain);
                    picture.set_can_shrink(true);
                    picture.set_height_request(240);
                    slot.append(&picture);
                }
                Err(_) => {
                    slot.append(&label("This image could not be loaded. Its attachment, if present, can still be saved.","caption"));
                }
            }
        },
    );
    slot.connect_unrealize(move |_| request.abort());
}
pub fn append(s: &Rc<State>, body: &gtk::Box, message: &Message, account: &str) {
    let sources = mail_images::sources(&message.payload);
    let gallery = vbox(12);
    let mut embedded_count = 0;
    let mut embedded_bytes = 0;
    let mut external = Vec::new();
    for source in sources {
        if source.starts_with("https://") {
            external.push(source);
            continue;
        }
        if embedded_count >= 4 {
            continue;
        }
        if let Some(cid) = source.strip_prefix("cid:") {
            let part = message
                .payload
                .attachments()
                .into_iter()
                .find(|p| p.header("Content-ID").trim_matches(['<', '>']) == cid);
            if let Some(part) = part
                && part.body.size <= preview::AUTO_IMAGE_BYTES
                && embedded_bytes + part.body.size <= 12 * 1024 * 1024
                && let Some(client) = s.clients.borrow().get(account).cloned()
            {
                embedded_count += 1;
                embedded_bytes += part.body.size;
                show(
                    s,
                    &gallery,
                    Image::Attachment(Source::Gmail {
                        client,
                        message: message.id.clone(),
                        part: part.clone(),
                    }),
                );
            }
        } else if source.starts_with("data:image/") {
            embedded_count += 1;
            show(s, &gallery, Image::Data(source));
        }
    }
    if !external.is_empty() {
        let bar = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        let notice = label("External images are blocked for privacy.", "caption");
        notice.set_wrap(true);
        notice.set_hexpand(true);
        bar.append(&notice);
        let load = gtk::Button::with_label("Load images");
        load.set_tooltip_text(Some("Contact the sender’s image servers for this message. This may tell the sender you opened it."));
        bar.append(&load);
        body.append(&bar);
        let state = s.clone();
        let weak_gallery = gallery.downgrade();
        load.connect_clicked(move |button| {
            button.set_sensitive(false);
            button.set_label("Images requested");
            if let Some(gallery) = weak_gallery.upgrade() {
                for url in external.iter().take(8) {
                    show(&state, &gallery, Image::External(url.clone()));
                }
            }
        });
    }
    body.append(&gallery);
}
