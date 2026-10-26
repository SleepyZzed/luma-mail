//! Desktop selection over already-loaded metadata. Never expands Gmail queries.
use super::*;
pub(super) fn update(s: &State) {
    if s.changing_selection.get() {
        return;
    }
    let count = keys(s).len();
    s.selection_tools
        .set_visible(s.list.selection_mode() == gtk::SelectionMode::Multiple);
    s.selection_count.set_text(&format!(
        "{count} selected · {} loaded",
        s.rows.borrow().len()
    ));
    fn visit(widget: &gtk::Widget, single: bool, labels: bool) {
        if widget.has_css_class("single-message-action") {
            widget.set_visible(single);
        }
        if widget.has_css_class("selected-label-action") {
            widget.set_visible(labels);
        }
        let mut child = widget.first_child();
        while let Some(current) = child {
            child = current.next_sibling();
            visit(&current, single, labels);
        }
    }
    let single = s
        .selected
        .borrow()
        .as_ref()
        .is_some_and(|key| single_target(s, key));
    visit(
        s.reader.upcast_ref(),
        single,
        count > 0 && s.selected.borrow().is_some(),
    );
}
pub(super) fn single_target(s: &State, target: &ThreadKey) -> bool {
    let keys = keys(s);
    if keys.is_empty() {
        s.list.selection_mode() != gtk::SelectionMode::Multiple
            && s.selected.borrow().as_ref() == Some(target)
    } else {
        keys.len() == 1 && &keys[0] == target
    }
}
pub(super) fn keys(s: &State) -> Vec<ThreadKey> {
    s.list
        .selected_rows()
        .iter()
        .filter_map(|row| rows::key(s, row))
        .collect()
}
pub(super) fn mode(s: &Rc<State>, active: bool, explicit: bool) {
    s.changing_selection.set(true);
    s.selection_explicit.set(active && explicit);
    s.select_button.set_active(active);
    s.list.unselect_all();
    s.list.set_selection_mode(if active {
        gtk::SelectionMode::Multiple
    } else {
        gtk::SelectionMode::Single
    });
    if active {
        s.list.add_css_class("selection-mode");
    } else {
        s.list.remove_css_class("selection-mode");
        if let Some(row) = s
            .selected
            .borrow()
            .as_ref()
            .and_then(|k| s.row_widgets.borrow().get(k).cloned())
        {
            s.list.select_row(Some(&row.row));
        }
    }
    s.changing_selection.set(false);
    update(s);
}
pub(super) fn all(s: &Rc<State>) {
    if s.list.selection_mode() != gtk::SelectionMode::Multiple {
        mode(s, true, true);
    }
    s.selection_explicit.set(true);
    s.changing_selection.set(true);
    for row in s.row_widgets.borrow().values() {
        if row.row.is_sensitive() {
            s.list.select_row(Some(&row.row));
        }
    }
    s.changing_selection.set(false);
    update(s);
}
pub(super) fn click(
    s: &Rc<State>,
    row: &gtk::ListBoxRow,
    modifiers: gtk::gdk::ModifierType,
) -> bool {
    let control = modifiers.contains(gtk::gdk::ModifierType::CONTROL_MASK);
    let shift = modifiers.contains(gtk::gdk::ModifierType::SHIFT_MASK);
    let Some(target) = rows::key(s, row) else {
        return false;
    };
    if !control && !shift && s.list.selection_mode() != gtk::SelectionMode::Multiple {
        *s.selection_anchor.borrow_mut() = Some(target);
        return false;
    }
    if !control && !shift && !s.selection_explicit.get() {
        mode(s, false, false);
        s.changing_selection.set(true);
        s.list.unselect_all();
        s.list.select_row(Some(row));
        s.changing_selection.set(false);
        *s.selection_anchor.borrow_mut() = Some(target.clone());
        open_thread(s, target);
        return true;
    }
    if s.list.selection_mode() != gtk::SelectionMode::Multiple {
        let previous = keys(s);
        mode(s, true, false);
        s.changing_selection.set(true);
        for key in previous {
            if let Some(w) = s.row_widgets.borrow().get(&key) {
                s.list.select_row(Some(&w.row));
            }
        }
        s.changing_selection.set(false);
    }
    s.changing_selection.set(true);
    if shift {
        let anchor = s
            .selection_anchor
            .borrow()
            .clone()
            .or_else(|| s.selected.borrow().clone());
        let index = anchor
            .as_ref()
            .and_then(|k| s.row_widgets.borrow().get(k).map(|w| w.row.index()))
            .unwrap_or(row.index());
        if !control {
            s.list.unselect_all();
        }
        for index in index.min(row.index())..=index.max(row.index()) {
            if let Some(row) = s.list.row_at_index(index)
                && row.is_sensitive()
            {
                s.list.select_row(Some(&row));
            }
        }
        if s.selection_anchor.borrow().is_none() {
            *s.selection_anchor.borrow_mut() = Some(target);
        }
    } else {
        if row.is_selected() {
            s.list.unselect_row(row);
        } else {
            s.list.select_row(Some(row));
        }
        *s.selection_anchor.borrow_mut() = Some(target);
    }
    s.changing_selection.set(false);
    update(s);
    true
}
pub(super) fn install(s: &Rc<State>) {
    let weak = Rc::downgrade(s);
    s.list.connect_selected_rows_changed(move |_| {
        if let Some(s) = weak.upgrade() {
            update(&s);
        }
    });
    let keyboard = gtk::EventControllerKey::new();
    keyboard.set_propagation_phase(gtk::PropagationPhase::Capture);
    let weak = Rc::downgrade(s);
    keyboard.connect_key_pressed(move |_, key, _, mods| {
        let Some(s) = weak.upgrade() else {
            return glib::Propagation::Proceed;
        };
        if key == gtk::gdk::Key::a && mods.contains(gtk::gdk::ModifierType::CONTROL_MASK) {
            all(&s);
            glib::Propagation::Stop
        } else if key == gtk::gdk::Key::Escape
            && s.list.selection_mode() == gtk::SelectionMode::Multiple
        {
            mode(&s, false, false);
            glib::Propagation::Stop
        } else {
            glib::Propagation::Proceed
        }
    });
    s.list.add_controller(keyboard);
}

