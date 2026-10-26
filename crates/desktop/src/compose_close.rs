//! Close-time draft policy. Typing and attachment callbacks never create drafts.
use super::*;
use luma_core::config::DraftMode;

pub(super) fn has_body(e: &Editor) -> bool {
    let b = e.text.buffer();
    let text = b.text(&b.start_iter(), &b.end_iter(), false);
    if e.html.is_active() {
        luma_core::composer::html_text(&text).is_ok_and(|text| !text.trim().is_empty())
    } else {
        !text.trim().is_empty()
    }
}
fn recipient(e: &Editor) -> bool {
    [&e.to, &e.cc, &e.bcc].iter().any(|field| {
        let value = recipients::value(e, field);
        !value.trim().is_empty()
            && luma_core::composer::recipients(&value).is_ok_and(|list| !list.is_empty())
    })
}
pub(super) fn request(e: &Rc<Editor>) -> glib::Propagation {
    let mode = e.state.settings.borrow().draft_mode;
    if mode == DraftMode::Manual {
        return glib::Propagation::Proceed;
    }
    if !has_body(e)
        && e.subject.text().trim().is_empty()
        && [&e.to, &e.cc, &e.bcc]
            .iter()
            .all(|field| recipients::value(e, field).trim().is_empty())
        && e.attachments.borrow().is_empty()
        && e.retained.borrow().is_empty()
        && e.drive_files.borrow().is_empty()
    {
        return glib::Propagation::Proceed;
    }
    if mode == DraftMode::OnClose {
        if mode.saves_on_close(recipient(e), has_body(e)) {
            e.close_after_save.set(true);
            persist(e, false);
            return glib::Propagation::Stop;
        }
        return glib::Propagation::Proceed;
    }
    if e.close_dialog.replace(true) {
        return glib::Propagation::Stop;
    }
    e.close_after_save.set(false);
    let dialog = adw::AlertDialog::builder()
        .heading("Save this draft before closing?")
        .body("Save your current changes to Gmail, or close without saving. Any previously saved Gmail draft will remain unchanged.")
        .build();
    dialog.add_responses(&[
        ("cancel", "Keep editing"),
        ("close", "Close without saving"),
        ("save", "Save draft"),
    ]);
    dialog.set_default_response(Some("cancel"));
    dialog.set_close_response("cancel");
    dialog.set_response_appearance("save", adw::ResponseAppearance::Suggested);
    let e = e.clone();
    glib::MainContext::default().spawn_local(async move {
        let response = dialog.choose_future(Some(&e.parent())).await;
        e.close_dialog.set(false);
        if e.closing.get() || e.sent.get() {
            return;
        }
        match response.as_str() {
            "save" => {
                e.close_after_save.set(true);
                persist(&e, false);
            }
            "close" => finish(&e),
            _ => (),
        }
    });
    glib::Propagation::Stop
}
pub(super) fn resume(e: &Rc<Editor>) {
    if e.close_pending.replace(false) && request_close(e) == glib::Propagation::Proceed {
        finish(e);
    }
}

#[cfg(debug_assertions)]
pub(super) fn verify(s: &Rc<State>) {
    let state = s.clone();
    glib::MainContext::default().spawn_local(async move {
        let previous = state.settings.borrow().draft_mode;
        let reduced = state.settings.borrow().reduced_motion;
        state.settings.borrow_mut().reduced_motion = true;
        make(&state, Compose::new("alex@example.com".into()), None);
        let e = state.editor.borrow().as_ref().unwrap().clone();
        e.text.buffer().set_text("Unsaved policy test");
        assert!(e.debounce.borrow().is_none(), "Typing never schedules a Gmail save");
        state.settings.borrow_mut().draft_mode = DraftMode::Manual;
        assert_eq!(request(&e), glib::Propagation::Proceed);
        assert!(!e.close_after_save.get() && e.draft.borrow().is_none());
        state.settings.borrow_mut().draft_mode = DraftMode::OnClose;
        assert_eq!(request(&e), glib::Propagation::Proceed, "A body without a recipient is not automatically saved");
        state.settings.borrow_mut().draft_mode = DraftMode::Ask;
        assert_eq!(request(&e), glib::Propagation::Stop);
        assert_eq!(request(&e), glib::Propagation::Stop);
        glib::timeout_future(Duration::from_millis(80)).await;
        let dialog = state.window.visible_dialog().unwrap().downcast::<adw::AlertDialog>().unwrap();
        assert_eq!(dialog.heading().as_deref(), Some("Save this draft before closing?"));
        dialog.emit_by_name::<()>("response", &[&"cancel"]);
        glib::timeout_future(Duration::from_millis(40)).await;
        assert!(!e.close_dialog.get() && !e.closing.get() && e.draft.borrow().is_none());
        assert!(!e.close_after_save.get(), "Keep editing does not save or arm a late close");
        assert_eq!(request(&e), glib::Propagation::Stop);
        glib::timeout_future(Duration::from_millis(40)).await;
        let dialog = state.window.visible_dialog().unwrap().downcast::<adw::AlertDialog>().unwrap();
        dialog.emit_by_name::<()>("response", &[&"close"]);
        glib::timeout_future(Duration::from_millis(40)).await;
        assert!(e.closing.get() && e.draft.borrow().is_none());
        state.settings.borrow_mut().draft_mode = previous;
        state.settings.borrow_mut().reduced_motion = reduced;
        println!("PASS Ask/Keep editing/Close without saving, duplicate-close guard, Manual and incomplete On-close policies; no automatic Gmail/local drafts");
        state.app.quit();
    });
}
