//! Rich editing history in bounded RAM only. No checkpoints on disk.
use super::*;
use std::collections::VecDeque;
const BUDGET: usize = 4 * 1024 * 1024;
const ENTRY_LIMIT: usize = 1024 * 1024;
#[derive(Clone)]
struct Entry {
    body: String,
    source: bool,
    cursor: i32,
    anchor: i32,
    font: (Option<String>, Option<u16>),
}
impl Entry {
    fn bytes(&self) -> usize {
        self.body.capacity() + self.font.0.as_ref().map_or(0, String::capacity) + 128
    }
    fn same_document(&self, other: &Self) -> bool {
        self.body == other.body && self.source == other.source && self.font == other.font
    }
}
#[derive(Default)]
pub(super) struct History {
    current: RefCell<Option<Entry>>,
    undo: RefCell<VecDeque<Entry>>,
    redo: RefCell<VecDeque<Entry>>,
    pending: RefCell<Option<glib::SourceId>>,
    pub(super) restoring: Cell<bool>,
    actions: RefCell<Option<(gtk::gio::SimpleAction, gtk::gio::SimpleAction)>>,
    buttons: RefCell<Option<(gtk::Button, gtk::Button)>>,
}
fn take(e: &Editor, source: bool) -> Option<Entry> {
    let buffer = e.text.buffer();
    let body = if source {
        buffer
            .text(&buffer.start_iter(), &buffer.end_iter(), false)
            .to_string()
    } else {
        formatted_html(&buffer)
    };
    if body.len() > ENTRY_LIMIT || (!source && luma_core::rich_draft::parse(&body).is_none()) {
        return None;
    }
    let cursor = buffer.cursor_position();
    let anchor = buffer
        .selection_bounds()
        .map(|(a, z)| {
            if a.offset() == cursor {
                z.offset()
            } else {
                a.offset()
            }
        })
        .unwrap_or(cursor);
    Some(Entry {
        body,
        source,
        cursor,
        anchor,
        font: e.typing_font.borrow().clone(),
    })
}
fn trim(history: &History) {
    let current = history.current.borrow().as_ref().map_or(0, Entry::bytes);
    loop {
        let bytes: usize = history
            .undo
            .borrow()
            .iter()
            .chain(history.redo.borrow().iter())
            .map(Entry::bytes)
            .sum();
        if bytes + current <= BUDGET
            && history.undo.borrow().len() + history.redo.borrow().len() <= 100
        {
            break;
        }
        if history.undo.borrow_mut().pop_front().is_none() {
            history.redo.borrow_mut().pop_front();
        }
    }
}
fn sync(e: &Editor) {
    let undo = !e.history.undo.borrow().is_empty() || e.history.pending.borrow().is_some();
    let redo = !e.history.redo.borrow().is_empty();
    if let Some((a, b)) = e.history.actions.borrow().as_ref() {
        a.set_enabled(undo);
        b.set_enabled(redo);
    }
    if let Some((a, b)) = e.history.buttons.borrow().as_ref() {
        a.set_sensitive(undo);
        b.set_sensitive(redo);
    }
}
pub(super) fn cancel_pending(e: &Editor) {
    if let Some(timer) = e.history.pending.borrow_mut().take() {
        timer.remove();
    }
}
pub(super) fn checkpoint(e: &Editor) {
    checkpoint_mode(e, e.html.is_active());
}
pub(super) fn checkpoint_mode(e: &Editor, source: bool) {
    if e.history.restoring.get() {
        return;
    }
    if let Some(timer) = e.history.pending.borrow_mut().take() {
        timer.remove();
    }
    let Some(next) = take(e, source) else {
        e.history.current.borrow_mut().take();
        e.history.undo.borrow_mut().clear();
        e.history.redo.borrow_mut().clear();
        sync(e);
        return;
    };
    if let Some(previous) = e.history.current.borrow_mut().replace(next.clone())
        && !previous.same_document(&next)
    {
        e.history.undo.borrow_mut().push_back(previous);
        e.history.redo.borrow_mut().clear();
    }
    trim(&e.history);
    sync(e);
}
fn observe(e: &Rc<Editor>) {
    if e.history.restoring.get() || e.sent.get() || e.closing.get() {
        return;
    }
    if let Some(timer) = e.history.pending.borrow_mut().take() {
        timer.remove();
    }
    e.history.redo.borrow_mut().clear();
    let weak = Rc::downgrade(e);
    *e.history.pending.borrow_mut() = Some(glib::timeout_add_local_once(
        Duration::from_millis(350),
        move || {
            if let Some(e) = weak.upgrade() {
                e.history.pending.borrow_mut().take();
                checkpoint(&e);
            }
        },
    ));
    sync(e);
}
fn restore(e: &Editor, entry: &Entry) {
    e.history.restoring.set(true);
    e.html.set_active(entry.source);
    let buffer = e.text.buffer();
    if entry.source {
        buffer.set_text(&entry.body);
    } else if let Some(rich) = luma_core::rich_draft::parse(&entry.body) {
        buffer.set_text(&rich.text);
        for span in rich.spans {
            let tag = if let Some(family) = span.tag.strip_prefix("font:family:") {
                Some(fonts::family_tag(&buffer, family))
            } else if let Some(size) = span
                .tag
                .strip_prefix("font:size:")
                .and_then(|v| v.parse().ok())
            {
                Some(fonts::size_tag(e, size))
            } else if let Some(url) = span.tag.strip_prefix("link:") {
                Some(link_tag(&buffer, url))
            } else {
                buffer.tag_table().lookup(&span.tag)
            };
            if let Some(tag) = tag {
                buffer.apply_tag(
                    &tag,
                    &buffer.iter_at_offset(span.start),
                    &buffer.iter_at_offset(span.end),
                );
            }
        }
    }
    let end = buffer.char_count();
    buffer.select_range(
        &buffer.iter_at_offset(entry.cursor.clamp(0, end)),
        &buffer.iter_at_offset(entry.anchor.clamp(0, end)),
    );
    *e.typing_font.borrow_mut() = entry.font.clone();
    fonts::sync_controls(e);
    e.history.restoring.set(false);
}
pub(super) fn undo(e: &Rc<Editor>, redo: bool) {
    if e.sent.get() || e.closing.get() || (e.busy.get() && !e.form.is_sensitive()) {
        return;
    }
    checkpoint(e);
    let previous = if redo {
        e.history.redo.borrow_mut().pop_back()
    } else {
        e.history.undo.borrow_mut().pop_back()
    };
    let Some(previous) = previous else {
        return;
    };
    if let Some(current) = e.history.current.borrow_mut().replace(previous.clone()) {
        if redo {
            e.history.undo.borrow_mut().push_back(current);
        } else {
            e.history.redo.borrow_mut().push_back(current);
        }
    }
    restore(e, &previous);
    trim(&e.history);
    sync(e);
    changed(e);
    e.text.grab_focus();
}
fn menu(e: &Rc<Editor>, x: f64, y: f64) {
    let model = gtk::gio::Menu::new();
    for (title, action) in [
        ("Undo", "undo"),
        ("Redo", "redo"),
        ("Cut", "cut"),
        ("Copy", "copy"),
        ("Paste", "paste"),
        ("Select all", "select-all"),
    ] {
        model.append(Some(title), Some(&format!("compose-edit.{action}")));
    }
    let pop = gtk::PopoverMenu::from_model(Some(&model));
    pop.set_has_arrow(false);
    pop.set_parent(&e.text);
    pop.set_pointing_to(Some(&gtk::gdk::Rectangle::new(x as i32, y as i32, 1, 1)));
    pop.connect_closed(|p| p.unparent());
    pop.popup();
}
pub(super) fn install(e: &Rc<Editor>, toolbar: &gtk::FlowBox) {
    e.text.buffer().set_enable_undo(false);
    *e.history.current.borrow_mut() = take(e, e.html.is_active());
    let group = gtk::gio::SimpleActionGroup::new();
    let undo = gtk::gio::SimpleAction::new("undo", None);
    let redo = gtk::gio::SimpleAction::new("redo", None);
    for (action, direction) in [(&undo, false), (&redo, true)] {
        let weak = Rc::downgrade(e);
        action.connect_activate(move |_, _| {
            if let Some(e) = weak.upgrade() {
                self::undo(&e, direction);
            }
        });
        group.add_action(action);
    }
    for name in ["cut", "copy", "paste", "select-all"] {
        let action = gtk::gio::SimpleAction::new(name, None);
        let weak = Rc::downgrade(e);
        action.connect_activate(move |_, _| {
            if let Some(e) = weak.upgrade() {
                let b = e.text.buffer();
                match name {
                    "cut" => b.cut_clipboard(&e.text.clipboard(), true),
                    "copy" => b.copy_clipboard(&e.text.clipboard()),
                    "paste" => e.text.emit_by_name::<()>("paste-clipboard", &[]),
                    _ => b.select_range(&b.start_iter(), &b.end_iter()),
                }
            }
        });
        group.add_action(&action);
    }
    e.text.insert_action_group("compose-edit", Some(&group));
    let back = ui::button("luma-undo-symbolic", "Undo text or formatting (Ctrl+Z)");
    let forward = ui::button(
        "luma-redo-symbolic",
        "Redo text or formatting (Ctrl+Shift+Z)",
    );
    // Buttons are outside the text view, so bind their callbacks directly.
    for (button, direction) in [(&back, false), (&forward, true)] {
        let weak = Rc::downgrade(e);
        button.connect_clicked(move |_| {
            if let Some(e) = weak.upgrade() {
                self::undo(&e, direction);
            }
        });
        button.add_css_class("flat");
        button.set_focus_on_click(false);
    }
    toolbar.insert(&back, 1);
    toolbar.insert(&forward, 2);
    *e.history.actions.borrow_mut() = Some((undo, redo));
    *e.history.buttons.borrow_mut() = Some((back, forward));
    sync(e);
    let weak = Rc::downgrade(e);
    e.text.buffer().connect_changed(move |_| {
        if let Some(e) = weak.upgrade() {
            observe(&e);
        }
    });
    for signal in ["apply-tag", "remove-tag"] {
        let weak = Rc::downgrade(e);
        e.text.buffer().connect_local(signal, true, move |values| {
            if let Some(e) = weak.upgrade()
                && let Ok(tag) = values[1].get::<gtk::TextTag>()
                && tag.name().is_some_and(|n| {
                    matches!(n.as_str(), "strong" | "em" | "u" | "s")
                        || n.starts_with("font:")
                        || n.starts_with("link:")
                })
            {
                observe(&e);
            }
            None
        });
    }
    let keys = gtk::EventControllerKey::new();
    keys.set_propagation_phase(gtk::PropagationPhase::Capture);
    let weak = Rc::downgrade(e);
    keys.connect_key_pressed(move |_, key, _, mods| {
        let Some(e) = weak.upgrade() else {
            return glib::Propagation::Proceed;
        };
        if key == gtk::gdk::Key::Menu
            || (key == gtk::gdk::Key::F10 && mods.contains(gtk::gdk::ModifierType::SHIFT_MASK))
        {
            menu(&e, 12., 12.);
            return glib::Propagation::Stop;
        }
        if mods.contains(gtk::gdk::ModifierType::CONTROL_MASK)
            && matches!(
                key,
                gtk::gdk::Key::z | gtk::gdk::Key::Z | gtk::gdk::Key::y | gtk::gdk::Key::Y
            )
        {
            self::undo(
                &e,
                matches!(key, gtk::gdk::Key::y | gtk::gdk::Key::Y)
                    || mods.contains(gtk::gdk::ModifierType::SHIFT_MASK),
            );
            return glib::Propagation::Stop;
        }
        glib::Propagation::Proceed
    });
    e.text.add_controller(keys);
    let gesture = gtk::GestureClick::new();
    gesture.set_button(3);
    gesture.set_propagation_phase(gtk::PropagationPhase::Capture);
    let weak = Rc::downgrade(e);
    gesture.connect_pressed(move |g, _, x, y| {
        if let Some(e) = weak.upgrade() {
            menu(&e, x, y);
            g.set_state(gtk::EventSequenceState::Claimed);
        }
    });
    e.text.add_controller(gesture);
}

