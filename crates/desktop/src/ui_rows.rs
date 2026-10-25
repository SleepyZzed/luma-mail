use super::*;

#[derive(Clone)]
pub(super) struct MailRow {
    pub row: gtk::ListBoxRow,
    reveal: gtk::Revealer,
}
pub(super) struct Update {
    pub key: ThreadKey,
    pub row: Option<Conversation>,
    pub visible: Option<bool>,
    pub selected_labels: Option<(String, Vec<String>)>,
}

pub(super) fn key(s: &State, row: &gtk::ListBoxRow) -> Option<ThreadKey> {
    s.row_widgets
        .borrow()
        .iter()
        .find(|(_, w)| w.row == *row)
        .map(|(k, _)| k.clone())
}
pub(super) fn clear_rows(s: &State) {
    s.changing_selection.set(true);
    s.rows.borrow_mut().clear();
    s.row_widgets.borrow_mut().clear();
    while let Some(child) = s.list.first_child() {
        s.list.remove(&child);
    }
    s.changing_selection.set(false);
}
pub(super) fn name(sender: &str) -> String {
    let value = sender
        .split('<')
        .next()
        .unwrap_or(sender)
        .trim()
        .trim_matches('"');
    let value = if value.is_empty() { sender } else { value };
    if value.contains('@') {
        value.split('@').next().unwrap_or(value).into()
    } else if value.is_empty() {
        "Unknown sender".into()
    } else {
        value.into()
    }
}
pub(super) fn avatar(sender: &str, size: i32) -> adw::Avatar {
    let a = adw::Avatar::new(size, Some(&name(sender)), true);
    a.set_tooltip_text(Some(sender));
    a
}
fn content(s: &Rc<State>, item: &Conversation) -> gtk::Box {
    let row = gtk::Box::new(gtk::Orientation::Horizontal, 12);
    margins(&row, if s.settings.borrow().compact { 10 } else { 14 });
    row.add_css_class("mail-row-content");
    let a = avatar(
        &item.sender,
        if s.settings.borrow().compact { 34 } else { 40 },
    );
    a.set_valign(gtk::Align::Start);
    row.append(&a);
    let text = vbox(4);
    text.set_hexpand(true);
    let top = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    let sender = label(&name(&item.sender), "mail-sender");
    sender.set_tooltip_text(Some(&item.sender));
    sender.set_ellipsize(gtk::pango::EllipsizeMode::End);
    sender.set_hexpand(true);
    if item.unread {
        sender.add_css_class("unread-sender");
    }
    top.append(&sender);
    let date = chrono::DateTime::from_timestamp_millis(item.date)
        .map(|t| t.with_timezone(&chrono::Local).format("%d %b").to_string())
        .unwrap_or_default();
    top.append(&label(&date, "mail-date"));
    text.append(&top);
    let subject = label(
        if item.subject.is_empty() {
            "(No subject)"
        } else {
            &item.subject
        },
        "mail-subject",
    );
    subject.set_ellipsize(gtk::pango::EllipsizeMode::End);
    if item.unread {
        subject.add_css_class("heading");
    }
    text.append(&subject);
    let snippet = label(&item.snippet, "mail-snippet");
    snippet.set_ellipsize(gtk::pango::EllipsizeMode::End);
    text.append(&snippet);
    let bottom = gtk::Box::new(gtk::Orientation::Horizontal, 6);
    let source = s
        .settings
        .borrow()
        .accounts
        .iter()
        .find(|a| a.id == item.key.account)
        .map(|a| a.email.clone())
        .unwrap_or_default();
    let source = label(&source, "account-caption");
    source.set_ellipsize(gtk::pango::EllipsizeMode::End);
    source.set_hexpand(true);
    bottom.append(&source);
    if item.attachments {
        let icon = gtk::Image::from_icon_name("mail-attachment-symbolic");
        icon.set_tooltip_text(Some("Contains attachments"));
        bottom.append(&icon);
    }
    if item.count > 1 {
        bottom.append(&label(&item.count.to_string(), "thread-count"));
    }
    if item.unread {
        bottom.append(&label("●", "unread-dot"));
    }
    text.append(&bottom);
    row.append(&text);
    let star = button(
        if item.starred {
            "starred-symbolic"
        } else {
            "non-starred-symbolic"
        },
        if item.starred {
            "Remove star"
        } else {
            "Star conversation"
        },
    );
    star.add_css_class("flat");
    star.add_css_class("circular");
    star.add_css_class("row-star");
    if item.starred {
        star.add_css_class("is-starred");
    }
    star.set_valign(gtk::Align::Center);
    let s = s.clone();
    let key = item.key.clone();
    let starred = item.starred;
    star.connect_clicked(move |_| {
        perform_actions(
            &s,
            vec![key.clone()],
            if starred {
                MailAction::Unstar
            } else {
                MailAction::Star
            },
        )
    });
    row.append(&star);
    row
}
fn make(s: &Rc<State>, item: &Conversation) -> MailRow {
    let reveal = gtk::Revealer::builder()
        .transition_type(gtk::RevealerTransitionType::SlideDown)
        .transition_duration(220)
        .reveal_child(false)
        .child(&content(s, item))
        .build();
    let row = gtk::ListBoxRow::new();
    row.set_widget_name(&rank(item));
    row.set_child(Some(&reveal));
    row.add_css_class("mail-row");
    let gesture = gtk::GestureClick::new();
    gesture.set_button(3);
    let state = s.clone();
    let target = item.key.clone();
    let weak = row.downgrade();
    gesture.connect_pressed(move |g, _, x, y| {
        if let Some(row) = weak.upgrade() {
            g.set_state(gtk::EventSequenceState::Claimed);
            menu(&state, &row, &target, x, y);
        }
    });
    row.add_controller(gesture);
    let keyboard = gtk::EventControllerKey::new();
    let state = s.clone();
    let target = item.key.clone();
    let weak = row.downgrade();
    keyboard.connect_key_pressed(move |_, key, _, mods| {
        if key == gtk::gdk::Key::Menu
            || (key == gtk::gdk::Key::F10 && mods.contains(gtk::gdk::ModifierType::SHIFT_MASK))
        {
            if let Some(row) = weak.upgrade() {
                menu(&state, &row, &target, 24., 24.);
            }
            glib::Propagation::Stop
        } else {
            glib::Propagation::Proceed
        }
    });
    row.add_controller(keyboard);
    let weak = reveal.downgrade();
    row.connect_map(move |_| {
        let weak = weak.clone();
        glib::idle_add_local_once(move || {
            if let Some(reveal) = weak.upgrade() {
                reveal.set_reveal_child(true);
            }
        });
    });
    MailRow { row, reveal }
}
fn rank(item: &Conversation) -> String {
    format!(
        "{:020}:{}:{}",
        (item.date as u64) ^ (1_u64 << 63),
        item.key.account,
        item.key.id
    )
}
pub(super) fn sort(a: &gtk::ListBoxRow, b: &gtk::ListBoxRow) -> gtk::Ordering {
    let a = a.widget_name();
    let b = b.widget_name();
    if a.len() < 21 || b.len() < 21 {
        return gtk::Ordering::Equal;
    }
    b[..20]
        .cmp(&a[..20])
        .then_with(|| a[20..].cmp(&b[20..]))
        .into()
}
pub(super) fn display(s: &Rc<State>, items: Vec<Conversation>, more: bool) {
    for item in items {
        if s.row_widgets.borrow().contains_key(&item.key) {
            continue;
        }
        let widget = make(s, &item);
        s.list.append(&widget.row);
        s.row_widgets.borrow_mut().insert(item.key.clone(), widget);
        s.rows.borrow_mut().push(item);
    }
    s.rows
        .borrow_mut()
        .sort_by(|a, b| b.date.cmp(&a.date).then_with(|| a.key.cmp(&b.key)));
    s.more.set_label("Load more conversations");
    s.more.set_sensitive(more && s.rows.borrow().len() < 1000);
    s.info(&format!(
        "{} conversations · Up to date",
        s.rows.borrow().len()
    ));
}
pub(super) fn reconcile(s: &Rc<State>, items: Vec<Conversation>, more: bool) {
    s.changing_selection.set(true);
    let keep: HashSet<_> = items.iter().map(|r| r.key.clone()).collect();
    let obsolete: Vec<_> = s
        .row_widgets
        .borrow()
        .keys()
        .filter(|k| !keep.contains(*k))
        .cloned()
        .collect();
    for key in obsolete {
        if let Some(widget) = s.row_widgets.borrow_mut().remove(&key) {
            widget.row.set_selectable(false);
            widget.reveal.set_reveal_child(false);
            let weak = Rc::downgrade(s);
            glib::timeout_add_local_once(Duration::from_millis(230), move || {
                if let Some(s) = weak.upgrade()
                    && widget.row.parent().is_some()
                {
                    s.changing_selection.set(true);
                    s.list.remove(&widget.row);
                    s.changing_selection.set(false);
                }
            });
        }
    }
    for item in &items {
        let current = s.row_widgets.borrow().get(&item.key).cloned();
        if let Some(widget) = current {
            if !s.rows.borrow().iter().any(|r| r == item) {
                widget.reveal.set_child(Some(&content(s, item)));
                widget.row.set_widget_name(&rank(item));
                widget.row.changed();
            }
        } else {
            let widget = make(s, item);
            s.list.append(&widget.row);
            s.row_widgets.borrow_mut().insert(item.key.clone(), widget);
        }
    }
    *s.rows.borrow_mut() = items;
    s.changing_selection.set(false);
    display(s, vec![], more);
}
pub(super) fn apply(s: &Rc<State>, update: Update) {
    let revision = s.mutation_serial.get().wrapping_add(1);
    s.mutation_serial.set(revision);
    let mut revisions = s.row_revisions.borrow_mut();
    revisions.insert(update.key.clone(), revision);
    if revisions.len() > 4096 {
        let cutoff = revision.saturating_sub(2048);
        revisions.retain(|_, v| *v >= cutoff);
    }
    drop(revisions);
    if let Some((id, labels)) = update.selected_labels
        && let Some(message) = s.selected_message.borrow_mut().as_mut()
        && message.id == id
    {
        message.label_ids = labels;
    }
    let current = s.row_widgets.borrow().get(&update.key).cloned();
    s.changing_selection.set(true);
    if update.visible == Some(false) || update.row.is_none() {
        s.rows.borrow_mut().retain(|r| r.key != update.key);
        s.row_widgets.borrow_mut().remove(&update.key);
        if let Some(widget) = current {
            widget.row.set_sensitive(false);
            widget.reveal.set_reveal_child(false);
            let weak = Rc::downgrade(s);
            glib::timeout_add_local_once(Duration::from_millis(150), move || {
                if let Some(s) = weak.upgrade()
                    && widget.row.parent().is_some()
                {
                    s.changing_selection.set(true);
                    s.list.remove(&widget.row);
                    s.changing_selection.set(false);
                }
            });
        }
        if s.selected.borrow().as_ref() == Some(&update.key) {
            crate::compose::park_inline(s);
            cancel_body_requests(s);
            s.reading.set(s.reading.get() + 1);
            if let Some(task) = s.reader_request.borrow_mut().take() {
                task.abort();
            }
            s.selected.borrow_mut().take();
            s.selected_message.borrow_mut().take();
            empty_reader(s);
        }
    } else if let Some(item) = update.row {
        if let Some(widget) = current {
            widget.reveal.set_child(Some(&content(s, &item)));
            widget.row.set_widget_name(&rank(&item));
            widget.row.changed();
            widget.row.remove_css_class("pending");
            if let Some(existing) = s.rows.borrow_mut().iter_mut().find(|r| r.key == item.key) {
                *existing = item;
            }
        } else if update.visible == Some(true) {
            let index = s.rows.borrow().partition_point(|r| {
                r.date > item.date || (r.date == item.date && r.key < item.key)
            });
            let widget = make(s, &item);
            widget.reveal.set_reveal_child(false);
            s.list.insert(&widget.row, index as i32);
            let reveal = widget.reveal.clone();
            s.row_widgets.borrow_mut().insert(item.key.clone(), widget);
            s.rows.borrow_mut().insert(index, item);
            glib::idle_add_local_once(move || reveal.set_reveal_child(true));
        }
    }
    s.changing_selection.set(false);
    if update.visible.is_none() {
        s.refresh_button.add_css_class("suggested-action");
    }
}
// A page started before a confirmed write must not resurrect removed rows or
// replace their new labels with an older response. Keep only bounded tombstones.
pub(super) fn merge_read(
    s: &State,
    incoming: Vec<Conversation>,
    start: u64,
    replace: bool,
) -> Vec<Conversation> {
    let revisions = s.row_revisions.borrow();
    let current = s.rows.borrow();
    let mut rows = Vec::new();
    let mut seen = HashSet::new();
    for row in incoming {
        let row = if revisions.get(&row.key).is_some_and(|v| *v > start) {
            current.iter().find(|r| r.key == row.key).cloned()
        } else {
            Some(row)
        };
        if let Some(row) = row
            && seen.insert(row.key.clone())
        {
            rows.push(row);
        }
    }
    if replace {
        for row in current
            .iter()
            .filter(|r| revisions.get(&r.key).is_some_and(|v| *v > start))
        {
            if seen.insert(row.key.clone()) {
                rows.push(row.clone());
            }
        }
        rows.sort_by(|a, b| b.date.cmp(&a.date).then_with(|| a.key.cmp(&b.key)));
        rows.truncate(1000);
    }
    rows
}
pub(super) fn menu(s: &Rc<State>, row: &gtk::ListBoxRow, target: &ThreadKey, x: f64, y: f64) {
    let Some(item) = s.rows.borrow().iter().find(|r| &r.key == target).cloned() else {
        return;
    };
    if !row.is_selected() {
        s.changing_selection.set(true);
        s.list.unselect_all();
        s.list.select_row(Some(row));
        s.changing_selection.set(false);
    }
    let targets: Vec<_> = s
        .list
        .selected_rows()
        .iter()
        .filter_map(|r| key(s, r))
        .collect();
    let pop = gtk::Popover::new();
    pop.set_has_arrow(false);
    pop.set_parent(row);
    pop.set_pointing_to(Some(&gtk::gdk::Rectangle::new(x as i32, y as i32, 1, 1)));
    let box_ = vbox(2);
    box_.add_css_class("context-menu");
    margins(&box_, 6);
    for (icon, title, action) in [
        (
            "mail-read-symbolic",
            if item.unread {
                "Mark as read"
            } else {
                "Mark as unread"
            },
            if item.unread {
                MailAction::Read
            } else {
                MailAction::Unread
            },
        ),
        (
            "starred-symbolic",
            if item.starred {
                "Remove star"
            } else {
                "Add star"
            },
            if item.starred {
                MailAction::Unstar
            } else {
                MailAction::Star
            },
        ),
        ("luma-archive-symbolic", "Archive", MailAction::Archive),
        ("luma-inbox-symbolic", "Move to Inbox", MailAction::Inbox),
        (
            "emblem-important-symbolic",
            "Mark important",
            MailAction::Important,
        ),
        (
            "user-trash-symbolic",
            if matches!(*s.folder.borrow(), Folder::Trash) {
                "Restore from Trash"
            } else {
                "Move to Trash"
            },
            if matches!(*s.folder.borrow(), Folder::Trash) {
                MailAction::Restore
            } else {
                MailAction::Trash
            },
        ),
    ] {
        let b = gtk::Button::new();
        b.add_css_class("flat");
        b.set_child(Some(
            &adw::ButtonContent::builder()
                .icon_name(icon)
                .label(title)
                .build(),
        ));
        b.update_property(&[gtk::accessible::Property::Label(title)]);
        let state = s.clone();
        let keys = targets.clone();
        let weak = pop.downgrade();
        b.connect_clicked(move |_| {
            if let Some(p) = weak.upgrade() {
                p.popdown();
            }
            perform_actions(&state, keys.clone(), action.clone());
        });
        box_.append(&b);
    }
    box_.append(&gtk::Separator::new(gtk::Orientation::Horizontal));
    let open = gtk::Button::with_label("Open conversation");
    open.add_css_class("flat");
    let state = s.clone();
    let target = target.clone();
    let weak = pop.downgrade();
    open.connect_clicked(move |_| {
        if let Some(p) = weak.upgrade() {
            p.popdown();
        }
        open_thread(&state, target.clone());
    });
    box_.append(&open);
    pop.set_child(Some(&box_));
    pop.connect_closed(|p| p.unparent());
    pop.popup();
}
