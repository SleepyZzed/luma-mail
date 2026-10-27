//! Preserve supported rich drafts when toggling source; ask before a lossy conversion.
use super::*;
fn apply(e: &Rc<Editor>, text: &str, rich: Option<luma_core::rich_draft::RichDraft>) {
    e.history.restoring.set(true);
    let b = e.text.buffer();
    b.set_text(text);
    if let Some(rich) = rich {
        for span in rich.spans {
            if let Some(tag) = styles::resolve(e, &span.tag) {
                b.apply_tag(
                    &tag,
                    &b.iter_at_offset(span.start),
                    &b.iter_at_offset(span.end),
                );
            }
        }
    }
    e.html.set_active(false);
    e.styling.typing.borrow_mut().clear();
    *e.typing_font.borrow_mut() = (None, None);
    e.history.restoring.set(false);
    changed(e);
    styles::sync(e);
    fonts::sync_controls(e);
    highlight_links(e);
    history::checkpoint(e);
}
pub(super) fn changed_mode(e: &Rc<Editor>, source: bool) {
    if e.history.restoring.get() {
        return;
    }
    history::checkpoint_mode(e, !source);
    let b = e.text.buffer();
    if source {
        let html = formatted_html(&b);
        b.set_text(&html);
        changed(e);
        history::checkpoint(e);
        return;
    }
    let source = b.text(&b.start_iter(), &b.end_iter(), false).to_string();
    if let Some(rich) = luma_core::rich_draft::parse(&source) {
        let text = rich.text.clone();
        apply(e, &text, Some(rich));
        return;
    }
    // Keep source mode truthful while the user decides. A cancelled confirmation
    // must never turn raw markup into an accidentally sent plain-text message.
    e.history.restoring.set(true);
    e.html.set_active(true);
    e.history.restoring.set(false);
    let dialog=adw::AlertDialog::builder().heading("Convert this HTML to plain text?").body("This message contains HTML the native editor cannot represent. Converting removes its layout and rich formatting. Keep HTML to preserve the original content.").build();
    dialog.add_responses(&[("keep", "Keep HTML"), ("convert", "Convert to plain text")]);
    dialog.set_default_response(Some("keep"));
    dialog.set_close_response("keep");
    dialog.set_response_appearance("convert", adw::ResponseAppearance::Destructive);
    let weak = Rc::downgrade(e);
    let parent = e.parent();
    glib::MainContext::default().spawn_local(async move {
        if dialog.choose_future(Some(&parent)).await == "convert"
            && let Some(e) = weak.upgrade()
            && !e.closing.get()
        {
            match luma_core::composer::html_text(&source) {
                Ok(text) => apply(&e, &text, None),
                Err(error) => e.status.set_text(&error.to_string()),
            }
        }
    });
}
#[cfg(debug_assertions)]
pub(super) fn verify(e: &Rc<Editor>) {
    let b = e.text.buffer();
    b.set_text("Source round trip");
    let (a, z) = b.bounds();
    b.select_range(&a, &z);
    styles::set(e, "style:color:#2456ab");
    let before = formatted_html(&b);
    e.html.set_active(true);
    e.html.set_active(false);
    assert_eq!(formatted_html(&b), before);
    assert!(!e.html.is_active());
    println!(
        "PASS source/visual round trip preserves rich colour metadata without silent HTML flattening"
    );
}
