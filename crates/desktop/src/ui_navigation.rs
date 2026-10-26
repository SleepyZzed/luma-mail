use super::*;

fn folder_name(folder: &Folder) -> &'static str {
    match folder {
        Folder::Inbox => "Inbox",
        Folder::AllMail => "All Mail",
        Folder::Unread => "Unread",
        Folder::Starred => "Starred",
        Folder::Sent => "Sent",
        Folder::Drafts => "Drafts",
        Folder::Archive => "Archive",
        Folder::Spam => "Junk / Spam",
        Folder::Trash => "Trash",
        Folder::Label(_) => "Label",
    }
}
const FOLDERS: [(&str, Folder); 9] = [
    ("luma-inbox-symbolic", Folder::Inbox),
    ("mail-unread-symbolic", Folder::AllMail),
    ("mail-mark-important-symbolic", Folder::Unread),
    ("starred-symbolic", Folder::Starred),
    ("mail-send-symbolic", Folder::Sent),
    ("document-edit-symbolic", Folder::Drafts),
    ("luma-archive-symbolic", Folder::Archive),
    ("mail-mark-junk-symbolic", Folder::Spam),
    ("user-trash-symbolic", Folder::Trash),
];
fn section(title: &str) -> gtk::Label {
    let label = label(title, "navigation-section");
    label.set_ellipsize(gtk::pango::EllipsizeMode::End);
    label.set_max_width_chars(20);
    label.set_tooltip_text(Some(title));
    label
}
fn nav_button(icon: &str, title: &str) -> gtk::Button {
    let row = gtk::Box::new(gtk::Orientation::Horizontal, 10);
    row.append(&gtk::Image::from_icon_name(icon));
    let name = label(title, "");
    name.set_hexpand(true);
    name.set_ellipsize(gtk::pango::EllipsizeMode::End);
    name.set_max_width_chars(22);
    row.append(&name);
    let button = gtk::Button::builder()
        .child(&row)
        .tooltip_text(title)
        .build();
    button.add_css_class("flat");
    button
}
pub(super) fn sidebar(s: &Rc<State>) {
    let scroll = s.sidebar_scroll.vadjustment().value();
    clear(&s.sidebar);
    // Preserve the pinned header widgets and their focus when account/labels change.
    if s.sidebar_top.first_child().is_none() {
        let brand = vbox(2);
        brand.add_css_class("sidebar-brand");
        brand.append(&label("Luma", "title-1"));
        brand.append(&label("Online-first mail", "caption"));
        s.sidebar_top.append(&brand);
        let compose = gtk::Button::new();
        compose.set_child(Some(
            &adw::ButtonContent::builder()
                .icon_name("mail-message-new-symbolic")
                .label("Compose")
                .build(),
        ));
        compose.add_css_class("suggested-action");
        compose.add_css_class("pill");
        s.sidebar_top.append(&compose);
        let state = s.clone();
        compose.connect_clicked(move |_| crate::compose::open(&state, None, None));
    }
    if s.local_compose.has_drafts() {
        let local = nav_button("document-open-symbolic", "Recover old local drafts");
        s.sidebar.append(&local);
        let state = s.clone();
        local.connect_clicked(move |_| crate::compose::local_drafts(&state));
    }
    let group = section("UNIFIED VIEWS");
    s.sidebar.append(&group);
    for (icon, folder) in FOLDERS {
        let title = if folder == Folder::Inbox {
            "Unified Inbox"
        } else {
            folder_name(&folder)
        };
        let button = nav_button(icon, title);
        if *s.folder.borrow() == folder && s.filter.borrow().is_none() {
            button.add_css_class("active-folder");
        }
        s.sidebar.append(&button);
        let state = s.clone();
        button.connect_clicked(move |_| select_folder(&state, None, folder.clone()));
    }
    let heading = section("ACCOUNTS · CHOOSE A MAILBOX");
    heading.set_margin_top(20);
    s.sidebar.append(&heading);
    let accounts = s.settings.borrow().accounts.clone();
    for account in accounts {
        let button = gtk::Button::new();
        let row = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        row.append(&rows::avatar(&account.email, 30));
        let text = vbox(3);
        text.set_hexpand(true);
        for (value, class) in [
            (&account.name, "account-name"),
            (&account.email, "account-caption"),
        ] {
            let name = label(value, class);
            name.set_ellipsize(gtk::pango::EllipsizeMode::End);
            name.set_max_width_chars(20);
            text.append(&name);
        }
        row.append(&text);
        let unread = s.unread.borrow().get(&account.id).copied().unwrap_or(0);
        if unread > 0 {
            let count = label(
                &if unread > 999 {
                    "999+".into()
                } else {
                    unread.to_string()
                },
                "thread-count",
            );
            count.set_tooltip_text(Some(&format!("{unread} unread messages")));
            row.append(&count);
        }
        button.set_child(Some(&row));
        button.add_css_class("flat");
        button.set_sensitive(account.enabled);
        button.set_tooltip_text(Some(&format!(
            "{} · Open this account's Inbox and folders",
            account.email
        )));
        let active = s.filter.borrow().as_ref() == Some(&account.id);
        if active {
            button.add_css_class("active-account");
        }
        s.sidebar.append(&button);
        let state = s.clone();
        let id = account.id.clone();
        button.connect_clicked(move |_| {
            select_folder(&state, Some(id.clone()), Folder::Inbox);
            load_labels(&state, &id);
        });
        if active {
            let folders = vbox(2);
            folders.add_css_class("account-folders");
            for (icon, folder) in FOLDERS {
                let button = nav_button(icon, folder_name(&folder));
                if *s.folder.borrow() == folder {
                    button.add_css_class("active-folder");
                }
                folders.append(&button);
                let state = s.clone();
                let id = account.id.clone();
                button.connect_clicked(move |_| {
                    select_folder(&state, Some(id.clone()), folder.clone())
                });
            }
            s.sidebar.append(&folders);
            if let Some(labels) = s.labels_cache.borrow().get(&account.id).cloned() {
                let heading = section("LABELS · RIGHT-CLICK TO MANAGE");
                s.sidebar.append(&heading);
                for item in labels.into_iter().filter(|l| l.kind == "user") {
                    let button = nav_button("luma-tag-symbolic", &item.name);
                    if *s.folder.borrow() == Folder::Label(item.id.clone()) {
                        button.add_css_class("active-folder");
                    }
                    s.sidebar.append(&button);
                    let state = s.clone();
                    let id = account.id.clone();
                    let label_id = item.id.clone();
                    button.connect_clicked(move |_| {
                        select_folder(&state, Some(id.clone()), Folder::Label(label_id.clone()))
                    });
                    label_menu(s, &button, &account.id, &item);
                }
            }
        }
    }
    let add = nav_button("list-add-symbolic", "Sign in with Google");
    s.sidebar.append(&add);
    let state = s.clone();
    add.connect_clicked(move |_| add_account(&state, false, None));
    s.sidebar_scroll.hadjustment().set_value(0.);
    let weak = Rc::downgrade(s);
    let frames = Cell::new(0);
    s.sidebar.add_tick_callback(move |_, _| {
        frames.set(frames.get() + 1);
        if frames.get() < 2 {
            return glib::ControlFlow::Continue;
        }
        if let Some(s) = weak.upgrade() {
            let adjustment = s.sidebar_scroll.vadjustment();
            adjustment.set_value(scroll.min((adjustment.upper() - adjustment.page_size()).max(0.)));
            s.sidebar_scroll.hadjustment().set_value(0.);
        }
        glib::ControlFlow::Break
    });
}
pub(super) fn select_folder(s: &Rc<State>, account: Option<String>, folder: Folder) {
    let title = match &folder {
        Folder::Label(id) => account
            .as_ref()
            .and_then(|a| {
                s.labels_cache
                    .borrow()
                    .get(a)
                    .and_then(|ls| ls.iter().find(|l| &l.id == id).map(|l| l.name.clone()))
            })
            .unwrap_or_else(|| "Label".into()),
        Folder::Inbox if account.is_none() => "Unified Inbox".into(),
        _ => folder_name(&folder).to_owned(),
    };
    let source = account.as_ref().and_then(|id| {
        s.settings
            .borrow()
            .accounts
            .iter()
            .find(|a| &a.id == id)
            .map(|a| a.email.clone())
    });
    s.title
        .set_text(&source.map_or(title.clone(), |email| format!("{title} · {email}")));
    *s.filter.borrow_mut() = account;
    *s.folder.borrow_mut() = folder;
    sidebar(s);
    reload(s);
}
fn label_menu(s: &Rc<State>, button: &gtk::Button, account: &str, item: &Label) {
    let gesture = gtk::GestureClick::new();
    gesture.set_button(3);
    let state = s.clone();
    let account = account.to_owned();
    let item = item.clone();
    let weak = button.downgrade();
    gesture.connect_pressed(move |gesture, _, x, y| {
        let Some(button) = weak.upgrade() else {
            return;
        };
        gesture.set_state(gtk::EventSequenceState::Claimed);
        let pop = gtk::Popover::new();
        pop.set_has_arrow(false);
        pop.set_parent(&button);
        pop.set_pointing_to(Some(&gtk::gdk::Rectangle::new(x as i32, y as i32, 1, 1)));
        let remove = gtk::Button::with_label("Delete label…");
        remove.add_css_class("flat");
        remove.add_css_class("destructive-action");
        let state = state.clone();
        let account = account.clone();
        let item = item.clone();
        let weak = pop.downgrade();
        remove.connect_clicked(move |_| {
            if let Some(pop) = weak.upgrade() {
                pop.popdown();
            }
            delete_label(&state, &account, &item);
        });
        pop.set_child(Some(&remove));
        pop.connect_closed(|p| p.unparent());
        pop.popup();
    });
    button.add_controller(gesture);
}
fn delete_label(s: &Rc<State>, account: &str, item: &Label) {
    if item.kind != "user" {
        return;
    }
    let dialog = adw::AlertDialog::builder().heading(format!("Delete label “{}”?", item.name)).body("This removes the label from Gmail and all messages using it. The messages themselves are kept. Label deletion cannot be undone.").build();
    dialog.add_responses(&[("cancel", "Cancel"), ("delete", "Delete label")]);
    dialog.set_response_appearance("delete", adw::ResponseAppearance::Destructive);
    dialog.set_default_response(Some("cancel"));
    let state = s.clone();
    let account = account.to_owned();
    let id = item.id.clone();
    glib::MainContext::default().spawn_local(async move {
        if dialog.choose_future(Some(&state.window)).await != "delete" {
            return;
        }
        let Some(client) = state.clients.borrow().get(&account).cloned() else {
            return;
        };
        async_work(
            &state,
            async move {
                client.delete_label(&id).await?;
                Ok((account, id))
            },
            |s, (account, id)| {
                if let Some(labels) = s.labels_cache.borrow_mut().get_mut(&account) {
                    labels.retain(|l| l.id != id);
                }
                if s.filter.borrow().as_ref() == Some(&account)
                    && *s.folder.borrow() == Folder::Label(id)
                {
                    select_folder(&s, Some(account), Folder::Inbox);
                } else {
                    sidebar(&s);
                }
                s.toast("Label deleted in Gmail · Messages kept");
            },
        );
    });
}