#[cfg(debug_assertions)]
pub(super) fn diagnostic(s: &Rc<State>) {
    assert!(s.clients.borrow().is_empty() && s.rows.borrow().len() >= 5);
    let row = |n| s.list.row_at_index(n).unwrap();
    let first = rows::key(s, &row(0)).unwrap();
    *s.selected.borrow_mut() = Some(first.clone());
    mode(s, false, false);
    let marker = button(
        "mail-reply-sender-symbolic",
        "Single message action fixture",
    );
    marker.add_css_class("single-message-action");
    s.reader.append(&marker);
    assert!(click(s, &row(2), gtk::gdk::ModifierType::CONTROL_MASK));
    assert_eq!(keys(s).len(), 2);
    assert!(s.select_button.is_active());
    assert!(!marker.is_visible());
    assert!(!single_target(s, &first));
    let windows = s.app.windows().len();
    crate::sender_controls::block(s, "design-fixture", "nobody@example.invalid");
    crate::compose::open(s, Some((Message::default(), 0)), None);
    assert_eq!(s.app.windows().len(), windows);
    assert!(s.editor.borrow().is_none());
    assert!(click(s, &row(2), gtk::gdk::ModifierType::CONTROL_MASK));
    assert_eq!(keys(s).len(), 1);
    assert!(marker.is_visible());
    click(s, &row(2), gtk::gdk::ModifierType::CONTROL_MASK);
    click(s, &row(4), gtk::gdk::ModifierType::SHIFT_MASK);
    assert_eq!(keys(s).len(), 3);
    assert!(!row(0).is_selected());
    assert!(row(2).is_selected() && row(4).is_selected());
    click(s, &row(1), gtk::gdk::ModifierType::SHIFT_MASK);
    assert_eq!(keys(s).len(), 2);
    assert!(row(1).is_selected() && row(2).is_selected());
    all(s);
    assert_eq!(keys(s).len(), s.rows.borrow().len());
    rows::menu(s, &row(0), &first, 12., 12.);
    fn labels(widget: &gtk::Widget, out: &mut Vec<String>) {
        if let Some(label) = widget.downcast_ref::<gtk::Label>() {
            out.push(label.text().to_string());
        }
        let mut child = widget.first_child();
        while let Some(current) = child {
            child = current.next_sibling();
            labels(&current, out);
        }
    }
    let mut text = Vec::new();
    labels(row(0).upcast_ref(), &mut text);
    assert!(text.iter().any(|t| t == "Mark as read") && text.iter().any(|t| t == "Mark as unread"));
    assert!(
        !text
            .iter()
            .any(|t| t == "Block sender…" || t == "Open conversation")
    );
    if let Some(pop) = row(0)
        .last_child()
        .and_then(|w| w.downcast::<gtk::Popover>().ok())
    {
        pop.popdown();
    }
    s.changing_selection.set(true);
    s.list.unselect_all();
    s.changing_selection.set(false);
    update(s);
    act_selected(s, MailAction::Read);
    assert!(
        !s.operation_busy.get(),
        "an empty bulk selection must not act on the opened message"
    );
    s.reader.remove(&marker);
    mode(s, false, false);
    println!(
        "PASS Ctrl toggle, Shift range, loaded-only Select all, safe empty selection and incompatible action guards/menu"
    );
}
