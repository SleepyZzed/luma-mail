use crate::{
    app::App,
    compose::{Close, Init, Intent, Phase, Session},
    protocol::Command,
};
use luma_core::{
    Error, Result, compose_document as document,
    composer::Compose,
    config::Account,
    model::{Header, Message, Part},
    provider::MailProvider,
    signatures::SignatureStore,
};
use serde_json::{Value, json};
use std::{
    future::Future,
    path::PathBuf,
    sync::{Arc, atomic::Ordering},
    time::Duration,
};

pub enum Prepared {
    Start,
    ComparisonCancelled,
    Update(Arc<Session>),
    Write(Arc<Session>, bool),
    Input(Arc<Session>, InputAction),
    Sender(Arc<Session>, u64, bool),
    Other,
}
pub enum InputAction {
    Begin,
    Ack(usize),
    Finish(Vec<u8>),
    Cancel,
    Failed(String),
}
pub fn session(app: &App, c: &Command) -> Result<Arc<Session>> {
    app.composer
        .lock()
        .unwrap()
        .as_ref()
        .filter(|s| s.id == c.text("session"))
        .cloned()
        .ok_or_else(|| {
            Error::Invalid("This composer has closed; late callbacks were ignored".into())
        })
}
pub fn prepare(app: &App, c: &Command) -> Result<Prepared> {
    match c.action.as_str() {
        "composeCompareCancel" => {
            let s = session(app, c)?;
            let mut d = s.data.lock().unwrap();
            d.comparison = None;
            d.comparison_ticket += 1;
            Ok(Prepared::ComparisonCancelled)
        }
        "composeIdentity" => {
            let s = session(app, c)?;
            let account = s.data.lock().unwrap().form.account.clone();
            let loading = !app.preview
                && app
                    .settings
                    .lock()
                    .unwrap()
                    .accounts
                    .iter()
                    .any(|a| Some(&a.id) == account.as_ref() && a.sender_controls);
            let epoch = s.select_identity(c.text("address"), loading)?;
            Ok(Prepared::Sender(s, epoch, false))
        }
        "composeAccount" => {
            let s = session(app, c)?;
            let account = app
                .settings
                .lock()
                .unwrap()
                .accounts
                .iter()
                .find(|a| a.id == c.text("account") && a.enabled)
                .cloned()
                .ok_or_else(|| Error::Invalid("Choose an enabled account".into()))?;
            let epoch = s.select_account(&account, !app.preview && account.sender_controls)?;
            Ok(Prepared::Sender(s, epoch, true))
        }
        "composeClipboardBegin" => {
            let s = session(app, c)?;
            s.begin_input(c.text("token"))?;
            Ok(Prepared::Input(s, InputAction::Begin))
        }
        "composeClipboardChunk" | "composeClipboardFinish" | "composeClipboardCancel" => {
            let s = session(app, c)?;
            let action = {
                let mut d = s.data.lock().unwrap();
                if !d.input_busy || d.input_token != c.text("token") {
                    return Err(Error::Invalid("This clipboard transfer has ended".into()));
                }
                if c.action == "composeClipboardCancel" {
                    InputAction::Cancel
                } else if c.action == "composeClipboardFinish" {
                    if d.clipboard.as_ref().is_some_and(|i| i.complete()) {
                        d.status = "Preparing clipboard image".into();
                        InputAction::Finish(d.clipboard.take().unwrap().bytes)
                    } else {
                        InputAction::Failed("The clipboard image was incomplete".into())
                    }
                } else {
                    let total = c.data["size"]
                        .as_u64()
                        .and_then(|v| usize::try_from(v).ok())
                        .unwrap_or(usize::MAX);
                    let offset = c.data["offset"]
                        .as_u64()
                        .and_then(|v| usize::try_from(v).ok())
                        .unwrap_or(usize::MAX);
                    match d
                        .clipboard
                        .as_mut()
                        .ok_or_else(|| {
                            Error::Invalid("The image is already being prepared".into())
                        })?
                        .append(total, offset, c.text("bytes"))
                    {
                        Ok(next) => InputAction::Ack(next),
                        Err(e) => InputAction::Failed(e.to_string()),
                    }
                }
            };
            Ok(Prepared::Input(s, action))
        }
        "composeStart" => Ok(Prepared::Start),
        "composeUpdate" => {
            let s = session(app, c)?;
            s.update(&c.data, c.id)?;
            Ok(Prepared::Update(s))
        }
        "composeSave" | "composeSend" => {
            if app.preview {
                return Err(Error::Invalid(
                    "Saving and sending are disabled in the isolated design preview".into(),
                ));
            }
            let s = session(app, c)?;
            if c.data["form"].is_object() {
                s.update(&c.data["form"], c.id)?;
            }
            let queued = s.queue(
                c.id,
                if c.action == "composeSend" {
                    Intent::Send
                } else {
                    Intent::Save
                },
            )?;
            Ok(Prepared::Write(s, queued))
        }
        _ => Ok(Prepared::Other),
    }
}
pub async fn run(app: &Arc<App>, c: &Command, prepared: Prepared) -> Result<()> {
    if c.action == "composePrepareQuit" {
        app.quit_pending.store(true, Ordering::SeqCst);
        if crate::signatures::prepare_quit(app, c.id).await? {
            return Ok(());
        }
        let current = app.composer.lock().unwrap().clone();
        if let Some(s) = current.filter(|s| s.active()) {
            if c.data["form"].is_object() && s.data.lock().unwrap().phase.writable() {
                s.update(&c.data["form"], c.id)?;
            }
            return close(app, c.id, &s).await;
        }
        app.events.emit(c.id, "quitReady", json!({})).await;
        return Ok(());
    }
    match prepared {
        Prepared::ComparisonCancelled => return Ok(()),
        Prepared::Sender(s, epoch, aliases) => {
            return refresh_sender(app, c.id, &s, epoch, aliases, true).await;
        }
        Prepared::Input(s, action) => return input(app, c, &s, action).await,
        Prepared::Start => return start(app, c).await,
        Prepared::Update(s) => {
            emit(app, c.id, &s, false).await;
            return Ok(());
        }
        Prepared::Write(s, queued) => {
            if queued {
                write(app, c.id, &s).await?
            } else {
                emit(app, c.id, &s, false).await;
            }
            return Ok(());
        }
        Prepared::Other => {}
    }
    let s = session(app, c)?;
    match c.action.as_str() {
        "composeClose" => {
            if c.data["form"].is_object() && s.data.lock().unwrap().phase.writable() {
                s.update(&c.data["form"], c.id)?;
            }
            close(app, c.id, &s).await
        }
        "composeCloseAnswer" => {
            let intent = s.close_answer(
                c.data["revision"].as_u64().unwrap_or(u64::MAX),
                c.text("answer"),
            )?;
            if c.text("answer") == "keep" && app.quit_pending.swap(false, Ordering::SeqCst) {
                app.events.emit(c.id, "quitCancelled", json!({})).await;
            }
            if let Some(intent) = intent {
                if app.preview {
                    return Err(Error::Invalid("Draft writes are disabled in design preview; choose Keep editing or Close without saving".into()));
                }
                if s.queue(c.id, intent)? {
                    write(app, c.id, &s).await?;
                }
            }
            emit(app, c.id, &s, false).await;
            Ok(())
        }
        "composeConvert" => {
            {
                let mut d = s.data.lock().unwrap();
                if !d.phase.writable() {
                    return Err(Error::Invalid("Wait for the current operation".into()));
                }
                let body = c.text("text");
                if body.len() > document::MAX_BODY {
                    return Err(Error::ResponseLimit);
                }
                d.form.text = document::clean(body)?;
                d.form.html = true;
                d.source_mode = false;
                d.original_html = None;
                d.revision += 1;
                d.initial_document = json!({"kind":"html","text":d.form.text});
            }
            emit(app, c.id, &s, true).await;
            Ok(())
        }
        "composeAttach" => attach(app, c, &s).await,
        "composeRemoveAttachment" => remove_attachment(app, c, &s).await,
        "composeCancelUpload" => {
            s.cancel.store(true, Ordering::Relaxed);
            s.data.lock().unwrap().pending = None;
            Ok(())
        }
        "composeRetryUpload" => upload(app, c.id, &s).await,
        "composeCompare" | "composeMerge" => crate::draft_conflict::run(app, c, &s).await,
        "composeSaveCopy" => {
            if app.preview {
                return Err(Error::Invalid(
                    "Draft writes are disabled in design preview".into(),
                ));
            }
            {
                let mut d = s.data.lock().unwrap();
                if d.phase != Phase::Conflict {
                    return Err(Error::Invalid(
                        "Only a conflicting draft can be saved as a separate copy".into(),
                    ));
                }
                d.draft = None;
                d.form.message_id = format!("<{}@luma.local>", luma_core::composer::new_id());
                d.phase = Phase::Editing;
                d.revision += 1;
                d.comparison = None;
                d.comparison_ticket += 1;
            }
            if s.queue(c.id, Intent::Save)? {
                write(app, c.id, &s).await?;
            }
            Ok(())
        }
        "composeRecipients" => {
            let account = s
                .data
                .lock()
                .unwrap()
                .form
                .account
                .clone()
                .unwrap_or_default();
            let prefix = c.text("prefix");
            if prefix.len() > 320 {
                return Err(Error::ResponseLimit);
            }
            let enabled = app.settings.lock().unwrap().recent_recipients;
            let rows = if enabled {
                let store = app.recent.clone();
                let account = account.clone();
                let prefix = prefix.to_string();
                tokio::task::spawn_blocking(move || store.recent(&account, &prefix))
                    .await
                    .unwrap_or_default()
            } else {
                vec![]
            };
            app.events
                .emit(
                    c.id,
                    "recipients",
                    json!({"session":s.id,"account":account,"request":c.id,"prefix":prefix,"items":rows}),
                )
                .await;
            Ok(())
        }
        _ => Err(Error::Invalid("Unknown composer action".into())),
    }
}
async fn input(app: &Arc<App>, c: &Command, s: &Arc<Session>, action: InputAction) -> Result<()> {
    let token = c.text("token");
    match action {
        InputAction::Begin => {
            app.events
                .emit(
                    c.id,
                    "clipboardAck",
                    json!({"session":s.id,"token":token,"next":0}),
                )
                .await;
            let owner = Arc::downgrade(app);
            let editor = Arc::downgrade(s);
            let token = token.to_string();
            tokio::spawn(async move {
                tokio::time::sleep(Duration::from_secs(30)).await;
                if let (Some(app), Some(s)) = (owner.upgrade(), editor.upgrade())
                    && s.cancel_input(
                        &token,
                        "Clipboard image preparation timed out; queued Send was cancelled",
                    )
                {
                    app.events.emit(0,"clipboardComplete",json!({"session":s.id,"token":token,"ok":false,"message":"Clipboard image preparation timed out"})).await;
                    emit(&app, 0, &s, false).await;
                    resume_close(&app, 0, &s).await.ok();
                }
            });
        }
        InputAction::Ack(next) => {
            app.events
                .emit(
                    c.id,
                    "clipboardAck",
                    json!({"session":s.id,"token":token,"next":next}),
                )
                .await
        }
        InputAction::Cancel | InputAction::Failed(_) => {
            let message = match action {
                InputAction::Failed(m) => m,
                _ => "Clipboard image cancelled; queued Send was cancelled".into(),
            };
            s.cancel_input(token, &message);
            app.events
                .emit(
                    c.id,
                    "clipboardComplete",
                    json!({"session":s.id,"token":token,"ok":false,"message":message}),
                )
                .await;
        }
        InputAction::Finish(bytes) => {
            let number = s.data.lock().unwrap().form.retained.len() + 1;
            let result = tokio::task::spawn_blocking(move || {
                luma_core::clipboard_image::attachment(bytes, number)
            })
            .await
            .map_err(|_| Error::Invalid("Could not prepare the clipboard image".into()))
            .and_then(|r| r)
            .and_then(|file| s.accept_input(token, file));
            if matches!(result, Ok(false)) {
                return Ok(());
            }
            let error = result.err().map(|e| e.to_string());
            if let Some(message) = &error {
                s.cancel_input(token, message);
            }
            app.events
                .emit(
                    c.id,
                    "clipboardComplete",
                    json!({"session":s.id,"token":token,"ok":error.is_none(),"message":error}),
                )
                .await;
        }
    }
    emit(app, c.id, s, false).await;
    resume_close(app, c.id, s).await
}
async fn resume_close(app: &Arc<App>, id: u64, s: &Arc<Session>) -> Result<()> {
    let waiting = {
        let mut d = s.data.lock().unwrap();
        if !d.input_busy && !d.identity_loading && d.pending.is_none() {
            std::mem::take(&mut d.close_pending)
        } else {
            false
        }
    };
    if waiting {
        Box::pin(close(app, id, s)).await?;
    }
    Ok(())
}
pub async fn emit(app: &App, id: u64, s: &Session, replace: bool) {
    let event = if s.data.lock().unwrap().phase == Phase::Closed {
        "composeClosed"
    } else {
        "compose"
    };
    app.events.emit(id, event, s.snapshot(replace)).await;
    if !s.active() && app.quit_pending.load(Ordering::SeqCst) {
        app.events.emit(id, "quitReady", json!({})).await;
    }
}
async fn close(app: &Arc<App>, id: u64, s: &Arc<Session>) -> Result<()> {
    let mode = app.settings.lock().unwrap().draft_mode;
    match s.close(mode) {
        Close::Closed => emit(app, id, s, false).await,
        Close::Ask(revision) => {
            let phase = s.data.lock().unwrap().phase;
            let data = json!({"session":s.id,"revision":revision,"uncertain":phase==Phase::Uncertain,"conflict":phase==Phase::Conflict});
            app.events.emit(id, "composeCloseQuestion", data).await;
        }

        Close::Wait => {
            emit(app, id, s, false).await;
        }
        Close::Save => {
            if app.preview {
                return Err(Error::Invalid(
                    "Automatic close-time saving is disabled in design preview".into(),
                ));
            }
            if s.queue(id, Intent::SaveAndClose)? {
                write(app, id, s).await?;
            }
        }
    }
    Ok(())
}
async fn start(app: &Arc<App>, c: &Command) -> Result<()> {
    let _start = app.compose_start.lock().await;
    if app.quit_pending.load(Ordering::SeqCst) {
        return Err(Error::Invalid(
            "Finish closing the current message before opening another".into(),
        ));
    }
    let current = app.composer.lock().unwrap().clone();
    let changing = current
        .as_ref()
        .filter(|s| s.active() && c.flag("changeMode"))
        .cloned();
    if c.flag("changeMode") && changing.is_none() {
        return Err(Error::Invalid("This reply has closed".into()));
    }
    if let Some(s) = current.filter(|s| s.active() && !c.flag("changeMode")) {
        app.events
            .emit(c.id, "composeFocus", json!({"session":s.id}))
            .await;
        if !c.text("mailto").is_empty() {
            return Err(Error::Invalid(
                "A message is already open. Finish or close it, then open this email link again."
                    .into(),
            ));
        }
        return Ok(());
    }
    let mode = c.text("mode");
    if !["new", "reply", "replyAll", "forward", "forwardAll", "draft"].contains(&mode) {
        return Err(Error::Invalid("Unknown compose mode".into()));
    }
    let revision = if let Some(s) = &changing {
        if s.id != c.text("session") {
            return Err(Error::Invalid("This composer has changed".into()));
        }
        if c.data["form"].is_object() {
            s.update(&c.data["form"], c.id)?;
        }
        let d = s.data.lock().unwrap();
        if d.phase != Phase::Editing
            || d.pending.is_some()
            || d.input_busy
            || d.draft.is_some()
            || d.source_mode
            || !["reply", "replyAll", "forward", "forwardAll"].contains(&d.mode.as_str())
            || !["reply", "replyAll", "forward", "forwardAll"].contains(&mode)
        {
            return Err(Error::Invalid(
                "Finish the current draft or upload before changing reply mode".into(),
            ));
        }
        Some((d.revision, d.attachment_generation))
    } else {
        None
    };
    let key = if c.data["key"].is_object() {
        Some(crate::app::key(&c.data["key"])?)
    } else {
        None
    };
    let requested = c.data["account"]
        .as_str()
        .filter(|s| !s.is_empty())
        .or_else(|| key.as_ref().map(|k| k.account.as_str()));
    if let Some(s) = &changing {
        let d = s.data.lock().unwrap();
        if d.key != key
            || requested != d.form.account.as_deref()
            || c.data["message"]["id"].as_str() != d.source_message.as_deref()
        {
            return Err(Error::Invalid(
                "Reply mode must remain in its original account and conversation".into(),
            ));
        }
    }
    let account = {
        let settings = app.settings.lock().unwrap();
        let id = requested.or(settings.default_account.as_deref());
        settings
            .accounts
            .iter()
            .find(|a| a.enabled && Some(a.id.as_str()) == id)
            .or_else(|| settings.accounts.iter().find(|a| a.enabled))
            .cloned()
            .ok_or_else(|| Error::Invalid("Connect an account before composing".into()))?
    };
    if requested.is_some_and(|id| id != account.id) {
        return Err(Error::Invalid(
            "The selected sending account is disabled or disconnected".into(),
        ));
    }
    let mut init = Init {
        form: Compose::new(account.email.clone()),
        primary: account.email.clone(),
        mode: mode.into(),
        key: key.clone(),
        source_message: None,
        draft: None,
        quote: String::new(),
        signature: String::new(),
        original_html: None,
        source_mode: false,
        document: json!({"kind":"plain","text":""}),
    };
    if mode == "new" && !c.text("mailto").is_empty() {
        let link = luma_core::mailto::parse(c.text("mailto"))?;
        init.form.to = link.to;
        init.form.cc = link.cc;
        init.form.bcc = link.bcc;
        init.form.subject = link.subject;
        init.form.text = link.body;
        init.document = json!({"kind":"plain","text":init.form.text});
    }
    if mode != "new" {
        let key = key
            .as_ref()
            .ok_or_else(|| Error::Invalid("Choose a message first".into()))?;
        let id = if let Some(id) = c.data["message"]["id"].as_str() {
            id.to_owned()
        } else if app.preview {
            key.id.clone()
        } else {
            let resident = {
                let view = app.view.lock().unwrap();
                if view.selected.as_ref() == Some(key) {
                    view.thread.clone()
                } else {
                    None
                }
            };
            let thread = match resident {
                Some(t) => t,
                None => app.client(&key.account)?.thread_metadata(&key.id).await?,
            };
            thread
                .messages
                .iter()
                .filter(|m| mode != "draft" || m.has("DRAFT"))
                .max_by_key(|m| m.date())
                .ok_or_else(|| {
                    Error::Invalid(if mode == "draft" {
                        "This conversation no longer contains a Gmail draft".into()
                    } else {
                        "This conversation has no message to reply to".into()
                    })
                })?
                .id
                .clone()
        };
        init.source_message = Some(id.clone());
        let original = if app.preview {
            synthetic(&id, &key.id)
        } else {
            let cached = app.view.lock().unwrap().bodies.get(&id);
            if let Some(m) = cached {
                (*m).clone()
            } else {
                app.client(&key.account)?.message(&id).await?
            }
        };
        if original.thread_id != key.id {
            return Err(Error::Invalid(
                "The reply message belongs to a different conversation".into(),
            ));
        }
        if mode == "draft" {
            if app.preview {
                return Err(Error::Invalid(
                    "No Gmail draft exists for synthetic mail".into(),
                ));
            }
            let client = app.client(&account.id)?;
            let found = client
                .find_draft(&original.id, original.payload.header("Message-ID"))
                .await?;
            let (draft, retained) = client.edit_draft(&found.id).await?;
            init = from_draft(&account, draft, retained)?;
        } else if matches!(mode, "forward" | "forwardAll") {
            init.form.subject = format!("Fwd: {}", original.payload.header("Subject"));
            let mut messages = if mode == "forwardAll" {
                if app.preview {
                    vec![original.clone(), synthetic("previous", &key.id)]
                } else {
                    let resident = {
                        let view = app.view.lock().unwrap();
                        if view.selected.as_ref() == Some(key) {
                            view.thread.clone()
                        } else {
                            None
                        }
                    };
                    let thread = match resident {
                        Some(t) => t,
                        None => app.client(&account.id)?.thread_metadata(&key.id).await?,
                    };
                    if thread.messages.len() > 100 {
                        return Err(Error::Invalid("This conversation exceeds the 100-message forwarding limit. Forward individual messages instead.".into()));
                    }
                    thread.messages
                }
            } else {
                vec![original.clone()]
            };
            messages.sort_by_key(|m| m.date());
            for metadata in messages {
                let message = if metadata.id == original.id {
                    original.clone()
                } else if app.preview {
                    metadata
                } else {
                    app.client(&account.id)?.message(&metadata.id).await?
                };
                append_forward(&mut init.quote, &message)?;
                if !app.preview {
                    merge_attachments(
                        &mut init.form.retained,
                        app.client(&account.id)?
                            .message_attachments(&message)
                            .await?,
                    )?;
                }
            }
        } else {
            init.form = Compose::reply(account.email.clone(), &original, mode == "replyAll");
            let previous = luma_core::composer::readable(&original.payload)?;
            let excerpt: String = previous.chars().take(32768).collect();
            init.quote = format!(
                "On {}, {} wrote:\n{}",
                original.payload.header("Date"),
                original.payload.header("From"),
                excerpt
            );
        }
    }
    init.form.account = Some(account.id.clone());
    if app.preview && std::env::var("LUMA_PREVIEW_CONFLICT").as_deref() == Ok("1") {
        crate::draft_conflict::fixture_init(&mut init);
    }
    if let Some(s) = changing {
        {
            let mut d = s.data.lock().unwrap();
            if Some((d.revision, d.attachment_generation)) != revision
                || d.phase != Phase::Editing
                || d.pending.is_some()
                || d.input_busy
            {
                return Err(Error::Invalid("Your message changed while preparing the forward. Your edits are preserved; try again.".into()));
            }
            let mut retained = d.form.retained.clone();
            merge_attachments(&mut retained, init.form.retained)?;
            d.form.retained = retained;
            if mode == "reply" || mode == "replyAll" {
                d.form.to = init.form.to;
                d.form.cc = init.form.cc;
                d.form.bcc = init.form.bcc;
            } else if d.mode == "reply" || d.mode == "replyAll" {
                d.form.to.clear();
                d.form.cc.clear();
                d.form.bcc.clear();
            }
            d.form.subject = init.form.subject;
            d.form.thread = init.form.thread;
            d.form.reply_to = init.form.reply_to;
            d.form.references = init.form.references;
            d.quote = init.quote;
            d.include_quote = true;
            d.mode = mode.into();
            d.source_message = init.source_message;
            d.revision += 1;
            d.attachment_generation += 1;
            d.close_question = None;
            d.initial_document = if d.form.html {
                json!({"kind":"html","text":d.form.text})
            } else {
                json!({"kind":"plain","text":d.form.text})
            };
        }
        emit(app, c.id, &s, true).await;
        return Ok(());
    }
    install(app, c.id, init, None).await
}
fn append_forward(output: &mut String, message: &Message) -> Result<()> {
    let text = luma_core::composer::readable(&message.payload)?;
    let header = format!(
        "---------- Forwarded message ----------\nFrom: {}\nDate: {}\nTo: {}\nSubject: {}\n\n",
        message.payload.header("From"),
        message.payload.header("Date"),
        message.payload.header("To"),
        message.payload.header("Subject")
    );
    if output
        .len()
        .saturating_add(header.len())
        .saturating_add(text.len())
        .saturating_add(2)
        > document::MAX_BODY
    {
        return Err(Error::Invalid("Forwarded content exceeds the in-memory message limit. Forward individual messages instead.".into()));
    }
    output.push_str(&header);
    output.push_str(&text);
    output.push_str("\n\n");
    Ok(())
}
#[cfg(test)]
mod forwarding_tests {
    use super::*;
    #[test]
    fn preserves_complete_history_and_refuses_overflow_without_truncating() {
        let message = synthetic("one", "thread");
        let mut quote = String::new();
        append_forward(&mut quote, &message).unwrap();
        append_forward(&mut quote, &message).unwrap();
        assert_eq!(quote.matches("Forwarded message").count(), 2);
        assert!(quote.contains("From:"));
        assert!(quote.contains("To:"));
        let mut full = "x".repeat(document::MAX_BODY);
        let before = full.len();
        assert!(append_forward(&mut full, &message).is_err());
        assert_eq!(full.len(), before);
    }
    #[test]
    fn forwarded_attachments_deduplicate_and_keep_the_memory_limit() {
        let file = luma_core::composer::RetainedAttachment {
            name: "image.png".into(),
            mime_type: "image/png".into(),
            data: Arc::new(vec![1, 2, 3]),
            content_id: None,
        };
        let mut files = vec![file.clone()];
        merge_attachments(&mut files, vec![file]).unwrap();
        assert_eq!(files.len(), 1);
        let oversized = luma_core::composer::RetainedAttachment {
            name: "large.bin".into(),
            mime_type: "application/octet-stream".into(),
            data: Arc::new(vec![0; 16 * 1024 * 1024]),
            content_id: None,
        };
        assert!(merge_attachments(&mut files, vec![oversized]).is_err());
    }
}
fn merge_attachments(
    output: &mut Vec<luma_core::composer::RetainedAttachment>,
    incoming: Vec<luma_core::composer::RetainedAttachment>,
) -> Result<()> {
    for file in incoming {
        if !output.iter().any(|old| {
            old.name == file.name
                && old.mime_type == file.mime_type
                && old.data == file.data
                && old.content_id == file.content_id
        }) {
            output.push(file);
        }
    }
    if output.len() > 32
        || output.iter().map(|f| f.data.len() as u64).sum::<u64>()
            > luma_core::composer::MAX_ATTACHMENT_BYTES
    {
        return Err(Error::Invalid("Forwarded attachments exceed the 16 MiB/32-file memory limit. Forward individual messages or use Drive links.".into()));
    }
    Ok(())
}
pub(crate) async fn install(
    app: &Arc<App>,
    id: u64,
    init: Init,
    replacing: Option<(&Arc<Session>, u64, u64)>,
) -> Result<()> {
    let account = app
        .settings
        .lock()
        .unwrap()
        .accounts
        .iter()
        .find(|a| Some(&a.id) == init.form.account.as_ref() && a.enabled)
        .cloned()
        .ok_or_else(|| Error::Invalid("Reconnect this account before opening the draft".into()))?;
    let s = Session::new(init);
    if s.data.lock().unwrap().mode == "merged" {
        let mut d = s.data.lock().unwrap();
        d.revision = 1;
        d.status = "Merged copy · original Gmail draft preserved".into();
    }
    if app.preview
        && std::env::var("LUMA_PREVIEW_CONFLICT").as_deref() == Ok("1")
        && s.data.lock().unwrap().draft.is_some()
    {
        let mut d = s.data.lock().unwrap();
        d.phase = Phase::Conflict;
        d.form.text = "My edited message".into();
        d.revision = 1;
        d.initial_document = json!({"kind":"plain","text":d.form.text});
    }
    if account.sender_controls && !app.preview {
        s.data.lock().unwrap().identity_loading = true;
    }
    {
        let mut current = app.composer.lock().unwrap();
        if let Some((old, revision, ticket)) = replacing {
            let mut data = old.data.lock().unwrap();
            if !current.as_ref().is_some_and(|at| Arc::ptr_eq(at, old))
                || data.phase != Phase::Conflict
                || data.revision != revision
                || data.comparison_ticket != ticket
            {
                return Err(Error::Invalid(
                    "The draft changed while preparing the merge; your editor is preserved".into(),
                ));
            }
            crate::compose::clear(&mut data, Phase::Closed);
        }
        *current = Some(s.clone());
    }
    app.events.emit(id, "composeOpened", s.snapshot(true)).await;
    if account.sender_controls && !app.preview {
        refresh_sender(app, id, &s, 0, true, false).await
    } else {
        Ok(())
    }
}
pub(crate) fn from_draft(
    account: &Account,
    draft: luma_core::gmail::Draft,
    retained: Vec<luma_core::composer::RetainedAttachment>,
) -> Result<Init> {
    let p = &draft.message.payload;
    let mut form = Compose::new(account.email.clone());
    form.account = Some(account.id.clone());
    form.from = p.header("From").into();
    form.sender_reply_to = p.header("Reply-To").into();
    form.to = p.header("To").into();
    form.cc = p.header("Cc").into();
    form.bcc = p.header("Bcc").into();
    form.subject = p.header("Subject").into();
    form.thread = Some(draft.message.thread_id.clone());
    form.reply_to = p.header("In-Reply-To").into();
    form.references = p.header("References").into();
    if !p.header("Message-ID").is_empty() {
        form.message_id = p.header("Message-ID").into();
    }
    form.retained = retained;
    form.drive_files =
        luma_core::compose_drive::draft_references(p.header("X-Luma-Drive-Attachments"))?;
    form.text = luma_core::composer::readable(p)?;
    if form.text.len() > document::MAX_BODY {
        return Err(Error::ResponseLimit);
    }
    let mut init = Init {
        form,
        primary: account.email.clone(),
        mode: "draft".into(),
        key: Some(luma_core::model::ThreadKey {
            account: account.id.clone(),
            id: draft.message.thread_id.clone(),
        }),
        source_message: None,
        draft: None,
        quote: String::new(),
        signature: String::new(),
        original_html: None,
        source_mode: false,
        document: Value::Null,
    };
    if let Some(html) = luma_core::html_mail::body(p)? {
        if html.len() > document::MAX_BODY {
            return Err(Error::ResponseLimit);
        }
        init.original_html = Some(html.clone());
        if let Some(parts) = document::sections(&html) {
            init.form.text = parts.body.clone();
            init.form.html = true;
            init.quote = parts.quote;
            init.signature = parts.signature;
            init.document = json!({"kind":"html","text":parts.body});
        } else if let Some(rich) = luma_core::rich_draft::parse(&html) {
            let spans: Vec<_> = rich
                .spans
                .iter()
                .map(|s| json!({"start":s.start,"end":s.end,"tag":s.tag}))
                .collect();
            init.form.text = rich.text.clone();
            init.form.html = false;
            init.quote = rich.quote;
            init.signature = rich.signature;
            init.document = json!({"kind":"legacy","text":rich.text,"spans":spans});
        } else {
            init.form.text = html.clone();
            init.form.html = true;
            init.source_mode = true;
            init.document = json!({"kind":"source","text":html});
        }
    } else {
        init.document = json!({"kind":"plain","text":init.form.text});
    }
    init.draft = Some(draft);
    Ok(init)
}
fn synthetic(id: &str, thread: &str) -> Message {
    use base64::Engine;
    Message{id:id.into(),thread_id:thread.into(),payload:Part{mime_type:"text/plain".into(),headers:vec![
        Header{name:"From".into(),value:"Maya Chen <maya@example.com>".into()},Header{name:"To".into(),value:"alex@example.com".into()},Header{name:"Subject".into(),value:"A quick thought on the launch".into()},Header{name:"Message-ID".into(),value:"<synthetic@example.com>".into()},Header{name:"Date".into(),value:"Tuesday".into()}],body:luma_core::model::PartBody{data:Some(base64::engine::general_purpose::URL_SAFE_NO_PAD.encode("Earlier synthetic message. This must stay outside editable reply text.").into()),..Default::default()},..Default::default()},..Default::default()}
}
async fn refresh_sender(
    app: &Arc<App>,
    id: u64,
    s: &Arc<Session>,
    epoch: u64,
    aliases: bool,
    force: bool,
) -> Result<()> {
    let loading = s.data.lock().unwrap().identity_loading;
    if loading {
        emit(app, id, s, false).await;
    }
    let (account, address, needs_signature) = {
        let d = s.data.lock().unwrap();
        if d.identity_epoch != epoch || !d.phase.writable() {
            return Ok(());
        }
        (
            d.form.account.clone().unwrap_or_default(),
            luma_core::sender_rules::address(&d.form.from).unwrap_or_default(),
            !d.source_mode && (force || d.draft.is_none()),
        )
    };
    let config = app
        .settings
        .lock()
        .unwrap()
        .accounts
        .iter()
        .find(|a| a.id == account)
        .cloned();
    let result: Result<(Option<Vec<luma_core::send_as::SendAs>>, Option<String>)> = async {
        if address.is_empty() {
            return Err(Error::Invalid("Choose a verified From address".into()));
        }
        if app.preview || !config.as_ref().is_some_and(|a| a.sender_controls) {
            return Ok((None, None));
        }
        let client = app.client(&account)?;
        let rows = if aliases {
            Some(client.send_as(&config.as_ref().unwrap().email).await?)
        } else {
            None
        };
        let signature = if needs_signature {
            Some(client.signature(&address).await?.html)
        } else {
            None
        };
        Ok((rows, signature))
    }
    .await;
    let error = {
        let mut d = s.data.lock().unwrap();
        if d.identity_epoch != epoch
            || d.form.account.as_deref() != Some(&account)
            || !d.phase.writable()
        {
            return Ok(());
        }
        d.identity_loading = false;
        match result {
            Ok((rows, signature)) => {
                if let Some(rows) = rows {
                    d.identities = rows;
                }
                if let Some(html) = signature {
                    d.signature = html;
                }
                if let Some(i) = d
                    .identities
                    .iter()
                    .find(|i| i.send_as_email.eq_ignore_ascii_case(&address))
                    .cloned()
                {
                    d.form.from = i.header();
                    d.form.sender_reply_to = i.reply_to_address;
                }
                d.status = if d.draft.is_some() {
                    "Gmail draft ready"
                } else {
                    "Changes stay in memory until saved"
                }
                .into();
                None
            }
            Err(e) => {
                d.pending = None;
                d.status = format!("Sender settings could not load: {e}. Review before sending.");
                Some(e.to_string())
            }
        }
    };
    s.input_ready.notify_waiters();
    emit(app, id, s, false).await;
    if let Some(message) = error {
        app.events
            .error(id, &format!("Sender settings could not load: {message}"))
            .await;
    }
    resume_close(app, id, s).await
}
async fn watch_operation<T>(
    app: &App,
    id: u64,
    s: &Session,
    future: impl Future<Output = Result<T>>,
) -> Result<T> {
    tokio::pin!(future);
    let mut interval = tokio::time::interval(Duration::from_millis(100));
    let mut previous = Value::Null;
    loop {
        tokio::select! {
            result=&mut future=>{emit(app,id,s,false).await;return result;}
            _=interval.tick()=>{let state=s.snapshot(false);if state!=previous{previous=state.clone();app.events.emit(id,"compose",state).await;}}
        }
    }
}
async fn write(app: &Arc<App>, id: u64, s: &Arc<Session>) -> Result<()> {
    let previous_thread = s
        .data
        .lock()
        .unwrap()
        .draft
        .as_ref()
        .map(|d| d.message.thread_id.clone())
        .filter(|id| !id.is_empty());
    let (account, drive) = {
        let d = s.data.lock().unwrap();
        let account = d.form.account.clone().unwrap_or_default();
        let drive = app
            .settings
            .lock()
            .unwrap()
            .accounts
            .iter()
            .any(|a| a.id == account && a.drive_enabled);
        (account, drive)
    };
    let client = app.client(&account)?;
    app.mutation_serial.fetch_add(1, Ordering::SeqCst);
    let mut uploading = (*client).clone();
    uploading.http = client.http.with_upload_progress(s.transfer.clone());
    let result = watch_operation(app, id, s, s.persist(&uploading, Some(&client), drive, id)).await;
    app.mutation_serial.fetch_add(1, Ordering::SeqCst);
    let written = s.data.lock().unwrap().written_thread.take();
    if let Some(thread) = written {
        app.sync.track_write(luma_core::model::ThreadKey {
            account: account.clone(),
            id: thread.clone(),
        });
        if let Some(previous) = previous_thread.filter(|old| old != &thread) {
            app.sync.track_write(luma_core::model::ThreadKey {
                account: account.clone(),
                id: previous,
            });
        }
    }
    app.sync.reconcile();
    let phase = s.data.lock().unwrap().phase;
    if phase == Phase::Sent {
        let recent = s.data.lock().unwrap().sent_recipients.take();
        let remember = app.settings.lock().unwrap().recent_recipients;
        if remember && let Some(recent) = recent {
            let store = app.recent.clone();
            let account = account.clone();
            let _ = tokio::task::spawn_blocking(move || store.remember(&account, &recent)).await;
        }
        app.events
            .emit(id, "composeSent", json!({"session":s.id,"account":account}))
            .await;
    }
    let close_pending = {
        let mut d = s.data.lock().unwrap();
        std::mem::take(&mut d.close_pending)
    };
    if close_pending && !matches!(phase, Phase::Sent | Phase::Closed) {
        Box::pin(close(app, id, s)).await?;
    }
    result
}
async fn attach(app: &Arc<App>, c: &Command, s: &Arc<Session>) -> Result<()> {
    let paths: Vec<PathBuf> = c.data["paths"]
        .as_array()
        .ok_or_else(|| Error::Invalid("Choose attachment files".into()))?
        .iter()
        .map(|v| {
            v.as_str()
                .filter(|v| v.len() < 32768)
                .map(PathBuf::from)
                .ok_or(Error::ResponseLimit)
        })
        .collect::<Result<_>>()?;
    {
        let mut d = s.data.lock().unwrap();
        if !matches!(d.phase, Phase::Editing | Phase::Uploading) {
            return Err(Error::Invalid(
                "Wait for the current operation before changing attachments".into(),
            ));
        }
        if d.form.attachments.len() + d.form.retained.len() + d.form.drive_files.len() + paths.len()
            > 64
        {
            return Err(Error::ResponseLimit);
        }
        for path in &paths {
            luma_core::compose_drive::fingerprint("", path)?;
            d.file_sizes
                .insert(path.clone(), std::fs::metadata(path)?.len());
        }
        for path in paths {
            if !d.form.attachments.contains(&path) {
                d.form.attachments.push(path);
            }
        }
        d.revision += 1;
        d.attachment_generation += 1;
    }
    emit(app, c.id, s, false).await;
    if app.preview {
        return Ok(());
    }
    upload(app, c.id, s).await
}
async fn upload(app: &Arc<App>, id: u64, s: &Arc<Session>) -> Result<()> {
    let _operation = s.operation.lock().await;
    let (account, enabled) = {
        let d = s.data.lock().unwrap();
        if !matches!(d.phase, Phase::Editing | Phase::Uploading) {
            return Ok(());
        };
        let account = d.form.account.clone().unwrap_or_default();
        let enabled = app
            .settings
            .lock()
            .unwrap()
            .accounts
            .iter()
            .any(|a| a.id == account && a.drive_enabled);
        (account, enabled)
    };
    let client = app.client(&account)?;
    s.cancel.store(false, Ordering::Relaxed);
    let result = watch_operation(app, id, s, async {
        loop {
            let form = {
                let d = s.data.lock().unwrap();
                if !matches!(d.phase, Phase::Editing | Phase::Uploading) {
                    return Ok(());
                }
                d.form.clone()
            };
            if luma_core::compose_drive::plan(&form)?.is_empty() {
                break;
            }
            {
                let mut d = s.data.lock().unwrap();
                d.phase = Phase::Uploading;
                d.status = "Uploading large attachment privately to Drive".into();
            }
            let prepared = luma_core::compose_drive::prepare(
                form,
                &client,
                enabled,
                s.uploads.clone(),
                s.progress.clone(),
                s.cancel.clone(),
            )
            .await;
            reconcile_uploads(s, &account);
            prepared?;
        }
        Ok(())
    })
    .await;
    {
        let mut d = s.data.lock().unwrap();
        if d.phase == Phase::Uploading {
            d.phase = Phase::Editing;
        }
        d.status = if result.is_ok() {
            "Attachments ready".into()
        } else {
            result.as_ref().err().unwrap().to_string()
        };
    }
    emit(app, id, s, false).await;
    let should_close = {
        let mut d = s.data.lock().unwrap();
        std::mem::take(&mut d.close_pending) && d.pending.is_none()
    };
    if should_close {
        Box::pin(close(app, id, s)).await?;
    }
    result
}
fn reconcile_uploads(s: &Session, account: &str) {
    let mut d = s.data.lock().unwrap();
    // Only files still attached to the live form may be converted. A removed
    // file finishing its upload cannot reattach itself or create a draft.
    let completed: Vec<_> = d
        .form
        .attachments
        .iter()
        .filter_map(|p| {
            luma_core::compose_drive::cached(&s.uploads, account, p).map(|f| (p.clone(), f))
        })
        .collect();
    if completed.is_empty() {
        return;
    }
    for (path, file) in completed {
        d.form.attachments.retain(|p| p != &path);
        d.file_sizes.remove(&path);
        if !d.form.drive_files.iter().any(|f| f.id == file.id) {
            d.form.drive_files.push(file);
        }
    }
    d.attachment_generation += 1;
}

