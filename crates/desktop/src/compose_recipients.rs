use super::*;
use std::collections::HashMap;
struct Field {
    addresses: Vec<String>,
    flow: gtk::FlowBox,
    scroll: gtk::ScrolledWindow,
}
#[derive(Default)]
pub(super) struct Chips {
    fields: RefCell<HashMap<usize, Field>>,
}
fn key(field: &gtk::Entry) -> usize {
    field.as_ptr() as usize
}
pub(super) fn value(e: &Editor, field: &gtk::Entry) -> String {
    let mut addresses = e
        .chips
        .fields
        .borrow()
        .get(&key(field))
        .map(|f| f.addresses.clone())
        .unwrap_or_default();
    let text = field.text();
    if !text.trim().is_empty() {
        addresses.push(text.trim().to_owned());
    }
    addresses.join(", ")
}
fn render(e: &Rc<Editor>, entry: &gtk::Entry) {
    let fields = e.chips.fields.borrow();
    let Some(field) = fields.get(&key(entry)) else {
        return;
    };
    while let Some(child) = field.flow.first_child() {
        field.flow.remove(&child);
    }
    field.scroll.set_visible(!field.addresses.is_empty());
    for (index, address) in field.addresses.iter().enumerate() {
        let chip = gtk::Box::new(gtk::Orientation::Horizontal, 2);
        chip.add_css_class("recipient-chip");
        let name = label(address, "");
        name.set_ellipsize(gtk::pango::EllipsizeMode::End);
        name.set_max_width_chars(28);
        name.set_tooltip_text(Some(address));
        let remove = ui::button("luma-close-symbolic", &format!("Remove {address}"));
        remove.add_css_class("flat");
        chip.append(&name);
        chip.append(&remove);
        field.flow.insert(&chip, -1);
        let weak = Rc::downgrade(e);
        let input = entry.downgrade();
        remove.connect_clicked(move |_| {
            if let (Some(e), Some(entry)) = (weak.upgrade(), input.upgrade()) {
                if e.closing.get() {
                    return;
                }
                if let Some(field) = e.chips.fields.borrow_mut().get_mut(&key(&entry))
                    && index < field.addresses.len()
                {
                    field.addresses.remove(index);
                }
                render(&e, &entry);
                changed(&e);
                entry.grab_focus();
            }
        });
    }
}
fn commit(e: &Rc<Editor>, entry: &gtk::Entry, explicit: bool) -> bool {
    if e.field_sync.get() || e.closing.get() {
        return false;
    }
    let text = entry.text();
    let (ready, rest) = if explicit {
        (text.as_str(), "")
    } else {
        fragment(&text)
    };
    if ready.trim().is_empty() {
        return false;
    }
    let Ok(addresses) = luma_core::composer::recipients(ready) else {
        if explicit {
            entry.add_css_class("error");
            entry.set_tooltip_text(Some("Enter a complete email address"));
        }
        return false;
    };
    if addresses.is_empty() {
        return false;
    }
    let mut fields = e.chips.fields.borrow_mut();
    let Some(field) = fields.get_mut(&key(entry)) else {
        return false;
    };
    if field.addresses.len() + addresses.len() > 100 {
        e.status.set_text("Use at most 100 recipients per field.");
        return false;
    }
    for address in addresses {
        let email = luma_core::sender_rules::address(&address).unwrap_or_default();
        if !field
            .addresses
            .iter()
            .any(|a| luma_core::sender_rules::address(a).ok().as_deref() == Some(&email))
        {
            field.addresses.push(address);
        }
    }
    drop(fields);
    e.field_sync.set(true);
    entry.set_text(rest);
    entry.set_position(-1);
    entry.remove_css_class("error");
    e.field_sync.set(false);
    render(e, entry);
    changed(e);
    true
}
pub(super) fn commit_typed(e: &Rc<Editor>, entry: &gtk::Entry) -> bool {
    commit(e, entry, true)
}
fn install_chips(e: &Rc<Editor>, entry: &gtk::Entry) {
    let Some(parent) = entry.parent().and_downcast::<gtk::Box>() else {
        return;
    };
    let holder = vbox(2);
    holder.set_hexpand(true);
    let flow = gtk::FlowBox::builder()
        .selection_mode(gtk::SelectionMode::None)
        .column_spacing(4)
        .row_spacing(4)
        .max_children_per_line(4)
        .build();
    let scroll = gtk::ScrolledWindow::builder()
        .child(&flow)
        .hscrollbar_policy(gtk::PolicyType::Never)
        .max_content_height(88)
        .propagate_natural_height(true)
        .build();
    scroll.set_visible(false);
    holder.append(&scroll);
    parent.remove(entry);
    parent.insert_child_after(&holder, parent.first_child().as_ref());
    holder.append(entry);
    e.chips.fields.borrow_mut().insert(
        key(entry),
        Field {
            addresses: vec![],
            flow,
            scroll,
        },
    );
    let initial = entry.text();
    if let Ok(addresses) = luma_core::composer::recipients(&initial)
        && addresses.len() <= 100
    {
        e.field_sync.set(true);
        e.chips
            .fields
            .borrow_mut()
            .get_mut(&key(entry))
            .unwrap()
            .addresses = addresses;
        entry.set_text("");
        e.field_sync.set(false);
        render(e, entry);
    }
    let weak = Rc::downgrade(e);
    entry.connect_changed(move |entry| {
        if let Some(e) = weak.upgrade() {
            commit(&e, entry, false);
        }
    });
    let keys = gtk::EventControllerKey::new();
    keys.set_propagation_phase(gtk::PropagationPhase::Capture);
    let weak = Rc::downgrade(e);
    let input = entry.downgrade();
    keys.connect_key_pressed(move |_, keyval, _, mods| {
        let (Some(e), Some(entry)) = (weak.upgrade(), input.upgrade()) else {
            return glib::Propagation::Proceed;
        };
        if keyval == gtk::gdk::Key::Return && !mods.contains(gtk::gdk::ModifierType::CONTROL_MASK) {
            if !complete(&e, &entry) {
                commit_typed(&e, &entry);
            }
            return glib::Propagation::Stop;
        }
        if keyval == gtk::gdk::Key::BackSpace && entry.text().is_empty() {
            let address = e
                .chips
                .fields
                .borrow_mut()
                .get_mut(&key(&entry))
                .and_then(|f| f.addresses.pop());
            if let Some(address) = address {
                e.field_sync.set(true);
                entry.set_text(&address);
                entry.select_region(0, -1);
                e.field_sync.set(false);
                render(&e, &entry);
                changed(&e);
                return glib::Propagation::Stop;
            }
        }
        glib::Propagation::Proceed
    });
    entry.add_controller(keys);
}

