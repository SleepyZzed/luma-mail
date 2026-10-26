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
#[path = "compose_clipboard.rs"]
mod clipboard;
#[path = "compose_fonts.rs"]
mod fonts;
#[path = "compose_motion.rs"]
mod motion;
#[path = "compose_recipients.rs"]
mod recipients;

pub(crate) struct Editor {
    state: Rc<State>,
    window: adw::Window,
    root: gtk::Box,
    header: adw::HeaderBar,
    placement: Cell<u8>,
    motion_busy: Cell<bool>,
    detached_expanded: Cell<bool>,
    clipboard_busy: Cell<bool>,
    popout: gtk::Button,
    inline_origin: glib::WeakRef<gtk::Box>,
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
    drive_files: RefCell<Vec<luma_core::drive::DriveFile>>,
    uploads: luma_core::compose_drive::UploadCache,
    upload_cancel: std::sync::Arc<std::sync::atomic::AtomicBool>,
    cancel_upload: gtk::Button,
    template: Compose,
    draft: RefCell<Option<Draft>>,
    revision: Cell<u64>,
    saved: Cell<u64>,
    busy: Cell<bool>,
    uncertain: Cell<bool>,
    conflict: Cell<bool>,
    conflict_bar: gtk::Box,
    status: gtk::Label,
    progress: gtk::ProgressBar,
    send_reveal: gtk::Revealer,
    send: gtk::Button,
    save: gtk::Button,
    debounce: RefCell<Option<glib::SourceId>>,
    closing: Cell<bool>,
    dismiss_started: Cell<bool>,
    expand: gtk::Button,
    local_id: RefCell<String>,
    local_debounce: RefCell<Option<glib::SourceId>>,
    recipients: RefCell<Vec<luma_core::local_compose::Recipient>>,
    completion: RefCell<Option<String>>,
    local_sequence: Cell<u64>,
    local_closed: std::sync::Arc<std::sync::atomic::AtomicBool>,
    automatic_links: RefCell<Vec<gtk::TextTag>>,
    links_pending: Cell<bool>,
    font_family: gtk::FontDialogButton,
    font_size: gtk::DropDown,
    typing_font: RefCell<(Option<String>, Option<u16>)>,
    font_sync: Cell<bool>,
    font_inserting: Cell<bool>,
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
fn focus_message(e: &Editor) {
    if e.to.text().is_empty() {
        e.to.grab_focus();
    } else {
        e.text.grab_focus();
    }
}
// Handle traversal at the editor root, before GtkText/Entry/TextView consume it.
// In the body, Tab still indents lists; Shift+Tab returns to Subject.
fn focus_tab(e: &Editor, key: gtk::gdk::Key, mods: gtk::gdk::ModifierType) -> bool {
    if !matches!(key, gtk::gdk::Key::Tab | gtk::gdk::Key::ISO_Left_Tab)
        || mods.intersects(gtk::gdk::ModifierType::CONTROL_MASK | gtk::gdk::ModifierType::ALT_MASK)
    {
        return false;
    }
    let Some(focus) = gtk::prelude::GtkWindowExt::focus(&e.parent()) else {
        return false;
    };
    let backwards =
        key == gtk::gdk::Key::ISO_Left_Tab || mods.contains(gtk::gdk::ModifierType::SHIFT_MASK);
    let fields = [&e.to, &e.cc, &e.bcc, &e.subject];
    if let Some(index) = fields
        .iter()
        .position(|f| focus == **f || focus.is_ancestor(*f))
    {
        if !backwards && index < 3 && recipients::complete(e, fields[index]) {
            return true;
        }
        if backwards {
            if let Some(field) = fields[..index].iter().rev().find(|f| f.is_mapped()) {
                field.grab_focus();
            } else {
                e.from.grab_focus();
            }
        } else if let Some(field) = fields[index + 1..].iter().find(|f| f.is_mapped()) {
            field.grab_focus();
        } else {
            e.text.grab_focus();
        }
        return true;
    }
    if backwards && (focus == e.text || focus.is_ancestor(&e.text)) {
        e.subject.grab_focus();
        return true;
    }
    if !backwards && (focus == e.from || focus.is_ancestor(&e.from)) {
        e.to.grab_focus();
        return true;
    }
    false
}
fn expand_editor(e: &Rc<Editor>) {
    if e.motion_busy.get() || e.closing.get() {
        return;
    }
    match e.placement.get() {
        0 => motion::relocate(e, || {
            if let Some(slot) = e
                .inline_origin
                .upgrade()
                .filter(|slot| slot.root().is_some())
            {
                detach(e);
                e.placement.set(1);
                motion::reset_layout(e);
                e.root.add_css_class("inline-composer");
                e.text.set_height_request(200);
                slot.append(&e.root);
                e.state
                    .compose_stack
                    .set_visible_child_full("mail", gtk::StackTransitionType::None);
            } else {
                motion::compact(e);
            }
            motion::controls(e);
        }),
        2 => {
            let expanded = !e.detached_expanded.get();
            e.detached_expanded.set(expanded);
            if expanded {
                e.window.maximize();
            } else {
                e.window.unmaximize();
                e.window.set_default_size(760, 640);
            }
            motion::controls(e);
            crate::presentation::enter(&e.state, &e.root);
        }
        _ => show_full(e),
    }
}
#[cfg(debug_assertions)]
pub(crate) fn diagnostic(s: &Rc<State>) {
    assert!(s.clients.borrow().is_empty() && std::env::var_os("LUMA_TEST_ROOT").is_some());
    let recent = Compose {
        to: "Jamie Chen <jamie@example.invalid>".into(),
        ..Default::default()
    };
    s.local_compose.remember("design-fixture", &recent).unwrap();
    make(s, Compose::new("alex@example.com".into()), None);
    let original = s.editor.borrow().clone().unwrap();
    let state = s.clone();
    let mut stage = 0;
    let mut ticks = 0;
    let mut pasted_body = None;
    glib::timeout_add_local(Duration::from_millis(150), move || {
        ticks += 1;
        assert!(ticks < 80, "composer diagnostics timed out");
        let current = state.editor.borrow().clone();
        match stage {
            0 => {
                let e = current.unwrap();
                let focus =
                    gtk::prelude::GtkWindowExt::focus(&e.parent()).expect("new composer focus");
                assert!(
                    focus == e.to || focus.is_ancestor(&e.to),
                    "new composer starts in To"
                );
                e.to.set_text("ja");
                e.to.set_position(-1);
                if e.recipients.borrow().is_empty() {
                    return glib::ControlFlow::Continue;
                }
                assert!(recipients::complete(&e, &e.to));
                assert!(e.to.text().contains("jamie@example.invalid"));
                let key = |backwards: bool| {
                    let controllers = e.root.observe_controllers();
                    for i in 0..controllers.n_items() {
                        if let Some(key) = controllers
                            .item(i)
                            .and_then(|o| o.downcast::<gtk::EventControllerKey>().ok())
                            && key.propagation_phase() == gtk::PropagationPhase::Capture
                            && key.emit_by_name::<bool>(
                                "key-pressed",
                                &[
                                    &if backwards {
                                        gtk::gdk::Key::ISO_Left_Tab
                                    } else {
                                        gtk::gdk::Key::Tab
                                    },
                                    &0u32,
                                    &if backwards {
                                        gtk::gdk::ModifierType::SHIFT_MASK
                                    } else {
                                        gtk::gdk::ModifierType::empty()
                                    },
                                ],
                            )
                        {
                            break;
                        }
                    }
                };
                key(false);
                let focus = gtk::prelude::GtkWindowExt::focus(&e.parent()).unwrap();
                assert!(
                    focus == e.subject || focus.is_ancestor(&e.subject),
                    "Tab skips labels and hidden Cc/Bcc"
                );
                key(false);
                assert_eq!(
                    gtk::prelude::GtkWindowExt::focus(&e.parent()).unwrap(),
                    e.text.clone().upcast::<gtk::Widget>(),
                    "Subject Tab reaches body"
                );
                key(true);
                let focus = gtk::prelude::GtkWindowExt::focus(&e.parent()).unwrap();
                assert!(
                    focus == e.subject || focus.is_ancestor(&e.subject),
                    "Body Shift+Tab reaches Subject"
                );
                key(true);
                let focus = gtk::prelude::GtkWindowExt::focus(&e.parent()).unwrap();
                assert!(
                    focus == e.to || focus.is_ancestor(&e.to),
                    "Subject Shift+Tab skips hidden address fields"
                );
                let sample = gtk::TextView::new();
                insert_named_link(
                    &sample,
                    "Project notes.docx",
                    "https://drive.google.com/file/d/123/view?a=1&b=2",
                    false,
                );
                let output = formatted_html(&sample.buffer());
                assert!(output.contains(">Project notes.docx</a>"));
                assert_eq!(
                    output.matches("https://").count(),
                    1,
                    "Drive URL occurs only in href"
                );
                e.subject.set_text("Local recovery check");
                e.text
                    .buffer()
                    .set_text("Unsent words and https://example.com");
                e.text.buffer().apply_tag(
                    &e.text.buffer().tag_table().lookup("strong").unwrap(),
                    &e.text.buffer().start_iter(),
                    &e.text.buffer().iter_at_offset(6),
                );
                assert_eq!(e.placement.get(), 3, "new messages start compact");
                println!(
                    "PASS new compose To focus, recipient completion, Tab/Shift+Tab and filename hyperlink"
                );
                stage = 1;
            }
            1 => {
                let e = current.unwrap();
                if e.links_pending.get() || e.motion_busy.get() {
                    return glib::ControlFlow::Continue;
                }
                assert_eq!(
                    e.placement.get(),
                    3,
                    "Restore uses an in-app compact composer"
                );
                assert!(
                    !state.window.is_fullscreen(),
                    "Compose must not change the desktop fullscreen state"
                );
                assert!(
                    e.root.width() < state.window.width(),
                    "compact composer width is bounded"
                );
                assert!(
                    !e.automatic_links.borrow().is_empty(),
                    "typed links are highlighted"
                );
                fonts::verify(&e);
                e.expand.emit_clicked();
                stage = 2;
                println!("PASS compact Restore and automatic blue link tags");
            }
            2 if !original.motion_busy.get() => {
                assert_eq!(original.placement.get(), 0);
                assert_eq!(
                    state.compose_stack.visible_child_name().as_deref(),
                    Some("compose")
                );
                if std::env::args().any(|a| a == "--clipboard-diagnostics") {
                    let png = glib::base64_decode(
                        "iVBORw0KGgoAAAANSUhEUgAAAAIAAAACCAIAAAD91JpzAAAAEklEQVR4nGNwW3XJbdUlBggFAC8WBwmmUSZGAAAAAElFTkSuQmCC",
                    );
                    let clipboard = original.text.clipboard();
                    clipboard
                        .set_content(Some(&gtk::gdk::ContentProvider::for_bytes(
                            "image/png",
                            &glib::Bytes::from_owned(png),
                        )))
                        .unwrap();
                    pasted_body = Some(
                        original
                            .text
                            .buffer()
                            .text(
                                &original.text.buffer().start_iter(),
                                &original.text.buffer().end_iter(),
                                false,
                            )
                            .to_string(),
                    );
                    original.text.emit_by_name::<()>("paste-clipboard", &[]);
                    stage = 3;
                } else {
                    stage = 4;
                }
            }
            3 if !original.clipboard_busy.get() => {
                assert_eq!(original.retained.borrow().len(), 1);
                assert_eq!(original.retained.borrow()[0].mime_type, "image/png");
                assert!(original.retained.borrow()[0].content_id.is_none());
                assert_eq!(
                    original
                        .text
                        .buffer()
                        .text(
                            &original.text.buffer().start_iter(),
                            &original.text.buffer().end_iter(),
                            false
                        )
                        .as_str(),
                    pasted_body.as_deref().unwrap()
                );
                println!(
                    "PASS actual GTK image clipboard paste creates an in-memory attachment without replacing text"
                );
                stage = 4;
            }
            4 => {
                original.conflict.set(true);
                original.conflict_bar.set_visible(true);
                *original.draft.borrow_mut() = Some(Draft {
                    id: "newer-gmail-draft".into(),
                    message: Message::default(),
                });
                save_conflict_copy(&original);
                assert!(original.draft.borrow().is_none() && !original.conflict.get());
                assert!(!original.conflict_bar.is_visible());
                finish(&original);
                if state.compose_stack.is_transition_running() {
                    assert!(
                        original.root.parent().is_some(),
                        "outgoing composer stays alive for Back"
                    );
                }
                stage = 5;
            }
            5 if state.editor.borrow().is_none() => {
                assert!(original.root.parent().is_none());
                assert!(
                    state.motion_layer.first_child().is_none(),
                    "animation snapshot released"
                );
                assert_eq!(
                    state.compose_stack.visible_child_name().as_deref(),
                    Some("mail")
                );
                println!(
                    "PASS animated compact/full-page round trip and Back cleanup without desktop fullscreen changes"
                );
                assert!(state.local_compose.drafts().unwrap().is_empty());
                println!("PASS normal composition creates no local draft files; Gmail owns drafts");
                state.app.quit();
                return glib::ControlFlow::Break;
            }
            _ => (),
        }
        glib::ControlFlow::Continue
    });
}
fn detach(e: &Editor) {
    if e.placement.get() == 2 {
        e.window.set_content(None::<&gtk::Widget>);
    } else if let Some(parent) = e.root.parent() {
        if let Some(stack) = parent.downcast_ref::<gtk::Stack>() {
            stack.remove(&e.root);
        } else if let Some(overlay) = parent.downcast_ref::<gtk::Overlay>() {
            overlay.remove_overlay(&e.root);
        } else if let Some(box_) = parent.downcast_ref::<gtk::Box>() {
            box_.remove(&e.root);
        }
    }
}
fn show_full(e: &Rc<Editor>) {
    if e.motion_busy.get() || e.closing.get() {
        return;
    }
    if let Some(current) = e.state.editor.borrow().as_ref()
        && !Rc::ptr_eq(current, e)
    {
        e.status
            .set_text("Another draft is open in the main window. Finish or pop it out first.");
        return;
    }
    motion::relocate(e, || {
        let detached = e.placement.get() == 2;
        detach(e);
        e.placement.set(0);
        motion::reset_layout(e);
        e.header.set_show_end_title_buttons(false);
        e.state.compose_stack.add_named(&e.root, Some("compose"));
        e.state.compose_stack.set_visible_child_full(
            "compose",
            if e.motion_busy.get() {
                gtk::StackTransitionType::None
            } else {
                gtk::StackTransitionType::SlideLeft
            },
        );
        *e.state.editor.borrow_mut() = Some(e.clone());
        if detached {
            e.window.set_visible(false);
        }
        motion::controls(e);
        e.state.window.present();
    });
}
fn pop_out(e: &Rc<Editor>) {
    if e.motion_busy.get() || e.closing.get() {
        return;
    }
    if e.placement.get() == 2 {
        e.window.present();
        return;
    }
    detach(e);
    e.placement.set(2);
    e.detached_expanded.set(false);
    motion::reset_layout(e);
    e.header
        .set_show_end_title_buttons(e.state.settings.borrow().window_chrome);
    e.root.remove_css_class("inline-composer");
    e.window.set_content(Some(&e.root));
    e.state.editor.borrow_mut().take();
    e.state.compose_stack.set_visible_child_name("mail");
    motion::controls(e);
    e.window.present();
    crate::presentation::enter(&e.state, &e.root);
    focus_message(e);
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
    if e.dismiss_started.replace(true) {
        return;
    }
    e.closing.set(true);
    e.local_closed
        .store(true, std::sync::atomic::Ordering::SeqCst);
    if let Some(timer) = e.debounce.borrow_mut().take() {
        timer.remove();
    }
    if let Some(timer) = e.local_debounce.borrow_mut().take() {
        timer.remove();
    }
    motion::dismiss(e);
}
fn release_editor(e: &Rc<Editor>) {
    detach(e);
    let owns = e
        .state
        .editor
        .borrow()
        .as_ref()
        .is_some_and(|current| Rc::ptr_eq(current, e));
    if owns {
        e.state.editor.borrow_mut().take();
        if e.state.compose_stack.visible_child_name().as_deref() == Some("compose") {
            e.state.compose_stack.set_visible_child_name("mail");
        }
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
    if e.conflict.get() {
        let dialog = adw::AlertDialog::builder().heading("Keep both draft versions?").body("Gmail has a newer version. Your unsaved edits are still here. Save them as a separate Gmail draft, or close this editor and keep Gmail's version.").build();
        dialog.add_responses(&[
            ("cancel", "Keep editing"),
            ("copy", "Save separate draft"),
            ("close", "Close my unsaved version"),
        ]);
        dialog.set_response_appearance("close", adw::ResponseAppearance::Destructive);
        dialog.set_default_response(Some("cancel"));
        let editor = e.clone();
        glib::MainContext::default().spawn_local(async move {
            match dialog.choose_future(Some(&editor.parent())).await.as_str() {
                "copy" => {
                    save_conflict_copy(&editor);
                    editor.close_after_save.set(true);
                }
                "close" => finish(&editor),
                _ => (),
            }
        });
        return glib::Propagation::Stop;
    }
    if e.uncertain.get() {
        let dialog=adw::AlertDialog::builder().heading("Close after an uncertain request?").body("Check Gmail Sent and Drafts before sending again. Copy any unsaved text you need before closing.").build();
        dialog.add_responses(&[("cancel", "Keep editing"), ("close", "Close")]);
        let editor = e.clone();
        glib::MainContext::default().spawn_local(async move {
            if dialog.choose_future(Some(&editor.parent())).await == "close" {
                if editor.state.settings.borrow().local_drafts {
                    save_local(&editor, true);
                } else {
                    finish(&editor);
                }
            }
        });
        return glib::Propagation::Stop;
    }
    if e.revision.get() > 0 && e.state.settings.borrow().local_drafts {
        save_local(e, true);
        return glib::Propagation::Stop;
    }
    if e.revision.get() != e.saved.get() {
        e.close_after_save.set(true);
        persist(e, false);
        return glib::Propagation::Stop;
    }
    glib::Propagation::Proceed
}
pub fn prepare_quit(s: &Rc<State>) -> bool {
    let editor = s.editor.borrow().clone();
    if let Some(e) = editor {
        // A never-presented host window does not emit GTK close-request. Close
        // embedded/parked editors through their save policy explicitly.
        if request_close(&e) == glib::Propagation::Proceed {
            finish(&e);
        } else {
            show_full(&e);
            return false;
        }
    }
    true
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
    if reply.is_some() && !ui::single_message_actions(s) {
        s.info("Select one conversation before replying or forwarding.");
        return;
    }
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
        c.drive_files = match luma_core::compose_drive::draft_references(
            p.header("X-Luma-Drive-Attachments"),
        ) {
            Ok(files) => files,
            Err(error) => {
                s.error(&error);
                return;
            }
        };
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
    design_at(s, paths, None);
}
#[cfg(debug_assertions)]
pub(crate) fn design_reply(s: &Rc<State>, paths: Vec<PathBuf>, slot: gtk::Box) {
    design_at(s, paths, Some(slot));
}
#[cfg(debug_assertions)]
fn design_at(s: &Rc<State>, paths: Vec<PathBuf>, slot: Option<gtk::Box>) {
    let mut message = Compose::new("alex@example.com".into());
    message.to = "jamie@example.com".into();
    message.subject = "Re: Weekend by the coast".into();
    // Configure local cards after creation without scheduling a real save.
    message.attachments = paths;
    make_at(s, message, None, slot);
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
    crate::compose_lists::verify();
}
#[cfg(all(test, target_os = "linux"))]
pub(crate) fn verify_host_lifecycle(s: &Rc<State>) {
    let reduced = s.settings.borrow().reduced_motion;
    s.settings.borrow_mut().reduced_motion = true;
    crate::presentation::apply(s);
    let account = luma_core::config::Account {
        id: "composer-fixture".into(),
        email: "fixture@example.invalid".into(),
        name: "Fixture".into(),
        enabled: true,
        notifications: false,
        signature: String::new(),
        drive_enabled: false,
        sender_controls: false,
        contact_photos: false,
        blocked_senders: vec![],
    };
    s.settings.borrow_mut().accounts.push(account);
    make(s, Compose::new("fixture@example.invalid".into()), None);
    let e = s.editor.borrow().as_ref().unwrap().clone();
    assert_eq!(e.placement.get(), 3, "new composers start compact");
    assert_eq!(
        s.compose_stack.visible_child_name().as_deref(),
        Some("mail")
    );
    expand_editor(&e);
    assert_eq!(
        s.compose_stack.visible_child_name().as_deref(),
        Some("compose")
    );
    pop_out(&e);
    assert!(s.editor.borrow().is_none());
    assert_eq!(e.window.content().as_ref(), Some(e.root.upcast_ref()));
    show_full(&e);
    assert!(!e.window.is_visible());
    assert!(s.editor.borrow().is_some());
    e.window.close();
    assert!(s.editor.borrow().is_none());
    assert!(e.root.parent().is_none());
    let slot = ui::vbox(0);
    s.reader.append(&slot);
    make_at(
        s,
        Compose::new("fixture@example.invalid".into()),
        None,
        Some(slot.clone()),
    );
    let e = s.editor.borrow().as_ref().unwrap().clone();
    assert_eq!(e.placement.get(), 1);
    assert_eq!(e.root.parent().as_ref(), Some(slot.upcast_ref()));
    park_inline(s);
    assert_eq!(e.placement.get(), 0);
    assert_eq!(
        s.compose_stack.visible_child_name().as_deref(),
        Some("mail")
    );
    assert!(prepare_quit(s));
    assert!(s.editor.borrow().is_none());
    s.reader.remove(&slot);
    s.settings
        .borrow_mut()
        .accounts
        .retain(|a| a.id != "composer-fixture");
    s.settings.borrow_mut().reduced_motion = reduced;
    crate::presentation::apply(s);
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
    let rich = c
        .html
        .then(|| luma_core::rich_draft::parse(&c.text))
        .flatten();
    let window = adw::Window::builder()
        .transient_for(&s.window)
        .application(&s.app)
        .title("Compose · Luma Mail")
        .decorated(s.settings.borrow().window_chrome)
        .default_width(760)
        .default_height(720)
        .build();
    window.add_css_class("compose-window");
    let root = vbox(0);
    root.add_css_class("compose-scene");
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
    let conflict_bar = vbox(6);
    conflict_bar.add_css_class("draft-conflict");
    ui::margins(&conflict_bar, 12);
    let notice = label(
        "This draft changed in Gmail. Your unsaved text is kept here. Save a separate draft to preserve both versions.",
        "",
    );
    notice.set_wrap(true);
    conflict_bar.append(&notice);
    let save_copy = gtk::Button::with_label("Save my version as a separate Gmail draft");
    conflict_bar.append(&save_copy);
    conflict_bar.set_visible(false);
    root.append(&conflict_bar);
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
    let format = gtk::FlowBox::builder()
        .selection_mode(gtk::SelectionMode::None)
        .min_children_per_line(1)
        .max_children_per_line(2)
        .column_spacing(4)
        .row_spacing(4)
        .build();
    format.set_halign(gtk::Align::Start);
    let format_bar = vbox(0);
    format_bar.add_css_class("compose-toolbar");
    format_bar.append(&format);
    form.append(&format_bar);
    let text = gtk::TextView::builder()
        .wrap_mode(gtk::WrapMode::WordChar)
        .vexpand(true)
        .accepts_tab(true)
        .build();
    text.buffer().set_enable_undo(true);
    text.buffer()
        .set_text(rich.as_ref().map(|r| r.text.as_str()).unwrap_or(&c.text));
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
    html.set_active(c.html && rich.is_none());
    html.set_tooltip_text(Some(
        "Advanced: edit the original HTML instead of formatted text",
    ));
    let advanced = gtk::Expander::builder()
        .label("Advanced")
        .child(&html)
        .expanded(c.html && rich.is_none())
        .build();
    advanced.add_css_class("compose-advanced");
    form.append(&advanced);
    let progress = gtk::ProgressBar::new();
    progress.set_visible(false);
    progress.add_css_class("compose-progress");
    progress.set_show_text(true);
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
        .transition_duration(crate::presentation::duration(s, 260))
        .reveal_child(true)
        .vexpand(true)
        .child(&form)
        .build();
    root.append(&send_reveal);
    let cancel_upload = gtk::Button::with_label("Cancel Drive upload");
    cancel_upload.set_visible(false);
    cancel_upload.set_halign(gtk::Align::End);
    root.append(&progress);
    root.append(&cancel_upload);

    let is_new = draft.is_none();
    let mut template = c.clone();
    template.retained.clear();
    template.attachments.clear();
    template.drive_files.clear();
    let (font_family, font_size) = fonts::controls();
    let e = Rc::new(Editor {
        state: s.clone(),
        window,
        root,
        header,
        placement: Cell::new(if slot.is_some() { 1 } else { 0 }),
        motion_busy: Cell::new(false),
        detached_expanded: Cell::new(false),
        clipboard_busy: Cell::new(false),
        popout: popout.clone(),
        inline_origin: slot.as_ref().map(|s| s.downgrade()).unwrap_or_default(),
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
        drive_files: RefCell::new(c.drive_files.clone()),
        uploads: Default::default(),
        upload_cancel: Default::default(),
        cancel_upload,
        template,
        draft: RefCell::new(draft),
        revision: Cell::new(0),
        saved: Cell::new(0),
        busy: Cell::new(false),
        uncertain: Cell::new(false),
        conflict: Cell::new(false),
        conflict_bar,
        status,
        progress,
        send_reveal,
        send,
        save,
        debounce: RefCell::new(None),
        closing: Cell::new(false),
        dismiss_started: Cell::new(false),
        expand: expand.clone(),
        local_id: RefCell::new(luma_core::composer::new_id()),
        local_debounce: RefCell::new(None),
        recipients: RefCell::new(Vec::new()),
        completion: RefCell::new(None),
        local_sequence: Cell::new(0),
        local_closed: Default::default(),
        automatic_links: RefCell::new(Vec::new()),
        links_pending: Cell::new(false),
        font_family,
        font_size,
        typing_font: RefCell::new((None, None)),
        font_sync: Cell::new(false),
        font_inserting: Cell::new(false),
    });
    s.compose_editors
        .borrow_mut()
        .retain(|e| e.strong_count() > 0);
    s.compose_editors.borrow_mut().push(Rc::downgrade(&e));
    {
        let flag = e.upload_cancel.clone();
        e.cancel_upload
            .connect_clicked(move |_| flag.store(true, std::sync::atomic::Ordering::Relaxed));
    }
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
                if e.placement.get() == 2 {
                    show_full(&e);
                } else {
                    pop_out(&e);
                }
            }
        });
        let weak = Rc::downgrade(&e);
        expand.connect_clicked(move |_| {
            if let Some(e) = weak.upgrade() {
                expand_editor(&e);
            }
        });
    }
    if let Some(rich) = rich {
        let b = e.text.buffer();
        for span in rich.spans {
            let tag = if let Some(family) = span.tag.strip_prefix("font:family:") {
                Some(fonts::family_tag(&b, family))
            } else if let Some(size) = span
                .tag
                .strip_prefix("font:size:")
                .and_then(|v| v.parse().ok())
            {
                Some(fonts::size_tag(&e, size))
            } else if let Some(link) = span.tag.strip_prefix("link:") {
                Some(link_tag(&b, link))
            } else {
                b.tag_table().lookup(&span.tag)
            };
            if let Some(tag) = tag {
                b.apply_tag(
                    &tag,
                    &b.iter_at_offset(span.start),
                    &b.iter_at_offset(span.end),
                );
            }
        }
    }
    fonts::install(&e, &format);
    let styling = gtk::Box::new(gtk::Orientation::Horizontal, 4);
    format.insert(&styling, -1);
    for (icon, title, tag) in [
        ("format-text-bold-symbolic", "Bold", "strong"),
        ("format-text-italic-symbolic", "Italic", "em"),
        ("format-text-underline-symbolic", "Underline", "u"),
        ("format-text-strikethrough-symbolic", "Strikethrough", "s"),
    ] {
        let b = ui::button(icon, title);
        b.add_css_class("flat");
        styling.append(&b);
        let weak = Rc::downgrade(&e);
        b.connect_clicked(move |_| {
            if let Some(e) = weak.upgrade() {
                format_selection(&e, tag);
            }
        });
    }
    let list = ui::button("view-list-symbolic", "Toggle bullet list");
    list.add_css_class("flat");
    styling.append(&list);
    let weak = Rc::downgrade(&e);
    list.connect_clicked(move |_| {
        if let Some(e) = weak.upgrade() {
            if e.html.is_active() {
                e.status
                    .set_text("Switch off HTML source to edit a visual list.");
                return;
            }
            crate::compose_lists::toggle(&e.text.buffer());
            e.text.grab_focus();
        }
    });
    let link = ui::button("insert-link-symbolic", "Insert link");
    link.add_css_class("flat");
    styling.append(&link);
    let weak = Rc::downgrade(&e);
    link.connect_clicked(move |_| {
        if let Some(e) = weak.upgrade() {
            insert_link(&e);
        }
    });
    let keyboard = gtk::EventControllerKey::new();
    keyboard.set_propagation_phase(gtk::PropagationPhase::Capture);
    let weak = Rc::downgrade(&e);
    keyboard.connect_key_pressed(move |_, key, _, mods| {
        let Some(e) = weak.upgrade() else {
            return glib::Propagation::Proceed;
        };
        if focus_tab(&e, key, mods) {
            return glib::Propagation::Stop;
        }
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
    e.root.add_controller(keyboard);
    let list_keys = gtk::EventControllerKey::new();
    list_keys.set_propagation_phase(gtk::PropagationPhase::Capture);
    let weak = Rc::downgrade(&e);
    list_keys.connect_key_pressed(move |_, key, _, mods| {
        if let Some(e) = weak.upgrade()
            && !e.html.is_active()
            && crate::compose_lists::key(&e.text.buffer(), key, mods)
        {
            return glib::Propagation::Stop;
        }
        glib::Propagation::Proceed
    });
    e.text.add_controller(list_keys);
    let link_click = gtk::GestureClick::new();
    link_click.set_button(1);
    link_click.set_propagation_phase(gtk::PropagationPhase::Capture);
    let weak = Rc::downgrade(&e);
    link_click.connect_pressed(move |gesture, _, x, y| {
        if !gesture
            .current_event_state()
            .contains(gtk::gdk::ModifierType::CONTROL_MASK)
        {
            return;
        }
        if let Some(e) = weak.upgrade() {
            let (x, y) =
                e.text
                    .window_to_buffer_coords(gtk::TextWindowType::Widget, x as i32, y as i32);
            if let Some(iter) = e.text.iter_at_location(x, y)
                && let Some(uri) = iter.tags().iter().filter_map(|t| t.name()).find_map(|n| {
                    n.strip_prefix("link:")
                        .or_else(|| n.strip_prefix("auto-link:"))
                        .map(str::to_owned)
                })
            {
                crate::rendering::open_link(&e.state, &uri);
                gesture.set_state(gtk::EventSequenceState::Claimed);
            }
        }
    });
    e.text.add_controller(link_click);
    e.text
        .set_tooltip_text(Some("Ctrl+click a blue link to open it in your browser"));
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
                if !e.links_pending.replace(true) {
                    let weak = Rc::downgrade(&e);
                    glib::idle_add_local_once(move || {
                        if let Some(e) = weak.upgrade() {
                            e.links_pending.set(false);
                            highlight_links(&e);
                        }
                    });
                }
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
                if e.conflict.get() {
                    save_conflict_copy(&e);
                } else {
                    persist(&e, false);
                }
            }
        });
        let weak = Rc::downgrade(&e);
        save_copy.connect_clicked(move |_| {
            if let Some(e) = weak.upgrade() {
                save_conflict_copy(&e);
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
                crate::settings::drive_dialog(&e.state, &e.text, &a.id, e.html.is_active());
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
        focus_message(&e);
        motion::controls(&e);
    } else {
        motion::compact(&e);
        motion::controls(&e);
        s.window.present();
        focus_message(&e);
        crate::presentation::enter(s, &e.root);
    }
    render_files(&e);
    highlight_links(&e);
    let store = s.local_compose.clone();
    let weak = Rc::downgrade(&e);
    ui::async_work(
        s,
        async move {
            Ok(tokio::task::spawn_blocking(move || store.recipients())
                .await
                .unwrap_or_default())
        },
        move |_, rows| {
            if let Some(e) = weak.upgrade() {
                *e.recipients.borrow_mut() = rows;
            }
        },
    );
    recipients::install(&e);
    clipboard::install(&e);
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
    if e.conflict.get() {
        e.status.set_text("Draft changed in Gmail · Your version is kept here. Save a separate draft to continue.");
        return;
    }
    if let Some(timer) = e.local_debounce.borrow_mut().take() {
        timer.remove();
    }
    if e.state.settings.borrow().local_drafts {
        let weak = Rc::downgrade(e);
        *e.local_debounce.borrow_mut() = Some(glib::timeout_add_local_once(
            Duration::from_millis(450),
            move || {
                if let Some(e) = weak.upgrade() {
                    e.local_debounce.borrow_mut().take();
                    save_local(&e, false);
                }
            },
        ));
    }
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
fn save_local(e: &Rc<Editor>, close: bool) {
    if !e.state.settings.borrow().local_drafts || e.closing.get() {
        return;
    }
    let compose = match snapshot_impl(e, false) {
        Ok(c) => c,
        Err(error) => {
            e.status.set_text(&error.to_string());
            return;
        }
    };
    if compose.text.len() > luma_core::local_compose::MAX_DRAFT_BYTES {
        e.status.set_text(
            "This draft exceeds local recovery's 512 KiB limit. Save it to Gmail before closing.",
        );
        return;
    }
    let buffer = e.text.buffer();
    let mut at = buffer.start_iter();
    let mut spans = Vec::new();
    while at.offset() < buffer.char_count() {
        let start = at.offset();
        let tags = formatting(&at);
        if !at.forward_to_tag_toggle(None::<&gtk::TextTag>) {
            at = buffer.end_iter();
        }
        for tag in tags {
            spans.push(luma_core::local_compose::Span {
                start,
                end: at.offset(),
                tag,
            });
        }
        if spans.len() > 4096 {
            e.status
                .set_text("Too much formatting for local recovery. Save this draft to Gmail.");
            return;
        }
    }
    let account = e.accounts[e.from.selected() as usize].id.clone();
    let server = e
        .draft
        .borrow()
        .as_ref()
        .map(|d| luma_core::local_compose::ServerDraft {
            id: d.id.clone(),
            message: d.message.id.clone(),
            thread: d.message.thread_id.clone(),
        });
    let revision = e.revision.get();
    e.local_sequence
        .set(e.local_sequence.get().saturating_add(1));
    let draft = luma_core::local_compose::LocalDraft {
        id: e.local_id.borrow().clone(),
        account,
        compose,
        spans,
        server,
        retained_on_server: !e.retained.borrow().is_empty(),
        uncertain: e.uncertain.get(),
        updated: 0,
        revision,
        sequence: e.local_sequence.get(),
    };
    let store = e.state.local_compose.clone();
    let closed = e.local_closed.clone();
    let weak = Rc::downgrade(e);
    ui::async_work(
        &e.state,
        async move {
            tokio::task::spawn_blocking(move || store.save_if_open(draft, &closed))
                .await
                .map_err(|_| Error::Invalid("Local recovery write failed".into()))
        },
        move |_, result| {
            if let Some(e) = weak.upgrade() {
                match result {
                    Ok(()) if close && e.revision.get() == revision && !e.busy.get() => {
                        finish(&e);
                    }
                    Ok(())
                        if !e.busy.get()
                            && e.revision.get() == revision
                            && !e.uncertain.get()
                            && e.saved.get() != revision =>
                    {
                        e.status.set_text("Saved locally · Gmail save pending")
                    }
                    Ok(()) => (),
                    Err(error) if close && e.saved.get() == revision && !e.uncertain.get() => {
                        e.state.toast(&format!(
                            "Saved in Gmail. Local recovery unavailable: {error}"
                        ));
                        let _ = e.state.local_compose.remove(&e.local_id.borrow());
                        finish(&e);
                    }
                    Err(error) => e
                        .status
                        .set_text(&format!("Local recovery unavailable: {error}")),
                }
            }
        },
    );
}

pub fn local_drafts(s: &Rc<State>) {
    let window = adw::Window::builder()
        .title("Local drafts · Luma Mail")
        .transient_for(&s.window)
        .application(&s.app)
        .default_width(540)
        .default_height(420)
        .build();
    let root = vbox(12);
    root.append(&adw::HeaderBar::new());
    let notice = label(
        "Recovery from an older development build. Save these to Gmail Drafts or discard them. New drafts are stored in Gmail.",
        "caption",
    );
    notice.set_wrap(true);
    root.append(&notice);
    let list = vbox(8);
    root.append(
        &gtk::ScrolledWindow::builder()
            .child(&list)
            .vexpand(true)
            .build(),
    );
    window.set_content(Some(&root));
    window.present();
    let store = s.local_compose.clone();
    ui::async_work(
        s,
        async move {
            tokio::task::spawn_blocking(move || store.drafts())
                .await
                .map_err(|_| Error::Invalid("Cannot read local drafts".into()))?
        },
        {
            let window = window.clone();
            move |s, drafts| {
                let drafts: Vec<_> = drafts
                    .into_iter()
                    .filter(|d| {
                        s.settings
                            .borrow()
                            .accounts
                            .iter()
                            .any(|a| a.enabled && a.id == d.account)
                    })
                    .collect();
                if drafts.is_empty() {
                    list.append(&label("No local drafts to recover.", "dim-label"));
                }
                for draft in drafts {
                    let row = gtk::Box::new(gtk::Orientation::Horizontal, 8);
                    let title = if draft.compose.subject.is_empty() {
                        "(No subject)"
                    } else {
                        &draft.compose.subject
                    };
                    let button =
                        gtk::Button::with_label(&format!("{}\n{}", title, draft.compose.to));
                    button.set_hexpand(true);
                    row.append(&button);
                    let remove = ui::button("user-trash-symbolic", "Delete local recovery copy");
                    row.append(&remove);
                    list.append(&row);
                    let state = s.clone();
                    let recovery = draft.clone();
                    let host = window.clone();
                    button.connect_clicked(move |_| {
                        restore_local(&state, recovery.clone());
                        host.close();
                    });
                    let store = s.local_compose.clone();
                    let parent = window.clone();
                    let row = row.clone();
                    let list = list.clone();
                    remove.connect_clicked(move |_| {
                        let dialog = adw::AlertDialog::builder()
                            .heading("Delete this local draft?")
                            .body(
                                "Only the local recovery copy is removed. Any Gmail draft is kept.",
                            )
                            .build();
                        dialog
                            .add_responses(&[("cancel", "Keep"), ("delete", "Delete local copy")]);
                        dialog.set_response_appearance(
                            "delete",
                            adw::ResponseAppearance::Destructive,
                        );
                        let store = store.clone();
                        let id = draft.id.clone();
                        let row = row.clone();
                        let list = list.clone();
                        let parent = parent.clone();
                        glib::MainContext::default().spawn_local(async move {
                            if dialog.choose_future(Some(&parent)).await == "delete"
                                && store.remove(&id).is_ok()
                            {
                                list.remove(&row);
                            }
                        });
                    });
                }
            }
        },
    );
}
fn restore_local(s: &Rc<State>, record: luma_core::local_compose::LocalDraft) {
    if s.editor.borrow().is_some() {
        s.info("Finish or pop out the current composer before recovering another draft.");
        return;
    }
    let mut draft = record.server.as_ref().map(|d| Draft {
        id: d.id.clone(),
        message: Message {
            id: d.message.clone(),
            thread_id: d.thread.clone(),
            ..Default::default()
        },
    });
    if record.retained_on_server {
        if let (Some(server), Some(client)) = (
            &record.server,
            s.clients.borrow().get(&record.account).cloned(),
        ) {
            let id = server.id.clone();
            let version = server.message.clone();
            ui::async_work(
                s,
                async move {
                    let (d, parts) = client.edit_draft(&id).await?;
                    if d.message.id != version {
                        return Err(Error::DraftConflict);
                    }
                    Ok((d, parts))
                },
                move |s, (d, parts)| {
                    let mut record = record;
                    record.compose.retained = parts;
                    record.retained_on_server = false;
                    restore_local_ready(&s, record, Some(d));
                },
            );
            return;
        }
        let dialog = adw::AlertDialog::builder().heading("Recover text without forwarded attachments?").body("These attachments had not reached Gmail. Local recovery does not copy attachment contents; attach them again before sending.").build();
        dialog.add_responses(&[("cancel", "Cancel"), ("recover", "Recover text")]);
        let s = s.clone();
        glib::MainContext::default().spawn_local(async move {
            if dialog.choose_future(Some(&s.window)).await == "recover" {
                draft.take();
                restore_local_ready(&s, record, None);
            }
        });
        return;
    }
    restore_local_ready(s, record, draft);
}
fn restore_local_ready(
    s: &Rc<State>,
    record: luma_core::local_compose::LocalDraft,
    draft: Option<Draft>,
) {
    if s.editor.borrow().is_some() {
        s.info("A composer opened while recovery was loading. Finish it first.");
        return;
    }
    let mut compose = record.compose;
    let missing = compose.attachments.iter().any(|p| !p.is_file());
    // Keep missing paths visible: MIME validation prevents sending a draft that
    // silently lost a local attachment. The user can remove or reattach it.
    if let Some(account) = s
        .settings
        .borrow()
        .accounts
        .iter()
        .find(|a| a.id == record.account)
    {
        compose.from = account.email.clone();
    }
    make(s, compose, draft);
    if let Some(e) = s.editor.borrow().as_ref() {
        *e.local_id.borrow_mut() = record.id;
        e.revision.set(record.revision.saturating_add(1));
        e.saved.set(0);
        e.local_sequence.set(record.sequence);
        let buffer = e.text.buffer();
        for span in record.spans {
            if span.start < 0 || span.end > buffer.char_count() || span.end <= span.start {
                continue;
            }
            let tag = if let Some(tag) = buffer.tag_table().lookup(&span.tag) {
                Some(tag)
            } else if span.tag.starts_with("link:") {
                Some(link_tag(&buffer, span.tag.strip_prefix("link:").unwrap()))
            } else {
                None
            };
            if let Some(tag) = tag {
                buffer.apply_tag(
                    &tag,
                    &buffer.iter_at_offset(span.start),
                    &buffer.iter_at_offset(span.end),
                );
            }
        }
        e.uncertain.set(record.uncertain);
        if record.uncertain {
            e.send.set_sensitive(false);
            e.save.set_sensitive(false);
            e.status.set_text("Recovered after an uncertain request. Check Gmail Sent and Drafts before sending a new copy.");
        } else if missing {
            e.status.set_text("Recovered locally · An attachment has moved. Remove or reattach it before sending.");
        } else {
            e.status
                .set_text("Recovered locally · Save to Gmail when ready");
        }
    }
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
        .filter(|n| {
            matches!(n.as_str(), "strong" | "em" | "u" | "s")
                || n.starts_with("link:")
                || n.starts_with("font:")
        })
        .collect();
    names.sort();
    names
}
fn has_formatting(buffer: &gtk::TextBuffer) -> bool {
    if buffer
        .text(&buffer.start_iter(), &buffer.end_iter(), false)
        .lines()
        .any(|line| crate::compose_lists::prefix(line).is_some())
    {
        return true;
    }
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
    let mut output = String::from("<div data-luma-editor=\"1\" style=\"white-space:pre-wrap\">");
    let mut depth = 0;
    for n in 0..buffer.line_count() {
        let start = buffer.iter_at_line(n).unwrap_or_else(|| buffer.end_iter());
        let mut end = start;
        if !end.ends_line() {
            end.forward_to_line_end();
        }
        let text = buffer.text(&start, &end, false);
        if let Some((level, skip)) = crate::compose_lists::prefix(&text) {
            let target = (level + 1).min(depth + 1);
            if target > depth {
                while depth < target {
                    output.push_str("<ul><li>");
                    depth += 1;
                }
            } else {
                while depth > target {
                    output.push_str("</li></ul>");
                    depth -= 1;
                }
                output.push_str("</li><li>");
            }
            output.push_str(&formatted_range(
                buffer,
                start.offset() + skip,
                end.offset(),
            ));
        } else {
            while depth > 0 {
                output.push_str("</li></ul>");
                depth -= 1;
            }
            output.push_str(&formatted_range(buffer, start.offset(), end.offset()));
            if n + 1 < buffer.line_count() {
                output.push_str("<br>");
            }
        }
    }
    while depth > 0 {
        output.push_str("</li></ul>");
        depth -= 1;
    }
    output.push_str("</div>");
    output
}
fn formatted_range(buffer: &gtk::TextBuffer, begin: i32, end: i32) -> String {
    let mut output = String::new();
    let mut iter = buffer.iter_at_offset(begin);
    let mut previous: Vec<String> = Vec::new();
    while iter.offset() < end {
        let tags = formatting(&iter);
        if tags != previous {
            for tag in previous.iter().rev() {
                output.push_str(&format!(
                    "</{}>",
                    if tag.starts_with("link:") {
                        "a"
                    } else if tag.starts_with("font:") {
                        "span"
                    } else {
                        tag
                    }
                ));
            }
            for tag in &tags {
                if let Some(url) = tag.strip_prefix("link:") {
                    output.push_str(&format!("<a href=\"{}\">", escape(url)));
                } else if let Some(font) = fonts::open(tag) {
                    output.push_str(&font);
                } else {
                    output.push_str(&format!("<{tag}>"));
                }
            }
            previous = tags;
        }
        let ch = iter.char();
        if ch == '\n' {
            output.push_str("<br>");
        } else {
            output.push_str(&escape(&ch.to_string()));
        }
        iter.forward_char();
    }
    for tag in previous.iter().rev() {
        output.push_str(&format!(
            "</{}>",
            if tag.starts_with("link:") {
                "a"
            } else if tag.starts_with("font:") {
                "span"
            } else {
                tag
            }
        ));
    }
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
        let tag = link_tag(&buffer, url.as_str());
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
fn link_tag(buffer: &gtk::TextBuffer, url: &str) -> gtk::TextTag {
    let name = format!("link:{url}");
    buffer.tag_table().lookup(&name).unwrap_or_else(|| {
        let tag = gtk::TextTag::new(Some(&name));
        tag.set_underline(gtk::pango::Underline::Single);
        tag.set_foreground(Some("#4285f4"));
        buffer.tag_table().add(&tag);
        tag
    })
}
pub(crate) fn insert_named_link(text: &gtk::TextView, name: &str, url: &str, html: bool) {
    let buffer = text.buffer();
    if html {
        buffer.insert_at_cursor(&format!(
            "<p><a href=\"{}\">{}</a></p>",
            escape(url),
            escape(name)
        ));
    } else {
        buffer.begin_user_action();
        let block = buffer.cursor_position();
        buffer.insert_at_cursor(&format!("\n\n{name}\n"));
        let end = buffer.cursor_position() - 1;
        let start = block + 2;
        buffer.tag_table().foreach(|tag| {
            if tag.name().is_some_and(|name| name.starts_with("link:")) {
                buffer.remove_tag(
                    tag,
                    &buffer.iter_at_offset(block),
                    &buffer.iter_at_offset(end + 1),
                );
            }
        });
        buffer.apply_tag(
            &link_tag(&buffer, url),
            &buffer.iter_at_offset(start),
            &buffer.iter_at_offset(end),
        );
        buffer.end_user_action();
    }
}
fn has_visual_link(buffer: &gtk::TextBuffer, url: &str) -> bool {
    let Some(tag) = buffer.tag_table().lookup(&format!("link:{url}")) else {
        return false;
    };
    let mut iter = buffer.start_iter();
    iter.has_tag(&tag) || iter.forward_to_tag_toggle(Some(&tag))
}
fn highlight_links(e: &Editor) {
    let buffer = e.text.buffer();
    for tag in e.automatic_links.borrow_mut().drain(..) {
        buffer.tag_table().remove(&tag);
    }
    let text = buffer.text(&buffer.start_iter(), &buffer.end_iter(), false);
    if text.len() > 512 * 1024 {
        return;
    }
    for (start, end, url) in luma_core::html_mail::links(&text).into_iter().take(256) {
        let tag = gtk::TextTag::builder()
            .name(format!("auto-link:{url}"))
            .foreground("#4285f4")
            .underline(gtk::pango::Underline::Single)
            .build();
        // Repeated URLs can share one tag; do not accumulate tags while typing.
        let tag = if let Some(existing) = buffer.tag_table().lookup(&format!("auto-link:{url}")) {
            existing
        } else {
            buffer.tag_table().add(&tag);
            e.automatic_links.borrow_mut().push(tag.clone());
            tag
        };
        buffer.apply_tag(
            &tag,
            &buffer.iter_at_offset(text[..start].chars().count() as i32),
            &buffer.iter_at_offset(text[..end].chars().count() as i32),
        );
    }
}
fn snapshot(e: &Editor) -> Result<Compose> {
    snapshot_impl(e, true)
}
fn snapshot_impl(e: &Editor, mime: bool) -> Result<Compose> {
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
    if mime && !c.html && has_formatting(&b) {
        c.text = formatted_html(&b);
        c.html = true;
    }
    c.attachments = e.attachments.borrow().clone();
    if mime {
        c.retained = e.retained.borrow().clone();
    }
    c.drive_files = e.drive_files.borrow().clone();
    Ok(c)
}
fn persist(e: &Rc<Editor>, send: bool) {
    if e.conflict.get() {
        e.conflict_bar.set_visible(true);
        return;
    }
    if e.clipboard_busy.get() {
        e.status
            .set_text("Wait for the pasted image to finish attaching.");
        return;
    }
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
    save_local(e, false);
    let recent_account = a.id.clone();
    let recent = Compose {
        to: c.to.clone(),
        cc: c.cc.clone(),
        bcc: c.bcc.clone(),
        ..Default::default()
    };
    let existing = e.draft.borrow().clone();
    let drive_enabled = e
        .state
        .settings
        .borrow()
        .accounts
        .iter()
        .find(|account| account.id == a.id)
        .is_some_and(|a| a.drive_enabled);
    let cache = e.uploads.clone();
    let cancel = e.upload_cancel.clone();
    cancel.store(false, std::sync::atomic::Ordering::Relaxed);
    let (drive_progress, drive_rx) =
        tokio::sync::watch::channel(luma_core::compose_drive::Progress::default());
    let revision = e.revision.get();
    let editor = e.clone();
    e.busy.set(true);
    e.files.set_sensitive(false);
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
        let drive = drive_rx.borrow().clone();
        e.cancel_upload
            .set_visible(!drive.name.is_empty() && !drive.sharing);
        if drive.sharing {
            e.progress.pulse();
            e.progress.set_text(Some("Granting recipient access"));
            e.status.set_text(&format!(
                "Sharing {} with the message recipients…",
                drive.name
            ));
        } else if !drive.name.is_empty() {
            e.progress
                .set_fraction(drive.done as f64 / drive.total.max(1) as f64);
            let text = format!(
                "{} · {} / {} · {}%",
                drive.name,
                luma_core::preview::size(drive.done),
                luma_core::preview::size(drive.total),
                drive.done.saturating_mul(100) / drive.total.max(1)
            );
            e.progress.set_text(Some(&text));
            e.status
                .set_text("Uploading large attachment privately to Google Drive…");
        } else if stage.load(std::sync::atomic::Ordering::Relaxed) {
            e.progress.set_text(Some("Waiting for Gmail"));
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
            let c = match luma_core::compose_drive::prepare(
                c,
                &client,
                drive_enabled,
                cache,
                drive_progress.clone(),
                cancel,
            )
            .await
            {
                Ok(c) => c,
                Err(error) => return Ok((Err(error), None, false)),
            };
            let raw = match tokio::task::spawn_blocking(move || {
                let raw = c.mime()?;
                let mut sharing = c;
                sharing.attachments.clear();
                sharing.retained.clear();
                Ok::<_, Error>((raw, sharing.thread.clone(), sharing))
            })
            .await
            {
                Ok(v) => v,
                Err(_) => Err(Error::Invalid("MIME preparation failed".into())),
            };
            let (raw, thread, sharing) = match raw {
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
            if send
                && let Err(error) =
                    luma_core::compose_drive::share_for_send(&client, &sharing, &drive_progress)
                        .await
            {
                return Ok((Err(error), Some(draft), false));
            }
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
            editor.cancel_upload.set_visible(false);
            editor.files.set_sensitive(true);
            reconcile_drive_uploads(&editor);
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
                        editor
                            .local_closed
                            .store(true, std::sync::atomic::Ordering::SeqCst);
                        if let Err(error) =
                            editor.state.local_compose.remove(&editor.local_id.borrow())
                        {
                            editor.state.error(&error);
                        }
                        if editor.state.settings.borrow().recent_recipients {
                            let store = editor.state.local_compose.clone();
                            ui::async_work(
                                &editor.state,
                                async move {
                                    tokio::task::spawn_blocking(move || {
                                        store.remember(&recent_account, &recent)
                                    })
                                    .await
                                    .map_err(|_| {
                                        Error::Invalid("Cannot save recent recipients".into())
                                    })?
                                },
                                |_, ()| {},
                            );
                        }
                        editor.closing.set(true);
                        editor.send_reveal.set_reveal_child(false);
                        editor.state.toast("Sent · Confirmed by Gmail");
                        let delay = crate::presentation::duration(&editor.state, 280) as u64;
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
                        if !editor.state.settings.borrow().local_drafts {
                            // A recovered legacy copy is retired only after Gmail
                            // confirms saving this exact revision.
                            if editor.revision.get() == revision {
                                let _ =
                                    editor.state.local_compose.remove(&editor.local_id.borrow());
                            }
                        }
                        if editor.close_after_save.get() && editor.revision.get() == revision {
                            if editor.state.settings.borrow().local_drafts {
                                save_local(&editor, true);
                            } else {
                                finish(&editor);
                            }
                            return;
                        }
                        save_local(&editor, false);
                        editor.send.set_sensitive(true);
                        if editor.revision.get() != revision {
                            changed(&editor);
                        }
                    }
                }
                Err(err) => {
                    editor.status.set_text(&err.to_string());
                    if matches!(err, Error::DraftConflict) {
                        editor.conflict.set(true);
                        editor.conflict_bar.set_visible(true);
                        editor.close_after_save.set(false);
                        editor.send.set_sensitive(false);
                        editor
                            .save
                            .set_tooltip_text(Some("Save my version as a separate Gmail draft"));
                        if let Some(timer) = editor.debounce.borrow_mut().take() {
                            timer.remove();
                        }
                        return;
                    }
                    if uncertain {
                        // Google does not provide an idempotency key. An interrupted send may
                        // already have succeeded. Require reconciliation before another attempt.
                        editor.uncertain.set(true);
                        editor.save.set_sensitive(false);
                        editor.status.set_text(&format!("{err}. Saving and sending are locked for this window. Check Gmail Sent and Drafts before opening another composer."));
                    } else {
                        editor.send.set_sensitive(true);
                    }
                    save_local(&editor, editor.close_after_save.get());
                }
            }
        },
    );
}
fn save_conflict_copy(e: &Rc<Editor>) {
    if !e.conflict.get() || e.busy.get() || e.uncertain.get() {
        return;
    }
    // Never overwrite/delete the newer Gmail draft, and never send automatically.
    e.draft.borrow_mut().take();
    e.conflict.set(false);
    e.conflict_bar.set_visible(false);
    e.close_after_save.set(false);
    e.saved.set(0);
    e.save.set_tooltip_text(Some("Save draft to Gmail"));
    persist(e, false);
}
fn attach_files(e: &Rc<Editor>) {
    let dialog = gtk::FileDialog::builder()
        .title("Attach files · Large files use Google Drive")
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
fn reconcile_drive_uploads(e: &Rc<Editor>) {
    let Some(account) = e.accounts.get(e.from.selected() as usize) else {
        return;
    };
    let paths = e.attachments.borrow().clone();
    for path in paths {
        let Some(file) = luma_core::compose_drive::cached(&e.uploads, &account.id, &path) else {
            continue;
        };
        if !e.drive_files.borrow().iter().any(|f| f.id == file.id) {
            let Ok(link) = luma_core::drive::verified_link(&file) else {
                continue;
            };
            let buffer = e.text.buffer();
            let text = buffer.text(&buffer.start_iter(), &buffer.end_iter(), false);
            let source = Compose {
                text: text.to_string(),
                html: e.html.is_active(),
                ..Default::default()
            };
            if !has_visual_link(&buffer, &link)
                && !luma_core::compose_drive::contains_link(&source, &link)
            {
                let caret = buffer.create_mark(
                    None,
                    &buffer.iter_at_offset(buffer.cursor_position()),
                    false,
                );
                buffer.place_cursor(&buffer.end_iter());
                insert_named_link(&e.text, &file.name, &link, e.html.is_active());
                buffer.place_cursor(&buffer.iter_at_mark(&caret));
                buffer.delete_mark(&caret);
            }
            e.drive_files.borrow_mut().push(file);
        }
        e.attachments.borrow_mut().retain(|p| p != &path);
    }
    e.uploads.lock().expect("upload cache").clear();
    render_files(e);
}
fn remove_drive_link(e: &Rc<Editor>, file: &luma_core::drive::DriveFile) {
    if !e.html.is_active()
        && let Ok(link) = luma_core::drive::verified_link(file)
        && let Some(tag) = e.text.buffer().tag_table().lookup(&format!("link:{link}"))
    {
        let buffer = e.text.buffer();
        let mut start = buffer.start_iter();
        buffer.begin_user_action();
        while start.has_tag(&tag) || start.forward_to_tag_toggle(Some(&tag)) {
            let mut end = start;
            if !end.forward_to_tag_toggle(Some(&tag)) {
                end = buffer.end_iter();
            }
            buffer.delete(&mut start, &mut end);
        }
        buffer.end_user_action();
    }
    let mut block = Compose {
        html: true,
        ..Default::default()
    };
    if luma_core::compose_drive::append_link(&mut block, file).is_ok() {
        let buffer = e.text.buffer();
        let text = buffer.text(&buffer.start_iter(), &buffer.end_iter(), false);
        if let Some(index) = text.find(&block.text) {
            let start = text[..index].chars().count() as i32;
            let end = start + block.text.chars().count() as i32;
            buffer.begin_user_action();
            buffer.delete(
                &mut buffer.iter_at_offset(start),
                &mut buffer.iter_at_offset(end),
            );
            buffer.end_user_action();
        }
    }
    e.drive_files.borrow_mut().retain(|f| f.id != file.id);
    render_files(e);
    changed(e);
    e.status.set_text("Drive link removed from this draft. The uploaded file stays private or retains existing sharing in Drive.");
}
fn render_files(e: &Rc<Editor>) {
    use crate::attachments::{self, Source};
    use luma_core::preview::{AUTO_IMAGE_BYTES, Kind};
    ui::clear(&e.files);
    let count =
        e.retained.borrow().len() + e.attachments.borrow().len() + e.drive_files.borrow().len();
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
    for file in e.drive_files.borrow().clone() {
        let card = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        card.add_css_class("attachment-card");
        let icon = gtk::Image::from_icon_name("folder-remote-symbolic");
        card.append(&icon);
        let labels = vbox(3);
        let name = label(&file.name, "heading");
        name.set_ellipsize(gtk::pango::EllipsizeMode::Middle);
        name.set_max_width_chars(24);
        labels.append(&name);
        labels.append(&label("Google Drive · shared on Send", "caption"));
        card.append(&labels);
        let open = ui::button("external-link-symbolic", "Open Drive attachment");
        let state = e.state.clone();
        let link = luma_core::drive::verified_link(&file).ok();
        open.connect_clicked(move |_| {
            if let Some(link) = &link {
                crate::rendering::open_link(&state, link);
            }
        });
        card.append(&open);
        let remove = ui::button("window-close-symbolic", "Remove Drive link from draft");
        let weak = Rc::downgrade(e);
        remove.connect_clicked(move |_| {
            if let Some(e) = weak.upgrade() {
                remove_drive_link(&e, &file);
            }
        });
        card.append(&remove);
        flow.insert(&card, -1);
    }
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
    if e.conflict.get() {
        request_close(e);
        return;
    }
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
                            editor
                                .local_closed
                                .store(true, std::sync::atomic::Ordering::SeqCst);
                            let _ = editor.state.local_compose.remove(&editor.local_id.borrow());
                            finish(&editor);
                        }
                        Err(err) => editor.status.set_text(&err.to_string()),
                    }
                },
            );
        } else {
            e.local_closed
                .store(true, std::sync::atomic::Ordering::SeqCst);
            let _ = e.state.local_compose.remove(&e.local_id.borrow());
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

pub(crate) fn rescale(s: &Rc<State>) {
    s.compose_editors
        .borrow_mut()
        .retain(|e| e.strong_count() > 0);
    for e in s
        .compose_editors
        .borrow()
        .iter()
        .filter_map(|e| e.upgrade())
    {
        motion::resize(&e, s.window.width(), s.window.height());
        fonts::rescale(&e);
    }
}
pub(crate) fn resize_compact(s: &Rc<State>, width: i32, height: i32) {
    if let Some(e) = s.editor.borrow().as_ref() {
        motion::resize(e, width, height);
    }
}
