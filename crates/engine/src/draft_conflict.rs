//! Explicit, bounded three-way comparison. A merge opens an unsaved copy;
//! it never replaces, deletes or sends the original Gmail draft.
use crate::{
    app::App,
    compose::{self, Init, Phase, Session},
    protocol::Command,
};
use luma_core::{
    Error, Result,
    composer::Compose,
    draft_merge::{Fields, NAMES, Plan, Resolution},
    gmail::Draft,
    model::{Header, Message, Part, PartBody},
};
use serde_json::{Value, json};
use std::sync::Arc;
pub struct Prepared {
    pub revision: u64,
    pub ticket: u64,
    local: Compose,
    server: Compose,
    remote: Draft,
    plan: Plan,
}
fn remote_form(remote: &Draft, account: &str) -> Result<Compose> {
    let p = &remote.message.payload;
    let mut form = Compose::new(p.header("From").into());
    form.account = Some(account.into());
    form.to = p.header("To").into();
    form.cc = p.header("Cc").into();
    form.bcc = p.header("Bcc").into();
    form.sender_reply_to = p.header("Reply-To").into();
    form.subject = p.header("Subject").into();
    form.thread = Some(remote.message.thread_id.clone());
    form.reply_to = p.header("In-Reply-To").into();
    form.references = p.header("References").into();
    form.drive_files =
        luma_core::compose_drive::draft_references(p.header("X-Luma-Drive-Attachments"))?;
    if let Some(html) = luma_core::html_mail::body(p)? {
        form.text = html;
        form.html = true;
    } else {
        form.text = luma_core::composer::readable(p)?;
    }
    if form.text.len() > luma_core::compose_document::MAX_BODY + 256 * 1024 {
        return Err(Error::ResponseLimit);
    }
    Ok(form)
}
fn preview(text: &str, html: bool) -> String {
    let prefix: String = text.chars().take(128 * 1024).collect();
    let plain = if html {
        luma_core::composer::html_text(&prefix)
            .unwrap_or_else(|_| "Original HTML is preserved".into())
    } else {
        prefix
    };
    let mut value: String = plain.chars().take(32768).collect();
    if plain.chars().nth(32768).is_some() || text.chars().nth(128 * 1024).is_some() {
        value.push_str("\n[Preview shortened; the complete version is preserved.]");
    }
    value
}
fn prepare(
    local: Compose,
    remote: Draft,
    base: &Fields,
    revision: u64,
    ticket: u64,
) -> Result<Prepared> {
    let server = remote_form(&remote, local.account.as_deref().unwrap_or_default())?;
    if remote.message.payload.attachments().len() > 64 {
        return Err(Error::ResponseLimit);
    }
    let plan = Plan::new(base, Fields::of(&local), Fields::of(&server));
    Ok(Prepared {
        revision,
        ticket,
        local,
        server,
        remote,
        plan,
    })
}
impl Prepared {
    fn view(&self, session: &str) -> Value {
        let fields:Vec<_>=(0..6).map(|i|{
            let resolution=match self.plan.resolution[i]{Resolution::Same=>"same",Resolution::Mine=>"mine",Resolution::Gmail=>"gmail",Resolution::Choose=>"choose"};
            let a=&self.plan.local.0[i];let b=&self.plan.remote.0[i];
            json!({"name":NAMES[i],"resolution":resolution,"mine":preview(&a.text,a.html),"gmail":preview(&b.text,b.html)})
        }).collect();
        let mut mine: Vec<_> = self
            .local
            .attachments
            .iter()
            .map(|p| {
                p.file_name()
                    .unwrap_or_default()
                    .to_string_lossy()
                    .to_string()
            })
            .collect();
        mine.extend(self.local.retained.iter().map(|a| a.name.clone()));
        mine.extend(self.local.drive_files.iter().map(|f| f.name.clone()));
        let mut gmail: Vec<_> = self
            .remote
            .message
            .payload
            .attachments()
            .iter()
            .map(|p| {
                if p.filename.is_empty() {
                    "attachment".into()
                } else {
                    p.filename.clone()
                }
            })
            .collect();
        gmail.extend(self.server.drive_files.iter().map(|f| f.name.clone()));
        json!({"session":session,"revision":self.revision,"ticket":self.ticket,"fields":fields,"mineAttachments":mine,"gmailAttachments":gmail})
    }
    fn merged(&self, choices: [Option<bool>; 6], remote_attachments: bool) -> Result<Compose> {
        let fields = self.plan.resolve(choices)?;
        let mut form = Compose::new(self.local.from.clone());
        fields.apply(&mut form);
        form.account = self.local.account.clone();
        form.thread = self.server.thread.clone();
        form.reply_to = self.server.reply_to.clone();
        form.references = self.server.references.clone();
        form.sender_reply_to = if form.from == self.server.from {
            self.server.sender_reply_to.clone()
        } else {
            self.local.sender_reply_to.clone()
        };
        if remote_attachments {
            form.drive_files = self.server.drive_files.clone();
        } else {
            form.attachments = self.local.attachments.clone();
            form.retained = self.local.retained.clone();
            form.drive_files = self.local.drive_files.clone();
        }
        Ok(form)
    }
}
fn valid(d: &compose::Data, review: &Prepared) -> bool {
    d.phase == Phase::Conflict
        && d.revision == review.revision
        && d.comparison_ticket == review.ticket
        && !d.input_busy
        && !d.identity_loading
}
pub async fn run(app: &Arc<App>, c: &Command, s: &Arc<Session>) -> Result<()> {
    if c.action == "composeCompare" {
        if c.data["form"].is_object() {
            s.update(&c.data["form"], c.id)?;
        }
        let _operation = s
            .operation
            .try_lock()
            .map_err(|_| Error::Invalid("Wait for the current draft operation".into()))?;
        let (local, draft, base, revision, ticket) = {
            let mut d = s.data.lock().unwrap();
            if d.phase != Phase::Conflict || d.input_busy || d.identity_loading {
                return Err(Error::Invalid(
                    "Compare is available for a conflicting Gmail draft after inputs finish".into(),
                ));
            }
            let draft = d
                .draft
                .clone()
                .ok_or_else(|| Error::Invalid("The original Gmail draft is unavailable".into()))?;
            d.comparison = None;
            d.comparison_ticket += 1;
            (
                compose::delivery(&d)?,
                draft,
                d.base.clone(),
                d.revision,
                d.comparison_ticket,
            )
        };
        let remote = if app.preview && std::env::var("LUMA_PREVIEW_CONFLICT").as_deref() == Ok("1")
        {
            fixture_remote()
        } else {
            app.client(local.account.as_deref().unwrap_or_default())?
                .draft(&draft.id)
                .await?
        };
        let prepared =
            tokio::task::spawn_blocking(move || prepare(local, remote, &base, revision, ticket))
                .await
                .map_err(|_| Error::Invalid("Cannot prepare draft comparison".into()))??;
        let view = prepared.view(&s.id);
        {
            let mut d = s.data.lock().unwrap();
            if !valid(&d, &prepared) {
                return Ok(());
            }
            d.comparison = Some(Arc::new(prepared));
        }
        app.events.emit(c.id, "composeComparison", view).await;
        return Ok(());
    }
    let _operation = s
        .operation
        .try_lock()
        .map_err(|_| Error::Invalid("Wait for the current draft operation".into()))?;
    let review = {
        let d = s.data.lock().unwrap();
        let review = d
            .comparison
            .clone()
            .ok_or_else(|| Error::Invalid("Open the draft comparison again".into()))?;
        if !valid(&d, &review) || c.data["ticket"].as_u64() != Some(review.ticket) {
            return Err(Error::Invalid(
                "The comparison is out of date; your editor is preserved".into(),
            ));
        }
        review
    };
    let values = c.data["choices"]
        .as_array()
        .filter(|v| v.len() == 6)
        .ok_or_else(|| Error::Invalid("Choose each conflicting field".into()))?;
    let choices = std::array::from_fn(|i| match values[i].as_str() {
        Some("mine") => Some(false),
        Some("gmail") => Some(true),
        _ => None,
    });
    let remote_attachments = match c.text("attachments") {
        "mine" => false,
        "gmail" => true,
        _ => return Err(Error::Invalid("Choose the attachment set to keep".into())),
    };
    let mut merged = review.merged(choices, remote_attachments)?;
    let account = merged.account.clone().unwrap_or_default();
    let config = app
        .settings
        .lock()
        .unwrap()
        .accounts
        .iter()
        .find(|a| a.id == account && a.enabled)
        .cloned()
        .ok_or_else(|| {
            Error::Invalid("Reconnect this account before opening the merged copy".into())
        })?;
    if remote_attachments && !app.preview {
        merged.retained = app
            .client(&account)?
            .message_attachments(&review.remote.message)
            .await?;
    }
    let init = merged_init(merged, config.email)?;
    let _start = app.compose_start.lock().await;
    {
        let d = s.data.lock().unwrap();
        if !valid(&d, &review) {
            return Err(Error::Invalid(
                "The draft changed while preparing the merge; your editor is preserved".into(),
            ));
        }
    }
    crate::compose_commands::install(app, c.id, init, Some((s, review.revision, review.ticket)))
        .await?;
    Ok(())
}
fn merged_init(form: Compose, primary: String) -> Result<Init> {
    let key = form.thread.as_ref().map(|id| luma_core::model::ThreadKey {
        account: form.account.clone().unwrap_or_default(),
        id: id.clone(),
    });
    let mut init = Init {
        form,
        primary,
        mode: "merged".into(),
        key,
        source_message: None,
        draft: None,
        quote: String::new(),
        signature: String::new(),
        original_html: None,
        source_mode: false,
        document: Value::Null,
    };
    if init.form.html {
        let html = init.form.text.clone();
        init.original_html = Some(html.clone());
        if let Some(parts) = luma_core::compose_document::sections(&html) {
            init.form.text = parts.body.clone();
            init.quote = parts.quote;
            init.signature = parts.signature;
            init.document = json!({"kind":"html","text":parts.body});
        } else if let Some(rich) = luma_core::rich_draft::parse(&html) {
            init.form.text = rich.text.clone();
            init.form.html = false;
            init.quote = rich.quote;
            init.signature = rich.signature;
            init.document = json!({"kind":"legacy","text":rich.text,"spans":rich.spans.iter().map(|s|json!({"start":s.start,"end":s.end,"tag":s.tag})).collect::<Vec<_>>()});
        } else {
            init.source_mode = true;
            init.document = json!({"kind":"source","text":html});
        }
    } else {
        init.document = json!({"kind":"plain","text":init.form.text});
    }
    Ok(init)
}
fn fixture_remote() -> Draft {
    use base64::Engine;
    Draft {
        id: "synthetic-draft".into(),
        message: Message {
            id: "synthetic-message".into(),
            thread_id: "synthetic-thread".into(),
            payload: Part {
                mime_type: "text/plain".into(),
                headers: vec![
                    Header {
                        name: "From".into(),
                        value: "alex@example.com".into(),
                    },
                    Header {
                        name: "To".into(),
                        value: "maya@example.com".into(),
                    },
                    Header {
                        name: "Subject".into(),
                        value: "Gmail subject".into(),
                    },
                ],
                body: PartBody {
                    data: Some(
                        base64::engine::general_purpose::URL_SAFE_NO_PAD
                            .encode("Gmail edited message")
                            .into(),
                    ),
                    ..Default::default()
                },
                ..Default::default()
            },
            ..Default::default()
        },
    }
}
pub fn fixture_init(init: &mut Init) {
    init.form.to = "maya@example.com".into();
    init.form.subject = "Original subject".into();
    init.form.text = "Base message".into();
    init.draft = Some(fixture_remote());
    init.document = json!({"kind":"plain","text":init.form.text});
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn field_merge_preserves_original_and_requires_body_choice() {
        let mut base = Compose::new("alex@example.com".into());
        base.account = Some("preview".into());
        base.to = "maya@example.com".into();
        base.subject = "Original subject".into();
        base.text = "Base message".into();
        let mut local = base.clone();
        local.text = "My edited message".into();
        let review = prepare(local, fixture_remote(), &Fields::of(&base), 7, 9).unwrap();
        assert!(review.merged([None; 6], false).is_err());
        let mut choices = [None; 6];
        choices[5] = Some(false);
        let merged = review.merged(choices, false).unwrap();
        assert_eq!(merged.text, "My edited message");
        assert_eq!(merged.subject, "Gmail subject");
        assert_ne!(
            merged.message_id,
            review.remote.message.payload.header("Message-ID")
        );
        assert_eq!(review.view("s")["fields"][5]["resolution"], "choose");
        assert_eq!(review.remote.id, "synthetic-draft");
    }
    #[test]
    fn comparison_previews_are_plain_bounded_and_keep_full_source() {
        let long = "x".repeat(200000);
        let p = preview(&long, false);
        assert!(p.len() < 33000);
        assert!(p.ends_with("[Preview shortened; the complete version is preserved.]"));
        assert!(!preview("<b>hello</b>", true).contains("<b>"));
    }
}
