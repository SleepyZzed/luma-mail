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
        Some("connect") => {
            let drive = args.iter().any(|a| a == "--drive");
            let filters = args.iter().any(|a| a == "--filters");
            let authorization = Authorization::begin_with_settings(&config, drive, filters).await?;
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
        Some("check") | Some("exercise") | Some("filters") => {
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
        data: b"Luma synthetic attachment round trip\n".to_vec(),
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
                data: std::fs::read(root.join(name))?,
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