#[cfg(debug_assertions)]
pub(super) fn diagnostic(s: &Rc<State>) {
    assert!(s.clients.borrow().is_empty());
    let first = s.settings.borrow().accounts[0].clone();
    let mut second = first.clone();
    second.id = "second-fixture".into();
    second.name = "A very long display name ".repeat(8);
    second.email = "second@example.invalid".into();
    s.settings.borrow_mut().accounts.push(second.clone());
    let long = Label {
        id: "long-label".into(),
        name: "A very long label name ".repeat(12),
        kind: "user".into(),
        ..Default::default()
    };
    s.labels_cache
        .borrow_mut()
        .insert(second.id.clone(), vec![long]);
    s.settings.borrow_mut().ui_scale_percent = 100;
    crate::presentation::apply(s);
    let state = s.clone();
    let mut stage = 0;
    let mut baseline = None;
    glib::timeout_add_local(Duration::from_millis(160), move || {
        let nav = state.sidebar_top.parent().unwrap();
        let bounds = nav.compute_bounds(&state.window).unwrap();
        if stage > 0 {
            let old: gtk::graphene::Rect = baseline.unwrap();
            assert!(
                (bounds.x() - old.x()).abs() < 1. && (bounds.width() - old.width()).abs() < 1.,
                "switching account and long labels must not move/resize the sidebar"
            );
            assert!(state.sidebar_scroll.hadjustment().value().abs() < 1.);
        } else {
            baseline = Some(bounds);
        }
        match stage {
            0 => select_folder(&state, Some(first.id.clone()), Folder::Inbox),
            1 => select_folder(&state, Some(second.id.clone()), Folder::Archive),
            2 => {
                assert!(
                    state.title.text().contains("second@example.invalid")
                        && *state.folder.borrow() == Folder::Archive
                );
                select_folder(&state, Some(first.id.clone()), Folder::Spam);
            }
            _ => {
                assert_eq!(state.filter.borrow().as_deref(), Some(first.id.as_str()));
                assert_eq!(*state.folder.borrow(), Folder::Spam);
                println!(
                    "PASS account folder scoping and stable sidebar geometry with long account/label names"
                );
                state.app.quit();
                return glib::ControlFlow::Break;
            }
        }
        stage += 1;
        glib::ControlFlow::Continue
    });
}
