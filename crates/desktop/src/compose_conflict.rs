//! Explicit read-only conflict inspection. No attachment downloads or disk copy.
use super::*;

fn pane(title: &str, metadata: &str, body: &str) -> gtk::Box {
    let pane = vbox(8);
    pane.set_hexpand(true);
    pane.append(&label(title, "title-3"));
    let meta = label(metadata, "dim-label");
    meta.set_wrap(true);
    meta.set_selectable(true);
    pane.append(&meta);
    let text = gtk::TextView::new();
    text.set_editable(false);
    text.set_cursor_visible(false);
    text.set_wrap_mode(gtk::WrapMode::WordChar);
    text.buffer().set_text(body);
    let scroll = gtk::ScrolledWindow::builder()
        .vexpand(true)
        .min_content_height(240)
        .child(&text)
        .build();
    pane.append(&scroll);
    pane
}

pub(super) fn compare(e: &Rc<Editor>) {
    let Some(draft) = e.draft.borrow().clone() else {
        return;
    };
    let Some(account) = e.accounts.get(e.from.selected() as usize) else {
        return;
    };
    let Some(client) = e.state.clients.borrow().get(&account.id).cloned() else {
        return;
    };
    let local = match snapshot_impl(e, false) {
        Ok(local) => local,
        Err(error) => {
            e.status.set_text(&error.to_string());
            return;
        }
    };
    e.status
        .set_text("Reading the current Gmail draft for comparison…");
    let weak = Rc::downgrade(e);
    ui::async_work(
        &e.state,
        async move { client.draft(&draft.id).await },
        move |_, remote| {
            let Some(e) = weak.upgrade().filter(|e| !e.closing.get() && !e.sent.get()) else {
                return;
            };
            let body = match readable(&remote.message.payload) {
                Ok(body) => body,
                Err(error) => {
                    e.status.set_text(&error.to_string());
                    return;
                }
            };
            let content = vbox(12);
            ui::margins(&content, 20);
            let note = label(
                "Read-only text comparison. Attachments stay in Gmail. Close this view to keep editing or save your version as a separate draft.",
                "dim-label",
            );
            note.set_wrap(true);
            content.append(&note);
            let columns = gtk::Paned::new(gtk::Orientation::Horizontal);
            columns.set_wide_handle(true);
            columns.set_vexpand(true);
            columns.set_start_child(Some(&pane(
                "My version",
                &format!("To: {}\nSubject: {}", local.to, local.subject),
                &local.text,
            )));
            let payload = &remote.message.payload;
            columns.set_end_child(Some(&pane(
                "Current Gmail version",
                &format!(
                    "To: {}\nSubject: {}",
                    payload.header("To"),
                    payload.header("Subject")
                ),
                &body,
            )));
            content.append(&columns);
            let close = gtk::Button::with_label("Back to editing");
            close.set_halign(gtk::Align::End);
            content.append(&close);
            let dialog = adw::Dialog::builder()
                .title("Compare draft versions")
                .content_width(900)
                .content_height(540)
                .child(&content)
                .build();
            let weak = dialog.downgrade();
            close.connect_clicked(move |_| {
                if let Some(dialog) = weak.upgrade() {
                    dialog.close();
                }
            });
            dialog.present(Some(&e.parent()));
            e.status.set_text("Both draft versions are preserved");
        },
    );
}
