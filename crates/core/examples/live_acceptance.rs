//! Maintainer-only, opt-in real-account checks. Uses the application's actual
//! OAuth, OS vault and provider; never prints tokens or real message contents.
//! Set LUMA_TEST_ROOT to a dedicated directory, compile with Luma's app values,
//! then run `connect`, `check`, or `exercise --allow-self-mail`.
use luma_core::{
    Error, Result,
    auth::{Authorization, ClientConfig, Credentials, SystemVault, TokenManager, Vault},
    composer::{Compose, RetainedAttachment, readable},
    config::{Account, Paths},
    gmail::Gmail,
    http::Http,
    model::{Folder, MailAction},
    provider::MailProvider,
};
use std::{io::Write, sync::Arc};

fn client() -> Result<ClientConfig> {
    ClientConfig::from_registration(
        option_env!("LUMA_GOOGLE_CLIENT_ID").unwrap_or(""),
        option_env!("LUMA_GOOGLE_CLIENT_SECRET").unwrap_or(""),
    )
}

#[tokio::main]
async fn main() -> Result<()> {
    if std::env::var_os("LUMA_TEST_ROOT").is_none() {
        return Err(Error::Invalid(
            "Set an isolated LUMA_TEST_ROOT for live acceptance".into(),
        ));
    }
    let paths = Paths::discover()?;
    let http = Http::new()?;
    let config = client()?;
    let vault: Arc<dyn Vault> = Arc::new(SystemVault);
    let args: Vec<String> = std::env::args().collect();
    let mut settings = paths.load()?;
    match args.get(1).map(String::as_str) {
        Some("sanitize-local") => {
            let html = std::fs::read_to_string(paths.config.join("inspected-mail.html"))?;
            let safe = luma_core::html_mail::document(&html, std::collections::HashMap::new());
            std::fs::write(paths.config.join("inspected-mail-sanitized.html"), &safe)?;
            println!(
                "Local render: {} tables preserved; grey background={}, purple button={}",
                safe.matches("<table").count(),
                safe.to_ascii_lowercase()
                    .contains("background-color:#e5e3e8"),
                safe.to_ascii_lowercase()
                    .contains("background-color:#9147ff")
            );
        }
        Some("connect") => {
            let drive = args.iter().any(|a| a == "--drive");
            let filters = args.iter().any(|a| a == "--filters");
            let photos = args.iter().any(|a| a == "--photos");
            let authorization =
                Authorization::begin_with_photos(&config, drive, filters, photos).await?;
            println!("BROWSER_URL={}", authorization.url);
            std::io::stdout().flush()?;
            let token = authorization.complete(&http, &config).await?;
            let profile = Gmail::profile_with_token(&http, &token.access_token).await?;
            let refresh = token.refresh_token.as_ref().ok_or_else(|| {
                Error::Invalid(
                    "Google did not issue a refresh token; reconnect with consent".into(),
                )
            })?;
            let id = settings
                .accounts
                .iter()
                .find(|a| a.email == profile.email_address)
                .map(|a| a.id.clone())
                .unwrap_or_else(|| uuid::Uuid::new_v4().to_string());
            vault
                .store(
                    &id,
                    &Credentials {
                        refresh_token: refresh.clone(),
                        client_id: config.client_id.clone(),
                    },
                )
                .await?;
            if let Some(account) = settings.accounts.iter_mut().find(|a| a.id == id) {
                account.drive_enabled = drive;
                account.sender_controls = filters;
                account.contact_photos = photos;
            } else {
                settings.accounts.push(Account {
                    id: id.clone(),
                    email: profile.email_address,
                    name: "Acceptance account".into(),
                    enabled: true,
                    notifications: false,
                    signature: String::new(),
                    drive_enabled: drive,
                    sender_controls: filters,
                    contact_photos: photos,
                    blocked_senders: vec![],
                });
            }
            settings.default_account = Some(id.clone());
            paths.save(&settings)?;
            let gmail = Gmail::new(
                id.clone(),
                http.clone(),
                Arc::new(TokenManager::new(id, config, http, vault)),
            );
            gmail.profile().await?;
            println!(
                "PASS real Google consent, OS vault storage, fresh refresh-token exchange and Gmail profile"
            );
        }
        Some("check")
        | Some("exercise")
        | Some("filters")
        | Some("inspect-html")
        | Some("photos")
        | Some("photos-own")
        | Some("draft-merge-aliases")
        | Some("reply-regression")
        | Some("signature-read") => {
            let account = settings
                .accounts
                .last()
                .ok_or_else(|| Error::Invalid("Connect an acceptance account first".into()))?;
            let gmail = Gmail::new(
                account.id.clone(),
                http.clone(),
                Arc::new(TokenManager::new(account.id.clone(), config, http, vault)),
            );
            println!("Checking fresh OAuth refresh and profile…");
            if gmail.profile().await?.email_address != account.email {
                return Err(Error::Invalid("Connected account identity mismatch".into()));
            }
            if args[1] == "signature-read" {
                use luma_core::signatures::SignatureStore;
                let signature = gmail.signature(&account.email).await?;
                assert!(signature.identity.eq_ignore_ascii_case(&account.email));
                assert!(signature.html.len() <= luma_core::signatures::MAX_HTML);
                println!(
                    "PASS real projected Gmail signature read: {} bytes, primary={}; no settings writes, image upload or local signature cache",
                    signature.html.len(),
                    signature.can_update
                );
                return Ok(());
            }
            if args[1] == "draft-merge-aliases" {
                draft_merge_aliases(&gmail, &account.email).await?;
                return Ok(());
            }
            if args[1] == "reply-regression" {
                if !args.iter().any(|a| a == "--allow-self-mail") {
                    return Err(Error::Invalid(
                        "Explicit --allow-self-mail is required".into(),
                    ));
                }
                reply_regression(&gmail, &account.email, args.iter().any(|a| a == "--resume"))
                    .await?;
                return Ok(());
            }
            if args[1] == "photos" || args[1] == "photos-own" {
                if !account.contact_photos {
                    return Err(Error::Invalid(
                        "Grant the optional Contacts permission first".into(),
                    ));
                }
                let photos = luma_core::contact_photos::Photos::new(Arc::new(gmail));
                let photo = if args[1] == "photos-own" {
                    photos.own(&account.email).await?
                } else {
                    photos
                        .get(&format!("Luma Review <{}>", account.email))
                        .await?
                };
                let photo = photo.ok_or_else(|| {
                    Error::Invalid("Google has no available photo for this test identity".into())
                })?;
                println!(
                    "PASS real Google photo lookup and public fetch/decode: {}x{}, no contact/photo disk mirror",
                    photo.width, photo.height
                );
                return Ok(());
            }
            if args[1] == "inspect-html" {
                let query = args.get(2).ok_or_else(|| {
                    Error::Invalid("Specify the exact authorized message query".into())
                })?;
                let response: serde_json::Value = luma_core::http::json(
                    gmail
                        .call(
                            reqwest::Method::GET,
                            "/messages",
                            &[
                                ("q", query.clone()),
                                ("maxResults", "1".into()),
                                ("fields", "messages(id)".into()),
                            ],
                            None,
                        )
                        .await?,
                )
                .await?;
                let id = response["messages"][0]["id"]
                    .as_str()
                    .ok_or_else(|| Error::Invalid("No matching message".into()))?;
                let message = gmail.message(id).await?;
                let html = luma_core::html_mail::body(&message.payload)?
                    .ok_or_else(|| Error::Invalid("No HTML body".into()))?;
                let sources = luma_core::mail_images::sources(&message.payload);
                let safe = luma_core::html_mail::document(&html, std::collections::HashMap::new());
                // Explicitly requested diagnostic artifact in the isolated test root.
                std::fs::write(paths.config.join("inspected-mail.html"), &html)?;
                std::fs::write(paths.config.join("inspected-mail-sanitized.html"), &safe)?;
                println!(
                    "HTML diagnostic only: {} bytes, {} image sources, {} style blocks, {} class attributes, {} tables; sanitized {} bytes, {} tables. Private source saved only in the isolated test directory.",
                    html.len(),
                    sources.len(),
                    html.matches("<style").count(),
                    html.matches("class=").count(),
                    html.matches("<table").count(),
                    safe.len(),
                    safe.matches("<table").count()
                );
            }
            if args[1] == "filters" {
                if !account.sender_controls {
                    return Err(Error::Invalid("Connect with --filters first".into()));
                }
                let report = paths.config.join("filter-test.json");
                let (address, id) = if report.is_file() {
                    let saved: serde_json::Value =
                        serde_json::from_slice(&std::fs::read(&report)?)?;
                    (
                        saved["address"].as_str().unwrap().to_owned(),
                        saved["id"].as_str().unwrap().to_owned(),
                    )
                } else {
                    let address = format!(
                        "luma-test-{}@example.invalid",
                        uuid::Uuid::new_v4().simple()
                    );
                    let created = gmail.create_sender_block(&address).await?;
                    (address, created.id)
                };
                // Record only this synthetic rule so cleanup can be repeated after interruption.
                std::fs::write(
                    &report,
                    serde_json::to_vec(&serde_json::json!({"address":address,"id":id}))?,
                )?;
                let listed = gmail.sender_filters().await?;
                let found = listed.iter().any(|f| {
                    f.id == id
                        && f.criteria["from"] == address
                        && f.action["addLabelIds"]
                            .as_array()
                            .is_some_and(|a| a.iter().any(|id| id == "TRASH"))
                });
                if args.iter().any(|a| a == "--inspect-synthetic-rule") {
                    println!(
                        "Synthetic rule: {}",
                        serde_json::to_string(listed.iter().find(|f| f.id == id).unwrap())?
                    );
                }
                gmail.remove_sender_block(&id, &address).await?;
                let absent = !gmail.sender_filters().await?.iter().any(|f| f.id == id);
                if !found || !absent {
                    return Err(Error::Invalid(
                        "Filter create/list/delete verification failed".into(),
                    ));
                }
                std::fs::remove_file(report)?;
                println!(
                    "PASS real Gmail filter create, persisted server listing, ownership-checked delete and cleanup; no existing mail changed"
                );
            }
            if args[1] == "check" {
                let started = std::time::Instant::now();
                println!("Fetching bounded Inbox metadata page…");
                let inbox = gmail.list(&Folder::Inbox, "", None).await?;
                println!(
                    "PASS Inbox: {} conversations, {} snippets, {:.2}s",
                    inbox.items.len(),
                    inbox.items.iter().filter(|r| !r.snippet.is_empty()).count(),
                    started.elapsed().as_secs_f64()
                );
                if let Some(cursor) = inbox.next {
                    let started = std::time::Instant::now();
                    let page = gmail.list(&Folder::Inbox, "", Some(&cursor)).await?;
                    println!(
                        "PASS Load more: {} conversations, {:.2}s",
                        page.items.len(),
                        started.elapsed().as_secs_f64()
                    );
                }
                for (name, folder) in [("Archive", Folder::Archive), ("All Mail", Folder::AllMail)]
                {
                    let started = std::time::Instant::now();
                    let page = gmail.list(&folder, "", None).await?;
                    println!(
                        "PASS {name}: {} conversations, {:.2}s",
                        page.items.len(),
                        started.elapsed().as_secs_f64()
                    );
                }
            }
            if args[1] == "exercise" {
                if !args.iter().any(|a| a == "--allow-self-mail") {
                    return Err(Error::Invalid("Exercise sends synthetic mail to the connected account itself; pass --allow-self-mail explicitly".into()));
                }
                exercise(
                    &gmail,
                    &account.email,
                    &paths,
                    args.iter().any(|a| a == "--resume"),
                )
                .await?;
            }
        }
        _ => {
            return Err(Error::Invalid(
                "Usage: live_acceptance connect [--drive] | check | exercise --allow-self-mail"
                    .into(),
            ));
        }
    }
    Ok(())
}

