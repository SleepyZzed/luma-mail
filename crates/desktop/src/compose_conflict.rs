//! Explicit three-way resolution into a separate draft. Gmail's version is untouched.
use super::*;
use luma_core::draft_merge::{Fields, NAMES, Plan, Resolution};

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
    pane.append(
        &gtk::ScrolledWindow::builder()
            .vexpand(true)
            .min_content_height(160)
            .child(&text)
            .build(),
    );
    pane
}
fn remote_compose(remote: &Draft, account: &str) -> Result<Compose> {
    let p = &remote.message.payload;
    let mut c = Compose::new(p.header("From").into());
    c.account = Some(account.into());
    c.to = p.header("To").into();
    c.cc = p.header("Cc").into();
    c.bcc = p.header("Bcc").into();
    c.sender_reply_to = p.header("Reply-To").into();
    c.subject = p.header("Subject").into();
    c.text = readable(p)?;
    if let Some(html) = luma_core::html_mail::body(p)? {
        c.text = html;
        c.html = true;
    }
    c.thread = Some(remote.message.thread_id.clone());
    c.reply_to = p.header("In-Reply-To").into();
    c.references = p.header("References").into();
    c.drive_files =
        luma_core::compose_drive::draft_references(p.header("X-Luma-Drive-Attachments"))?;
    Ok(c)
}
fn preview(c: &Compose) -> String {
    let source: String = c.text.chars().take(128 * 1024).collect();
    let text = if c.html {
        luma_core::composer::html_text(&source)
            .unwrap_or_else(|_| "HTML source is preserved in the merged draft".into())
    } else {
        source
    };
    let mut output: String = text.chars().take(32768).collect();
    if text.chars().nth(32768).is_some() || c.text.chars().nth(128 * 1024).is_some() {
        output
            .push_str("\n[Preview shortened; the full content is preserved in the merged draft.]");
    }
    output
}
struct Prepared {
    local: Compose,
    server: Compose,
    remote: Draft,
    plan: Plan,
    previews: [String; 2],
}
fn prepare(local: Compose, remote: Draft, account: &str, base: &Fields) -> Result<Prepared> {
    let server = remote_compose(&remote, account)?;
    let previews = [preview(&local), preview(&server)];
    let plan = Plan::new(base, Fields::of(&local), Fields::of(&server));
    Ok(Prepared {
        local,
        server,
        remote,
        plan,
        previews,
    })
}

