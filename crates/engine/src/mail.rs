use crate::{
    app::{self, App},
    protocol::Command,
};
use luma_core::{
    Error, Result, composer, html_mail, model::*, pagination::UnifiedPager, provider::MailProvider,
};
use serde_json::{Value, json};
use std::{
    collections::{HashMap, HashSet},
    sync::{Arc, atomic::Ordering},
};

pub struct Undo {
    pub account: String,
    pub message: String,
    pub key: ThreadKey,
    pub actions: Vec<MailAction>,
}

pub async fn change_view(app: &Arc<App>, c: &Command) -> Result<()> {
    app.sync.acknowledge();
    let folder = app::folder(c.text("folder"))?;
    let accounts = c.data["accounts"].as_array().map(|a| {
        a.iter()
            .filter_map(|v| v.as_str().map(str::to_owned))
            .collect::<HashSet<_>>()
    });
    let search = c.text("search").trim().to_owned();
    if search.len() > 8192 {
        return Err(Error::ResponseLimit);
    }
    {
        let mut v = app.view.lock().unwrap();
        v.folder = folder.clone();
        v.search = search.clone();
        v.accounts = accounts.clone();
        v.rows.clear();
        v.selected = None;
        v.thread = None;
        v.headers.reset(c.id);
        v.bodies.clear();
    }
    app.events
        .emit(c.id, "viewReset", json!({"generation":c.id}))
        .await;
    if app.preview {
        fixture(app).await;
        return Ok(());
    }
    let clients: Vec<Arc<dyn MailProvider>> = app
        .clients
        .lock()
        .unwrap()
        .iter()
        .filter(|(id, _)| accounts.as_ref().is_none_or(|set| set.contains(*id)))
        .map(|(_, c)| c.clone() as Arc<dyn MailProvider>)
        .collect();
    *app.pager.lock().await = Some(UnifiedPager::new(clients, folder, search));
    page(app, c.id, true).await
}
pub async fn page(app: &Arc<App>, id: u64, reset: bool) -> Result<()> {
    if app.preview {
        return Ok(());
    }
    let generation = app.generation.load(Ordering::SeqCst);
    app.status(
        id,
        "loading",
        if reset {
            "Opening your mailbox"
        } else {
            "Loading the next 25 conversations"
        },
    )
    .await;
    let (items, more) = {
        let mut p = app.pager.lock().await;
        let Some(p) = p.as_mut() else { return Ok(()) };
        let items = p.next_page().await?;
        (items, p.has_more())
    };
    if !app.current(generation) {
        return Ok(());
    }
    {
        let mut v = app.view.lock().unwrap();
        let capacity = 1000usize.saturating_sub(v.rows.len());
        v.rows.extend(items.iter().take(capacity).cloned());
    }
    app.events
        .emit(
            id,
            "rows",
            json!({"generation":generation,"items":items,"more":more,"reset":reset}),
        )
        .await;
    app.status(id, "idle", "Up to date").await;
    Ok(())
}
pub async fn open(app: &Arc<App>, c: &Command) -> Result<()> {
    let key = app::key(&c.data)?;
    let reading = c.id;
    {
        let mut v = app.view.lock().unwrap();
        v.selected = Some(key.clone());
        v.thread = None;
        v.headers.reset(c.id);
        v.bodies.clear();
    }
    app.events
        .emit(c.id, "readerLoading", json!({"key":key}))
        .await;
    if app.preview {
        let row = app
            .view
            .lock()
            .unwrap()
            .rows
            .iter()
            .find(|r| r.key == key)
            .cloned();
        if let Some(row) = row {
            let count = std::env::var("LUMA_PREVIEW_THREAD_COUNT")
                .ok()
                .and_then(|s| s.parse::<usize>().ok())
                .unwrap_or(1)
                .clamp(1, 5000);
            let messages = (0..count)
                .map(|i| Message {
                    id: if i + 1 == count {
                        key.id.clone()
                    } else {
                        format!("{}-message-{i}", key.id)
                    },
                    thread_id: key.id.clone(),
                    internal_date: (row.date - count as i64 + i as i64).to_string(),
                    payload: Part {
                        headers: vec![
                            Header {
                                name: "From".into(),
                                value: row.sender.clone(),
                            },
                            Header {
                                name: "To".into(),
                                value: "alex@example.com".into(),
                            },
                            Header {
                                name: "Subject".into(),
                                value: row.subject.clone(),
                            },
                        ],
                        ..Default::default()
                    },
                    ..Default::default()
                })
                .collect();
            let mut thread = Thread {
                id: key.id.clone(),
                messages,
                ..Default::default()
            };
            if row.unread {
                thread
                    .messages
                    .last_mut()
                    .unwrap()
                    .label_ids
                    .push("UNREAD".into());
            }
            if row.starred {
                thread
                    .messages
                    .last_mut()
                    .unwrap()
                    .label_ids
                    .push("STARRED".into());
            }
            let (messages, window) = {
                let v = app.view.lock().unwrap();
                (
                    thread.messages[v.headers.range(&thread.messages)]
                        .iter()
                        .map(message_header)
                        .collect::<Vec<_>>(),
                    v.headers.describe(&thread.messages),
                )
            };
            app.view.lock().unwrap().thread = Some(thread);
            app.events
                .emit(
                    c.id,
                    "thread",
                    json!({"key":key,"subject":row.subject,"messages":messages,"window":window}),
                )
                .await;
            preview_body(app, c.id, &key.account, &key.id, reading).await;
        }
        return Ok(());
    }
    let client = app.client(&key.account)?;
    let mut thread = client.thread_metadata(&key.id).await?;
    thread.messages.sort_by_key(Message::date);
    if app.reading.load(Ordering::SeqCst) != reading {
        return Ok(());
    }
    let (messages, window, subject, last) = {
        let mut v = app.view.lock().unwrap();
        if app.reading.load(Ordering::SeqCst) != reading || v.selected.as_ref() != Some(&key) {
            return Ok(());
        }
        let messages = thread.messages[v.headers.range(&thread.messages)]
            .iter()
            .map(message_header)
            .collect::<Vec<_>>();
        let window = v.headers.describe(&thread.messages);
        let subject = thread
            .messages
            .last()
            .map(|m| m.payload.header("Subject").to_owned())
            .unwrap_or_default();
        let last = thread.messages.last().map(|m| m.id.clone());
        v.thread = Some(thread);
        (messages, window, subject, last)
    };
    app.events
        .emit(
            c.id,
            "thread",
            json!({"key":key,"subject":subject,"messages":messages,"window":window}),
        )
        .await;
    if let Some(message) = last {
        let request = Command {
            id: c.id,
            action: "message".into(),
            data: json!({"account":key.account,"id":message,"reading":reading}),
        };
        body(app, &request).await?;
    }
    Ok(())
}
async fn preview_body(app: &Arc<App>, id: u64, account: &str, message: &str, reading: u64) {
    let has_html =
        message == "preview0" && std::env::var("LUMA_PREVIEW_HTML").is_ok_and(|v| v == "1");
    let text = "This is a synthetic design preview. No Gmail account or local mailbox is used.\n\nhttps://example.com/review";
    app.events.emit(id,"body",json!({"account":account,"reading":reading,"id":message,"text":text,"links":crate::reader::links(text),"hasHtml":has_html,"externalImages":has_html,"attachments":[]})).await;
}
pub async fn thread_page(app: &Arc<App>, c: &Command) -> Result<()> {
    let key = app::key(&c.data["key"])?;
    let reading = app.reading.load(Ordering::SeqCst);
    if c.data["reading"].as_u64() != Some(reading) {
        return Ok(());
    }
    let (messages, window, last) = {
        let mut v = app.view.lock().unwrap();
        if v.selected.as_ref() != Some(&key) {
            return Ok(());
        }
        let app::View {
            thread, headers, ..
        } = &mut *v;
        let thread = thread
            .as_ref()
            .ok_or_else(|| Error::Invalid("Wait for the conversation headers".into()))?;
        headers.change(&thread.messages, c.text("direction"), c.id)?;
        let range = headers.range(&thread.messages);
        (
            thread.messages[range.clone()]
                .iter()
                .map(message_header)
                .collect::<Vec<_>>(),
            headers.describe(&thread.messages),
            range.last().map(|i| thread.messages[i].id.clone()),
        )
    };
    app.events
        .emit(
            c.id,
            "threadHeaders",
            json!({"key":key,"reading":reading,"messages":messages,"window":window}),
        )
        .await;
    if let Some(message) = last {
        body(
            app,
            &Command {
                id: c.id,
                action: "message".into(),
                data: json!({"account":key.account,"id":message,"reading":reading}),
            },
        )
        .await?;
    }
    Ok(())
}
pub fn message_header(m: &Message) -> Value {
    let attachments: Vec<_> = m
        .payload
        .attachments()
        .iter()
        .filter(|p| {
            p.header("Content-ID").is_empty()
                || p.header("Content-Disposition")
                    .to_ascii_lowercase()
                    .starts_with("attachment")
        })
        .map(|p| json!({"name":p.filename,"mime":p.mime_type,"size":p.body.size,"part":p.part_id}))
        .collect();
    let mut value = json!({"id":m.id,"from":m.payload.header("From"),"to":m.payload.header("To"),"cc":m.payload.header("Cc"),"bcc":m.payload.header("Bcc"),"replyTo":m.payload.header("Reply-To"),"messageId":m.payload.header("Message-ID"),"sentDate":m.payload.header("Date"),"date":m.date(),"subject":m.payload.header("Subject"),"unread":m.has("UNREAD"),"starred":m.has("STARRED"),"important":m.has("IMPORTANT"),"draft":m.has("DRAFT"),"attachments":attachments});
    // Metadata projections do not contain MIME structure. Absence is unknown,
    // not an empty attachment set that may erase an already-opened strip.
    if m.payload.mime_type.is_empty() {
        value.as_object_mut().unwrap().remove("attachments");
    }
    value
}
pub async fn body(app: &Arc<App>, c: &Command) -> Result<()> {
    let key = app::key(&c.data)?;
    let reading = app.reading.load(Ordering::SeqCst);
    if c.data["reading"]
        .as_u64()
        .is_some_and(|epoch| epoch != reading)
    {
        return Ok(());
    }
    let mutation_serial = app.mutation_serial.load(Ordering::SeqCst);
    {
        let v = app.view.lock().unwrap();
        if v.selected
            .as_ref()
            .is_none_or(|selected| selected.account != key.account)
            || !v
                .thread
                .as_ref()
                .is_some_and(|thread| thread.messages.iter().any(|m| m.id == key.id))
        {
            return Err(Error::Invalid(
                "Choose a message in the open conversation".into(),
            ));
        }
    }
    {
        let mut v = app.view.lock().unwrap();
        let app::View {
            thread, headers, ..
        } = &mut *v;
        if let Some(thread) = thread {
            headers.focus(&thread.messages, &key.id);
        }
    }
    if app.preview {
        preview_body(app, c.id, &key.account, &key.id, reading).await;
        return Ok(());
    }
    let client = app.client(&key.account)?;
    let resident = app.view.lock().unwrap().bodies.get(&key.id);
    let cached = if resident.is_some() {
        None
    } else {
        let mut cache = app.cache.lock().unwrap();
        cache.get(&key.account, &key.id)?
    };
    let message = if let Some(message) = resident {
        Some((*message).clone())
    } else if let Some(bytes) = cached {
        serde_json::from_slice::<Message>(&bytes).ok()
    } else {
        None
    };
    let mut message = match message {
        Some(m) => m,
        None => {
            let m = client.message(&key.id).await?;
            if !m
                .payload
                .attachments()
                .iter()
                .any(|p| p.body.data.is_some())
            {
                let bytes = serde_json::to_vec(&m)?;
                app.cache
                    .lock()
                    .unwrap()
                    .put(&key.account, &key.id, &bytes)?;
            }
            m
        }
    };
    if app.reading.load(Ordering::SeqCst) != reading {
        return Ok(());
    }
    if let Some(labels) = app
        .view
        .lock()
        .unwrap()
        .thread
        .as_ref()
        .and_then(|t| t.messages.iter().find(|m| m.id == message.id))
        .map(|m| m.label_ids.clone())
    {
        message.label_ids = labels;
    }
    let text = composer::readable(&message.payload)?;
    let html = html_mail::body(&message.payload)?.filter(|h| html_mail::needs_layout(h));
    let attachments = message_header(&message)["attachments"].clone();
    // Bodies are retained only for the selected reader. No historical preload.
    {
        let mut v = app.view.lock().unwrap();
        if app.reading.load(Ordering::SeqCst) != reading
            || v.selected.as_ref().is_none_or(|selected| {
                selected.account != key.account || selected.id != message.thread_id
            })
        {
            return Ok(());
        }
        v.bodies.insert(Arc::new(message.clone()));
    }
    let external = luma_core::mail_images::sources(&message.payload)
        .iter()
        .any(|s| s.starts_with("https://"));
    let links = crate::reader::links(&text);
    app.events.emit(c.id,"body",json!({"account":key.account,"reading":reading,"id":message.id,"text":text,"links":links,"hasHtml":html.is_some(),"attachments":attachments,"externalImages":external})).await;
    if message.has("UNREAD") {
        let _lock = app.mutation.lock().await;
        if !app.protect_reader_write(c.id, reading, mutation_serial) {
            return Ok(());
        }
        client.act(&message.id, true, &MailAction::Read).await?;
        app.read_revision.fetch_add(1, Ordering::SeqCst);
        app.sync.reconcile();
        if app.reading.load(Ordering::SeqCst) != reading {
            return Ok(());
        }
        let mut updated = None;
        {
            let mut v = app.view.lock().unwrap();
            let unread = if let Some(thread) = v.thread.as_mut() {
                if let Some(m) = thread.messages.iter_mut().find(|m| m.id == message.id) {
                    m.label_ids.retain(|l| l != "UNREAD");
                }
                thread.messages.iter().any(|m| m.has("UNREAD"))
            } else {
                false
            };
            if let Some(row) = v
                .rows
                .iter_mut()
                .find(|r| r.key.account == key.account && r.key.id == message.thread_id)
            {
                row.unread = unread;
                updated = Some(row.clone());
            }
        }
        if let Some(row) = updated {
            app.events
                .emit(c.id, "rowChanged", json!({"row":row}))
                .await;
        }
        app.events
            .emit(
                c.id,
                "messageState",
                json!({"account":key.account,"reading":reading,"id":message.id,"unread":false}),
            )
            .await;
    }
    Ok(())
}
fn action(value: &str, label: &str) -> Result<MailAction> {
    Ok(match value {
        "read" => MailAction::Read,
        "unread" => MailAction::Unread,
        "star" => MailAction::Star,
        "unstar" => MailAction::Unstar,
        "archive" => MailAction::Archive,
        "inbox" => MailAction::Inbox,
        "trash" => MailAction::Trash,
        "restore" => MailAction::Restore,
        "spam" => MailAction::Spam,
        "notSpam" => MailAction::NotSpam,
        "important" => MailAction::Important,
        "unimportant" => MailAction::Unimportant,
        "addLabel" => MailAction::AddLabel(label.into()),
        "removeLabel" => MailAction::RemoveLabel(label.into()),
        _ => return Err(Error::Invalid("Unknown mail action".into())),
    })
}
pub async fn mutate(app: &Arc<App>, c: &Command) -> Result<()> {
    let keys: Vec<ThreadKey> = c.data["keys"]
        .as_array()
        .map(|a| a.iter().map(app::key).collect())
        .transpose()?
        .unwrap_or_default();
    if keys.is_empty() || keys.len() > 1000 {
        return Err(Error::Invalid("Select loaded conversations first".into()));
    }
    let action = action(c.text("action"), c.text("label"))?;
    let message = c.data["message"].as_str().filter(|id| !id.is_empty());
    if message.is_some() && keys.len() != 1 {
        return Err(Error::Invalid(
            "An individual message action needs one conversation".into(),
        ));
    }
    let _lock = app.mutation.lock().await;
    let generation = app.generation.load(Ordering::SeqCst);
    app.status(c.id, "saving", "Saving your changes to Gmail")
        .await;
    if app.preview {
        if let Some(message) = message {
            let view = app.view.lock().unwrap();
            if view.selected.as_ref() != keys.first() || message != keys[0].id {
                return Err(Error::Invalid(
                    "The message is no longer in the open conversation".into(),
                ));
            }
        }
        let mut changed = Vec::new();
        let mut removed = Vec::new();
        let mut flags = Vec::new();
        {
            let mut v = app.view.lock().unwrap();
            for key in &keys {
                if let Some(row) = v.rows.iter_mut().find(|r| &r.key == key) {
                    match action {
                        MailAction::Trash | MailAction::Archive => removed.push(key.clone()),
                        MailAction::Read => row.unread = false,
                        MailAction::Unread => row.unread = true,
                        MailAction::Star => row.starred = true,
                        MailAction::Unstar => row.starred = false,
                        MailAction::Important => row.important = true,
                        MailAction::Unimportant => row.important = false,
                        _ => (),
                    };
                    changed.push(row.clone());
                    flags.push(json!({"account":key.account,"thread":key.id,"message":{"id":key.id,"unread":row.unread,"starred":row.starred,"important":row.important}}));
                }
            }
            v.rows.retain(|r| !removed.contains(&r.key));
        }
        app.events
            .emit(
                c.id,
                "mutation",
                json!({"generation":generation,"removed":removed,"changed":changed,"undo":false,"messageFlags":flags}),
            )
            .await;
        app.status(c.id, "idle", "Preview updated").await;
        return Ok(());
    }
    let source = app.client(&keys[0].account)?;
    let label_name = if let MailAction::AddLabel(id) | MailAction::RemoveLabel(id) = &action {
        Some(
            source
                .labels()
                .await?
                .into_iter()
                .find(|l| &l.id == id)
                .ok_or_else(|| Error::Invalid("Label no longer exists".into()))?
                .name,
        )
    } else {
        None
    };
    let mut changed = Vec::new();
    let mut removed = Vec::new();
    let mut inverse = Vec::new();
    let mut failures = Vec::new();
    let mut needs_refresh = false;
    let mut flags = Vec::new();
    let mut label_ids = HashMap::new();
    for key in keys {
        let client = match app.client(&key.account) {
            Ok(c) => c,
            Err(e) => {
                failures.push(e.to_string());
                continue;
            }
        };
        let mut operation = action.clone();
        if let Some(name) = &label_name
            && key.account != source.account
        {
            if !label_ids.contains_key(&key.account) {
                label_ids.insert(
                    key.account.clone(),
                    client.matching_label(name).await.map_err(|e| e.to_string()),
                );
            }
            match &label_ids[&key.account] {
                Ok(id) => {
                    operation = if matches!(action, MailAction::AddLabel(_)) {
                        MailAction::AddLabel(id.clone())
                    } else {
                        MailAction::RemoveLabel(id.clone())
                    }
                }
                Err(e) => {
                    failures.push(e.clone());
                    continue;
                }
            }
        }
        let before = match client.thread_metadata(&key.id).await {
            Ok(t) => t,
            Err(e) => {
                failures.push(e.to_string());
                continue;
            }
        };
        if message.is_some_and(|id| !before.messages.iter().any(|m| m.id == id)) {
            failures.push("The message no longer belongs to this conversation".into());
            continue;
        }
        let confirmed = match client
            .act_confirmed(message.unwrap_or(&key.id), message.is_some(), &operation)
            .await
        {
            Err(e) => {
                failures.push(e.to_string());
                continue;
            }
            Ok(messages) => messages,
        };
        for m in &before.messages {
            if message.is_some_and(|id| id != m.id) {
                continue;
            }
            let actions = operation.undo_for(m);
            if !actions.is_empty() {
                inverse.push(Undo {
                    account: key.account.clone(),
                    message: m.id.clone(),
                    key: key.clone(),
                    actions,
                })
            }
        }
        let after = confirmed_thread(before, &confirmed, &operation, message);
        {
            let mut view = app.view.lock().unwrap();
            if view.selected.as_ref() == Some(&key) {
                flags.extend(after.messages[view.headers.range(&after.messages)].iter().map(|m|json!({"account":key.account,"thread":key.id,"message":message_header(m)})));
                view.thread = Some(after.clone());
            }
        }
        app.sync.track_write(key.clone());
        let (folder, query, attached) = {
            let v = app.view.lock().unwrap();
            (
                v.folder.clone(),
                v.search.clone(),
                v.rows
                    .iter()
                    .find(|r| r.key == key)
                    .is_some_and(|r| r.attachments),
            )
        };
        let visible = client
            .matches_view(&after, &folder, &query)
            .await
            .ok()
            .flatten();
        needs_refresh |= visible.is_none();
        if visible != Some(false) {
            if let Some(mut row) = Conversation::from_thread(&key.account, &after) {
                row.attachments = attached;
                changed.push(row)
            }
        } else {
            removed.push(key)
        }
    }
    if !inverse.is_empty() {
        *app.undo.lock().unwrap() = inverse;
    }
    if app.current(generation) {
        {
            let mut v = app.view.lock().unwrap();
            v.rows.retain(|r| !removed.contains(&r.key));
            for row in &changed {
                if let Some(old) = v.rows.iter_mut().find(|r| r.key == row.key) {
                    *old = row.clone()
                }
            }
        }
        let has_undo = !app.undo.lock().unwrap().is_empty();
        app.events.emit(c.id,"mutation",json!({"generation":generation,"removed":removed,"changed":changed,"undo":has_undo,"needsRefresh":needs_refresh,"messageFlags":flags})).await;
    }
    if !failures.is_empty() {
        return Err(Error::Invalid(failures.join("\n")));
    }
    app.status(
        c.id,
        "idle",
        if needs_refresh {
            "Saved in Gmail · Refresh to reconcile this search"
        } else {
            "Changes saved · Undo available"
        },
    )
    .await;
    Ok(())
}
fn confirmed_thread(
    mut before: Thread,
    confirmed: &[Message],
    action: &MailAction,
    only: Option<&str>,
) -> Thread {
    for m in &mut before.messages {
        if only.is_some_and(|id| m.id != id) {
            continue;
        }
        if let Some(after) = confirmed.iter().find(|a| a.id == m.id) {
            m.label_ids = after.label_ids.clone();
            continue;
        }
        let (add, remove) = action.labels();
        m.label_ids.retain(|l| !remove.contains(l));
        for l in add {
            if !m.label_ids.contains(&l) {
                m.label_ids.push(l)
            }
        }
        if *action == MailAction::Trash {
            m.label_ids.retain(|l| l != "INBOX");
            if !m.has("TRASH") {
                m.label_ids.push("TRASH".into())
            }
        }
        if *action == MailAction::Restore {
            m.label_ids.retain(|l| l != "TRASH")
        }
    }
    before
}