async fn draft_merge_aliases(gmail: &Gmail, email: &str) -> Result<()> {
    use luma_core::draft_merge::{Fields, Plan};
    let aliases = gmail.send_as(email).await?;
    assert!(
        aliases
            .iter()
            .any(|a| a.is_primary && a.send_as_email.eq_ignore_ascii_case(email))
    );
    println!(
        "PASS live verified Send-as lookup: {} available address(es); no new scope",
        aliases.len()
    );
    let mut base = Compose::new(email.into());
    base.to = email.into();
    base.subject = format!("Luma-Review-20261026-Merge-{}", uuid::Uuid::new_v4());
    base.text = "Synthetic base text".into();
    let original = gmail.save_draft(None, &base.mime()?, None).await?;
    let mut created = vec![original.id.clone()];
    let outcome: Result<()> = async {
        let mut server = base.clone();
        server.text = "Synthetic remote text".into();
        let updated = gmail
            .save_draft(Some(&original), &server.mime()?, None)
            .await?;
        let mut mine = base.clone();
        mine.subject.push_str(" local subject");
        assert!(matches!(
            gmail.save_draft(Some(&original), &mine.mime()?, None).await,
            Err(Error::DraftConflict)
        ));
        let plan = Plan::new(&Fields::of(&base), Fields::of(&mine), Fields::of(&server));
        let mut merged = Compose::new(email.into());
        plan.resolve([None; 6])?.apply(&mut merged);
        assert_eq!(merged.subject, mine.subject);
        assert_eq!(merged.text, server.text);
        let copy = gmail.save_draft(None, &merged.mime()?, None).await?;
        created.push(copy.id.clone());
        assert_ne!(copy.id, updated.id);
        let preserved = gmail.draft(&updated.id).await?;
        assert_eq!(preserved.message.id, updated.message.id);
        assert_eq!(readable(&preserved.message.payload)?.trim(), server.text);
        let restored = gmail.draft(&copy.id).await?;
        assert_eq!(restored.message.payload.header("Subject"), mine.subject);
        assert_eq!(readable(&restored.message.payload)?.trim(), server.text);
        println!(
            "PASS live stale-save refusal and merged separate Gmail draft; original preserved"
        );
        // The exact newly created conversation contains only our synthetic data.
        let count = gmail.discard_thread_drafts(&copy.message.thread_id).await?;
        assert!(count > 0);
        created.retain(|id| id != &copy.id);
        println!("PASS live draft-only delete");
        Ok(())
    }
    .await;
    for id in created {
        if let Err(error) = gmail.discard_draft(&id).await
            && !matches!(error, Error::Api { status: 404, .. })
        {
            return Err(error);
        }
    }
    outcome?;
    println!("PASS synthetic drafts cleaned up; no mail sent");
    Ok(())
}

