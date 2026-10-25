use crate::ui::{self, State, label, vbox};
use adw::prelude::*;
use gtk::glib;
use luma_core::{
    Error, Result,
    composer::{Compose, RetainedAttachment, readable},
    gmail::Draft,
    model::Message,
};
use std::{
    cell::{Cell, RefCell},
    path::PathBuf,
    rc::Rc,
    time::Duration,
};

pub(crate) struct Editor {
    state: Rc<State>,
    window: adw::Window,
    root: gtk::Box,
    header: adw::HeaderBar,
    placement: Cell<u8>,
    close_after_save: Cell<bool>,
    from: gtk::DropDown,
    accounts: Vec<luma_core::config::Account>,
    to: gtk::Entry,
    cc: gtk::Entry,
    bcc: gtk::Entry,
    subject: gtk::Entry,
    text: gtk::TextView,
    html: gtk::CheckButton,
    files: gtk::Box,
    form: gtk::Box,
    attachments: RefCell<Vec<PathBuf>>,
    retained: RefCell<Vec<RetainedAttachment>>,
    template: Compose,
    draft: RefCell<Option<Draft>>,
    revision: Cell<u64>,
    saved: Cell<u64>,
    busy: Cell<bool>,
    uncertain: Cell<bool>,
    status: gtk::Label,
    progress: gtk::ProgressBar,
    send_reveal: gtk::Revealer,
    send: gtk::Button,
    save: gtk::Button,
    debounce: RefCell<Option<glib::SourceId>>,
    closing: Cell<bool>,
}
impl Editor {
    fn parent(&self) -> gtk::Window {
        if self.placement.get() == 2 {
            self.window.clone().upcast()
        } else {
            self.state.window.clone().upcast()
        }
    }
}
fn detach(e: &Editor) {
    if e.placement.get() == 2 {
        e.window.set_content(None::<&gtk::Widget>);
    } else if let Some(parent) = e.root.parent() {
        if let Some(stack) = parent.downcast_ref::<gtk::Stack>() {
            stack.remove(&e.root);
        } else if let Some(box_) = parent.downcast_ref::<gtk::Box>() {
            box_.remove(&e.root);
        }
    }
}
fn show_full(e: &Rc<Editor>) {
    if let Some(current) = e.state.editor.borrow().as_ref()
        && !Rc::ptr_eq(current, e)
    {
        e.status
            .set_text("Another draft is open in the main window. Finish or pop it out first.");
        return;
    }
    let detached = e.placement.get() == 2;
    detach(e);
    e.placement.set(0);
    e.header.set_show_end_title_buttons(false);
    e.root.remove_css_class("inline-composer");
    e.state.compose_stack.add_named(&e.root, Some("compose"));
    e.state.compose_stack.set_visible_child_name("compose");
    *e.state.editor.borrow_mut() = Some(e.clone());
    if detached {
        e.window.set_visible(false);
    }
    e.state.window.present();
    e.text.grab_focus();
}
fn pop_out(e: &Rc<Editor>) {
    if e.placement.get() == 2 {
        e.window.present();
        return;
    }
    detach(e);
    e.placement.set(2);
    e.header.set_show_end_title_buttons(true);
    e.root.remove_css_class("inline-composer");
    e.window.set_content(Some(&e.root));
    e.state.editor.borrow_mut().take();
    e.state.compose_stack.set_visible_child_name("mail");
    e.window.present();
    e.text.grab_focus();
}
pub fn park_inline(s: &Rc<State>) {
    let editor = s.editor.borrow().clone();
    if let Some(e) = editor
        && e.placement.get() == 1
    {
        detach(&e);
        e.placement.set(0);
        e.root.remove_css_class("inline-composer");
        s.compose_stack.add_named(&e.root, Some("compose"));
        s.compose_stack.set_visible_child_name("mail");
    }
}
fn finish(e: &Rc<Editor>) {
    e.closing.set(true);
    detach(e);
    let owns = e
        .state
        .editor
        .borrow()
        .as_ref()
        .is_some_and(|current| Rc::ptr_eq(current, e));
    if owns {
        e.state.editor.borrow_mut().take();
        e.state.compose_stack.set_visible_child_name("mail");
    }
    e.window.destroy();
}
fn request_close(e: &Rc<Editor>) -> glib::Propagation {
    if e.closing.get() {
        return glib::Propagation::Proceed;
    }
    #[cfg(debug_assertions)]
    if std::env::args().any(|arg| arg == "--design-preview") {
        return glib::Propagation::Proceed;
    }
    if e.busy.get() {
        e.status.set_text("Finishing the current Gmail request…");
        e.close_after_save.set(true);
        return glib::Propagation::Stop;
    }
    if e.uncertain.get() {
        let dialog=adw::AlertDialog::builder().heading("Close after an uncertain request?").body("Check Gmail Sent and Drafts before sending again. Copy any unsaved text you need before closing.").build();
        dialog.add_responses(&[("cancel", "Keep editing"), ("close", "Close")]);
        let editor = e.clone();
        glib::MainContext::default().spawn_local(async move {
            if dialog.choose_future(Some(&editor.parent())).await == "close" {
                finish(&editor);
            }
        });
        return glib::Propagation::Stop;
    }
    if e.revision.get() != e.saved.get() {
        e.close_after_save.set(true);
        persist(e, false);
        return glib::Propagation::Stop;
    }
    glib::Propagation::Proceed
}