fn choices(inputs: &[gtk::DropDown]) -> [Option<bool>; 6] {
    std::array::from_fn(|i| match inputs[i].selected() {
        1 => Some(false),
        2 => Some(true),
        _ => None,
    })
}
fn ready(plan: &Plan, inputs: &[gtk::DropDown], attachments: &gtk::DropDown) -> bool {
    plan.is_resolved(choices(inputs)) && attachments.selected() != 0
}
pub(super) fn compare(e: &Rc<Editor>) {
    if e.busy.get() || e.comparison_busy.replace(true) {
        return;
    }
    let Some(draft) = e.draft.borrow().clone() else {
        e.comparison_busy.set(false);
        return;
    };
    let Some(account) = e.account() else {
        e.comparison_busy.set(false);
        return;
    };
    let account_id = account.id.clone();
    let Some(client) = e.state.clients.borrow().get(&account.id).cloned() else {
        e.comparison_busy.set(false);
        return;
    };
    let local = match snapshot(e) {
        Ok(c) => c,
        Err(error) => {
            e.comparison_busy.set(false);
            e.status.set_text(&error.to_string());
            return;
        }
    };
    let revision = e.revision.get();
    let base = e.merge_base.borrow().clone();
    let weak = Rc::downgrade(e);
    e.status
        .set_text("Reading the current Gmail draft for comparison...");
    ui::async_work(
        &e.state,
        async move {
            let remote = match client.draft(&draft.id).await {
                Ok(d) => d,
                Err(error) => return Ok(Err(error)),
            };
            Ok(
                tokio::task::spawn_blocking(move || prepare(local, remote, &account_id, &base))
                    .await
                    .unwrap_or_else(|_| {
                        Err(Error::Invalid("Cannot prepare the draft comparison".into()))
                    }),
            )
        },
        move |_, result| {
            let Some(e) = weak.upgrade() else {
                return;
            };
            e.comparison_busy.set(false);
            if e.closing.get() || e.sent.get() {
                return;
            }
            let prepared = match result {
                Ok(r) => r,
                Err(error) => {
                    e.status.set_text(&error.to_string());
                    return;
                }
            };
            if revision != e.revision.get() {
                e.status.set_text("Your draft changed while comparing. Open comparison again to include the latest edits.");
                return;
            }
            show(&e, prepared, revision);
        },
    );
}
type MergeView = (
    adw::Dialog,
    gtk::Button,
    Rc<Vec<gtk::DropDown>>,
    gtk::DropDown,
);
fn show(e: &Rc<Editor>, prepared: Prepared, revision: u64) -> MergeView {
    let Prepared {
        local,
        server,
        remote,
        plan,
        previews,
    } = prepared;
    let plan = Rc::new(plan);
    let content = vbox(12);
    ui::margins(&content, 20);
    let note = label(
        "Changes to different fields are combined. Choose a version for each conflicting field and choose the attachment set. This opens a new merged copy for review; the current Gmail draft is preserved. Nothing is sent.",
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
        &previews[0],
    )));
    columns.set_end_child(Some(&pane(
        "Current Gmail version",
        &format!("To: {}\nSubject: {}", server.to, server.subject),
        &previews[1],
    )));
    content.append(&columns);
    let resolution = vbox(6);
    let mut inputs = Vec::new();
    for (i, name) in NAMES.iter().enumerate() {
        let row = gtk::Box::new(gtk::Orientation::Horizontal, 12);
        let title = label(name, "");
        title.set_hexpand(true);
        row.append(&title);
        let input =
            gtk::DropDown::from_strings(&["Choose a version", "My version", "Gmail version"]);
        let status = match plan.resolution[i] {
            Resolution::Same => "Same in both versions",
            Resolution::Mine => "Your changes",
            Resolution::Gmail => "Gmail changes",
            Resolution::Choose => "Both changed",
        };
        row.append(&label(status, "caption"));
        match plan.resolution[i] {
            Resolution::Same | Resolution::Mine => input.set_selected(1),
            Resolution::Gmail => input.set_selected(2),
            Resolution::Choose => {}
        }
        input.set_sensitive(plan.resolution[i] == Resolution::Choose);
        row.append(&input);
        resolution.append(&row);
        inputs.push(input);
    }
    let local_count = local.retained.len() + local.attachments.len() + local.drive_files.len();
    let remote_count = remote.message.payload.attachments().len() + server.drive_files.len();
    let my_files = format!("My attachments ({local_count})");
    let server_files = format!("Gmail attachments ({remote_count})");
    let attachments =
        gtk::DropDown::from_strings(&["Choose attachments", &my_files, &server_files]);
    if local_count == 0 && remote_count == 0 {
        attachments.set_selected(1);
        attachments.set_sensitive(false);
    }
    let files_row = gtk::Box::new(gtk::Orientation::Horizontal, 12);
    let title = label("Attachments", "");
    title.set_hexpand(true);
    files_row.append(&title);
    files_row.append(&attachments);
    resolution.append(&files_row);
    content.append(
        &gtk::ScrolledWindow::builder()
            .max_content_height(270)
            .min_content_height(180)
            .propagate_natural_height(true)
            .child(&resolution)
            .build(),
    );
    let buttons = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    buttons.set_halign(gtk::Align::End);
    let close = gtk::Button::with_label("Keep editing");
    let merge = gtk::Button::with_label("Open merged copy");
    merge.add_css_class("suggested-action");
    merge.set_sensitive(ready(&plan, &inputs, &attachments));
    buttons.append(&close);
    buttons.append(&merge);
    content.append(&buttons);
    let dialog = adw::Dialog::builder()
        .title("Compare and merge draft versions")
        .content_width(980)
        .content_height(760)
        .child(&content)
        .build();
    dialog.connect_closed(|dialog| dialog.set_child(None::<&gtk::Widget>));
    let inputs = Rc::new(inputs);
    for input in inputs.iter().chain(std::iter::once(&attachments)) {
        let controls: Vec<_> = inputs.iter().map(|input| input.downgrade()).collect();
        let plan = plan.clone();
        let files = attachments.downgrade();
        let button = merge.downgrade();
        input.connect_selected_notify(move |_| {
            let (Some(button), Some(files)) = (button.upgrade(), files.upgrade()) else {
                return;
            };
            let controls: Vec<_> = controls
                .iter()
                .filter_map(|input| input.upgrade())
                .collect();
            button.set_sensitive(controls.len() == 6 && ready(&plan, &controls, &files));
        });
    }
    let weak = dialog.downgrade();
    close.connect_clicked(move |_| {
        if let Some(d) = weak.upgrade() {
            d.close();
        }
    });
    let view = (
        dialog.clone(),
        merge.clone(),
        inputs.clone(),
        attachments.clone(),
    );
    let context = Rc::new(MergeContext {
        plan,
        inputs,
        attachments,
        local,
        server,
        remote,
        revision,
        resolution,
        close,
        sheet: dialog.downgrade(),
    });
    let weak = Rc::downgrade(e);
    merge.connect_clicked(move |button| {
        if let Some(e) = weak.upgrade() {
            start_merge(&e, &context, button);
        }
    });
    dialog.present(Some(&e.parent()));
    e.status
        .set_text("Both versions preserved. Choose resolutions to open a merged copy.");
    view
}
struct MergeContext {
    plan: Rc<Plan>,
    inputs: Rc<Vec<gtk::DropDown>>,
    attachments: gtk::DropDown,
    local: Compose,
    server: Compose,
    remote: Draft,
    revision: u64,
    resolution: gtk::Box,
    close: gtk::Button,
    sheet: glib::WeakRef<adw::Dialog>,
}
fn start_merge(e: &Rc<Editor>, context: &Rc<MergeContext>, button: &gtk::Button) {
    if e.busy.get() || e.closing.get() || e.revision.get() != context.revision {
        e.status.set_text(
            "Your edits changed. Close this comparison and open it again before merging.",
        );
        return;
    }
    let fields = match context.plan.resolve(choices(&context.inputs)) {
        Ok(fields) => fields,
        Err(error) => {
            e.status.set_text(&error.to_string());
            return;
        }
    };
    let use_remote = context.attachments.selected() == 2;
    if context.attachments.selected() == 0 {
        return;
    }
    let Some(account) = e.account() else {
        return;
    };
    let Some(client) = e.state.clients.borrow().get(&account.id).cloned() else {
        return;
    };
    let mut merged = Compose::new(context.local.from.clone());
    fields.apply(&mut merged);
    merged.account = Some(account.id.clone());
    merged.thread = context.server.thread.clone();
    merged.reply_to = context.server.reply_to.clone();
    merged.references = context.server.references.clone();
    if use_remote {
        merged.drive_files = context.server.drive_files.clone();
    } else {
        merged.retained = context.local.retained.clone();
        merged.attachments = context.local.attachments.clone();
        merged.drive_files = context.local.drive_files.clone();
    }
    e.busy.set(true);
    button.set_sensitive(false);
    context.resolution.set_sensitive(false);
    context.close.set_sensitive(false);
    e.status
        .set_text("Preparing the selected attachment set for the merged copy...");
    let message = context.remote.message.clone();
    let weak = Rc::downgrade(e);
    let context = context.clone();
    let button = button.clone();
    ui::async_work(
        &e.state,
        async move {
            if use_remote {
                match client.message_attachments(&message).await {
                    Ok(files) => merged.retained = files,
                    Err(error) => return Ok(Err(error)),
                }
            }
            Ok(Ok(merged))
        },
        move |_, result| {
            let Some(e) = weak.upgrade() else {
                return;
            };
            e.busy.set(false);
            button.set_sensitive(true);
            context.resolution.set_sensitive(true);
            context.close.set_sensitive(true);
            let merged = match result {
                Ok(compose) => compose,
                Err(error) => {
                    e.status.set_text(&error.to_string());
                    return;
                }
            };
            if e.closing.get() || e.sent.get() || e.revision.get() != context.revision {
                e.status.set_text(
                    "Draft changed while preparing the merge. Your original editor is preserved.",
                );
                return;
            }
            if !e
                .account()
                .is_some_and(|account| e.state.clients.borrow().contains_key(&account.id))
            {
                e.status
                    .set_text("Reconnect this account before opening the merged copy.");
                return;
            }
            if let Some(sheet) = context.sheet.upgrade() {
                sheet.close();
            }
            let state = e.state.clone();
            let slot = (e.placement.get() == 1)
                .then(|| e.inline_origin.upgrade())
                .flatten();
            e.closing.set(true);
            e.local_closed
                .store(true, std::sync::atomic::Ordering::SeqCst);
            history::cancel_pending(&e);
            if let Some(timer) = e.debounce.borrow_mut().take() {
                timer.remove();
            }
            if let Some(timer) = e.local_debounce.borrow_mut().take() {
                timer.remove();
            }
            release_editor(&e);
            make_at(&state, merged, None, slot);
            state.toast("Merged copy opened. Gmail's version is preserved; review before sending.");
        },
    );
}

