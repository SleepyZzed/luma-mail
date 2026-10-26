use super::*;
const TYPES: [&str; 4] = ["image/png", "image/jpeg", "image/webp", "image/gif"];

pub(super) fn install(e: &Rc<Editor>) {
    let weak = Rc::downgrade(e);
    e.text.connect_paste_clipboard(move |text| {
        let Some(e) = weak.upgrade() else {
            return;
        };
        let clipboard = text.clipboard();
        if !TYPES
            .iter()
            .any(|mime| clipboard.formats().contain_mime_type(mime))
        {
            return;
        }
        text.stop_signal_emission_by_name("paste-clipboard");
        paste(&e, clipboard);
    });
}
fn paste(e: &Rc<Editor>, clipboard: gtk::gdk::Clipboard) {
    if e.closing.get() || e.busy.get() || e.clipboard_busy.replace(true) {
        return;
    }
    e.status.set_text("Reading image from clipboard…");
    let weak = Rc::downgrade(e);
    glib::MainContext::default().spawn_local(async move {
        let result = glib::future_with_timeout(Duration::from_secs(15), async move {
            let (stream, _) = clipboard
                .read_future(&TYPES, glib::Priority::DEFAULT)
                .await
                .map_err(|_| {
                    Error::Invalid("The clipboard image is no longer available.".into())
                })?;
            let mut data = Vec::new();
            let read = async {
                loop {
                    let chunk = stream
                        .read_bytes_future(32 * 1024, glib::Priority::DEFAULT)
                        .await
                        .map_err(|_| Error::Invalid("Cannot read the clipboard image.".into()))?;
                    if chunk.is_empty() {
                        break;
                    }
                    if data.len() + chunk.len() > luma_core::composer::MAX_ATTACHMENT_BYTES as usize
                    {
                        return Err(Error::Invalid("Pasted images must fit within 16 MiB. Save the image and attach it as a file to use Google Drive for a larger upload.".into()));
                    }
                    data.extend_from_slice(&chunk);
                }
                Ok(data)
            }
            .await;
            let _ = stream.close_future(glib::Priority::DEFAULT).await;
            read
        })
        .await
        .unwrap_or(Err(Error::Timeout));
        let Some(e) = weak.upgrade() else {
            return;
        };
        if e.closing.get() {
            e.clipboard_busy.set(false);
            return;
        }
        let bytes = match result {
            Ok(data) => data,
            Err(error) => {
                e.clipboard_busy.set(false);
                e.status.set_text(&error.to_string());
                return;
            }
        };
        let number = e.retained.borrow().len() + 1;
        let weak = Rc::downgrade(&e);
        ui::async_work(
            &e.state,
            async move {
                let _permit = crate::attachments::PREVIEWS
                    .acquire()
                    .await
                    .expect("preview semaphore");
                Ok(tokio::task::spawn_blocking(move || {
                    luma_core::clipboard_image::attachment(bytes, number)
                })
                .await
                .unwrap_or_else(|_| Err(Error::Invalid("Cannot prepare the pasted image.".into()))))
            },
            move |_, result| {
                let Some(e) = weak.upgrade() else {
                    return;
                };
                e.clipboard_busy.set(false);
                if e.closing.get() {
                    return;
                }
                match result {
                    Ok(file) => {
                        let total = e
                            .retained
                            .borrow()
                            .iter()
                            .map(|f| f.data.len() as u64)
                            .sum::<u64>()
                            + file.data.len() as u64;
                        if total > luma_core::composer::MAX_ATTACHMENT_BYTES
                            || e.retained.borrow().len() >= 64
                        {
                            e.status.set_text(
                                "Pasted/reopened attachments must fit within 16 MiB and 64 files.",
                            );
                            return;
                        }
                        e.retained.borrow_mut().push(file);
                        render_files(&e);
                        changed(&e);
                        e.status.set_text("Image attached · Autosaves to Gmail");
                    }
                    Err(error) => e.status.set_text(&error.to_string()),
                }
            },
        );
    });
}
