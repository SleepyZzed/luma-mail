//! RAM-only composer state. Gmail owns saved drafts; input never schedules a
//! draft write. One serialized operation owns each save/upload/send lifecycle.
use async_trait::async_trait;
use luma_core::{
    Error, Result, compose_document as document,
    composer::{self, Compose},
    config::DraftMode,
    draft_merge::Fields,
    gmail::{Draft, Gmail},
    http::UploadProgress,
    model::Message,
    send_as::SendAs,
};
use serde_json::{Value, json};
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicBool, Ordering},
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Phase {
    Editing,
    Uploading,
    Saving,
    Sending,
    Conflict,
    Uncertain,
    Sent,
    Closed,
}
impl Phase {
    fn label(self) -> &'static str {
        match self {
            Self::Editing => "editing",
            Self::Uploading => "uploading",
            Self::Saving => "saving",
            Self::Sending => "sending",
            Self::Conflict => "conflict",
            Self::Uncertain => "uncertain",
            Self::Sent => "sent",
            Self::Closed => "closed",
        }
    }
    pub fn writable(self) -> bool {
        matches!(
            self,
            Self::Editing | Self::Uploading | Self::Saving | Self::Conflict
        )
    }
}
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Intent {
    Save,
    Send,
    SaveAndClose,
}
pub enum Close {
    Closed,
    Ask(u64),
    Save,
    Wait,
}

pub struct Data {
    pub file_sizes: std::collections::HashMap<std::path::PathBuf, u64>,
    pub sent_recipients: Option<Compose>,
    pub form: Compose,
    pub mode: String,
    pub key: Option<luma_core::model::ThreadKey>,
    pub phase: Phase,
    pub revision: u64,
    pub saved_revision: u64,
    pub input_revision: u64,
    pub draft: Option<Draft>,
    pub base: Fields,
    pub quote: String,
    pub signature: String,
    pub include_quote: bool,
    pub include_signature: bool,
    pub original_html: Option<String>,
    pub source_mode: bool,
    pub initial_document: Value,
    pub identities: Vec<SendAs>,
    pub pending: Option<(u64, Intent)>,
    pub close_question: Option<u64>,
    pub close_pending: bool,
    pub status: String,
    pub attachment_generation: u64,
}
pub struct Session {
    pub id: String,
    pub data: Mutex<Data>,
    pub operation: tokio::sync::Mutex<()>,
    pub uploads: luma_core::compose_drive::UploadCache,
    pub cancel: Arc<AtomicBool>,
    pub transfer: Arc<UploadProgress>,
    pub progress: tokio::sync::watch::Sender<luma_core::compose_drive::Progress>,
    _progress_lifetime: tokio::sync::watch::Receiver<luma_core::compose_drive::Progress>,
}
pub struct Init {
    pub form: Compose,
    pub primary: String,
    pub mode: String,
    pub key: Option<luma_core::model::ThreadKey>,
    pub draft: Option<Draft>,
    pub quote: String,
    pub signature: String,
    pub original_html: Option<String>,
    pub source_mode: bool,
    pub document: Value,
}

#[async_trait]
pub trait DraftTransport: Send + Sync {
    async fn save(
        &self,
        existing: Option<&Draft>,
        raw: &[u8],
        thread: Option<&str>,
    ) -> Result<Draft>;
    async fn send(&self, draft: &Draft) -> Result<Message>;
}
#[async_trait]
impl DraftTransport for Gmail {
    async fn save(
        &self,
        existing: Option<&Draft>,
        raw: &[u8],
        thread: Option<&str>,
    ) -> Result<Draft> {
        self.save_draft(existing, raw, thread).await
    }
    async fn send(&self, draft: &Draft) -> Result<Message> {
        self.send_draft(draft).await
    }
}

