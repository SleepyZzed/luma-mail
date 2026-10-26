use crate::ui::{self, State, label, vbox};
use adw::prelude::*;
use gtk::glib;
use luma_core::{
    Error,
    config::BlockedSender,
    model::{MailAction, Message},
    sender_rules,
};
use std::rc::Rc;

pub fn block(s: &Rc<State>, account_id: &str, sender: &str) {
    if !ui::valid_single_selection(s) {
        s.info("Select one conversation before blocking a sender.");
        return;
    }
    let address = match sender_rules::address(sender) {
        Ok(a) => a,
        Err(e) => {
            s.error(&e);
            return;
        }
    };
    let account = s
        .settings
        .borrow()
        .accounts
        .iter()
        .find(|a| a.id == account_id)
        .cloned();
    let Some(account) = account else { return };
    if address.eq_ignore_ascii_case(&account.email) {
        s.info("You cannot block your own sending account.");
        return;
    }
    if account.blocked_senders.iter().any(|b| b.address == address) {
        s.info("This sender is already blocked. Use Blocked senders in Settings to unblock them.");
        return;
    }
    if account.blocked_senders.len() >= 1000 {
        s.info(
            "Gmail’s filter limit has been reached. Remove an existing rule before adding another.",
        );
        return;
    }
    let text = format!(
        "Future mail from {address} will go straight to Gmail Trash for {}. This rule works while Luma is closed. Existing messages are kept.{}",
        account.email,
        if account.sender_controls {
            ""
        } else {
            " Google will ask for permission to manage Gmail filters first."
        }
    );
    let dialog = adw::AlertDialog::builder()
        .heading(format!("Block {address}?"))
        .body(text)
        .build();
    dialog.add_responses(&[("cancel", "Cancel"), ("block", "Block sender")]);
    dialog.set_response_appearance("block", adw::ResponseAppearance::Destructive);
    let state = s.clone();
    glib::MainContext::default().spawn_local(async move {
        if dialog.choose_future(Some(&state.window)).await != "block" {
            return;
        }
        if !ui::valid_single_selection(&state) {
            return;
        }
        if account.sender_controls {
            create(&state, &account.id, address);
        } else {
            if state.signin_window.borrow().is_some() {
                state.info("Finish the current Google sign-in, then try blocking again.");
                return;
            }
            crate::signin::sender_controls(&state, &account, move |state, id| {
                create(&state, &id, address)
            });
        }
    });
}
fn create(s: &Rc<State>, account: &str, address: String) {
    let Some(client) = s.clients.borrow().get(account).cloned() else {
        return;
    };
    let account = account.to_owned();
    let sender = address.clone();
    s.info("Creating the sender rule in Gmail…");
    ui::async_work(
        s,
        async move { client.create_sender_block(&sender).await },
        move |s, filter| {
            if let Some(account) = s
                .settings
                .borrow_mut()
                .accounts
                .iter_mut()
                .find(|a| a.id == account)
            {
                account.blocked_senders.push(BlockedSender {
                    address,
                    filter_id: filter.id,
                });
            }
            if let Err(e) = s.paths.save(&s.settings.borrow()) {
                s.error(&Error::Invalid(format!("The rule was created in Gmail, but its local reference could not be saved: {e}. Manage the rule in Gmail Settings.")));
                return;
            }
            s.info("Sender blocked in Gmail · Future mail goes to Trash");
            s.toast("Sender blocked");
        },
    );
}
pub fn manage(s: &Rc<State>, account: &str) {
    let account = s
        .settings
        .borrow()
        .accounts
        .iter()
        .find(|a| a.id == account)
        .cloned();
    let Some(account) = account else { return };
    let window = adw::Window::builder()
        .application(&s.app)
        .transient_for(&s.window)
        .title("Blocked senders")
        .default_width(600)
        .default_height(440)
        .build();
    let root = vbox(12);
    let header = adw::HeaderBar::new();
    header.set_title_widget(Some(&adw::WindowTitle::new(
        "Blocked senders",
        &account.email,
    )));
    root.append(&header);
    let list = vbox(8);
    ui::margins(&list, 18);
    let note = label(
        "These Gmail filters move future messages to Trash. Unblocking removes only the rule Luma created.",
        "caption",
    );
    note.set_wrap(true);
    list.append(&note);
    for blocked in account.blocked_senders {
        let row = gtk::Box::new(gtk::Orientation::Horizontal, 10);
        row.add_css_class("attachment-card");
        let name = label(&blocked.address, "");
        name.set_hexpand(true);
        name.set_ellipsize(gtk::pango::EllipsizeMode::Middle);
        row.append(&name);
        let button = gtk::Button::with_label("Unblock");
        row.append(&button);
        list.append(&row);
        let state = s.clone();
        let id = account.id.clone();
        let weak = row.downgrade();
        button.connect_clicked(move |button| {
            let Some(client) = state.clients.borrow().get(&id).cloned() else {
                return;
            };
            button.set_sensitive(false);
            let button = button.downgrade();
            let row = weak.clone();
            let block = blocked.clone();
            let filter_id = blocked.filter_id.clone();
            let id = id.clone();
            ui::async_work(
                &state,
                async move {
                    Ok(client
                        .remove_sender_block(&block.filter_id, &block.address)
                        .await)
                },
                move |s, result| match result {
                    Ok(()) => {
                        if let Some(account) = s
                            .settings
                            .borrow_mut()
                            .accounts
                            .iter_mut()
                            .find(|a| a.id == id)
                        {
                            account.blocked_senders.retain(|b| b.filter_id != filter_id);
                        }
                        s.save();
                        if let Some(row) = row.upgrade() {
                            row.set_visible(false);
                        }
                        s.toast("Sender unblocked in Gmail");
                    }
                    Err(error) => {
                        s.error(&error);
                        if let Some(button) = button.upgrade() {
                            button.set_sensitive(true);
                        }
                    }
                },
            );
        });
    }
    if list.first_child().and_then(|c| c.next_sibling()).is_none() {
        list.append(&label(
            "No senders blocked by Luma for this account.",
            "dim-label",
        ));
    }
    let gmail = gtk::Button::with_label("Open Gmail filter settings");
    list.append(&gmail);
    let state = s.clone();
    let email = account.email;
    gmail.connect_clicked(move |_| {
        let mut url = url::Url::parse("https://mail.google.com/mail/").expect("constant");
        url.query_pairs_mut().append_pair("authuser", &email);
        url.set_fragment(Some("settings/filters"));
        crate::rendering::open_link(&state, url.as_str());
    });
    root.append(
        &gtk::ScrolledWindow::builder()
            .vexpand(true)
            .child(&list)
            .build(),
    );
    window.set_content(Some(&root));
    window.present();
}
pub fn menu(s: &Rc<State>, message: &Message, account: &str) -> gtk::MenuButton {
    let button = gtk::MenuButton::builder()
        .icon_name("luma-more-symbolic")
        .tooltip_text("Message actions")
        .build();
    button.add_css_class("flat");
    button.add_css_class("single-message-action");
    let pop = gtk::Popover::new();
    let items = vbox(4);
    ui::margins(&items, 6);
    let spam = gtk::Button::with_label(if message.has("SPAM") {
        "Not spam"
    } else {
        "Report spam"
    });
    spam.add_css_class("flat");
    items.append(&spam);
    let state = s.clone();
    let id = message.id.clone();
    let source = account.to_owned();
    let action = if message.has("SPAM") {
        MailAction::NotSpam
    } else {
        MailAction::Spam
    };
    let weak = pop.downgrade();
    spam.connect_clicked(move |_| {
        if let Some(pop) = weak.upgrade() {
            pop.popdown();
        }
        ui::act_message(&state, &source, &id, action.clone());
    });
    let block_button = gtk::Button::with_label("Block sender…");
    block_button.add_css_class("flat");
    items.append(&block_button);
    let state = s.clone();
    let sender = message.payload.header("From").to_owned();
    let source = account.to_owned();
    let weak = pop.downgrade();
    block_button.connect_clicked(move |_| {
        if let Some(pop) = weak.upgrade() {
            pop.popdown();
        }
        if ui::single_message_actions(&state) {
            block(&state, &source, &sender);
        }
    });
    pop.set_child(Some(&items));
    button.set_popover(Some(&pop));
    button
}
