//! Remote signature settings, independent of any local draft or mail cache.
use crate::{
    signature_rich::RichEditor,
    ui::{self, State},
};
use adw::prelude::*;
use gtk::glib;
use luma_core::{
    Error, Result,
    config::Account,
    signatures::{Signature, SignatureStore},
};
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
    sync::Arc,
};

struct Editor {
    state: Rc<State>,
    account: Account,
    window: adw::Window,
    rich: RichEditor,
    baseline: RefCell<Signature>,
    status: gtk::Label,
    save: gtk::Button,
    busy: Cell<bool>,
    allow_close: Cell<bool>,
    checking_close: Cell<bool>,
    on_saved: Option<Box<dyn Fn()>>,
    progress: gtk::ProgressBar,
    cancel_upload: gtk::Button,
    uploading: Cell<bool>,
    cancelled: Arc<std::sync::atomic::AtomicBool>,
}
fn store(s: &State, account: &str) -> Option<Arc<dyn SignatureStore>> {
    s.clients
        .borrow()
        .get(account)
        .cloned()
        .map(|c| c as Arc<dyn SignatureStore>)
}
pub fn open(s: &Rc<State>, account: &str, identity: &str, on_saved: Option<Box<dyn Fn()>>) {
    let Some(account) = s
        .settings
        .borrow()
        .accounts
        .iter()
        .find(|a| a.id == account)
        .cloned()
    else {
        return;
    };
    let Some(provider) = store(s, &account.id) else {
        s.info("Connect this account before editing its signature");
        return;
    };
    let identity = identity.to_string();
    let state = s.clone();
    ui::async_work(
        s,
        async move { Ok(provider.signature(&identity).await) },
        move |_, result| match result {
            Ok(signature) => match loaded(&state, account, signature, on_saved) {
                Ok(_) => (),
                Err(e) => state.error(&e),
            },
            Err(e) => state.error(&e),
        },
    );
}
fn apply(e: &Rc<Editor>, command: &str, value: &str) {
    if e.busy.get() {
        return;
    }
    let weak = Rc::downgrade(e);
    match e.rich.command(command, value, move |result| {
        if let Some(e) = weak.upgrade() {
            match result {
                Ok(()) => e.status.set_text("Unsaved signature changes"),
                Err(error) => e.status.set_text(&error.to_string()),
            }
        }
    }) {
        Ok(()) => (),
        Err(error) => e.status.set_text(&error.to_string()),
    }
}
fn action(flow: &gtk::FlowBox, e: &Rc<Editor>, icon: &str, title: &str, command: &'static str) {
    let b = ui::button(icon, title);
    b.add_css_class("flat");
    b.set_focus_on_click(false);
    flow.insert(&b, -1);
    let weak = Rc::downgrade(e);
    b.connect_clicked(move |_| {
        if let Some(e) = weak.upgrade() {
            apply(&e, command, "");
        }
    });
}
fn choice(flow: &gtk::FlowBox, e: &Rc<Editor>, title: &str, values: &[&str], commands: &[&str]) {
    let input = gtk::DropDown::from_strings(values);
    input.set_tooltip_text(Some(title));
    flow.insert(&input, -1);
    let commands = commands.iter().map(|s| s.to_string()).collect::<Vec<_>>();
    let weak = Rc::downgrade(e);
    input.connect_selected_notify(move |input| {
        if let Some(e) = weak.upgrade()
            && let Some(command) = commands.get(input.selected() as usize)
        {
            apply(&e, command, "");
        }
    });
}
fn loaded(
    s: &Rc<State>,
    account: Account,
    signature: Signature,
    on_saved: Option<Box<dyn Fn()>>,
) -> Result<Rc<Editor>> {
    let window = adw::Window::builder()
        .application(&s.app)
        .transient_for(&s.window)
        .title(format!("Signature · {}", signature.identity))
        .default_width(840)
        .default_height(660)
        .build();
    crate::presentation::configure_window(&window, s.settings.borrow().window_chrome);
    let root = ui::vbox(0);
    root.append(&adw::HeaderBar::new());
    let intro = ui::label(
        "Saved online in Gmail. Changes also affect the signature used by Gmail web; no local signature archive is kept.",
        "caption",
    );
    intro.set_wrap(true);
    ui::margins(&intro, 12);
    root.append(&intro);
    #[cfg(not(any(target_os = "linux", target_os = "windows")))]
    {
        let note = ui::label(
            "Windows development preview edits HTML source. The Linux build provides the visual rich-text editor.",
            "caption",
        );
        note.set_wrap(true);
        root.append(&note);
    }
    let flow = gtk::FlowBox::builder()
        .halign(gtk::Align::Fill)
        .selection_mode(gtk::SelectionMode::None)
        .min_children_per_line(1)
        .max_children_per_line(32)
        .column_spacing(3)
        .row_spacing(3)
        .hexpand(true)
        .build();
    flow.add_css_class("compose-toolbar");
    root.append(
        &gtk::ScrolledWindow::builder()
            .child(&flow)
            .hscrollbar_policy(gtk::PolicyType::Never)
            .vscrollbar_policy(gtk::PolicyType::Automatic)
            .min_content_height(112)
            .max_content_height(168)
            .propagate_natural_height(true)
            .build(),
    );
    let initial =
        if signature.can_update && signature.html.is_empty() && !account.signature.is_empty() {
            format!(
                "<div>{}</div>",
                glib::markup_escape_text(&account.signature).replace('\n', "<br>")
            )
        } else {
            signature.html.clone()
        };
    let rich = RichEditor::new(s, &initial, signature.can_update)?;
    root.append(&rich.widget);
    let status = ui::label(
        if signature.can_update {
            "Edit the signature, then Save to Gmail"
        } else {
            "Alias signature · Read-only in this integration. Edit the primary address's signature in Settings."
        },
        "caption",
    );
    status.set_wrap(true);
    ui::margins(&status, 10);
    root.append(&status);
    let progress = gtk::ProgressBar::new();
    progress.set_show_text(true);
    progress.set_visible(false);
    root.append(&progress);
    let cancel_upload = gtk::Button::with_label("Cancel picture upload");
    cancel_upload.set_visible(false);
    root.append(&cancel_upload);
    let footer = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    ui::margins(&footer, 12);
    let images = gtk::Button::with_label("Load signature pictures");
    footer.append(&images);
    let save = gtk::Button::with_label("Save to Gmail");
    save.add_css_class("suggested-action");
    save.set_sensitive(signature.can_update);
    footer.append(&save);
    let close = gtk::Button::with_label("Close");
    footer.append(&close);
    root.append(&footer);
    window.set_content(Some(&root));
    let e = Rc::new(Editor {
        state: s.clone(),
        account,
        window,
        rich,
        baseline: RefCell::new(signature),
        status,
        save,
        busy: Cell::new(false),
        allow_close: Cell::new(false),
        checking_close: Cell::new(false),
        on_saved,
        progress,
        cancel_upload,
        uploading: Cell::new(false),
        cancelled: Arc::new(std::sync::atomic::AtomicBool::new(false)),
    });
    let family = gtk::FontDialogButton::new(Some(gtk::FontDialog::new()));
    family.set_level(gtk::FontLevel::Family);
    family.set_use_font(false);
    family.set_use_size(false);
    family.set_tooltip_text(Some("Any installed system font"));
    flow.insert(&family, -1);
    let weak = Rc::downgrade(&e);
    family.connect_font_desc_notify(move |family| {
        if let Some(e) = weak.upgrade()
            && let Some(name) = family.font_desc().and_then(|f| f.family())
        {
            apply(&e, "fontName", &name);
        }
    });
    let sizes = gtk::DropDown::from_strings(&[
        "8 pt", "9 pt", "10 pt", "11 pt", "12 pt", "14 pt", "16 pt", "18 pt", "20 pt", "24 pt",
        "32 pt", "48 pt", "72 pt",
    ]);
    sizes.set_selected(3);
    sizes.set_tooltip_text(Some("Font size"));
    flow.insert(&sizes, -1);
    let weak = Rc::downgrade(&e);
    sizes.connect_selected_notify(move |input| {
        if let Some(e) = weak.upgrade() {
            let values = [8, 9, 10, 11, 12, 14, 16, 18, 20, 24, 32, 48, 72];
            if let Some(size) = values.get(input.selected() as usize) {
                apply(&e, "fontSizePt", &size.to_string());
            }
        }
    });
    for (icon, title, command) in [
        ("luma-undo-symbolic", "Undo", "undo"),
        ("luma-redo-symbolic", "Redo", "redo"),
        ("luma-bold-symbolic", "Bold", "bold"),
        ("luma-italic-symbolic", "Italic", "italic"),
        ("luma-underline-symbolic", "Underline", "underline"),
        (
            "luma-strikethrough-symbolic",
            "Strikethrough",
            "strikeThrough",
        ),
        ("luma-list-symbolic", "Bullet list", "insertUnorderedList"),
        (
            "luma-ordered-symbolic",
            "Numbered list",
            "insertOrderedList",
        ),
        ("luma-back-symbolic", "Decrease indent", "outdent"),
        ("luma-forward-symbolic", "Increase indent", "indent"),
        ("luma-clear-symbolic", "Clear formatting", "removeFormat"),
    ] {
        action(&flow, &e, icon, title, command);
    }
    choice(
        &flow,
        &e,
        "Alignment",
        &["Left", "Centre", "Right", "Justify"],
        &[
            "justifyLeft",
            "justifyCenter",
            "justifyRight",
            "justifyFull",
        ],
    );
    let spacing =
        gtk::DropDown::from_strings(&["1.0 lines", "1.15 lines", "1.5 lines", "2.0 lines"]);
    spacing.set_tooltip_text(Some("Line spacing"));
    flow.insert(&spacing, -1);
    let weak = Rc::downgrade(&e);
    spacing.connect_selected_notify(move |input| {
        if let Some(e) = weak.upgrade() {
            apply(
                &e,
                "lineHeight",
                ["1", "1.15", "1.5", "2"]
                    .get(input.selected() as usize)
                    .copied()
                    .unwrap_or("1"),
            );
        }
    });
    for (title, command, initial) in [
        ("Text colour", "foreColor", s.window.color()),
        (
            "Highlight colour",
            "hiliteColor",
            gtk::gdk::RGBA::new(1., 1., 0., 1.),
        ),
    ] {
        let colour = gtk::ColorDialogButton::new(Some(gtk::ColorDialog::new()));
        colour.set_rgba(&initial);
        colour.set_tooltip_text(Some(title));
        flow.insert(&colour, -1);
        let weak = Rc::downgrade(&e);
        colour.connect_rgba_notify(move |colour| {
            if let Some(e) = weak.upgrade() {
                apply(&e, command, &colour.rgba().to_string());
            }
        });
    }
    for (title, command) in [("Superscript", "superscript"), ("Subscript", "subscript")] {
        let b = gtk::Button::with_label(title);
        flow.insert(&b, -1);
        let weak = Rc::downgrade(&e);
        b.connect_clicked(move |_| {
            if let Some(e) = weak.upgrade() {
                apply(&e, command, "");
            }
        });
    }
    let link = ui::button("luma-link-symbolic", "Insert hyperlink");
    flow.insert(&link, -1);
    let weak = Rc::downgrade(&e);
    link.connect_clicked(move |_| {
        if let Some(e) = weak.upgrade() {
            link_dialog(&e);
        }
    });
    let picture = ui::button(
        "luma-image-symbolic",
        "Insert picture from a public HTTPS image URL",
    );
    flow.insert(&picture, -1);
    let weak = Rc::downgrade(&e);
    picture.connect_clicked(move |_| {
        if let Some(e) = weak.upgrade() {
            picture_dialog(&e);
        }
    });
    let table = gtk::Button::with_label("Table");
    flow.insert(&table, -1);
    let weak = Rc::downgrade(&e);
    table.connect_clicked(move |_| {
        if let Some(e) = weak.upgrade() {
            table_dialog(&e);
        }
    });
    let picture_size = gtk::SpinButton::with_range(16., 600., 8.);
    picture_size.set_value(160.);
    picture_size.set_tooltip_text(Some("Select a picture, then change its width in pixels"));
    flow.insert(&picture_size, -1);
    let weak = Rc::downgrade(&e);
    picture_size.connect_value_changed(move |v| {
        if let Some(e) = weak.upgrade() {
            apply(&e, "imageWidth", &v.value_as_int().to_string());
        }
    });
    action(
        &flow,
        &e,
        "luma-trash-symbolic",
        "Remove the selected picture",
        "removeImage",
    );
    flow.set_sensitive(e.baseline.borrow().can_update);
    let weak = Rc::downgrade(&e);
    e.cancel_upload.connect_clicked(move |_| {
        if let Some(e) = weak.upgrade() {
            e.cancelled
                .store(true, std::sync::atomic::Ordering::Relaxed);
            e.status.set_text("Cancelling at the next upload boundary…");
        }
    });
    let weak = Rc::downgrade(&e);
    e.save.connect_clicked(move |_| {
        if let Some(e) = weak.upgrade() {
            self::save(&e);
        }
    });
    let weak = Rc::downgrade(&e);
    close.connect_clicked(move |_| {
        if let Some(e) = weak.upgrade() {
            e.window.close();
        }
    });
    let weak = Rc::downgrade(&e);
    images.connect_clicked(move |_| {
        if let Some(e) = weak.upgrade() {
            load_images(&e);
        }
    });
    let weak = Rc::downgrade(&e);
    e.window.connect_close_request(move |_|{
        let Some(e)=weak.upgrade() else{return glib::Propagation::Proceed;};
        if e.allow_close.get(){return glib::Propagation::Proceed;}
        if e.busy.get(){e.status.set_text("Wait for the current signature request before closing");return glib::Propagation::Stop;}
        if e.checking_close.replace(true){return glib::Propagation::Stop;}
        let weak=Rc::downgrade(&e);e.rich.dirty(move|dirty|{let Some(e)=weak.upgrade()else{return;};if !dirty{e.allow_close.set(true);e.window.destroy();return;}
            let dialog=adw::AlertDialog::builder().heading("Close without saving the signature?").body("Your changes are only in this editor. The saved Gmail signature will stay unchanged.").build();dialog.add_responses(&[("cancel","Keep editing"),("close","Close without saving")]);dialog.set_default_response(Some("cancel"));let e=e.clone();glib::MainContext::default().spawn_local(async move{let response=dialog.choose_future(Some(&e.window)).await;e.checking_close.set(false);if response=="close"{e.allow_close.set(true);e.window.destroy();}});
        });
        glib::Propagation::Stop
    });
    let owner = RefCell::new(Some(e.clone()));
    // Dispose/destroy runs after final unref: the editor also owns this window.
    // Break that keep-alive cycle when its native surface is unrealized.
    e.window.connect_unrealize(move |_| {
        owner.borrow_mut().take();
        crate::resources::trim();
    });
    e.window.present();
    Ok(e)
}
fn link_dialog(e: &Rc<Editor>) {
    let input = gtk::Entry::builder()
        .placeholder_text("https://… or mailto:…")
        .build();
    let d = adw::AlertDialog::builder()
        .heading("Link selected text")
        .extra_child(&input)
        .build();
    d.add_responses(&[("cancel", "Cancel"), ("add", "Insert link")]);
    let e = e.clone();
    glib::MainContext::default().spawn_local(async move {
        if d.choose_future(Some(&e.window)).await == "add" {
            match url::Url::parse(input.text().trim()) {
                Ok(url) if matches!(url.scheme(), "http" | "https" | "mailto") => {
                    apply(&e, "createLink", url.as_str())
                }
                _ => e.status.set_text("Use an HTTP, HTTPS or mailto link"),
            }
        }
    });
}
fn picture_dialog(e: &Rc<Editor>) {
    let input = gtk::Entry::builder()
        .placeholder_text("https://example.com/logo.png")
        .build();
    let d=adw::AlertDialog::builder().heading("Insert signature picture").body("Use a public HTTPS image URL. Luma loads an anonymous, bounded preview in memory; Gmail stores the URL with your signature.").extra_child(&input).build();
    d.add_responses(&[
        ("cancel", "Cancel"),
        ("file", "Choose a local image…"),
        ("add", "Insert URL"),
    ]);
    let e = e.clone();
    glib::MainContext::default().spawn_local(async move {
        let response = d.choose_future(Some(&e.window)).await;
        if response == "file" {
            upload_picture(&e);
        } else if response == "add" {
            match luma_core::signatures::image_url(&input.text()) {
                Ok(url) => insert_picture(&e, url),
                Err(error) => e.status.set_text(&error.to_string()),
            }
        }
    });
}
fn table_dialog(e: &Rc<Editor>) {
    let fields = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    let rows = gtk::SpinButton::with_range(1., 8., 1.);
    rows.set_value(2.);
    let cols = gtk::SpinButton::with_range(1., 8., 1.);
    cols.set_value(2.);
    fields.append(&ui::label("Rows", "caption"));
    fields.append(&rows);
    fields.append(&ui::label("Columns", "caption"));
    fields.append(&cols);
    let d = adw::AlertDialog::builder()
        .heading("Insert table")
        .extra_child(&fields)
        .build();
    d.add_responses(&[("cancel", "Cancel"), ("add", "Insert")]);
    let e = e.clone();
    glib::MainContext::default().spawn_local(async move {
        if d.choose_future(Some(&e.window)).await == "add" {
            apply(
                &e,
                "insertTable",
                &format!("{}x{}", rows.value_as_int(), cols.value_as_int()),
            );
        }
    });
}
fn upload_picture(e: &Rc<Editor>) {
    if e.busy.get() {
        return;
    }
    let e = e.clone();
    glib::MainContext::default().spawn_local(async move {
        let filter = gtk::FileFilter::new();
        filter.set_name(Some("Pictures"));
        for mime in ["image/png", "image/jpeg", "image/webp", "image/gif"] {
            filter.add_mime_type(mime);
        }
        let filters = gtk::gio::ListStore::new::<gtk::FileFilter>();
        filters.append(&filter);
        let dialog = gtk::FileDialog::builder()
            .title("Choose a signature picture")
            .filters(&filters)
            .build();
        if let Ok(file) = dialog.open_future(Some(&e.window)).await {
            if let Some(path) = file.path() {
                upload_path(&e, path);
            } else {
                e.status.set_text("Choose a local picture file");
            }
        }
    });
}
fn upload_path(e: &Rc<Editor>, path: std::path::PathBuf) {
    let connected = e
        .state
        .settings
        .borrow()
        .accounts
        .iter()
        .any(|a| a.id == e.account.id && a.drive_enabled);
    if !connected {
        let dialog=adw::AlertDialog::builder().heading("Connect Drive for signature pictures?").body("Luma needs its optional Drive file permission to upload an optimized picture copy. You will review public link visibility before anything is uploaded.").build();
        dialog.add_responses(&[("cancel", "Cancel"), ("connect", "Continue to Google")]);
        let e = e.clone();
        glib::MainContext::default().spawn_local(async move {
            if dialog.choose_future(Some(&e.window)).await == "connect" {
                let weak = Rc::downgrade(&e);
                crate::signin::drive_files(&e.state, &e.account, move |_, _| {
                    if let Some(e) = weak.upgrade() {
                        upload_path(&e, path);
                    }
                });
            }
        });
        return;
    }
    let name = path.file_name().unwrap_or_default().to_string_lossy();
    let dialog=adw::AlertDialog::builder().heading("Upload a public signature picture?").body(format!("An optimized PNG copy of {name} will be stored in your Google Drive. Anyone with its link can view the copy, including people outside Gmail. Your original file is kept. Removing the picture from a signature does not delete the Drive copy.")).build();
    dialog.add_responses(&[("cancel", "Cancel"), ("upload", "Upload public copy")]);
    let e = e.clone();
    glib::MainContext::default().spawn_local(async move {
        if dialog.choose_future(Some(&e.window)).await == "upload" {
            upload_approved(&e, path);
        }
    });
}
fn upload_approved(e: &Rc<Editor>, path: std::path::PathBuf) {
    if e.busy.replace(true) {
        return;
    }
    let Some(client) = e.state.clients.borrow().get(&e.account.id).cloned() else {
        e.busy.set(false);
        e.status
            .set_text("Reconnect the sending account before uploading");
        return;
    };
    e.uploading.set(true);
    e.cancelled
        .store(false, std::sync::atomic::Ordering::Relaxed);
    e.progress.set_fraction(0.);
    e.progress.set_visible(true);
    e.cancel_upload.set_visible(true);
    e.save.set_sensitive(false);
    e.rich.widget.set_sensitive(false);
    e.status.set_text("Preparing the signature picture…");
    let (progress, received) = tokio::sync::watch::channel((0u64, 0u64));
    let weak = Rc::downgrade(e);
    glib::timeout_add_local(std::time::Duration::from_millis(100), move || {
        let Some(e) = weak.upgrade().filter(|e| e.uploading.get()) else {
            return glib::ControlFlow::Break;
        };
        let (done, total) = *received.borrow();
        if total > 0 {
            e.progress.set_fraction(done as f64 / total as f64);
            e.progress.set_text(Some(&if done == total {
                "Finalising the Drive picture…".into()
            } else {
                format!("Uploading picture · {}%", done * 100 / total)
            }));
        } else {
            e.progress.pulse();
        }
        glib::ControlFlow::Continue
    });
    let cancel = e.cancelled.clone();
    let weak = Rc::downgrade(e);
    ui::async_work(
        &e.state,
        async move {
            Ok(async {
                use tokio::io::AsyncReadExt;
                let _permit = crate::attachments::PREVIEWS
                    .acquire()
                    .await
                    .expect("preview semaphore");
                let file = tokio::fs::File::open(path).await?;
                let mut bytes = Vec::new();
                file.take(luma_core::preview::MAX_PREVIEW_BYTES as u64 + 1)
                    .read_to_end(&mut bytes)
                    .await?;
                let (png, preview) = tokio::task::spawn_blocking(move || {
                    let png = luma_core::signature_images::prepare(&bytes)?;
                    let preview = luma_core::html_mail::image_data(&png)?;
                    Ok::<_, Error>((png, preview))
                })
                .await
                .map_err(|_| Error::Invalid("Could not prepare the picture".into()))??;
                let hosted =
                    luma_core::signature_images::upload_public(&client, png, progress, cancel)
                        .await?;
                Ok::<_, Error>((hosted, preview))
            }
            .await)
        },
        move |_, result| {
            let Some(e) = weak.upgrade() else {
                return;
            };
            e.busy.set(false);
            e.uploading.set(false);
            e.progress.set_visible(false);
            e.cancel_upload.set_visible(false);
            e.save.set_sensitive(e.baseline.borrow().can_update);
            e.rich.widget.set_sensitive(true);
            match result {
                Ok((hosted, data)) => {
                    apply(
                        &e,
                        "insertImage",
                        &serde_json::json!({"url":hosted.url,"data":data,"width":160}).to_string(),
                    );
                    e.status.set_text(
                        "Picture uploaded and inserted · Save the signature to keep it in Gmail",
                    );
                }
                Err(error) => e.status.set_text(&error.to_string()),
            }
        },
    );
}
fn insert_picture(e: &Rc<Editor>, url: String) {
    if e.busy.replace(true) {
        return;
    }
    e.save.set_sensitive(false);
    e.status.set_text("Loading picture…");
    let weak = Rc::downgrade(e);
    let source = url.clone();
    ui::async_work(
        &e.state,
        async move {
            Ok(async {
                let _permit = crate::attachments::PREVIEWS
                    .acquire()
                    .await
                    .expect("preview semaphore");
                let bytes = luma_core::mail_images::fetch_external(&source).await?;
                tokio::task::spawn_blocking(move || luma_core::html_mail::image_data(&bytes))
                    .await
                    .map_err(|_| Error::Invalid("Picture decoding failed".into()))?
            }
            .await)
        },
        move |_, data| {
            let Some(e) = weak.upgrade() else {
                return;
            };
            e.busy.set(false);
            e.save.set_sensitive(e.baseline.borrow().can_update);
            let data = match data {
                Ok(data) => data,
                Err(error) => {
                    e.status.set_text(&error.to_string());
                    return;
                }
            };
            let value = serde_json::json!({"url":url,"data":data,"width":160}).to_string();
            apply(&e, "insertImage", &value);
        },
    );
}
fn load_images(e: &Rc<Editor>) {
    let weak = Rc::downgrade(e);
    e.rich.images(move |urls| {
        let Some(e) = weak.upgrade() else {
            return;
        };
        if urls.is_empty() {
            e.status.set_text("No hosted pictures in this signature");
            return;
        }
        e.status.set_text("Loading signature pictures…");
        let weak = Rc::downgrade(&e);
        ui::async_work(
            &e.state,
            async move {
                let mut out = Vec::new();
                let mut budget = 0;
                for url in urls.into_iter().take(8) {
                    let _permit = crate::attachments::PREVIEWS
                        .acquire()
                        .await
                        .expect("preview semaphore");
                    if budget >= 12 * 1024 * 1024 {
                        break;
                    }
                    if let Ok(bytes) = luma_core::mail_images::fetch_external(&url).await
                        && let Ok(Ok(data)) = tokio::task::spawn_blocking(move || {
                            luma_core::html_mail::image_data(&bytes)
                        })
                        .await
                        && budget + data.len() <= 12 * 1024 * 1024
                    {
                        budget += data.len();
                        out.push((url, data));
                    }
                }
                Ok(out)
            },
            move |_, pictures| {
                if let Some(e) = weak.upgrade() {
                    for (url, data) in &pictures {
                        e.rich.image(url, data);
                    }
                    e.status.set_text(&format!(
                        "{} pictures loaded anonymously in memory",
                        pictures.len()
                    ));
                }
            },
        );
    });
}
fn save(e: &Rc<Editor>) {
    if e.busy.get() || !e.baseline.borrow().can_update {
        return;
    }
    let permitted = e
        .state
        .settings
        .borrow()
        .accounts
        .iter()
        .any(|a| a.id == e.account.id && a.sender_controls);
    if !permitted {
        let dialog=adw::AlertDialog::builder().heading("Allow Gmail signature updates?").body("Google will ask for permission to manage basic Gmail settings. Saving here updates only this sender's signature; no mail is sent.").build();
        dialog.add_responses(&[("cancel", "Cancel"), ("allow", "Continue to Google")]);
        let e = e.clone();
        glib::MainContext::default().spawn_local(async move {
            if dialog.choose_future(Some(&e.window)).await == "allow" {
                let weak = Rc::downgrade(&e);
                crate::signin::sender_controls(&e.state, &e.account, move |_, _| {
                    if let Some(e) = weak.upgrade() {
                        save(&e);
                    }
                });
            }
        });
        return;
    }
    e.busy.set(true);
    e.save.set_sensitive(false);
    e.rich.widget.set_sensitive(false);
    let weak = Rc::downgrade(e);
    e.rich.snapshot(move|result| {
        let Some(e)=weak.upgrade() else { return; };
        let html=match result {
            Ok(html)=>html,
            Err(error)=>{ e.busy.set(false); e.save.set_sensitive(true);
                e.rich.widget.set_sensitive(true); e.status.set_text(&error.to_string()); return; }
        };
        let Some(provider)=store(&e.state,&e.account.id) else {
            e.busy.set(false); e.save.set_sensitive(true);
                e.rich.widget.set_sensitive(true); e.status.set_text("Reconnect the account before saving"); return;
        };
        let expected=e.baseline.borrow().clone();
        e.status.set_text("Saving signature to Gmail…");
        let weak=Rc::downgrade(&e);
        ui::async_work(&e.state,async move { Ok(provider.save_signature(&expected,&html).await) },move|_,result| {
            let Some(e)=weak.upgrade() else { return; };
            e.busy.set(false);
            e.save.set_sensitive(true);
                e.rich.widget.set_sensitive(true);
            match result {
                Ok(signature)=> {
                    let display=e.rich.replace(&signature.html);
                    *e.baseline.borrow_mut()=signature;
                    e.rich.mark_saved();
                    match display {
                        Ok(())=>e.status.set_text("Signature saved in Gmail"),
                        Err(error)=>e.status.set_text(&format!("Saved in Gmail, but the editor could not reload it: {error}")),
                    }
                    if let Some(account)=e.state.settings.borrow_mut().accounts.iter_mut().find(|a|a.id==e.account.id) {
                        account.signature.clear();
                    }
                    e.state.save();
                    if let Some(saved)=e.on_saved.as_ref(){saved();}
                },
                Err(Error::DraftConflict)=>e.status.set_text("The Gmail signature changed elsewhere. Your edits remain here; close and reload the signature before saving."),
                Err(error)=>e.status.set_text(&error.to_string()),
            }
        });
    });
}