async fn remove_attachment(app: &Arc<App>, c: &Command, s: &Arc<Session>) -> Result<()> {
    {
        let mut d = s.data.lock().unwrap();
        if !matches!(d.phase, Phase::Editing | Phase::Uploading) {
            return Err(Error::Invalid(
                "Wait for the attachment operation to finish".into(),
            ));
        }
        if c.data["generation"].as_u64() != Some(d.attachment_generation) {
            return Err(Error::Invalid(
                "Attachments changed; choose the file again".into(),
            ));
        }
        let id = c.text("attachment");
        if let Some(path) = id.strip_prefix("file:") {
            d.form.attachments.retain(|p| p.to_string_lossy() != path);
        } else if let Some(id) = id.strip_prefix("drive:") {
            if d.source_mode {
                return Err(Error::Invalid(
                    "Convert the original HTML before removing its Drive links".into(),
                ));
            }
            d.form.drive_files.retain(|f| f.id != id);
            d.original_html = None;
        } else if let Some(index) = id
            .strip_prefix("retained:")
            .and_then(|i| i.parse::<usize>().ok())
        {
            if index < d.form.retained.len() {
                d.form.retained.remove(index);
            }
        } else {
            return Err(Error::Invalid("Unknown attachment".into()));
        }
        d.revision += 1;
        d.attachment_generation += 1;
        let paths = d.form.attachments.clone();
        d.file_sizes.retain(|p, _| paths.contains(p));
    }
    emit(app, c.id, s, false).await;
    Ok(())
}

