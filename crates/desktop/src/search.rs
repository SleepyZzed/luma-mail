use crate::ui::{self, State};
use adw::prelude::*;
use luma_core::{model::Folder, search::Search};
use std::{collections::HashSet, rc::Rc};

pub fn open(s: &Rc<State>) {
    let accounts: Vec<_> = s
        .settings
        .borrow()
        .accounts
        .iter()
        .filter(|a| a.enabled)
        .cloned()
        .collect();
    if accounts.is_empty() {
        s.info("Connect or enable an account before searching Gmail.");
        return;
    }
    let window = adw::Window::builder()
        .application(&s.app)
        .transient_for(&s.window)
        .modal(true)
        .title("Search Gmail")
        .default_width(560)
        .default_height(680)
        .build();
    let root = ui::vbox(0);
    root.append(&adw::HeaderBar::new());
    let form = ui::vbox(8);
    ui::margins(&form, 20);
    let intro = ui::label(
        "Search All Mail on Google across the accounts you choose. Spam and Trash are excluded; no local index is created.",
        "dim-label",
    );
    intro.set_wrap(true);
    form.append(&intro);
    let field = |title: &str, hint: &str| {
        form.append(&ui::label(title, "heading"));
        let entry = gtk::Entry::builder().placeholder_text(hint).build();
        entry.update_property(&[gtk::accessible::Property::Label(title)]);
        form.append(&entry);
        entry
    };
    let from = field("From", "Sender name or address");
    let to = field("To", "Recipient name or address");
    let subject = field("Subject contains", "Words or a phrase");
    let words = field("Query", "Keywords or Gmail search operators");
    words.set_text(&s.search.text());
    let after = field("After", "YYYY-MM-DD (exclusive; Gmail's date timezone)");
    let before = field("Before", "YYYY-MM-DD (exclusive)");
    let flags = gtk::Box::new(gtk::Orientation::Horizontal, 12);
    let unread = gtk::CheckButton::with_label("Unread");
    let starred = gtk::CheckButton::with_label("Starred");
    let attachments = gtk::CheckButton::with_label("Has attachments");
    for button in [&unread, &starred, &attachments] {
        flags.append(button);
    }
    form.append(&flags);
    form.append(&ui::label("Accounts", "heading"));
    let all = gtk::CheckButton::with_label("Select all accounts");
    form.append(&all);
    let mut choices = Vec::new();
    for account in accounts {
        let button = gtk::CheckButton::with_label(&format!("{} · {}", account.name, account.email));
        button.set_active(if let Some(id) = s.filter.borrow().as_ref() {
            id == &account.id
        } else {
            s.account_subset
                .borrow()
                .as_ref()
                .is_none_or(|set| set.contains(&account.id))
        });
        form.append(&button);
        choices.push((account.id, button));
    }
    {
        let choices = choices.clone();
        all.connect_toggled(move |all| {
            for (_, choice) in &choices {
                choice.set_active(all.is_active());
            }
        });
    }
    let error = ui::label("", "error");
    error.set_wrap(true);
    form.append(&error);
    let apply = gtk::Button::with_label("Search Gmail");
    apply.add_css_class("suggested-action");
    form.append(&apply);
    {
        let s = s.clone();
        let window = window.clone();
        apply.connect_clicked(move |_| {
            let selected: HashSet<_> = choices
                .iter()
                .filter(|(_, button)| button.is_active())
                .map(|(id, _)| id.clone())
                .collect();
            if selected.is_empty() {
                error.set_text("Choose at least one account.");
                return;
            }
            let query = Search {
                from: from.text().into(),
                to: to.text().into(),
                subject: subject.text().into(),
                words: words.text().into(),
                after: after.text().into(),
                before: before.text().into(),
                unread: unread.is_active(),
                starred: starred.is_active(),
                attachments: attachments.is_active(),
            }
            .query();
            match query {
                Ok(query) => {
                    *s.filter.borrow_mut() = None;
                    *s.account_subset.borrow_mut() = if selected.len() == choices.len() {
                        None
                    } else {
                        Some(selected)
                    };
                    *s.folder.borrow_mut() = Folder::AllMail;
                    s.title.set_text("Search · All Mail");
                    s.search.set_text(&query);
                    ui::reload(&s);
                    window.close();
                }
                Err(err) => error.set_text(&err.to_string()),
            }
        });
    }
    root.append(
        &gtk::ScrolledWindow::builder()
            .vexpand(true)
            .child(&form)
            .build(),
    );
    window.set_content(Some(&root));
    window.present();
}