#[cfg(all(debug_assertions, any(target_os = "linux", target_os = "windows")))]
#[path = "signatures_diagnostic.rs"]
mod diagnostic;
#[cfg(all(debug_assertions, any(target_os = "linux", target_os = "windows")))]
pub fn diagnostic(s: &Rc<State>) {
    diagnostic::start(s);
}

#[cfg(all(debug_assertions, not(target_os = "linux")))]
pub fn close_diagnostic(s: &Rc<State>) {
    fn find(widget: &gtk::Widget, title: &str) -> Option<gtk::Button> {
        if let Some(b) = widget.downcast_ref::<gtk::Button>()
            && b.label().as_deref() == Some(title)
        {
            return Some(b.clone());
        }
        let mut child = widget.first_child();
        while let Some(w) = child {
            if let Some(b) = find(&w, title) {
                return Some(b);
            }
            child = w.next_sibling();
        }
        None
    }
    let state = s.clone();
    glib::MainContext::default().spawn_local(async move{
        let account=state.settings.borrow().accounts[0].clone();
        let e=loaded(&state,account.clone(),Signature{identity:account.email,html:"Synthetic signature".into(),can_update:true},None).unwrap();
        glib::timeout_future(std::time::Duration::from_millis(350)).await;
        apply(&e,"bold","");
        e.window.close();
        glib::timeout_future(std::time::Duration::from_millis(350)).await;
        let d=e.window.visible_dialog().unwrap();
        find(d.upcast_ref(),"Keep editing").unwrap().emit_clicked();drop(d);
        glib::timeout_future(std::time::Duration::from_millis(350)).await;
        assert!(!e.allow_close.get()&&!e.checking_close.get());
        e.window.close();
        glib::timeout_future(std::time::Duration::from_millis(350)).await;
        let d=e.window.visible_dialog().unwrap();
        find(d.upcast_ref(),"Close without saving").unwrap().emit_clicked();drop(d);
        let weak=Rc::downgrade(&e);let view=e.rich.widget.downgrade();drop(e);
        glib::timeout_future(std::time::Duration::from_millis(700)).await;
        assert!(weak.upgrade().is_none(),"Signature owner must be released");
        assert!(view.upgrade().is_none(),"Signature surface must be released");
        println!("PASS native Windows signature source, real Keep editing/discard buttons, window close and complete owner/widget release; no account writes");
        state.app.quit();
    });
}
