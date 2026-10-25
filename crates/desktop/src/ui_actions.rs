use super::*;

#[derive(Clone)]
pub(super) struct Undo {
    account: String,
    message: String,
    thread: ThreadKey,
    actions: Vec<MailAction>,
    previous: Option<Conversation>,
}
#[derive(Default)]
struct Outcome {
    success: usize,
    failures: Vec<String>,
    undo: Vec<Undo>,
    updates: Vec<rows::Update>,
    refresh_needed: bool,
}
async fn read_update(
    client: &Gmail,
    key: ThreadKey,
    previous: Option<&Conversation>,
    folder: &Folder,
    search: &str,
    selected: Option<&str>,
) -> Result<rows::Update> {
    let thread = client.thread_metadata(&key.id).await?;
    let mut row = Conversation::from_thread(&key.account, &thread);
    if let Some(row) = row.as_mut() {
        row.attachments = previous.is_some_and(|p| p.attachments);
    }
    let visible = client.matches_view(&thread, folder, search).await?;
    let selected_labels = selected
        .and_then(|id| thread.messages.iter().find(|m| m.id == id))
        .map(|m| (m.id.clone(), m.label_ids.clone()));
    Ok(rows::Update {
        key,
        row,
        visible,
        selected_labels,
    })
}
fn pending(s: &State, keys: &[(ThreadKey, bool)], active: bool) {
    for (key, _) in keys {
        if let Some(row) = s.row_widgets.borrow().get(key) {
            if active {
                row.row.add_css_class("pending")
            } else {
                row.row.remove_css_class("pending")
            }
        }
    }
}
pub(super) fn perform(s: &Rc<State>, keys: Vec<(ThreadKey, bool)>, action: MailAction) {
    if s.operation_busy.replace(true) {
        s.info("Finishing the current Gmail change…");
        return;
    }
    let generation = s.generation.get();
    let folder = s.folder.borrow().clone();
    let search = s.search.text().to_string();
    let selected = s.selected_message.borrow().as_ref().map(|m| m.id.clone());
    let previous: HashMap<_, _> = s
        .rows
        .borrow()
        .iter()
        .map(|r| (r.key.clone(), r.clone()))
        .collect();
    let source = s
        .selected
        .borrow()
        .as_ref()
        .and_then(|key| s.clients.borrow().get(&key.account).cloned());
    let jobs: Vec<_> = keys
        .iter()
        .filter_map(|(key, message)| {
            s.clients
                .borrow()
                .get(&key.account)
                .cloned()
                .map(|c| (key.clone(), c, *message))
        })
        .collect();
    pending(s, &keys, true);
    s.undo_button.set_sensitive(false);
    s.info("Saving your change to Gmail…");
    let label = match action {
        MailAction::Trash => "Moved to Trash",
        MailAction::Archive => "Archived",
        MailAction::Star => "Star added",
        MailAction::Unstar => "Star removed",
        MailAction::Read => "Marked as read",
        MailAction::Unread => "Marked as unread",
        MailAction::Restore => "Restored from Trash",
        _ => "Changes saved",
    }
    .to_owned();
    async_work(
        s,
        async move {
            let mut out = Outcome::default();
            if jobs.len() != keys.len() {
                out.failures
                    .push("An account is no longer connected".into());
            }
            let label_name = if let MailAction::AddLabel(id) | MailAction::RemoveLabel(id) = &action
            {
                let Some(source) = &source else {
                    out.failures
                        .push("Open a source conversation before applying labels".into());
                    return Ok(out);
                };
                match source.labels().await.and_then(|labels| {
                    labels
                        .into_iter()
                        .find(|l| &l.id == id)
                        .map(|l| l.name)
                        .ok_or_else(|| Error::Invalid("The source label no longer exists".into()))
                }) {
                    Ok(name) => Some(name),
                    Err(e) => {
                        out.failures.push(e.to_string());
                        return Ok(out);
                    }
                }
            } else {
                None
            };
            let mut mapped: HashMap<String, std::result::Result<String, String>> = HashMap::new();
            for (key, client, is_message) in jobs {
                let mut action = action.clone();
                if let Some(name) = &label_name
                    && source.as_ref().is_some_and(|c| c.account != client.account)
                {
                    if !mapped.contains_key(&client.account) {
                        mapped.insert(
                            client.account.clone(),
                            client.matching_label(name).await.map_err(|e| e.to_string()),
                        );
                    }
                    match &mapped[&client.account] {
                        Ok(id) => {
                            action = if matches!(action, MailAction::AddLabel(_)) {
                                MailAction::AddLabel(id.clone())
                            } else {
                                MailAction::RemoveLabel(id.clone())
                            }
                        }
                        Err(e) => {
                            out.failures.push(e.clone());
                            continue;
                        }
                    }
                }
                let before = if is_message {
                    client.message_metadata(&key.id).await.map(|m| vec![m])
                } else {
                    client.thread_metadata(&key.id).await.map(|t| t.messages)
                };
                let before = match before {
                    Ok(before) => before,
                    Err(e) => {
                        out.failures.push(e.to_string());
                        continue;
                    }
                };
                let confirmed = match client.act_confirmed(&key.id, is_message, &action).await {
                    Ok(messages) => messages,
                    Err(e) => {
                        out.failures.push(e.to_string());
                        continue;
                    }
                };
                out.success += 1;
                let thread = ThreadKey {
                    account: key.account.clone(),
                    id: if is_message {
                        before
                            .first()
                            .map(|m| m.thread_id.clone())
                            .unwrap_or_default()
                    } else {
                        key.id.clone()
                    },
                };
                let prior = previous.get(&thread).cloned();
                for message in &before {
                    let actions = action.undo_for(message);
                    if !actions.is_empty() {
                        out.undo.push(Undo {
                            account: key.account.clone(),
                            message: message.id.clone(),
                            thread: thread.clone(),
                            actions,
                            previous: prior.clone(),
                        });
                    }
                }
                if !is_message {
                    let mut after = Thread {
                        id: thread.id.clone(),
                        messages: before,
                        ..Default::default()
                    };
                    for message in &mut after.messages {
                        if let Some(labels) = confirmed.iter().find(|m| m.id == message.id) {
                            message.label_ids = labels.label_ids.clone();
                        } else {
                            // A successful action is authoritative even if Gmail
                            // omits message labels from its projected response.
                            let (add, remove) = action.labels();
                            message.label_ids.retain(|id| !remove.contains(id));
                            for id in add {
                                if !message.label_ids.contains(&id) {
                                    message.label_ids.push(id);
                                }
                            }
                            if action == MailAction::Trash {
                                message.label_ids.retain(|id| id != "INBOX");
                                if !message.has("TRASH") {
                                    message.label_ids.push("TRASH".into());
                                }
                            }
                            if action == MailAction::Restore {
                                message.label_ids.retain(|id| id != "TRASH");
                            }
                        }
                    }
                    let mut row = Conversation::from_thread(&key.account, &after);
                    if let Some(row) = row.as_mut() {
                        row.attachments = prior.as_ref().is_some_and(|p| p.attachments);
                    }
                    let visible = client
                        .matches_view(&after, &folder, &search)
                        .await
                        .ok()
                        .flatten();
                    let selected_labels = selected
                        .as_deref()
                        .and_then(|id| after.messages.iter().find(|m| m.id == id))
                        .map(|m| (m.id.clone(), m.label_ids.clone()));
                    out.refresh_needed |= visible.is_none();
                    out.updates.push(rows::Update {
                        key: thread,
                        row,
                        visible,
                        selected_labels,
                    });
                    continue;
                }
                match read_update(
                    &client,
                    thread,
                    prior.as_ref(),
                    &folder,
                    &search,
                    selected.as_deref(),
                )
                .await
                {
                    Ok(update) => {
                        out.refresh_needed |= update.visible.is_none();
                        out.updates.push(update)
                    }
                    Err(_) => out.refresh_needed = true,
                }
            }
            Ok(out)
        },
        move |s, out| {
            s.operation_busy.set(false);
            for row in s.row_widgets.borrow().values() {
                row.row.remove_css_class("pending");
            }
            if !out.undo.is_empty() {
                *s.undo.borrow_mut() = out.undo;
            }
            s.undo_button.set_sensitive(!s.undo.borrow().is_empty());
            if s.generation.get() == generation {
                for update in out.updates {
                    rows::apply(&s, update);
                }
            }
            if out.refresh_needed || s.generation.get() != generation {
                s.refresh_button.add_css_class("suggested-action");
            }
            if out.failures.is_empty() && out.success > 0 {
                let text = if out.success == 1 {
                    label.clone()
                } else {
                    format!("{label} · {} conversations", out.success)
                };
                s.info(if out.refresh_needed {"Saved in Gmail. Current view is kept; Refresh will reconcile any unavailable metadata."}else{"Saved in Gmail · Your place is kept"});
                let toast = adw::Toast::new(&text);
                toast.set_timeout(5);
                if !s.undo.borrow().is_empty() {
                    toast.set_button_label(Some("Undo"));
                    let weak = Rc::downgrade(&s);
                    toast.connect_button_clicked(move |_| {
                        if let Some(s) = weak.upgrade() {
                            undo(&s);
                        }
                    });
                }
                s.toasts.add_toast(toast);
            } else if let Some(error) = out.failures.first() {
                s.error(&Error::Invalid(format!(
                    "{} changes saved; {} failed. {error}",
                    out.success,
                    out.failures.len()
                )));
            }
        },
    );
}
pub(super) fn undo(s: &Rc<State>) {
    if s.operation_busy.replace(true) {
        s.info("Finishing the current Gmail change…");
        return;
    }
    let records = std::mem::take(&mut *s.undo.borrow_mut());
    let clients = s.clients.borrow().clone();
    let folder = s.folder.borrow().clone();
    let search = s.search.text().to_string();
    let selected = s.selected_message.borrow().as_ref().map(|m| m.id.clone());
    let generation = s.generation.get();
    s.undo_button.set_sensitive(false);
    s.info("Undoing on Gmail…");
    async_work(
        s,
        async move {
            let mut failed = Vec::new();
            let mut threads = HashMap::new();
            for mut record in records {
                let Some(client) = clients.get(&record.account) else {
                    failed.push(record);
                    continue;
                };
                let mut remaining = Vec::new();
                for action in &record.actions {
                    if client.act(&record.message, true, action).await.is_err() {
                        remaining.push(action.clone());
                    }
                }
                threads.insert(record.thread.clone(), record.previous.clone());
                if !remaining.is_empty() {
                    record.actions = remaining;
                    failed.push(record);
                }
            }
            let mut updates = Vec::new();
            let mut refresh = false;
            for (key, prior) in threads {
                if let Some(client) = clients.get(&key.account) {
                    match read_update(
                        client,
                        key,
                        prior.as_ref(),
                        &folder,
                        &search,
                        selected.as_deref(),
                    )
                    .await
                    {
                        Ok(update) => updates.push(update),
                        Err(_) => refresh = true,
                    }
                }
            }
            Ok((failed, updates, refresh))
        },
        move |s, (failed, updates, refresh)| {
            s.operation_busy.set(false);
            let count = failed.len();
            *s.undo.borrow_mut() = failed;
            s.undo_button.set_sensitive(count > 0);
            if s.generation.get() == generation {
                for update in updates {
                    rows::apply(&s, update);
                }
            }
            if refresh || s.generation.get() != generation {
                s.refresh_button.add_css_class("suggested-action");
            }
            if count == 0 {
                s.info("Undo saved in Gmail · Your place is kept");
                s.toasts.add_toast(adw::Toast::new("Change undone"));
            } else {
                s.error(&Error::Invalid(format!(
                    "{count} messages could not be undone; use Undo to retry"
                )));
            }
        },
    );
}