#[cfg(debug_assertions)]
pub(super) fn verify(e: &Rc<Editor>) {
    let local = snapshot(e).unwrap();
    let mut server = local.clone();
    server.subject = "Gmail subject".into();
    server.text = "Gmail body".into();
    server.html = false;
    let plan = Plan::new(
        &e.merge_base.borrow(),
        Fields::of(&local),
        Fields::of(&server),
    );
    assert_eq!(plan.resolution[5], Resolution::Choose);
    let view = show(
        e,
        Prepared {
            previews: [preview(&local), preview(&server)],
            local,
            server,
            remote: Draft {
                id: "synthetic-remote".into(),
                message: Message::default(),
            },
            plan,
        },
        e.revision.get(),
    );
    assert!(!view.1.is_sensitive());
    view.2[5].set_selected(1);
    assert!(view.1.is_sensitive());
    let body_choice = view.2[5].downgrade();
    view.0.force_close();
    // GTK may retain the closing AdwDialog for its final animation frame.
    // Detach its child to exercise content ownership independently of that frame.
    view.0.set_child(None::<&gtk::Widget>);
    drop(view);
    assert!(
        body_choice.upgrade().is_none(),
        "Closed comparison controls and captured message bodies must be released"
    );
    println!(
        "PASS native field-merge choices, explicit conflicting-body resolution and comparison-memory release"
    );
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn comparison_previews_are_bounded_without_truncating_the_draft() {
        let c = Compose {
            text: "Large body ".repeat(40000),
            ..Default::default()
        };
        let p = preview(&c);
        assert!(p.chars().count() < 33000);
        assert!(p.contains("Preview shortened"));
        assert_eq!(c.text.len(), 440000);
    }
}