fn fragment(text: &str) -> (&str, &str) {
    let mut quoted = false;
    let mut angle = false;
    let mut escaped = false;
    let mut start = 0;
    for (i, ch) in text.char_indices() {
        if escaped {
            escaped = false;
            continue;
        }
        match ch {
            '\\' if quoted => escaped = true,
            '"' => quoted = !quoted,
            '<' if !quoted => angle = true,
            '>' if !quoted => angle = false,
            ',' | ';' if !quoted && !angle => start = i + 1,
            _ => (),
        }
    }
    let whitespace = text[start..].len() - text[start..].trim_start().len();
    text.split_at(start + whitespace)
}
fn choices(e: &Editor, field: &gtk::Entry) -> Vec<String> {
    if !e.state.settings.borrow().recent_recipients
        || field.position() != field.text().chars().count() as i32
    {
        return vec![];
    }
    let text = field.text();
    let (_, prefix) = fragment(&text);
    let prefix = prefix.to_lowercase();
    if prefix.len() < 2
        || e.recipients
            .borrow()
            .iter()
            .any(|r| r.address.eq_ignore_ascii_case(&prefix))
    {
        return vec![];
    }
    let Some(account) = e.account() else {
        return vec![];
    };
    let now = chrono::Utc::now().timestamp().max(0) as u64;
    e.recipients
        .borrow()
        .iter()
        .filter(|r| {
            r.account == account.id
                && now.saturating_sub(r.last_sent) < 90 * 86400
                && (r.address.to_lowercase().starts_with(&prefix)
                    || r.address
                        .split('<')
                        .nth(1)
                        .is_some_and(|s| s.to_lowercase().starts_with(&prefix)))
        })
        .take(5)
        .map(|r| r.address.clone())
        .collect()
}
pub(super) fn complete(e: &Editor, field: &gtk::Entry) -> bool {
    let rows = choices(e, field);
    let Some(candidate) = e
        .completion
        .borrow()
        .as_ref()
        .filter(|c| rows.contains(c))
        .cloned()
        .or_else(|| rows.first().cloned())
    else {
        return false;
    };
    let text = field.text();
    let (prefix, _) = fragment(&text);
    field.set_text(&format!("{prefix}{candidate}, "));
    field.set_position(-1);
    e.completion.borrow_mut().take();
    true
}
pub(super) fn install(e: &Rc<Editor>) {
    for field in [&e.to, &e.cc, &e.bcc] {
        install_chips(e, field);
        let popover = gtk::Popover::builder()
            .autohide(false)
            .has_arrow(false)
            .position(gtk::PositionType::Bottom)
            .build();
        popover.set_parent(field);
        let list = gtk::ListBox::new();
        list.add_css_class("boxed-list");
        popover.set_child(Some(&list));
        let weak = Rc::downgrade(e);
        let popup = popover.clone();
        let rows = list.clone();
        field.connect_cursor_position_notify(move |field| {
            let Some(e) = weak.upgrade() else {
                return;
            };
            if e.field_sync.get() {
                return;
            }
            while let Some(child) = rows.first_child() {
                rows.remove(&child);
            }
            let focused = gtk::prelude::GtkWindowExt::focus(&e.parent())
                .is_some_and(|w| w == *field || w.is_ancestor(field));
            let choices: Vec<String> = if focused { choices(&e, field) } else { vec![] };
            *e.completion.borrow_mut() = choices.first().cloned();
            for address in choices {
                let button = gtk::Button::with_label(&address);
                button.add_css_class("flat");
                button.set_focus_on_click(false);
                let entry = field.downgrade();
                let weak = Rc::downgrade(&e);
                let popup = popup.clone();
                button.connect_clicked(move |_| {
                    if let (Some(e), Some(field)) = (weak.upgrade(), entry.upgrade()) {
                        *e.completion.borrow_mut() = Some(address.clone());
                        complete(&e, &field);
                        popup.popdown();
                        field.grab_focus();
                    }
                });
                rows.append(&button);
            }
            if rows.first_child().is_some() {
                rows.select_row(rows.row_at_index(0).as_ref());
                popup.popup();
            } else {
                popup.popdown();
            }
        });
        let keys = gtk::EventControllerKey::new();
        keys.set_propagation_phase(gtk::PropagationPhase::Capture);
        let weak = Rc::downgrade(e);
        let entry = field.downgrade();
        let popup = popover.clone();
        keys.connect_key_pressed(move |_, key, _, _| {
            let (Some(e), Some(field)) = (weak.upgrade(), entry.upgrade()) else {
                return glib::Propagation::Proceed;
            };
            if popup.is_visible() {
                if key == gtk::gdk::Key::Escape {
                    popup.popdown();
                    e.completion.borrow_mut().take();
                    return glib::Propagation::Stop;
                }
                if key == gtk::gdk::Key::Return && complete(&e, &field) {
                    popup.popdown();
                    return glib::Propagation::Stop;
                }
                let rows = choices(&e, &field);
                if !rows.is_empty() && matches!(key, gtk::gdk::Key::Down | gtk::gdk::Key::Up) {
                    let current = e
                        .completion
                        .borrow()
                        .as_ref()
                        .and_then(|c| rows.iter().position(|s| s == c))
                        .unwrap_or(0);
                    let index = if key == gtk::gdk::Key::Down {
                        (current + 1) % rows.len()
                    } else {
                        (current + rows.len() - 1) % rows.len()
                    };
                    *e.completion.borrow_mut() = Some(rows[index].clone());
                    list.select_row(list.row_at_index(index as i32).as_ref());
                    return glib::Propagation::Stop;
                }
            }
            glib::Propagation::Proceed
        });
        field.add_controller(keys);
        let focus = gtk::EventControllerFocus::new();
        let popup = popover.clone();
        focus.connect_leave(move |_| popup.popdown());
        field.add_controller(focus);
        field.connect_unrealize(move |_| popover.popdown());
    }
}
#[cfg(debug_assertions)]
pub(super) fn verify(e: &Rc<Editor>) {
    e.to.set_text("\"Doe, Jane\" <jane@example.invalid>, other@example.invalid, ");
    let text = value(e, &e.to);
    assert_eq!(luma_core::composer::recipients(&text).unwrap().len(), 3);
    assert!(text.contains("Doe, Jane"));
    assert!(e.to.text().is_empty());
    e.to.set_text("invalid, third@example.invalid, ");
    assert!(
        value(e, &e.to).contains("invalid, third"),
        "invalid pasted addresses are retained for correction"
    );
    assert!(luma_core::composer::recipients(&value(e, &e.to)).is_err());
    e.to.set_text("");
    let flow = e.chips.fields.borrow()[&key(&e.to)].flow.clone();
    let chip = flow
        .child_at_index(0)
        .unwrap()
        .child()
        .unwrap()
        .downcast::<gtk::Box>()
        .unwrap();
    chip.last_child()
        .unwrap()
        .downcast::<gtk::Button>()
        .unwrap()
        .emit_clicked();
    assert_eq!(
        luma_core::composer::recipients(&value(e, &e.to))
            .unwrap()
            .len(),
        2
    );
    println!(
        "PASS removable recipient chips preserve quoted names, typed fragments and canonical MIME headers"
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn quoted_commas_do_not_split_recipient_names() {
        assert_eq!(
            fragment("\"Doe, Jane\" <jane@example.com>, jo"),
            ("\"Doe, Jane\" <jane@example.com>, ", "jo")
        );
        assert_eq!(fragment("jo"), ("", "jo"));
    }
}
