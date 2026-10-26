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
    ("luma-unread-symbolic", Folder::AllMail),
    ("luma-important-symbolic", Folder::Unread),
    ("luma-star-filled-symbolic", Folder::Starred),
    ("luma-send-symbolic", Folder::Sent),
    ("luma-draft-symbolic", Folder::Drafts),
    ("luma-archive-symbolic", Folder::Archive),
    ("luma-spam-symbolic", Folder::Spam),
    ("luma-trash-symbolic", Folder::Trash),
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
#[derive(Default)]
pub(super) struct Navigation {
    widgets: RefCell<Option<NavigationWidgets>>,
    accounts: RefCell<HashMap<String, AccountWidgets>>,
    unified_open: Cell<bool>,
    expanded_account: RefCell<Option<String>>,
    last_filter: RefCell<Option<String>>,
}
struct Disclosure {
    button: gtk::Button,
    arrow: gtk::Image,
    reveal: gtk::Revealer,
    content: gtk::Box,
}
impl Disclosure {
    fn new(button: gtk::Button, arrow: gtk::Image) -> Self {
        let content = vbox(2);
        let reveal = gtk::Revealer::builder()
            .transition_type(gtk::RevealerTransitionType::SlideDown)
            .child(&content)
            .build();
        Self {
            button,
            arrow,
            reveal,
            content,
        }
    }
    fn expand(&self, s: &State, open: bool) {
        self.reveal
            .set_transition_duration(crate::presentation::duration(s, 260));
        self.reveal.set_can_target(open);
        self.reveal.set_reveal_child(open);
        self.arrow.set_icon_name(Some(if open {
            "luma-chevron-down-symbolic"
        } else {
            "luma-chevron-right-symbolic"
        }));
        self.button
            .update_state(&[gtk::accessible::State::Expanded(Some(open))]);
    }
}
struct NavigationWidgets {
    recovery: gtk::Button,
    unified: Disclosure,
    folders: Vec<(Folder, gtk::Button)>,
    accounts: gtk::Box,
}
struct AccountWidgets {
    root: gtk::Box,
    disclosure: Disclosure,
    name: gtk::Label,
    email: gtk::Label,
    unread: gtk::Label,
    folders: Vec<(Folder, gtk::Button)>,
    labels: gtk::Box,
    label_signature: Vec<(String, String)>,
    label_buttons: Vec<(String, gtk::Button)>,
}
fn active(button: &gtk::Button, class: &str, selected: bool) {
    if selected {
        button.add_css_class(class);
    } else {
        button.remove_css_class(class);
    }
}
fn folder_buttons(
    s: &Rc<State>,
    parent: &gtk::Box,
    account: Option<String>,
) -> Vec<(Folder, gtk::Button)> {
    FOLDERS
        .into_iter()
        .map(|(icon, folder)| {
            let title = if folder == Folder::Inbox && account.is_none() {
                "Unified Inbox"
            } else {
                folder_name(&folder)
            };
            let button = nav_button(icon, title);
            parent.append(&button);
            let state = Rc::downgrade(s);
            let target = folder.clone();
            let account = account.clone();
            button.connect_clicked(move |_| {
                if let Some(s) = state.upgrade() {
                    select_folder(&s, account.clone(), target.clone());
                }
            });
            (folder, button)
        })
        .collect()
}
fn create_navigation(s: &Rc<State>) -> NavigationWidgets {
    let recovery = nav_button("luma-folder-symbolic", "Recover old local drafts");
    s.sidebar.append(&recovery);
    let state = Rc::downgrade(s);
    recovery.connect_clicked(move |_| {
        if let Some(s) = state.upgrade() {
            crate::compose::local_drafts(&s);
        }
    });
    let row = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    let arrow = gtk::Image::from_icon_name("luma-chevron-right-symbolic");
    row.append(&section("UNIFIED MAIL"));
    row.first_child().unwrap().set_hexpand(true);
    row.append(&arrow);
    let button = gtk::Button::builder()
        .child(&row)
        .tooltip_text("Expand or collapse unified mail folders")
        .build();
    button.add_css_class("flat");
    button.add_css_class("navigation-disclosure");
    let unified = Disclosure::new(button, arrow);
    let folders = folder_buttons(s, &unified.content, None);
    s.sidebar.append(&unified.button);
    s.sidebar.append(&unified.reveal);
    let state = Rc::downgrade(s);
    unified.button.connect_clicked(move |_| {
        if let Some(s) = state.upgrade() {
            s.navigation
                .unified_open
                .set(!s.navigation.unified_open.get());
            sidebar(&s);
        }
    });
    let heading = section("ACCOUNTS · CHOOSE A MAILBOX");
    heading.set_margin_top(20);
    s.sidebar.append(&heading);
    let accounts = vbox(4);
    s.sidebar.append(&accounts);
    let add = nav_button("luma-add-symbolic", "Sign in with Google");
    s.sidebar.append(&add);
    let state = Rc::downgrade(s);
    add.connect_clicked(move |_| {
        if let Some(s) = state.upgrade() {
            add_account(&s, false, None);
        }
    });
    NavigationWidgets {
        recovery,
        unified,
        folders,
        accounts,
    }
}
fn create_account(s: &Rc<State>, account: &luma_core::config::Account) -> AccountWidgets {
    let root = vbox(2);
    let row = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    let avatar = rows::avatar(&account.email, 30);
    crate::contact_photos::register(s, &account.id, &account.email, &avatar);
    row.append(&avatar);
    let text = vbox(3);
    text.set_hexpand(true);
    let name = label(&account.name, "account-name");
    let email = label(&account.email, "account-caption");
    for name in [&name, &email] {
        name.set_ellipsize(gtk::pango::EllipsizeMode::End);
        name.set_max_width_chars(20);
        text.append(name);
    }
    row.append(&text);
    let unread = label("", "thread-count");
    row.append(&unread);
    let arrow = gtk::Image::from_icon_name("luma-chevron-right-symbolic");
    row.append(&arrow);
    let button = gtk::Button::builder().child(&row).build();
    button.add_css_class("flat");
    let disclosure = Disclosure::new(button, arrow);
    disclosure.content.add_css_class("account-folders");
    root.append(&disclosure.button);
    root.append(&disclosure.reveal);
    let folders = folder_buttons(s, &disclosure.content, Some(account.id.clone()));
    let labels = vbox(2);
    disclosure.content.append(&labels);
    let state = Rc::downgrade(s);
    let id = account.id.clone();
    disclosure.button.connect_clicked(move |_| {
        if let Some(s) = state.upgrade() {
            if s.filter.borrow().as_ref() == Some(&id) {
                let open = s.navigation.expanded_account.borrow().as_ref() != Some(&id);
                *s.navigation.expanded_account.borrow_mut() = open.then(|| id.clone());
                sidebar(&s);
            } else {
                select_folder(&s, Some(id.clone()), Folder::Inbox);
                load_labels(&s, &id);
            }
        }
    });
    AccountWidgets {
        root,
        disclosure,
        name,
        email,
        unread,
        folders,
        labels,
        label_signature: vec![],
        label_buttons: vec![],
    }
}
pub(super) fn sidebar(s: &Rc<State>) {
    // Never destroy/reparent the active disclosure during selection or unread
    // updates: GTK needs the outgoing child to animate its closing height.
    if s.sidebar_top.first_child().is_none() {
        let brand = vbox(2);
        brand.add_css_class("sidebar-brand");
        brand.append(&label("Luma", "title-1"));
        brand.append(&label("Online-first mail", "caption"));
        s.sidebar_top.append(&brand);
        let compose = gtk::Button::new();
        compose.set_child(Some(
            &adw::ButtonContent::builder()
                .icon_name("luma-compose-symbolic")
                .label("Compose")
                .build(),
        ));
        compose.add_css_class("suggested-action");
        compose.add_css_class("pill");
        s.sidebar_top.append(&compose);
        let state = Rc::downgrade(s);
        compose.connect_clicked(move |_| {
            if let Some(s) = state.upgrade() {
                crate::compose::open(&s, None, None);
            }
        });
    }
    if s.navigation.widgets.borrow().is_none() {
        *s.navigation.widgets.borrow_mut() = Some(create_navigation(s));
    }
    let filter = s.filter.borrow().clone();
    if *s.navigation.last_filter.borrow() != filter {
        *s.navigation.last_filter.borrow_mut() = filter.clone();
        *s.navigation.expanded_account.borrow_mut() = filter.clone();
    }
    let widgets = s.navigation.widgets.borrow();
    let widgets = widgets.as_ref().unwrap();
    widgets.recovery.set_visible(s.local_compose.has_drafts());
    widgets.unified.expand(s, s.navigation.unified_open.get());
    active(&widgets.unified.button, "active-account", filter.is_none());
    for (folder, button) in &widgets.folders {
        active(
            button,
            "active-folder",
            filter.is_none() && *s.folder.borrow() == *folder,
        );
    }
    let accounts = s.settings.borrow().accounts.clone();
    let mut rows = s.navigation.accounts.borrow_mut();
    rows.retain(|id, row| {
        let keep = accounts.iter().any(|a| &a.id == id);
        if !keep {
            widgets.accounts.remove(&row.root);
        }
        keep
    });
    let mut previous = None::<gtk::Widget>;
    for account in accounts {
        let row = rows.entry(account.id.clone()).or_insert_with(|| {
            let row = create_account(s, &account);
            widgets.accounts.append(&row.root);
            row
        });
        widgets
            .accounts
            .reorder_child_after(&row.root, previous.as_ref());
        previous = Some(row.root.clone().upcast());
        row.name.set_text(&account.name);
        row.email.set_text(&account.email);
        row.disclosure.button.set_sensitive(account.enabled);
        row.disclosure.button.set_tooltip_text(Some(&format!(
            "{} · Inbox and account folders",
            account.email
        )));
        let count = s.unread.borrow().get(&account.id).copied().unwrap_or(0);
        row.unread.set_visible(count > 0);
        row.unread.set_text(&if count > 999 {
            "999+".into()
        } else {
            count.to_string()
        });
        row.unread
            .set_tooltip_text(Some(&format!("{count} unread messages")));
        let selected = filter.as_ref() == Some(&account.id);
        active(&row.disclosure.button, "active-account", selected);
        row.disclosure.expand(
            s,
            s.navigation.expanded_account.borrow().as_ref() == Some(&account.id),
        );
        for (folder, button) in &row.folders {
            active(
                button,
                "active-folder",
                selected && *s.folder.borrow() == *folder,
            );
        }
        let labels: Vec<_> = s
            .labels_cache
            .borrow()
            .get(&account.id)
            .cloned()
            .unwrap_or_default()
            .into_iter()
            .filter(|l| l.kind == "user")
            .collect();
        let signature = labels
            .iter()
            .map(|l| (l.id.clone(), l.name.clone()))
            .collect::<Vec<_>>();
        if row.label_signature != signature {
            clear(&row.labels);
            row.label_buttons.clear();
            if !labels.is_empty() {
                row.labels
                    .append(&section("LABELS · RIGHT-CLICK TO MANAGE"));
            }
            for item in labels {
                let button = nav_button("luma-tag-symbolic", &item.name);
                row.labels.append(&button);
                let state = Rc::downgrade(s);
                let account_id = account.id.clone();
                let id = item.id.clone();
                button.connect_clicked(move |_| {
                    if let Some(s) = state.upgrade() {
                        select_folder(&s, Some(account_id.clone()), Folder::Label(id.clone()));
                    }
                });
                label_menu(s, &button, &account.id, &item);
                row.label_buttons.push((item.id, button));
            }
            row.label_signature = signature;
        }
        for (id, button) in &row.label_buttons {
            active(
                button,
                "active-folder",
                selected && *s.folder.borrow() == Folder::Label(id.clone()),
            );
        }
    }
    s.sidebar_scroll.hadjustment().set_value(0.);
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
    *s.navigation.expanded_account.borrow_mut() = account.clone();
    *s.filter.borrow_mut() = account;
    *s.folder.borrow_mut() = folder;
    sidebar(s);
    reload(s);
}
fn label_menu(s: &Rc<State>, button: &gtk::Button, account: &str, item: &Label) {
    let gesture = gtk::GestureClick::new();
    gesture.set_button(3);
    let state = Rc::downgrade(s);
    let account = account.to_owned();
    let item = item.clone();
    let weak = button.downgrade();
    gesture.connect_pressed(move |gesture, _, x, y| {
        let Some(state) = state.upgrade() else {
            return;
        };
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
pub(super) fn fixture_unified(s: &Rc<State>, open: bool) {
    s.navigation.unified_open.set(open);
    sidebar(s);
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
    assert!(
        !s.navigation.unified_open.get(),
        "unified mail starts collapsed"
    );
    let unified = s
        .navigation
        .widgets
        .borrow()
        .as_ref()
        .unwrap()
        .unified
        .reveal
        .clone();
    let toggle = s
        .navigation
        .widgets
        .borrow()
        .as_ref()
        .unwrap()
        .unified
        .button
        .clone();
    assert!(!unified.reveals_child());
    toggle.emit_clicked();
    assert!(unified.reveals_child());
    let first_root = s.navigation.accounts.borrow()[&first.id].root.clone();
    let second_root = s.navigation.accounts.borrow()[&second.id].root.clone();
    let state = s.clone();
    let mut stage = 0;
    let mut baseline = None;
    glib::timeout_add_local(Duration::from_millis(340), move || {
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
            0 => {
                assert!(
                    unified.is_child_revealed(),
                    "unified folders finish opening"
                );
                toggle.emit_clicked();
                select_folder(&state, Some(first.id.clone()), Folder::Inbox);
            }
            1 => {
                assert!(
                    !unified.is_child_revealed(),
                    "unified folders finish closing"
                );
                assert!(
                    state.navigation.accounts.borrow()[&first.id]
                        .disclosure
                        .reveal
                        .is_child_revealed()
                );
                select_folder(&state, Some(second.id.clone()), Folder::Archive);
                let rows = state.navigation.accounts.borrow();
                assert_eq!(
                    rows[&first.id].root, first_root,
                    "outgoing widgets stay alive for collapse"
                );
                assert_eq!(rows[&second.id].root, second_root);
                assert!(!rows[&first.id].disclosure.reveal.reveals_child());
                assert!(rows[&second.id].disclosure.reveal.reveals_child());
            }
            2 => {
                {
                    let rows = state.navigation.accounts.borrow();
                    assert!(!rows[&first.id].disclosure.reveal.is_child_revealed());
                    assert!(rows[&second.id].disclosure.reveal.is_child_revealed());
                }
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
                    "PASS collapsed unified mail, animated folder expansion/collapse, persistent account widgets and stable sidebar geometry"
                );
                state.app.quit();
                return glib::ControlFlow::Break;
            }
        }
        stage += 1;
        glib::ControlFlow::Continue
    });
}