async fn reply_regression(gmail: &Gmail, email: &str, resume: bool) -> Result<()> {
    let mut compose = Compose::new(email.into());
    compose.to = email.into();
    compose.subject = format!("Luma-Reply-Regression-{}", uuid::Uuid::new_v4());
    compose.html = true;
    compose.text = luma_core::rich_draft::with_quote(
        "<div data-luma-editor=\"1\"><b>Synthetic new reply</b></div>",
        "Synthetic previous message. Read-only in the Luma editor.",
    );
    let sent = if resume {
        let page = gmail
            .list(
                &Folder::Sent,
                "subject:Luma-Reply-Regression from:me to:me",
                None,
            )
            .await?;
        let row = page
            .items
            .iter()
            .find(|r| {
                r.subject
                    .strip_prefix("Luma-Reply-Regression-")
                    .is_some_and(|id| uuid::Uuid::parse_str(id).is_ok())
            })
            .ok_or_else(|| Error::Invalid("No exact synthetic regression mail to resume".into()))?;
        let thread = gmail.thread_metadata(&row.key.id).await?;
        let message = thread
            .messages
            .iter()
            .find(|m| m.payload.header("Subject") == row.subject)
            .ok_or_else(|| Error::Invalid("Synthetic message missing".into()))?;
        compose.subject = row.subject.clone();
        gmail.message(&message.id).await?
    } else {
        let mut draft = gmail.save_draft(None, &compose.mime()?, None).await?;
        let id = draft.id.clone();
        for index in 0..3 {
            compose.retained.push(RetainedAttachment {
                name: format!("synthetic-paste-{index}.txt"),
                mime_type: "text/plain".into(),
                data: vec![b'X'; 64].into(),
                content_id: None,
            });
            draft = gmail
                .save_draft(Some(&draft), &compose.mime()?, None)
                .await?;
            assert_eq!(
                draft.id, id,
                "Every autosave must update the same server draft"
            );
        }
        let sent = gmail.send_draft(&draft).await?;
        let drafts = gmail.list_drafts(None).await?;
        assert!(!drafts.items.iter().any(|d| d.id == id));
        sent
    };
    let remote = gmail.message(&sent.id).await?;
    if !readable(&remote.payload)?.contains("Synthetic new reply") {
        return Err(Error::Invalid(
            "Refusing to modify mail without synthetic marker".into(),
        ));
    }
    assert_eq!(gmail.message_attachments(&remote).await?.len(), 3);
    // Search indexing can lag direct message retrieval. Never resend during this wait.
    let mut delivered = 0;
    for _ in 0..5 {
        let response: serde_json::Value = luma_core::http::json(
            gmail
                .call(
                    reqwest::Method::GET,
                    "/messages",
                    &[
                        ("q", format!("subject:\"{}\"", compose.subject)),
                        ("labelIds", "SENT".into()),
                        ("maxResults", "3".into()),
                    ],
                    None,
                )
                .await?,
        )
        .await?;
        delivered = response
            .get("messages")
            .and_then(|v| v.as_array())
            .map_or(0, Vec::len);
        if delivered != 0 {
            break;
        }
        tokio::time::sleep(std::time::Duration::from_secs(2)).await;
    }
    assert_eq!(
        delivered, 1,
        "Exactly one synthetic message delivered (not thread count)"
    );
    gmail.act(&sent.id, true, &MailAction::Trash).await?;
    println!(
        "PASS real Gmail: three attachment autosaves reuse one draft; one rich message delivered; sent draft absent; synthetic self-mail moved to Trash"
    );
    Ok(())
}