pub fn mailto(s: &Rc<State>, uri: &str) {
    let Ok(url) = url::Url::parse(uri) else {
        return;
    };
    if url.scheme() != "mailto" {
        return;
    }
    let mut c = Compose::new(String::new());
    c.to = url.path().into();
    for (k, v) in url.query_pairs() {
        match k.as_ref() {
            "subject" => c.subject = v.into(),
            "body" => c.text = v.into(),
            "cc" => c.cc = v.into(),
            "bcc" => c.bcc = v.into(),
            _ => (),
        }
    }
    make(s, c, None);
}
pub fn open(
    s: &Rc<State>,
    reply: Option<(Message, u8)>,
    prepared: Option<(String, Draft, Vec<RetainedAttachment>)>,
) {
    if let Some((message, 2)) = &reply
        && !message.payload.attachments().is_empty()
    {
        let Some(account) = s.selected.borrow().as_ref().map(|k| k.account.clone()) else {
            return;
        };
        let Some(c) = s.clients.borrow().get(&account).cloned() else {
            return;
        };
        let m = message.clone();
        s.info("Preparing the selected message's attachments for forwarding…");
        ui::async_work(
            s,
            async move {
                let mut parts = c.message_attachments(&m).await?;
                for p in &mut parts {
                    p.content_id = None;
                }
                Ok((account, m, parts))
            },
            |s, (account, message, parts)| {
                open_internal(&s, Some((message, 2)), None, Some((account, parts)), None)
            },
        );
        return;
    }
    let slot = if reply.as_ref().is_some_and(|(_, mode)| *mode < 2) {
        s.reply_slot.borrow().upgrade()
    } else {
        None
    };
    open_internal(s, reply, prepared, None, slot);
}
fn open_internal(
    s: &Rc<State>,
    reply: Option<(Message, u8)>,
    prepared: Option<(String, Draft, Vec<RetainedAttachment>)>,
    forward: Option<(String, Vec<RetainedAttachment>)>,
    slot: Option<gtk::Box>,
) {
    let (forced, draft, retained) = match prepared {
        Some((a, d, r)) => (Some(a), Some(d), r),
        None => match forward {
            Some((a, r)) => (Some(a), None, r),
            None => (None, None, vec![]),
        },
    };
    let account = s.settings.borrow().default_account.clone().or_else(|| {
        s.settings
            .borrow()
            .accounts
            .iter()
            .find(|a| a.enabled)
            .map(|a| a.id.clone())
    });
    let account = forced
        .or_else(|| s.selected.borrow().as_ref().map(|k| k.account.clone()))
        .or(account);
    let a = s
        .settings
        .borrow()
        .accounts
        .iter()
        .find(|a| Some(&a.id) == account.as_ref() && a.enabled)
        .cloned();
    let Some(a) = a else {
        s.info("Connect an account before composing a message.");
        return;
    };
    let mut c = Compose::new(a.email.clone());
    if let Some(d) = &draft {
        let p = &d.message.payload;
        c.to = p.header("To").into();
        c.cc = p.header("Cc").into();
        c.bcc = p.header("Bcc").into();
        c.subject = p.header("Subject").into();
        c.text = readable(p).unwrap_or_default();
        c.thread = Some(d.message.thread_id.clone());
        c.reply_to = p.header("In-Reply-To").into();
        c.references = p.header("References").into();
        let mut html = None;
        p.walk(&mut |p| {
            if p.mime_type == "text/html" && p.filename.is_empty() {
                html = Some(p)
            }
        });
        if let Some(p) = html
            && let Ok(bytes) = luma_core::composer::decode_part(p)
        {
            c.text = String::from_utf8_lossy(&bytes).into();
            c.html = true;
        }
    } else if let Some((m, mode)) = reply {
        if mode < 2 {
            c = Compose::reply(a.email, &m, mode == 1);
            c.text = format!(
                "{}\n\nOn {}, {} wrote:\n{}",
                a.signature,
                m.payload.header("Date"),
                m.payload.header("From"),
                readable(&m.payload)
                    .unwrap_or_default()
                    .lines()
                    .map(|l| format!("> {l}"))
                    .collect::<Vec<_>>()
                    .join("\n")
            );
        } else {
            c.subject = format!("Fwd: {}", m.payload.header("Subject"));
            c.text = format!(
                "{}\n\n---------- Forwarded message ----------\nFrom: {}\nSubject: {}\n\n{}",
                a.signature,
                m.payload.header("From"),
                m.payload.header("Subject"),
                readable(&m.payload).unwrap_or_default()
            );
        }
    } else {
        c.text = a.signature;
    }
    c.retained = retained;
    make_at(s, c, draft, slot);
}
#[cfg(debug_assertions)]
pub(crate) fn design_preview(s: &Rc<State>, paths: Vec<PathBuf>) {
    let mut message = Compose::new("alex@example.com".into());
    message.to = "jamie@example.com".into();
    message.subject = "Re: Weekend by the coast".into();
    // Configure local cards after creation without scheduling a real save.
    message.attachments = paths;
    make(s, message, None);
}
#[cfg(any(debug_assertions, test))]
pub(crate) fn verify_formatting() {
    let buffer = gtk::TextBuffer::new(None::<&gtk::TextTagTable>);
    buffer.set_text("Hello <world> & friends");
    let tag = gtk::TextTag::new(Some("strong"));
    buffer.tag_table().add(&tag);
    buffer.apply_tag(&tag, &buffer.start_iter(), &buffer.iter_at_offset(5));
    let html = formatted_html(&buffer);
    assert!(html.contains("<strong>Hello</strong> &lt;world&gt; &amp; friends"));
    assert!(has_formatting(&buffer));
}
fn make(s: &Rc<State>, c: Compose, draft: Option<Draft>) {
    make_at(s, c, draft, None);
}
fn make_at(s: &Rc<State>, c: Compose, draft: Option<Draft>, slot: Option<gtk::Box>) {
    let existing = s.editor.borrow().clone();
    if let Some(editor) = existing {
        show_full(&editor);
        editor
            .status
            .set_text("Finish or pop out this draft before starting another.");
        return;
    }
    let accounts: Vec<_> = s
        .settings
        .borrow()
        .accounts
        .iter()
        .filter(|a| a.enabled)
        .cloned()
        .collect();
    if accounts.is_empty() {
        s.info("Connect a Google account before composing.");
        return;
    }
    let window = adw::Window::builder()
        .transient_for(&s.window)
        .application(&s.app)
        .title("Compose · Luma Mail")
        .default_width(760)
        .default_height(720)
        .build();
    window.add_css_class("compose-window");
    let root = vbox(0);
    let header = adw::HeaderBar::new();
    header.set_show_end_title_buttons(false);
    header.set_show_start_title_buttons(false);
    let back = ui::button("go-previous-symbolic", "Save draft and return to mail");
    header.pack_start(&back);
    let popout = ui::button("window-new-symbolic", "Pop out into a separate window");
    let expand = ui::button("view-fullscreen-symbolic", "Expand composer");
    header.pack_end(&popout);
    header.pack_end(&expand);
    header.set_title_widget(Some(&adw::WindowTitle::new("New message", "Luma Mail")));
    let save = ui::button("document-save-symbolic", "Save draft to Gmail");
    save.add_css_class("flat");
    header.pack_end(&save);
    root.append(&header);
    let form = vbox(0);
    form.set_vexpand(true);
    let fields = vbox(2);
    fields.add_css_class("compose-fields");
    let names: Vec<_> = accounts.iter().map(|a| a.email.as_str()).collect();
    let from = gtk::DropDown::from_strings(&names);
    from.set_hexpand(true);
    from.set_selected(accounts.iter().position(|a| a.email == c.from).unwrap_or(0) as u32);
    let from_row = gtk::Box::new(gtk::Orientation::Horizontal, 12);
    from_row.append(&label("From", "compose-field-label"));
    from_row.append(&from);
    fields.append(&from_row);
    let entry = |title: &str, hint: &str, value: &str| {
        let row = gtk::Box::new(gtk::Orientation::Horizontal, 12);
        row.append(&label(title, "compose-field-label"));
        let field = gtk::Entry::builder()
            .placeholder_text(hint)
            .text(value)
            .hexpand(true)
            .build();
        row.append(&field);
        fields.append(&row);
        (field, row)
    };
    let (to, to_row) = entry("To", "Recipients, separated by commas", &c.to);
    let (cc, cc_row) = entry("Cc", "Copy recipients", &c.cc);
    let (bcc, bcc_row) = entry("Bcc", "Hidden recipients", &c.bcc);
    cc_row.set_visible(!c.cc.is_empty());
    bcc_row.set_visible(!c.bcc.is_empty());
    for (title, row) in [("Cc", cc_row), ("Bcc", bcc_row)] {
        let toggle = gtk::ToggleButton::with_label(title);
        toggle.add_css_class("flat");
        toggle.set_active(row.is_visible());
        toggle.connect_toggled(move |b| row.set_visible(b.is_active()));
        to_row.append(&toggle);
    }
    fields.append(&gtk::Separator::new(gtk::Orientation::Horizontal));
    let subject = gtk::Entry::builder()
        .placeholder_text("Subject")
        .text(&c.subject)
        .build();
    subject.add_css_class("subject-entry");
    fields.append(&subject);
    let files = vbox(4);
    files.add_css_class("attachment-strip");
    fields.append(
        &gtk::ScrolledWindow::builder()
            .hscrollbar_policy(gtk::PolicyType::Never)
            .max_content_height(180)
            .propagate_natural_height(true)
            .child(&files)
            .build(),
    );
    form.append(&fields);
    let format = gtk::Box::new(gtk::Orientation::Horizontal, 4);
    format.add_css_class("compose-toolbar");
    form.append(&format);
    let text = gtk::TextView::builder()
        .wrap_mode(gtk::WrapMode::WordChar)
        .vexpand(true)
        .accepts_tab(true)
        .build();
    text.buffer().set_enable_undo(true);
    text.buffer().set_text(&c.text);
    for (name, property) in [
        ("strong", "weight"),
        ("em", "style"),
        ("u", "underline"),
        ("s", "strikethrough"),
    ] {
        let tag = gtk::TextTag::new(Some(name));
        match property {
            "weight" => tag.set_weight(700),
            "style" => tag.set_style(gtk::pango::Style::Italic),
            "underline" => tag.set_underline(gtk::pango::Underline::Single),
            _ => tag.set_strikethrough(true),
        }
        text.buffer().tag_table().add(&tag);
    }
    form.append(
        &gtk::ScrolledWindow::builder()
            .vexpand(true)
            .min_content_height(240)
            .child(&text)
            .build(),
    );
    let html = gtk::CheckButton::with_label("Edit HTML source");
    html.set_active(c.html);
    html.set_tooltip_text(Some(
        "Advanced: edit the original HTML instead of formatted text",
    ));
    let advanced = gtk::Expander::builder()
        .label("Advanced")
        .child(&html)
        .expanded(c.html)
        .build();
    advanced.add_css_class("compose-advanced");
    form.append(&advanced);
    let progress = gtk::ProgressBar::new();
    progress.set_visible(false);
    progress.add_css_class("compose-progress");
    form.append(&progress);
    let bar = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    bar.add_css_class("compose-footer");
    let send = gtk::Button::new();
    send.set_child(Some(
        &adw::ButtonContent::builder()
            .icon_name("mail-send-symbolic")
            .label("Send")
            .build(),
    ));
    send.add_css_class("suggested-action");
    send.set_tooltip_text(Some("Send message (Ctrl+Enter)"));
    let attach = ui::button("mail-attachment-symbolic", "Attach files");
    let drive = ui::button("folder-remote-symbolic", "Insert a Google Drive link");
    let discard = ui::button("user-trash-symbolic", "Discard draft");
    for b in [&attach, &drive, &discard] {
        b.add_css_class("flat");
    }
    let status = label(
        if draft.is_some() {
            "Saved in Gmail"
        } else {
            "Not yet saved"
        },
        "caption",
    );
    status.set_hexpand(true);
    status.set_wrap(true);
    bar.append(&send);
    bar.append(&attach);
    bar.append(&drive);
    bar.append(&status);
    bar.append(&discard);
    form.append(&bar);
    let send_reveal = gtk::Revealer::builder()
        .transition_type(gtk::RevealerTransitionType::SlideUp)
        .transition_duration(220)
        .reveal_child(true)
        .vexpand(true)
        .child(&form)
        .build();
    root.append(&send_reveal);

    let is_new = draft.is_none();
    let e = Rc::new(Editor {
        state: s.clone(),
        window,
        root,
        header,
        placement: Cell::new(if slot.is_some() { 1 } else { 0 }),
        close_after_save: Cell::new(false),
        from,
        accounts,
        to,
        cc,
        bcc,
        subject,
        text,
        html,
        files,
        form,
        attachments: RefCell::new(c.attachments.clone()),
        retained: RefCell::new(c.retained.clone()),
        template: c,
        draft: RefCell::new(draft),
        revision: Cell::new(0),
        saved: Cell::new(0),
        busy: Cell::new(false),
        uncertain: Cell::new(false),
        status,
        progress,
        send_reveal,
        send,
        save,
        debounce: RefCell::new(None),
        closing: Cell::new(false),
    });
    {
        let weak = Rc::downgrade(&e);
        back.connect_clicked(move |_| {
            if let Some(e) = weak.upgrade()
                && request_close(&e) == glib::Propagation::Proceed
            {
                finish(&e);
            }
        });
        let weak = Rc::downgrade(&e);
        popout.connect_clicked(move |_| {
            if let Some(e) = weak.upgrade() {
                pop_out(&e);
            }
        });
        let weak = Rc::downgrade(&e);
        expand.connect_clicked(move |_| {
            if let Some(e) = weak.upgrade() {
                show_full(&e);
            }
        });
    }
    for (icon, title, tag) in [
        ("format-text-bold-symbolic", "Bold", "strong"),
        ("format-text-italic-symbolic", "Italic", "em"),
        ("format-text-underline-symbolic", "Underline", "u"),
        ("format-text-strikethrough-symbolic", "Strikethrough", "s"),
    ] {
        let b = ui::button(icon, title);
        b.add_css_class("flat");
        format.append(&b);
        let weak = Rc::downgrade(&e);
        b.connect_clicked(move |_| {
            if let Some(e) = weak.upgrade() {
                format_selection(&e, tag);
            }
        });
    }
    let list = ui::button("view-list-symbolic", "Insert bullet");
    list.add_css_class("flat");
    format.append(&list);
    let weak = Rc::downgrade(&e);
    list.connect_clicked(move |_| {
        if let Some(e) = weak.upgrade() {
            e.text.buffer().insert_at_cursor("• ");
            e.text.grab_focus();
        }
    });
    let link = ui::button("insert-link-symbolic", "Insert link");
    link.add_css_class("flat");
    format.append(&link);
    let weak = Rc::downgrade(&e);
    link.connect_clicked(move |_| {
        if let Some(e) = weak.upgrade() {
            insert_link(&e);
        }
    });
    let keyboard = gtk::EventControllerKey::new();
    let weak = Rc::downgrade(&e);
    keyboard.connect_key_pressed(move |_, key, _, mods| {
        let Some(e) = weak.upgrade() else {
            return glib::Propagation::Proceed;
        };
        if mods.contains(gtk::gdk::ModifierType::CONTROL_MASK) {
            if key == gtk::gdk::Key::Return {
                persist(&e, true);
                return glib::Propagation::Stop;
            }
            let tag = match key {
                gtk::gdk::Key::b => Some("strong"),
                gtk::gdk::Key::i => Some("em"),
                gtk::gdk::Key::u => Some("u"),
                _ => None,
            };
            if let Some(tag) = tag {
                format_selection(&e, tag);
                return glib::Propagation::Stop;
            }
        }
        glib::Propagation::Proceed
    });
    e.window.add_controller(keyboard);
    for field in [&e.to, &e.cc, &e.bcc, &e.subject] {
        let weak = Rc::downgrade(&e);
        field.connect_changed(move |_| {
            if let Some(e) = weak.upgrade() {
                changed(&e);
            }
        });
    }
    {
        let weak = Rc::downgrade(&e);
        e.text.buffer().connect_changed(move |_| {
            if let Some(e) = weak.upgrade() {
                changed(&e);
            }
        });
    }
    {
        let weak = Rc::downgrade(&e);
        e.html.connect_toggled(move |mode| {
            if let Some(e) = weak.upgrade() {
                let buffer = e.text.buffer();
                if mode.is_active() {
                    let source = formatted_html(&buffer);
                    buffer.set_text(&source);
                } else {
                    let source = buffer.text(&buffer.start_iter(), &buffer.end_iter(), false);
                    match luma_core::composer::html_text(&source) {
                        Ok(text) => buffer.set_text(&text),
                        Err(error) => {
                            e.status.set_text(&error.to_string());
                            return;
                        }
                    }
                }
                changed(&e);
            }
        });
    }
    {
        let weak = Rc::downgrade(&e);
        e.from.connect_selected_notify(move |_| {
            if let Some(e) = weak.upgrade() {
                changed(&e);
            }
        });
    }
    {
        let weak = Rc::downgrade(&e);
        e.save.connect_clicked(move |_| {
            if let Some(e) = weak.upgrade() {
                persist(&e, false);
            }
        });
    }
    {
        let weak = Rc::downgrade(&e);
        e.send.connect_clicked(move |_| {
            if let Some(e) = weak.upgrade() {
                persist(&e, true);
            }
        });
    }
    {
        let weak = Rc::downgrade(&e);
        attach.connect_clicked(move |_| {
            if let Some(e) = weak.upgrade() {
                attach_files(&e);
            }
        });
    }
    {
        let weak = Rc::downgrade(&e);
        drive.connect_clicked(move |_| {
            let Some(e) = weak.upgrade() else { return };
            if let Some(a) = e.accounts.get(e.from.selected() as usize) {
                crate::settings::drive_dialog(&e.state, &e.text, &a.id);
            }
        });
    }
    {
        let weak = Rc::downgrade(&e);
        discard.connect_clicked(move |_| {
            if let Some(e) = weak.upgrade() {
                discard_editor(&e);
            }
        });
    }
    {
        let weak = Rc::downgrade(&e);
        e.window.connect_close_request(move |_| {
            let Some(e) = weak.upgrade() else {
                return glib::Propagation::Proceed;
            };
            if e.closing.get() {
                return glib::Propagation::Proceed;
            }
            if request_close(&e) == glib::Propagation::Proceed {
                finish(&e);
            }
            glib::Propagation::Stop
        });
    }
    // File-list drops use the same size checks as the native attachment picker.
    let drop = gtk::DropTarget::new(
        gtk::gdk::FileList::static_type(),
        gtk::gdk::DragAction::COPY,
    );
    {
        let weak = Rc::downgrade(&e);
        drop.connect_drop(move |_, v, _, _| {
            let Some(e) = weak.upgrade() else {
                return false;
            };
            let Ok(files) = v.get::<gtk::gdk::FileList>() else {
                return false;
            };
            for f in files.files() {
                if let Some(p) = f.path() {
                    e.attachments.borrow_mut().push(p);
                }
            }
            render_files(&e);
            changed(&e);
            true
        });
    }
    e.text.add_controller(drop);
    if e.draft.borrow().is_some() {
        e.from.set_sensitive(false);
    }
    {
        let lifetime = RefCell::new(Some(e.clone()));
        e.window.connect_destroy(move |_| {
            lifetime.borrow_mut().take();
        });
    }
    *s.editor.borrow_mut() = Some(e.clone());
    if let Some(slot) = slot {
        e.root.add_css_class("inline-composer");
        slot.append(&e.root);
        e.text.set_height_request(200);
        s.window.present();
        e.text.grab_focus();
    } else {
        show_full(&e);
    }
    render_files(&e);
    if is_new
        && (!e.template.text.is_empty()
            || !e.template.to.is_empty()
            || !e.template.subject.is_empty())
    {
        changed(&e);
    }
}
fn changed(e: &Rc<Editor>) {
    e.revision.set(e.revision.get() + 1);
    if e.uncertain.get() {
        e.status.set_text("Unsaved changes. Check Gmail Sent/Drafts after the interrupted request; automatic saves and sending are locked.");
        return;
    }
    e.status.set_text("Unsaved changes");
    if let Some(id) = e.debounce.borrow_mut().take() {
        id.remove();
    }
    let weak = Rc::downgrade(e);
    let id = glib::timeout_add_local_once(Duration::from_secs(2), move || {
        if let Some(e) = weak.upgrade() {
            e.debounce.borrow_mut().take();
            if !e.busy.get() {
                persist(&e, false);
            }
        }
    });
    *e.debounce.borrow_mut() = Some(id);
}
fn format_selection(e: &Rc<Editor>, tag: &str) {
    if e.html.is_active() {
        e.status
            .set_text("Switch off HTML source to use visual formatting.");
        return;
    }
    let buffer = e.text.buffer();
    if let Some((a, z)) = buffer.selection_bounds() {
        if let Some(format) = buffer.tag_table().lookup(tag) {
            if a.has_tag(&format) {
                buffer.remove_tag(&format, &a, &z);
            } else {
                buffer.apply_tag(&format, &a, &z);
            }
            changed(e);
        }
    } else {
        e.status
            .set_text("Select some text, then apply formatting.");
    }
    e.text.grab_focus();
}
fn escape(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}
fn formatting(iter: &gtk::TextIter) -> Vec<String> {
    let mut names: Vec<_> = iter
        .tags()
        .iter()
        .filter_map(|tag| tag.name().map(|n| n.to_string()))
        .filter(|n| matches!(n.as_str(), "strong" | "em" | "u" | "s") || n.starts_with("link:"))
        .collect();
    names.sort();
    names
}
fn has_formatting(buffer: &gtk::TextBuffer) -> bool {
    let mut iter = buffer.start_iter();
    while !iter.is_end() {
        if !formatting(&iter).is_empty() {
            return true;
        }
        iter.forward_char();
    }
    false
}
fn formatted_html(buffer: &gtk::TextBuffer) -> String {
    let mut output = String::from("<div style=\"white-space:pre-wrap\">");
    let mut iter = buffer.start_iter();
    let mut previous: Vec<String> = Vec::new();
    while !iter.is_end() {
        let tags = formatting(&iter);
        if tags != previous {
            for tag in previous.iter().rev() {
                output.push_str(&format!(
                    "</{}>",
                    if tag.starts_with("link:") { "a" } else { tag }
                ));
            }
            for tag in &tags {
                if let Some(url) = tag.strip_prefix("link:") {
                    output.push_str(&format!("<a href=\"{}\">", escape(url)));
                } else {
                    output.push_str(&format!("<{tag}>"));
                }
            }
            previous = tags;
        }
        let ch = iter.char();
        if ch == '\n' {
            output.push_str("<br>\n");
        } else {
            output.push_str(&escape(&ch.to_string()));
        }
        iter.forward_char();
    }
    for tag in previous.iter().rev() {
        output.push_str(&format!(
            "</{}>",
            if tag.starts_with("link:") { "a" } else { tag }
        ));
    }
    output.push_str("</div>");
    output
}
fn insert_link(e: &Rc<Editor>) {
    if e.html.is_active() {
        e.status
            .set_text("Switch off HTML source to insert a link visually.");
        return;
    }
    let url = gtk::Entry::builder()
        .placeholder_text("https://example.com")
        .build();
    let dialog = adw::AlertDialog::builder()
        .heading("Insert link")
        .extra_child(&url)
        .build();
    dialog.add_responses(&[("cancel", "Cancel"), ("insert", "Insert")]);
    dialog.set_default_response(Some("insert"));
    let e = e.clone();
    glib::MainContext::default().spawn_local(async move {
        if dialog.choose_future(Some(&e.parent())).await != "insert" {
            return;
        }
        let Ok(url) = url::Url::parse(&url.text()) else {
            e.status.set_text("Enter a valid https:// link.");
            return;
        };
        if !matches!(url.scheme(), "https" | "http" | "mailto") {
            e.status
                .set_text("Use an https://, http:// or mailto: link.");
            return;
        }
        let buffer = e.text.buffer();
        let (a, z) = if let Some(bounds) = buffer.selection_bounds() {
            bounds
        } else {
            let start = buffer.cursor_position();
            buffer.insert_at_cursor(url.as_str());
            (
                buffer.iter_at_offset(start),
                buffer.iter_at_offset(buffer.cursor_position()),
            )
        };
        let name = format!("link:{url}");
        let tag = buffer.tag_table().lookup(&name).unwrap_or_else(|| {
            let tag = gtk::TextTag::new(Some(&name));
            tag.set_underline(gtk::pango::Underline::Single);
            buffer.tag_table().add(&tag);
            tag
        });
        let existing: Vec<_> = a
            .tags()
            .into_iter()
            .filter(|t| t.name().is_some_and(|n| n.starts_with("link:")))
            .collect();
        for tag in existing {
            buffer.remove_tag(&tag, &a, &z);
        }
        buffer.apply_tag(&tag, &a, &z);
        changed(&e);
        e.text.grab_focus();
    });
}
fn snapshot(e: &Editor) -> Result<Compose> {
    let a = e
        .accounts
        .get(e.from.selected() as usize)
        .ok_or_else(|| Error::Invalid("Choose a sending account".into()))?;
    let mut c = e.template.clone();
    c.from = a.email.clone();
    c.to = e.to.text().into();
    c.cc = e.cc.text().into();
    c.bcc = e.bcc.text().into();
    c.subject = e.subject.text().into();
    let b = e.text.buffer();
    c.text = b.text(&b.start_iter(), &b.end_iter(), false).into();
    c.html = e.html.is_active();
    if !c.html && has_formatting(&b) {
        c.text = formatted_html(&b);
        c.html = true;
    }
    c.attachments = e.attachments.borrow().clone();
    c.retained = e.retained.borrow().clone();
    Ok(c)
}
fn persist(e: &Rc<Editor>, send: bool) {
    if e.busy.get() || e.uncertain.get() {
        return;
    }
    let c = match snapshot(e) {
        Ok(c) => c,
        Err(err) => {
            e.status.set_text(&err.to_string());
            return;
        }
    };
    if send && let Err(err) = c.validate_send() {
        e.status.set_text(&err.to_string());
        return;
    }
    let Some(a) = e.accounts.get(e.from.selected() as usize) else {
        return;
    };
    let Some(client) = e.state.clients.borrow().get(&a.id).cloned() else {
        return;
    };
    let existing = e.draft.borrow().clone();
    let revision = e.revision.get();
    let editor = e.clone();
    e.busy.set(true);
    if send {
        e.form.set_sensitive(false);
    }
    e.send.set_sensitive(false);
    e.save.set_sensitive(false);
    e.from.set_sensitive(false);
    e.status.set_text(if send {
        "Saving, then sending through Gmail…"
    } else {
        "Saving to Gmail…"
    });
    let progress = std::sync::Arc::new(luma_core::http::UploadProgress::default());
    let phase = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
    e.progress.set_visible(true);
    e.progress.set_fraction(0.);
    let weak = Rc::downgrade(e);
    let transfer = progress.clone();
    let stage = phase.clone();
    glib::timeout_add_local(Duration::from_millis(80), move || {
        let Some(e) = weak.upgrade() else {
            return glib::ControlFlow::Break;
        };
        if !e.busy.get() {
            return glib::ControlFlow::Break;
        }
        let (sent, total) = transfer.snapshot();
        if stage.load(std::sync::atomic::Ordering::Relaxed) {
            e.progress.pulse();
            e.status
                .set_text("Sending · Waiting for Gmail confirmation…");
        } else if total == 0 {
            e.progress.pulse();
            e.status.set_text("Preparing message and attachments…");
        } else if sent < total {
            e.progress.set_fraction(sent as f64 / total as f64);
            e.status.set_text(&format!(
                "Uploading message and attachments · {}%",
                sent * 100 / total
            ));
        } else {
            e.progress.pulse();
            e.status.set_text("Upload transferred · Waiting for Gmail…");
        }
        glib::ControlFlow::Continue
    });
    let mut uploading = (*client).clone();
    uploading.http = client.http.with_upload_progress(progress);
    ui::async_work(
        &e.state,
        async move {
            // Keep errors in the completion value so controls recover on failure.
            let raw = match tokio::task::spawn_blocking(move || {
                let raw = c.mime()?;
                Ok::<_, Error>((raw, c.thread))
            })
            .await
            {
                Ok(v) => v,
                Err(_) => Err(Error::Invalid("MIME preparation failed".into())),
            };
            let (raw, thread) = match raw {
                Ok(value) => value,
                Err(e) => return Ok((Err(e), None, false)),
            };
            let draft = match uploading
                .save_draft(existing.as_ref(), &raw, thread.as_deref())
                .await
            {
                Ok(d) => d,
                Err(e) => {
                    let uncertain = e.write_may_have_succeeded();
                    return Ok((Err(e), None, uncertain));
                }
            };
            phase.store(true, std::sync::atomic::Ordering::Relaxed);
            if send && let Err(e) = client.send_draft(&draft).await {
                let uncertain = e.write_may_have_succeeded();
                return Ok((Err(e), Some(draft), uncertain));
            }
            Ok((Ok(draft), None, false))
        },
        move |_, (result, persisted, uncertain)| {
            editor.busy.set(false);
            editor.progress.set_visible(false);
            editor.form.set_sensitive(true);
            editor.save.set_sensitive(true);
            if let Some(draft) = persisted {
                *editor.draft.borrow_mut() = Some(draft);
                editor.saved.set(revision);
            }
            match result {
                Ok(d) => {
                    *editor.draft.borrow_mut() = Some(d);
                    editor.saved.set(revision);
                    editor.status.set_text(if send {
                        "Sent · confirmed by Gmail"
                    } else {
                        "Saved in Gmail"
                    });
                    if send {
                        editor.closing.set(true);
                        editor.send_reveal.set_reveal_child(false);
                        editor.state.toast("Sent · Confirmed by Gmail");
                        let delay = if gtk::Settings::default()
                            .is_some_and(|s| s.is_gtk_enable_animations())
                        {
                            220
                        } else {
                            0
                        };
                        let sent = editor.clone();
                        glib::timeout_add_local_once(Duration::from_millis(delay), move || {
                            finish(&sent);
                            let selected = sent.state.selected.borrow().clone();
                            if let Some(key) = selected
                                && sent
                                    .draft
                                    .borrow()
                                    .as_ref()
                                    .is_some_and(|d| d.message.thread_id == key.id)
                            {
                                ui::open_thread(&sent.state, key);
                            }
                            ui::refresh_background(&sent.state);
                        });
                    } else {
                        if editor.close_after_save.get() && editor.revision.get() == revision {
                            finish(&editor);
                            return;
                        }
                        editor.send.set_sensitive(true);
                        if editor.revision.get() != revision {
                            changed(&editor);
                        }
                    }
                }
                Err(err) => {
                    editor.status.set_text(&err.to_string());
                    if uncertain {
                        // Google does not provide an idempotency key. An interrupted send may
                        // already have succeeded. Require reconciliation before another attempt.
                        editor.uncertain.set(true);
                        editor.save.set_sensitive(false);
                        editor.status.set_text(&format!("{err}. Saving and sending are locked for this window. Check Gmail Sent and Drafts before opening another composer."));
                    } else {
                        editor.send.set_sensitive(true);
                    }
                }
            }
        },
    );
}
fn attach_files(e: &Rc<Editor>) {
    let dialog = gtk::FileDialog::builder()
        .title("Attach files (16 MiB combined)")
        .build();
    let e = e.clone();
    glib::MainContext::default().spawn_local(async move {
        let Ok(files) = dialog.open_multiple_future(Some(&e.parent())).await else {
            return;
        };
        for i in 0..files.n_items() {
            if let Some(file) = files
                .item(i)
                .and_then(|o| o.downcast::<gtk::gio::File>().ok())
                && let Some(path) = file.path()
            {
                e.attachments.borrow_mut().push(path);
            }
        }
        render_files(&e);
        changed(&e);
    });
}
fn render_files(e: &Rc<Editor>) {
    use crate::attachments::{self, Source};
    use luma_core::preview::{AUTO_IMAGE_BYTES, Kind};
    ui::clear(&e.files);
    let count = e.retained.borrow().len() + e.attachments.borrow().len();
    e.files.set_visible(count > 0);
    if count == 0 {
        return;
    }
    e.files
        .append(&label(&format!("{count} attachments"), "caption"));
    let flow = gtk::FlowBox::builder()
        .selection_mode(gtk::SelectionMode::None)
        .column_spacing(8)
        .row_spacing(8)
        .max_children_per_line(3)
        .min_children_per_line(1)
        .build();
    e.files.append(&flow);
    let mut auto_count = 0;
    let mut auto_bytes = 0;
    for (index, file) in e.retained.borrow().iter().enumerate() {
        let card = attachments::card(&file.name, &file.mime_type, file.data.len() as u64, true);
        let weak = Rc::downgrade(e);
        card.open.connect_clicked(move |_| {
            if let Some(e) = weak.upgrade()
                && let Some(file) = e.retained.borrow().get(index)
            {
                attachments::open(
                    &e.state,
                    &file.name,
                    &file.mime_type,
                    file.data.len() as u64,
                    Source::Bytes(std::sync::Arc::new(file.data.clone())),
                );
            }
        });
        let weak = Rc::downgrade(e);
        card.action.connect_clicked(move |_| {
            if let Some(e) = weak.upgrade() {
                e.retained.borrow_mut().remove(index);
                render_files(&e);
                changed(&e);
            }
        });
        flow.insert(&card.root, -1);
        if Kind::of(&file.name, &file.mime_type) == Kind::Image
            && file.data.len() as u64 <= AUTO_IMAGE_BYTES
            && auto_count < 4
            && auto_bytes + file.data.len() <= 12 * 1024 * 1024
        {
            auto_count += 1;
            auto_bytes += file.data.len();
            attachments::thumbnail(
                &e.state,
                &card,
                Source::Bytes(std::sync::Arc::new(file.data.clone())),
            );
        }
    }
    for (index, path) in e.attachments.borrow().iter().enumerate() {
        let name = path
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .into_owned();
        let bytes = std::fs::metadata(path).map(|m| m.len()).unwrap_or(0);
        let source = Source::Local(path.clone());
        let card = attachments::card(&name, "", bytes, true);
        let weak = Rc::downgrade(e);
        let open_source = source.clone();
        let title = name.clone();
        card.open.connect_clicked(move |_| {
            if let Some(e) = weak.upgrade() {
                attachments::open(&e.state, &title, "", bytes, open_source.clone());
            }
        });
        let weak = Rc::downgrade(e);
        card.action.connect_clicked(move |_| {
            if let Some(e) = weak.upgrade() {
                e.attachments.borrow_mut().remove(index);
                render_files(&e);
                changed(&e);
            }
        });
        flow.insert(&card.root, -1);
        if Kind::of(&name, "") == Kind::Image
            && bytes <= AUTO_IMAGE_BYTES
            && auto_count < 4
            && auto_bytes as u64 + bytes <= 12 * 1024 * 1024
        {
            auto_count += 1;
            auto_bytes += bytes as usize;
            attachments::thumbnail(&e.state, &card, source);
        }
    }
}
fn discard_editor(e: &Rc<Editor>) {
    if e.busy.get() {
        return;
    }
    let d = adw::AlertDialog::builder()
        .heading("Discard this draft?")
        .body("The saved Gmail draft will be deleted. This cannot be undone.")
        .build();
    d.add_responses(&[("cancel", "Keep editing"), ("discard", "Discard")]);
    d.set_response_appearance("discard", adw::ResponseAppearance::Destructive);
    let e = e.clone();
    glib::MainContext::default().spawn_local(async move {
        if d.choose_future(Some(&e.parent())).await != "discard" {
            return;
        }
        // A save may have started while the confirmation was open.
        if e.busy.get() {
            e.status
                .set_text("Wait for saving to finish, then discard again.");
            return;
        }
        if let Some(source) = e.debounce.borrow_mut().take() {
            source.remove();
        }
        let draft = e.draft.borrow().clone();
        if let Some(draft) = draft {
            let Some(a) = e.accounts.get(e.from.selected() as usize) else {
                return;
            };
            let Some(c) = e.state.clients.borrow().get(&a.id).cloned() else {
                return;
            };
            let editor = e.clone();
            e.busy.set(true);
            ui::async_work(
                &e.state,
                async move { Ok(c.discard_draft(&draft.id).await) },
                move |_, result| {
                    editor.busy.set(false);
                    match result {
                        Ok(()) => {
                            finish(&editor);
                        }
                        Err(err) => editor.status.set_text(&err.to_string()),
                    }
                },
            );
        } else {
            finish(&e);
        }
    });
}
pub fn find_draft(s: &Rc<State>, message: &str) {
    let Some(key) = s.selected.borrow().clone() else {
        return;
    };
    let Some(c) = s.clients.borrow().get(&key.account).cloned() else {
        return;
    };
    let id = message.to_owned();
    let rfc = s
        .selected_message
        .borrow()
        .as_ref()
        .map(|m| m.payload.header("Message-ID").to_owned())
        .unwrap_or_default();
    ui::async_work(
        s,
        async move {
            let d = c.find_draft(&id, &rfc).await?;
            let (draft, parts) = c.edit_draft(&d.id).await?;
            Ok((key.account, draft, parts))
        },
        |s, d| open(&s, None, Some(d)),
    );
}