#[cfg(debug_assertions)]
pub(super) fn verify(e: &Rc<Editor>) {
    let b = e.text.buffer();
    b.set_text("Alpha βeta");
    checkpoint(e);
    b.select_range(&b.iter_at_offset(6), &b.end_iter());
    checkpoint(e);
    b.apply_tag(
        &b.tag_table().lookup("strong").unwrap(),
        &b.iter_at_offset(6),
        &b.end_iter(),
    );
    b.apply_tag(
        &fonts::family_tag(&b, "Serif"),
        &b.iter_at_offset(6),
        &b.end_iter(),
    );
    checkpoint(e);
    b.delete(&mut b.iter_at_offset(6), &mut b.end_iter());
    checkpoint(e);
    undo(e, false);
    assert_eq!(b.text(&b.start_iter(), &b.end_iter(), false), "Alpha βeta");
    assert!(
        b.iter_at_offset(6)
            .has_tag(&b.tag_table().lookup("strong").unwrap())
    );
    assert!(b.iter_at_offset(6).has_tag(&fonts::family_tag(&b, "Serif")));
    undo(e, false);
    assert!(
        !b.iter_at_offset(6)
            .has_tag(&b.tag_table().lookup("strong").unwrap())
    );
    undo(e, true);
    assert!(
        b.iter_at_offset(6)
            .has_tag(&b.tag_table().lookup("strong").unwrap())
    );
    println!(
        "PASS rich undo/redo restores deleted Unicode text, font and formatting with selection"
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn history_releases_oldest_states_at_its_ram_and_count_limits() {
        let history = History::default();
        let entry = Entry {
            body: "x".repeat(150_000),
            source: true,
            cursor: 0,
            anchor: 0,
            font: (None, None),
        };
        *history.current.borrow_mut() = Some(entry.clone());
        history
            .undo
            .borrow_mut()
            .extend(std::iter::repeat_n(entry.clone(), 120));
        trim(&history);
        let bytes: usize = history
            .undo
            .borrow()
            .iter()
            .map(Entry::bytes)
            .sum::<usize>()
            + entry.bytes();
        assert!(bytes <= BUDGET);
        assert!(history.undo.borrow().len() <= 100);
    }
}