async fn exercise(gmail: &Gmail, email: &str, paths: &Paths, resume: bool) -> Result<()> {
    let mut run = format!("Luma-Acceptance-{}", uuid::Uuid::new_v4());
    let mut compose = Compose::new(email.into());
    compose.to = email.into();
    compose.subject = run.clone();
    compose.text = format!(
        "{run}\nSynthetic mail created by the Luma acceptance check. No private mailbox contents are copied."
    );
    compose.retained.push(RetainedAttachment {
        name: "luma-acceptance.txt".into(),
        mime_type: "text/plain".into(),
        data: b"Luma synthetic attachment round trip\n".to_vec().into(),
        content_id: None,
    });
    if !resume && let Some(root) = std::env::var_os("LUMA_FIXTURE_DIR") {
        let root = std::path::PathBuf::from(root);
        for (name, mime, cid) in [
            ("Coast.jpg", "image/jpeg", Some("luma-preview-photo")),
            (
                "Weekend plan.docx",
                "application/vnd.openxmlformats-officedocument.wordprocessingml.document",
                None,
            ),
            ("Voice note.wav", "audio/wav", None),
        ] {
            compose.retained.push(RetainedAttachment {
                name: name.into(),
                mime_type: mime.into(),
                data: std::fs::read(root.join(name))?.into(),
                content_id: cid.map(String::from),
            });
        }
        compose.html = true;
        compose.text = format!(
            "<div style=\"padding:24px;background-color:#f4f0ff;color:#252638\"><h1>Luma preview check</h1><p>{run}</p><p>Synthetic mail created by the Luma acceptance check. No private mailbox contents are copied.</p><p><strong>A formatted message</strong> with a <a href=\"https://github.com/SleepyZzed/luma-mail\">real hyperlink</a>.</p><img src=\"cid:luma-preview-photo\" width=\"600\" alt=\"Synthetic coast fixture\"><p>See the attachment strip for the Word document and audio note.</p></div>"
        );
    }
    compose.validate_send()?;
    let sent = if resume {
        let page = gmail
            .list(
                &Folder::AllMail,
                "from:me to:me subject:Luma-Acceptance",
                None,
            )
            .await?;
        let row = page
            .items
            .into_iter()
            .find(|r| {
                r.subject
                    .strip_prefix("Luma-Acceptance-")
                    .is_some_and(|id| uuid::Uuid::parse_str(id).is_ok())
            })
            .ok_or_else(|| Error::Invalid("No synthetic acceptance mail to resume".into()))?;
        run = row.subject;
        let thread = gmail.thread_metadata(&row.key.id).await?;
        let id = thread
            .messages
            .iter()
            .find(|m| m.payload.header("Subject") == run)
            .ok_or_else(|| Error::Invalid("Synthetic message disappeared".into()))?
            .id
            .clone();
        let message = gmail.message(&id).await?;
        if !readable(&message.payload)?
            .contains("Synthetic mail created by the Luma acceptance check.")
        {
            return Err(Error::Invalid(
                "Refusing to modify mail without the synthetic acceptance marker".into(),
            ));
        }
        println!("Resuming only the existing synthetic self-mail");
        message
    } else {
        let draft = gmail.save_draft(None, &compose.mime()?, None).await?;
        let reloaded = gmail.draft(&draft.id).await?;
        assert_eq!(reloaded.message.payload.header("Subject"), run);
        compose.text.push_str("\nUpdated draft contents.");
        let updated = gmail
            .save_draft(Some(&reloaded), &compose.mime()?, None)
            .await?;
        let (restored, attachments) = gmail.edit_draft(&updated.id).await?;
        assert!(readable(&restored.message.payload)?.contains("Updated draft contents."));
        assert_eq!(attachments.len(), compose.retained.len());
        for expected in &compose.retained {
            assert!(
                attachments
                    .iter()
                    .any(|part| part.name == expected.name && part.data == expected.data)
            );
        }
        assert!(matches!(
            gmail
                .save_draft(Some(&reloaded), &compose.mime()?, None)
                .await,
            Err(Error::DraftConflict)
        ));
        println!(
            "PASS real draft create/reload/update, attachment preservation and stale-edit conflict"
        );
        gmail.send_draft(&updated).await?
    };
    let message = gmail.message(&sent.id).await?;
    assert_eq!(message.payload.header("Subject"), run);
    let parts = gmail.message_attachments(&message).await?;
    assert!(
        parts
            .iter()
            .any(|part| part.name == compose.retained[0].name
                && part.data == compose.retained[0].data)
    );
    println!("PASS send to self, message retrieval and on-demand attachment bytes");
    let label = if let Some(existing) = gmail
        .labels()
        .await?
        .into_iter()
        .find(|l| l.name == run || l.name == format!("{run}-verified"))
    {
        existing
    } else {
        gmail.create_label(&run).await?
    };
    let renamed = gmail
        .rename_label(&label.id, &format!("{run}-verified"))
        .await?;
    gmail
        .act(&sent.id, true, &MailAction::AddLabel(label.id.clone()))
        .await?;
    assert!(
        gmail
            .message_metadata(&sent.id)
            .await?
            .label_ids
            .contains(&label.id)
    );
    gmail.act(&sent.id, true, &MailAction::Star).await?;
    assert!(
        gmail
            .message_metadata(&sent.id)
            .await?
            .label_ids
            .iter()
            .any(|x| x == "STARRED")
    );
    gmail.act(&sent.id, true, &MailAction::Unread).await?;
    assert!(
        gmail
            .message_metadata(&sent.id)
            .await?
            .label_ids
            .iter()
            .any(|x| x == "UNREAD")
    );
    gmail.act(&sent.id, true, &MailAction::Read).await?;
    let archived = gmail
        .act_confirmed(&sent.thread_id, false, &MailAction::Archive)
        .await?;
    assert!(!archived.is_empty() && archived.iter().all(|m| !m.has("INBOX")));
    println!("PASS direct archive confirmation supplies updated row labels");
    assert!(
        !gmail
            .message_metadata(&sent.id)
            .await?
            .label_ids
            .iter()
            .any(|x| x == "INBOX")
    );
    gmail.act(&sent.id, true, &MailAction::Inbox).await?;
    assert!(
        gmail
            .message_metadata(&sent.id)
            .await?
            .label_ids
            .iter()
            .any(|x| x == "INBOX")
    );
    let found = gmail
        .list(&Folder::AllMail, &format!("subject:{run}"), None)
        .await?;
    assert!(found.items.iter().any(|c| c.key.id == sent.thread_id));
    gmail.act(&sent.id, true, &MailAction::Trash).await?;
    assert!(
        gmail
            .message_metadata(&sent.id)
            .await?
            .label_ids
            .iter()
            .any(|x| x == "TRASH")
    );
    gmail.act(&sent.id, true, &MailAction::Restore).await?;
    assert!(
        !gmail
            .message_metadata(&sent.id)
            .await?
            .label_ids
            .iter()
            .any(|x| x == "TRASH")
    );
    println!(
        "PASS live label create/rename/apply, star, read/unread, archive/inbox, server search, Trash/restore"
    );
    let report = serde_json::json!({ "run":run, "message_id":sent.id, "thread_id":sent.thread_id, "label_id":renamed.id, "scope":"gmail.modify", "result":"passed", "notes":"Synthetic self-mail and label retained for visual inspection. No permanent message deletion." });
    std::fs::write(
        paths.config.join("acceptance-report.json"),
        serde_json::to_vec_pretty(&report)?,
    )?;
    println!("PASS report saved; synthetic self-mail and label retained for visual inspection");
    Ok(())
}