pub async fn undo(app: &Arc<App>, id: u64) -> Result<()> {
    let _lock = app.mutation.lock().await;
    let undo = std::mem::take(&mut *app.undo.lock().unwrap());
    let mut keys = HashSet::new();
    for item in undo {
        let client = app.client(&item.account)?;
        for action in item.actions {
            client.act(&item.message, true, &action).await?;
        }
        keys.insert(item.key);
    }
    for key in keys {
        app.sync.track_write(key.clone());
        let client = app.client(&key.account)?;
        let thread = client.thread_metadata(&key.id).await?;
        let (folder, query) = {
            let v = app.view.lock().unwrap();
            (v.folder.clone(), v.search.clone())
        };
        if client.matches_view(&thread, &folder, &query).await? == Some(true)
            && let Some(mut row) = Conversation::from_thread(&key.account, &thread)
        {
            {
                let mut v = app.view.lock().unwrap();
                row.attachments |= v.rows.iter().any(|old| old.key == key && old.attachments);
                v.rows.retain(|r| r.key != row.key);
                v.rows.push(row.clone());
                v.rows.sort_by_key(|row| std::cmp::Reverse(row.date));
                v.rows.truncate(1000);
            }
            app.events
                .emit(id, "rowChanged", json!({"row":row,"insert":true}))
                .await;
        }
    }
    app.status(id, "idle", "Change undone").await;
    Ok(())
}
pub async fn fixture(app: &App) {
    fixture_with_count(app, 8).await;
}
pub async fn fixture_with_count(app: &App, count: usize) {
    let samples = [
        (
            "Linear",
            "A clearer way to move work forward",
            "Your workspace digest is ready. A few small improvements, one calmer week.",
        ),
        (
            "Maya Chen",
            "A quick thought on the launch",
            "The new direction is looking good. I left a few thoughts for us to discuss.",
        ),
        (
            "The Design Brief",
            "Small details. A different feeling.",
            "This week: interfaces that make room for you.",
        ),
        (
            "GitHub",
            "Your pull request is ready to review",
            "All checks have passed. Take a look when you have a moment.",
        ),
        (
            "Alex Morgan",
            "Re: Friday plans",
            "Perfect — let's make it happen.",
        ),
        (
            "Figma",
            "A little inspiration for your next idea",
            "Fresh resources, thoughtful details and a few things worth saving.",
        ),
        (
            "Notion",
            "Your week, a little more organised",
            "Make space for the work that matters.",
        ),
        (
            "Luma",
            "Welcome to a little more calm",
            "Your inbox, with room to breathe. Online first. Always yours.",
        ),
    ];
    let now = chrono::Utc::now().timestamp_millis();
    let rows: Vec<_> = samples
        .iter()
        .cycle()
        .take(count.clamp(1, 1000))
        .enumerate()
        .map(|(i, (sender, subject, snippet))| Conversation {
            key: ThreadKey {
                account: "preview".into(),
                id: format!("preview{i}"),
            },
            subject: (*subject).into(),
            sender: (*sender).into(),
            snippet: (*snippet).into(),
            date: now - i as i64 * 3_600_000,
            unread: i < 3,
            starred: i == 1,
            important: false,
            attachments: i == 2,
            count: if i == 1 { 3 } else { 1 },
        })
        .collect();
    app.view.lock().unwrap().rows = rows.clone();
    app.events.emit(0,"rows",json!({"items":rows,"reset":true,"more":false,"generation":app.generation.load(Ordering::SeqCst)})).await;
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn projected_headers_do_not_clear_previously_opened_attachment_strips() {
        let metadata = Message {
            id: "m".into(),
            ..Default::default()
        };
        assert!(message_header(&metadata).get("attachments").is_none());
        let plain = Message {
            payload: Part {
                mime_type: "text/plain".into(),
                ..Default::default()
            },
            ..metadata
        };
        assert_eq!(message_header(&plain)["attachments"], json!([]));
    }
    #[test]
    fn confirmed_single_message_changes_do_not_modify_sibling_labels() {
        let before = Thread {
            id: "thread".into(),
            messages: vec![
                Message {
                    id: "first".into(),
                    label_ids: vec!["INBOX".into(), "UNREAD".into()],
                    ..Default::default()
                },
                Message {
                    id: "second".into(),
                    label_ids: vec!["INBOX".into(), "UNREAD".into()],
                    ..Default::default()
                },
            ],
            ..Default::default()
        };
        let updated = Message {
            id: "first".into(),
            label_ids: vec!["UNREAD".into(), "TRASH".into()],
            ..Default::default()
        };
        let after = confirmed_thread(before, &[updated], &MailAction::Trash, Some("first"));
        assert!(after.messages[0].has("TRASH"));
        assert!(!after.messages[0].has("INBOX"));
        assert!(after.messages[1].has("INBOX"));
        assert!(!after.messages[1].has("TRASH"));
        let after = confirmed_thread(after, &[], &MailAction::Star, Some("second"));
        assert!(!after.messages[0].has("STARRED"));
        assert!(after.messages[1].has("STARRED"));
    }
}