#[cfg(test)]
mod upload_tests {
    use super::*;
    #[test]
    fn completed_upload_cannot_reattach_a_file_removed_while_it_was_in_flight() {
        let dir = tempfile::tempdir().unwrap();
        let removed = dir.path().join("removed.bin");
        let kept = dir.path().join("kept.bin");
        std::fs::write(&removed, b"removed").unwrap();
        std::fs::write(&kept, b"kept").unwrap();
        let mut form = Compose::new("alex@example.com".into());
        form.account = Some("a".into());
        form.attachments.push(kept.clone());
        let s = Session::new(Init {
            form,
            primary: "alex@example.com".into(),
            mode: "new".into(),
            key: None,
            source_message: None,
            draft: None,
            quote: String::new(),
            signature: String::new(),
            original_html: None,
            source_mode: false,
            document: Value::Null,
        });
        for (path, id) in [(&removed, "removed"), (&kept, "kept")] {
            s.uploads.lock().unwrap().insert(
                luma_core::compose_drive::fingerprint("a", path).unwrap(),
                luma_core::drive::DriveFile {
                    id: id.into(),
                    name: id.into(),
                    web_view_link: None,
                    owned_by_me: true,
                    app_properties: Default::default(),
                },
            );
        }
        reconcile_uploads(&s, "a");
        reconcile_uploads(&s, "a");
        let d = s.data.lock().unwrap();
        assert!(d.form.attachments.is_empty());
        assert_eq!(d.form.drive_files.len(), 1);
        assert_eq!(d.form.drive_files[0].id, "kept");
    }
}
