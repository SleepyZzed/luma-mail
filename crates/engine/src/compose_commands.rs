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
    Update(Arc<Session>),
    Write(Arc<Session>, bool),
    Other,
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
        "composeIdentity" => identity(app, c, &s).await,
        "composeAttach" => attach(app, c, &s).await,
        "composeRemoveAttachment" => remove_attachment(app, c, &s).await,
        "composeCancelUpload" => {
            s.cancel.store(true, Ordering::Relaxed);
            s.data.lock().unwrap().pending = None;
            Ok(())
        }
        "composeRetryUpload" => upload(app, c.id, &s).await,
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
            let rows = app.recent.recent(&account, prefix);
            app.events
                .emit(
                    c.id,
                    "recipients",
                    json!({"session":s.id,"prefix":prefix,"items":rows}),
                )
                .await;
            Ok(())
        }
        _ => Err(Error::Invalid("Unknown composer action".into())),
    }
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
    if let Some(s) = current.filter(|s| s.active()) {
        app.events
            .emit(c.id, "composeFocus", json!({"session":s.id}))
            .await;
        return Ok(());
    }
    let mode = c.text("mode");
    if !["new", "reply", "replyAll", "forward", "draft"].contains(&mode) {
        return Err(Error::Invalid("Unknown compose mode".into()));
    }
    let key = if c.data["key"].is_object() {
        Some(crate::app::key(&c.data["key"])?)
    } else {
        None
    };
    let requested = c.data["account"]
        .as_str()
        .filter(|s| !s.is_empty())
        .or_else(|| key.as_ref().map(|k| k.account.as_str()));
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
        draft: None,
        quote: String::new(),
        signature: String::new(),
        original_html: None,
        source_mode: false,
        document: json!({"kind":"plain","text":""}),
    };
    if mode != "new" {
        let key = key
            .as_ref()
            .ok_or_else(|| Error::Invalid("Choose a message first".into()))?;
        let id = c.data["message"]["id"].as_str().unwrap_or(&key.id);
        let original = if app.preview {
            synthetic(id, &key.id)
        } else {
            let cached = app.view.lock().unwrap().bodies.get(id);
            if let Some(m) = cached {
                (*m).clone()
            } else {
                app.client(&key.account)?.message(id).await?
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
        } else if mode == "forward" {
            init.form.subject = format!("Fwd: {}", original.payload.header("Subject"));
            init.form.text = format!(
                "---------- Forwarded message ----------\nFrom: {}\nSubject: {}\n\n{}",
                original.payload.header("From"),
                original.payload.header("Subject"),
                luma_core::composer::readable(&original.payload)?
            );
            init.document = json!({"kind":"plain","text":init.form.text});
            if !app.preview {
                init.form.retained = app
                    .client(&account.id)?
                    .message_attachments(&original)
                    .await?;
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
    let s = Session::new(init);
    *app.composer.lock().unwrap() = Some(s.clone());
    app.events
        .emit(c.id, "composeOpened", s.snapshot(true))
        .await;
    if account.sender_controls && !app.preview {
        let client = app.client(&account.id)?;
        let identities = client.send_as(&account.email).await;
        if s.active() {
            if let Ok(rows) = identities {
                s.data.lock().unwrap().identities = rows;
            }
            let address = luma_core::sender_rules::address(&s.data.lock().unwrap().form.from)?;
            if let Ok(signature) = client.signature(&address).await {
                let mut d = s.data.lock().unwrap();
                if d.signature.is_empty() && d.draft.is_none() && d.phase == Phase::Editing {
                    d.signature = signature.html;
                }
            }
            emit(app, c.id, &s, false).await;
        }
    }
    Ok(())
}
fn from_draft(
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
async fn identity(app: &Arc<App>, c: &Command, s: &Arc<Session>) -> Result<()> {
    let address = c.text("address");
    let (account, can_signature) = {
        let mut d = s.data.lock().unwrap();
        if !d.phase.writable() {
            return Err(Error::Invalid("Wait for the current send operation".into()));
        }
        let choice = d
            .identities
            .iter()
            .find(|i| i.send_as_email.eq_ignore_ascii_case(address))
            .cloned()
            .ok_or_else(|| Error::Invalid("Choose a verified sending address".into()))?;
        d.form.from = choice.header();
        d.form.sender_reply_to = choice.reply_to_address;
        d.revision += 1;
        let account = d.form.account.clone().unwrap_or_default();
        let can = app
            .settings
            .lock()
            .unwrap()
            .accounts
            .iter()
            .any(|a| a.id == account && a.sender_controls);
        (account, can)
    };
    if can_signature && !app.preview {
        let signature = app.client(&account)?.signature(address).await?;
        let mut d = s.data.lock().unwrap();
        if luma_core::sender_rules::address(&d.form.from)? == address {
            d.signature = signature.html;
        }
    }
    emit(app, c.id, s, false).await;
    Ok(())
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
    let mut uploading = (*client).clone();
    uploading.http = client.http.with_upload_progress(s.transfer.clone());
    let result = watch_operation(app, id, s, s.persist(&uploading, Some(&client), drive, id)).await;
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
