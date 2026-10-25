use super::*;

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
    let Some(account) = e.accounts.get(e.from.selected() as usize) else {
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