impl Session {
    pub fn new(init: Init) -> Arc<Self> {
        let Init {
            form,
            primary,
            mode,
            key,
            draft,
            quote,
            signature,
            original_html,
            source_mode,
            document: initial_document,
        } = init;
        let base = Fields::of(&form);
        let identities = vec![SendAs::primary(&primary)];
        let (progress, progress_lifetime) = tokio::sync::watch::channel(Default::default());
        Arc::new(Self {
            id: composer::new_id(),
            data: Mutex::new(Data {
                file_sizes: Default::default(),
                sent_recipients: None,
                form,
                mode,
                key,
                phase: Phase::Editing,
                revision: 0,
                saved_revision: 0,
                input_revision: 0,
                draft: draft.map(stamp),
                base,
                quote,
                signature,
                include_quote: true,
                include_signature: true,
                original_html,
                source_mode,
                initial_document,
                identities,
                pending: None,
                close_question: None,
                close_pending: false,
                status: String::new(),
                attachment_generation: 0,
            }),
            operation: tokio::sync::Mutex::new(()),
            uploads: Default::default(),
            cancel: Arc::new(AtomicBool::new(false)),
            transfer: Default::default(),
            progress,
            _progress_lifetime: progress_lifetime,
        })
    }
    pub fn active(&self) -> bool {
        !matches!(self.data.lock().unwrap().phase, Phase::Sent | Phase::Closed)
    }
    pub fn update(&self, input: &Value, id: u64) -> Result<()> {
        let mut d = self.data.lock().unwrap();
        if id <= d.input_revision {
            return Ok(());
        }
        if !d.phase.writable() {
            return Err(Error::Invalid(
                "This composer is locked while Gmail confirms the operation".into(),
            ));
        }
        let mut form = d.form.clone();
        for (name, target) in [
            ("to", &mut form.to),
            ("cc", &mut form.cc),
            ("bcc", &mut form.bcc),
            ("subject", &mut form.subject),
        ] {
            if let Some(value) = input[name].as_str() {
                if value.len() > 8192 || value.chars().any(|c| c == '\r' || c == '\n' || c == '\0')
                {
                    return Err(Error::Invalid(
                        "A mail field exceeds its safe input limit".into(),
                    ));
                }
                *target = value.into();
            }
        }
        if let Some(body) = input["text"].as_str() {
            if body.len() > document::MAX_BODY {
                return Err(Error::ResponseLimit);
            }
            let html = input["html"].as_bool().unwrap_or(false);
            if d.source_mode && (body != form.text || html != form.html) {
                return Err(Error::Invalid(
                    "Convert the original HTML explicitly before changing its body".into(),
                ));
            }
            if body != form.text || html != form.html {
                d.original_html = None;
            }
            form.text = body.into();
            form.html = html;
        }
        let quote = input["includeQuote"].as_bool().unwrap_or(d.include_quote);
        let signature = input["includeSignature"]
            .as_bool()
            .unwrap_or(d.include_signature);
        if d.source_mode && (quote != d.include_quote || signature != d.include_signature) {
            return Err(Error::Invalid(
                "Convert the original HTML before changing its signature or quoted history".into(),
            ));
        }
        let changed = form.to != d.form.to
            || form.cc != d.form.cc
            || form.bcc != d.form.bcc
            || form.subject != d.form.subject
            || form.text != d.form.text
            || form.html != d.form.html
            || quote != d.include_quote
            || signature != d.include_signature;
        if quote != d.include_quote || signature != d.include_signature {
            d.original_html = None;
        }
        d.form = form;
        d.include_quote = quote;
        d.include_signature = signature;
        d.input_revision = id;
        if changed {
            d.revision += 1;
            d.close_question = None;
        }
        Ok(())
    }
    pub fn queue(&self, ticket: u64, intent: Intent) -> Result<bool> {
        let mut d = self.data.lock().unwrap();
        if matches!(d.phase, Phase::Sent | Phase::Closed) {
            return Err(Error::Invalid(
                "This message has already been sent or closed".into(),
            ));
        }
        if matches!(d.phase, Phase::Uncertain | Phase::Conflict) {
            return Err(Error::Invalid(
                "Resolve the Gmail draft state before saving or sending again".into(),
            ));
        }
        if d.pending.is_some() || d.phase == Phase::Sending {
            return Ok(false);
        }
        if d.phase == Phase::Saving && intent != Intent::Send {
            return Ok(false);
        }
        d.pending = Some((ticket, intent));
        Ok(true)
    }
    pub fn close(&self, mode: DraftMode) -> Close {
        let mut d = self.data.lock().unwrap();
        if matches!(d.phase, Phase::Saving | Phase::Sending | Phase::Uploading)
            || d.pending.is_some()
        {
            d.close_pending = true;
            return Close::Wait;
        }
        let has_body = if d.form.html {
            composer::html_text(&d.form.text).is_ok_and(|s| !s.trim().is_empty())
        } else {
            !d.form.text.trim().is_empty()
        };
        let has_recipient = [&d.form.to, &d.form.cc, &d.form.bcc]
            .iter()
            .any(|s| composer::recipients(s).is_ok_and(|a| !a.is_empty()));
        let dirty = d.revision != d.saved_revision;
        let content = has_body
            || !d.form.subject.trim().is_empty()
            || [&d.form.to, &d.form.cc, &d.form.bcc]
                .iter()
                .any(|s| !s.trim().is_empty())
            || !d.form.attachments.is_empty()
            || !d.form.retained.is_empty()
            || !d.form.drive_files.is_empty();
        if matches!(d.phase, Phase::Conflict | Phase::Uncertain)
            || (mode == DraftMode::Ask && dirty && content)
        {
            let revision = d.revision;
            d.close_question = Some(revision);
            return Close::Ask(revision);
        }
        if mode.saves_on_close(has_recipient, has_body) && dirty {
            return Close::Save;
        }
        clear(&mut d, Phase::Closed);
        Close::Closed
    }
    pub fn close_answer(&self, revision: u64, answer: &str) -> Result<Option<Intent>> {
        let mut d = self.data.lock().unwrap();
        if d.close_question != Some(revision) || d.revision != revision {
            return Err(Error::Invalid(
                "The message changed; review the close question again".into(),
            ));
        }
        d.close_question = None;
        d.close_pending = false;
        match answer {
            "keep" => Ok(None),
            "close" => {
                clear(&mut d, Phase::Closed);
                Ok(None)
            }
            "save" if !matches!(d.phase, Phase::Uncertain | Phase::Conflict) => {
                Ok(Some(Intent::SaveAndClose))
            }
            _ => Err(Error::Invalid(
                "Resolve the Gmail state before saving this draft".into(),
            )),
        }
    }
    pub fn snapshot(&self, replace: bool) -> Value {
        let d = self.data.lock().unwrap();
        let progress = self.progress.borrow().clone();
        let (done, total) = self.transfer.snapshot();
        let attachments = attachment_rows(&d.form, &d.file_sizes);
        let identities: Vec<_> = d
            .identities
            .iter()
            .map(|i| json!({"email":i.send_as_email,"name":i.display_name,"header":i.header()}))
            .collect();
        let mut value = json!({"session":self.id,"mode":d.mode,"key":d.key,"account":d.form.account,"from":d.form.from,
            "revision":d.revision,"savedRevision":d.saved_revision,"dirty":d.revision!=d.saved_revision,
            "state":d.phase.label(),"phase":if d.phase==Phase::Sending{"sending"}else if d.phase==Phase::Sent{"sent"}else{"editing"},
            "locked":matches!(d.phase,Phase::Sending|Phase::Uncertain),"pending":d.pending.is_some(),"status":d.status,
            "attachments":attachments,"attachmentGeneration":d.attachment_generation,"identities":identities,
            "quote":d.quote,"signatureHtml":d.signature,"includeQuote":d.include_quote,"includeSignature":d.include_signature,
            "sourceMode":d.source_mode,"draftId":d.draft.as_ref().map(|v|&v.id),
            "progress":{"name":progress.name,"done":if progress.total>0{progress.done}else{done},"total":if progress.total>0{progress.total}else{total},"sharing":progress.sharing},
            "replace":replace});
        if replace {
            value["to"] = json!(d.form.to);
            value["cc"] = json!(d.form.cc);
            value["bcc"] = json!(d.form.bcc);
            value["subject"] = json!(d.form.subject);
            value["document"] = d.initial_document.clone();
        }
        value
    }
    pub async fn persist(
        &self,
        transport: &dyn DraftTransport,
        client: Option<&Gmail>,
        drive: bool,
        ticket: u64,
    ) -> Result<()> {
        let _operation = self.operation.lock().await;
        let intent = {
            let mut d = self.data.lock().unwrap();
            let Some((id, intent)) = d.pending else {
                return Ok(());
            };
            if id != ticket {
                return Ok(());
            }
            if matches!(
                d.phase,
                Phase::Sent | Phase::Closed | Phase::Conflict | Phase::Uncertain
            ) {
                d.pending = None;
                return Ok(());
            }
            d.pending = None;
            d.phase = if intent == Intent::Send {
                Phase::Sending
            } else {
                Phase::Saving
            };
            d.status = if intent == Intent::Send {
                "Preparing your message"
            } else {
                "Saving to Gmail"
            }
            .into();
            intent
        };
        let result = self.persist_inner(transport, client, drive, intent).await;
        if let Err(error) = &result {
            let mut d = self.data.lock().unwrap();
            if !matches!(d.phase, Phase::Uncertain | Phase::Conflict) {
                d.phase = Phase::Editing;
            }
            d.status = error.to_string();
        }
        result
    }
    async fn persist_inner(
        &self,
        transport: &dyn DraftTransport,
        client: Option<&Gmail>,
        drive: bool,
        intent: Intent,
    ) -> Result<()> {
        let send = intent == Intent::Send;
        let (form, revision, existing) = {
            let d = self.data.lock().unwrap();
            (delivery(&d)?, d.revision, d.draft.clone())
        };
        let sender = luma_core::sender_rules::address(&form.from)?;
        if !self
            .data
            .lock()
            .unwrap()
            .identities
            .iter()
            .any(|i| i.send_as_email.eq_ignore_ascii_case(&sender))
        {
            return Err(Error::Invalid(
                "Choose a verified sending address before saving or sending".into(),
            ));
        }
        if send {
            form.validate_send()?;
        }
        self.cancel.store(false, Ordering::Relaxed);
        let form = if let Some(client) = client {
            luma_core::compose_drive::prepare(
                form,
                client,
                drive,
                self.uploads.clone(),
                self.progress.clone(),
                self.cancel.clone(),
            )
            .await?
        } else {
            form
        };
        let fields = Fields::of(&form);
        self.check_cancelled()?;
        let prepared = form.clone();
        let raw = tokio::task::spawn_blocking(move || prepared.mime())
            .await
            .map_err(|_| Error::Invalid("MIME preparation failed".into()))??;
        self.check_cancelled()?;
        let draft = match transport
            .save(existing.as_ref(), &raw, form.thread.as_deref())
            .await
        {
            Ok(draft) => stamp(draft),
            Err(error) => {
                self.failed_write(&error);
                return Err(error);
            }
        };
        {
            let mut d = self.data.lock().unwrap();
            d.draft = Some(draft.clone());
            d.saved_revision = revision;
            d.base = fields;
            // Only merge attachment/upload results; never replace text typed
            // while Save was in flight with its older snapshot.
            for file in &form.drive_files {
                if !d.form.drive_files.iter().any(|f| f.id == file.id) {
                    d.form.drive_files.push(file.clone());
                }
            }
            let account = d.form.account.clone().unwrap_or_default();
            d.form
                .attachments
                .retain(|p| luma_core::compose_drive::cached(&self.uploads, &account, p).is_none());
            d.attachment_generation += 1;
        }
        if send {
            self.check_cancelled()?;
            if let Some(client) = client {
                luma_core::compose_drive::share_for_send(client, &form, &self.progress).await?;
            }
            {
                self.data.lock().unwrap().status = "Waiting for Gmail confirmation".into();
            }
            self.check_cancelled()?;
            if let Err(error) = transport.send(&draft).await {
                self.failed_write(&error);
                return Err(error);
            }
            let mut d = self.data.lock().unwrap();
            clear(&mut d, Phase::Sent);
            d.sent_recipients = Some(Compose {
                to: form.to,
                cc: form.cc,
                bcc: form.bcc,
                ..Default::default()
            });
            d.status = "Sent · confirmed by Gmail".into();
        } else {
            let mut d = self.data.lock().unwrap();
            d.phase = Phase::Editing;
            d.status = "Saved in Gmail".into();
            if intent == Intent::SaveAndClose && d.revision == revision {
                clear(&mut d, Phase::Closed);
            } else if intent == Intent::SaveAndClose {
                d.close_pending = true;
            }
        }
        Ok(())
    }
    fn failed_write(&self, error: &Error) {
        let mut d = self.data.lock().unwrap();
        if matches!(error, Error::DraftConflict) {
            d.phase = Phase::Conflict;
        } else if error.write_may_have_succeeded() {
            d.phase = Phase::Uncertain;
        }
    }
    fn check_cancelled(&self) -> Result<()> {
        if self.cancel.load(Ordering::Relaxed) {
            Err(Error::Invalid(
                "Attachment operation cancelled; your message is still open".into(),
            ))
        } else {
            Ok(())
        }
    }
}
fn stamp(mut draft: Draft) -> Draft {
    draft.message.payload = Default::default();
    draft.message.snippet.clear();
    draft
}
fn clear(d: &mut Data, phase: Phase) {
    d.form = Compose::default();
    d.base = Fields::of(&d.form);
    d.original_html = None;
    d.initial_document = Value::Null;
    d.quote.clear();
    d.signature.clear();
    d.draft = None;
    d.pending = None;
    d.close_question = None;
    d.close_pending = false;
    d.phase = phase;
    d.identities.clear();
    d.file_sizes.clear();
    d.sent_recipients = None;
}
fn delivery(d: &Data) -> Result<Compose> {
    let mut form = d.form.clone();
    if let Some(html) = &d.original_html {
        form.text = html.clone();
        form.html = true;
    } else if form.html
        || (d.include_quote && !d.quote.is_empty())
        || (d.include_signature && !d.signature.is_empty())
    {
        form.text = document::envelope(
            &form.text,
            form.html,
            if d.include_quote { &d.quote } else { "" },
            if d.include_signature {
                &d.signature
            } else {
                ""
            },
        )?;
        form.html = true;
    }
    for file in form.drive_files.clone() {
        luma_core::compose_drive::append_link(&mut form, &file)?;
    }
    Ok(form)
}
fn attachment_rows(
    c: &Compose,
    sizes: &std::collections::HashMap<std::path::PathBuf, u64>,
) -> Vec<Value> {
    let mut rows = Vec::new();
    for path in &c.attachments {
        rows.push(json!({"id":format!("file:{}",path.to_string_lossy()),"name":path.file_name().unwrap_or_default().to_string_lossy(),"size":sizes.get(path).copied().unwrap_or(0),"kind":"file"}));
    }
    for (index, a) in c.retained.iter().enumerate() {
        rows.push(json!({"id":format!("retained:{index}"),"name":a.name,"size":a.data.len(),"mime":a.mime_type,"kind":"retained"}));
    }
    for file in &c.drive_files {
        rows.push(json!({"id":format!("drive:{}",file.id),"name":file.name,"size":0,"kind":"drive","url":luma_core::drive::verified_link(file).unwrap_or_default()}));
    }
    rows
}

#[cfg(test)]
#[path = "compose_tests.rs"]
mod tests;
