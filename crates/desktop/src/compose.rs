use crate::ui::{self, State, label, margins, vbox};
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

struct Editor {
    state: Rc<State>,
    window: adw::Window,
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
    send: gtk::Button,
    save: gtk::Button,
    debounce: RefCell<Option<glib::SourceId>>,
    closing: Cell<bool>,
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
                open_internal(&s, Some((message, 2)), None, Some((account, parts)))
            },
        );
        return;
    }
    open_internal(s, reply, prepared, None);
}
fn open_internal(
    s: &Rc<State>,
    reply: Option<(Message, u8)>,
    prepared: Option<(String, Draft, Vec<RetainedAttachment>)>,
    forward: Option<(String, Vec<RetainedAttachment>)>,
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
    make(s, c, draft);
}
fn make(s: &Rc<State>, c: Compose, draft: Option<Draft>) {
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
    let root = vbox(10);
    root.append(&adw::HeaderBar::new());
    let form = vbox(8);
    margins(&form, 18);
    let names: Vec<_> = accounts.iter().map(|a| a.email.as_str()).collect();
    let from = gtk::DropDown::from_strings(&names);
    from.set_selected(accounts.iter().position(|a| a.email == c.from).unwrap_or(0) as u32);
    form.append(&label("Sending account", "heading"));
    form.append(&from);
    let entry = |hint: &str, value: &str| {
        let e = gtk::Entry::builder()
            .placeholder_text(hint)
            .text(value)
            .build();
        form.append(&e);
        e
    };
    let to = entry("To · comma-separated recipients", &c.to);
    let cc = entry("Cc", &c.cc);
    let bcc = entry("Bcc", &c.bcc);
    let subject = entry("Subject", &c.subject);
    let html = gtk::CheckButton::with_label("HTML source mode");
    html.set_active(c.html);
    html.set_tooltip_text(Some(
        "Send the editor contents as HTML. Received mail is always rendered safely as text.",
    ));
    form.append(&html);
    let format = gtk::Box::new(gtk::Orientation::Horizontal, 4);
    form.append(&format);
    let text = gtk::TextView::builder()
        .wrap_mode(gtk::WrapMode::WordChar)
        .vexpand(true)
        .accepts_tab(true)
        .build();
    text.buffer().set_enable_undo(true);
    text.buffer().set_text(&c.text);
    form.append(
        &gtk::ScrolledWindow::builder()
            .vexpand(true)
            .min_content_height(240)
            .child(&text)
            .build(),
    );
    let files = vbox(4);
    form.append(&files);
    let bar = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    let attach = gtk::Button::with_label("Attach files");
    let drive = gtk::Button::with_label("Drive link…");
    let save = gtk::Button::with_label("Save draft");
    let send = gtk::Button::with_label("Send");
    send.add_css_class("suggested-action");
    let discard = gtk::Button::with_label("Discard");
    for b in [&attach, &drive, &save, &discard, &send] {
        bar.append(b);
    }
    form.append(&bar);
    let status = label(
        if draft.is_some() {
            "Saved in Gmail"
        } else {
            "Not yet saved"
        },
        "dim-label",
    );
    status.set_wrap(true);
    form.append(&status);
    root.append(&form);
    window.set_content(Some(&root));
    let is_new = draft.is_none();
    let e = Rc::new(Editor {
        state: s.clone(),
        window,
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
        attachments: RefCell::new(vec![]),
        retained: RefCell::new(c.retained.clone()),
        template: c,
        draft: RefCell::new(draft),
        revision: Cell::new(0),
        saved: Cell::new(0),
        busy: Cell::new(false),
        uncertain: Cell::new(false),
        status,
        send,
        save,
        debounce: RefCell::new(None),
        closing: Cell::new(false),
    });
    for (name, start, end) in [
        ("Bold", "<strong>", "</strong>"),
        ("Italic", "<em>", "</em>"),
        ("Underline", "<u>", "</u>"),
        ("Strike", "<s>", "</s>"),
        ("List", "<ul><li>", "</li></ul>"),
        ("Numbered", "<ol><li>", "</li></ol>"),
        ("Link", "<a href=\"https://example.com\">", "</a>"),
    ] {
        let b = gtk::Button::with_label(name);
        format.append(&b);
        let weak = Rc::downgrade(&e);
        b.connect_clicked(move |_| {
            let Some(e) = weak.upgrade() else { return };
            e.html.set_active(true);
            let buffer = e.text.buffer();
            if let Some((mut a, mut z)) = buffer.selection_bounds() {
                let selected = buffer.text(&a, &z, false);
                buffer.begin_user_action();
                buffer.delete(&mut a, &mut z);
                buffer.insert(&mut a, &format!("{start}{selected}{end}"));
                buffer.end_user_action();
            } else {
                buffer.insert_at_cursor(&format!("{start}{end}"));
            }
        });
    }
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
        e.html.connect_toggled(move |_| {
            if let Some(e) = weak.upgrade() {
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
            let Some(e)=weak.upgrade()else{return glib::Propagation::Proceed};
            if e.closing.get() {
                return glib::Propagation::Proceed;
            }
            if e.busy.get() {
                e.status
                    .set_text("Wait for the current Gmail request to finish.");
                return glib::Propagation::Stop;
            }
            if e.uncertain.get(){
                let d=adw::AlertDialog::builder().heading("Close after an uncertain request?").body("Check Gmail Sent and Drafts before sending again. Copy any unsaved text you need before closing.").build();d.add_responses(&[("cancel","Keep open"),("close","Close")]);
                glib::MainContext::default().spawn_local(async move{if d.choose_future(Some(&e.window)).await=="close"{e.closing.set(true);e.window.close();}});
                return glib::Propagation::Stop;
            }
            if e.revision.get() != e.saved.get() {
                persist(&e, false);
                e.status
                    .set_text("Saving changes. Close again after Gmail confirms Saved.");
                return glib::Propagation::Stop;
            }
            glib::Propagation::Proceed
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
    e.window.present();
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
            let draft = match client
                .save_draft(existing.as_ref(), &raw, thread.as_deref())
                .await
            {
                Ok(d) => d,
                Err(e) => {
                    let uncertain = e.write_may_have_succeeded();
                    return Ok((Err(e), None, uncertain));
                }
            };
            if send && let Err(e) = client.send_draft(&draft).await {
                let uncertain = e.write_may_have_succeeded();
                return Ok((Err(e), Some(draft), uncertain));
            }
            Ok((Ok(draft), None, false))
        },
        move |_, (result, persisted, uncertain)| {
            editor.busy.set(false);
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
                        editor.window.close();
                        ui::reload(&editor.state);
                    } else {
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
        let Ok(files) = dialog.open_multiple_future(Some(&e.window)).await else {
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
    ui::clear(&e.files);
    for (index, file) in e.retained.borrow().iter().enumerate() {
        let row = gtk::Box::new(gtk::Orientation::Horizontal, 6);
        row.append(&label(
            &format!(
                "{} · {} bytes · Gmail attachment",
                file.name,
                file.data.len()
            ),
            "dim-label",
        ));
        let remove = gtk::Button::with_label("Remove");
        row.append(&remove);
        e.files.append(&row);
        let weak = Rc::downgrade(e);
        remove.connect_clicked(move |_| {
            if let Some(e) = weak.upgrade() {
                e.retained.borrow_mut().remove(index);
                render_files(&e);
                changed(&e);
            }
        });
    }
    for (index, path) in e.attachments.borrow().clone().iter().enumerate() {
        let row = gtk::Box::new(gtk::Orientation::Horizontal, 6);
        row.append(&label(
            &format!(
                "{} · {} bytes",
                path.file_name().unwrap_or_default().to_string_lossy(),
                std::fs::metadata(path).map(|m| m.len()).unwrap_or(0)
            ),
            "dim-label",
        ));
        let remove = gtk::Button::with_label("Remove");
        row.append(&remove);
        e.files.append(&row);
        let weak = Rc::downgrade(e);
        remove.connect_clicked(move |_| {
            let Some(e) = weak.upgrade() else { return };
            e.attachments.borrow_mut().remove(index);
            render_files(&e);
            changed(&e);
        });
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
        if d.choose_future(Some(&e.window)).await != "discard" {
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
                            editor.closing.set(true);
                            editor.window.close();
                        }
                        Err(err) => editor.status.set_text(&err.to_string()),
                    }
                },
            );
        } else {
            e.closing.set(true);
            e.window.close();
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
