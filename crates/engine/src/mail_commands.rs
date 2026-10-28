//! Explicit destructive draft actions and existing Gmail-side sender filters.
use crate::{
    app::{self, App},
    protocol::Command,
};
use luma_core::{
    Error, Result,
    config::BlockedSender,
    model::{Folder, ThreadKey},
    sender_rules,
};
use serde_json::json;
use std::sync::Arc;
pub async fn delete_drafts(app: &Arc<App>, c: &Command) -> Result<()> {
    if !c.flag("confirmed") {
        return Err(Error::Invalid(
            "Confirm permanent draft deletion first".into(),
        ));
    }
    let keys: Vec<ThreadKey> = c.data["keys"]
        .as_array()
        .ok_or_else(|| Error::Invalid("Select draft conversations first".into()))?
        .iter()
        .map(app::key)
        .collect::<Result<_>>()?;
    if keys.is_empty() || keys.len() > 1000 {
        return Err(Error::Invalid("Select loaded drafts first".into()));
    }
    let _start = app.compose_start.lock().await;
    if app
        .composer
        .lock()
        .unwrap()
        .as_ref()
        .is_some_and(|s| s.active())
    {
        return Err(Error::Invalid(
            "Close the composer before deleting Gmail drafts".into(),
        ));
    }
    let _write = app.mutation.lock().await;
    let generation = app.generation.load(std::sync::atomic::Ordering::SeqCst);
    {
        let view = app.view.lock().unwrap();
        if view.folder != Folder::Drafts
            || keys
                .iter()
                .any(|key| !view.rows.iter().any(|r| &r.key == key))
        {
            return Err(Error::Invalid(
                "Delete draft is available only for loaded conversations in Drafts".into(),
            ));
        }
    }
    app.status(c.id, "saving", "Deleting the selected Gmail drafts")
        .await;
    let mut removed = Vec::new();
    let mut failures = Vec::new();
    let mut count = 0;
    for key in keys {
        let result = if app.preview {
            Ok(1)
        } else {
            app.client(&key.account)?
                .discard_thread_drafts(&key.id)
                .await
        };
        match result {
            Ok(n) => {
                count += n;
                removed.push(key);
            }
            Err(e) => failures.push(e.to_string()),
        }
    }
    if !removed.is_empty() {
        app.undo.lock().unwrap().clear();
    }
    if app.current(generation) {
        app.view
            .lock()
            .unwrap()
            .rows
            .retain(|r| !removed.contains(&r.key));
        app.events
            .emit(
                c.id,
                "mutation",
                json!({"generation":generation,"removed":removed,"changed":[],"undo":false}),
            )
            .await;
    }
    app.events
        .emit(
            c.id,
            "draftsDeleted",
            json!({"count":count,"errors":failures}),
        )
        .await;
    if !failures.is_empty() {
        return Err(Error::Invalid(failures.join("\n")));
    }
    app.status(c.id, "idle", "Drafts deleted · sent and received mail kept")
        .await;
    Ok(())
}
pub async fn sender_rule(app: &Arc<App>, c: &Command) -> Result<()> {
    if !c.flag("confirmed") {
        return Err(Error::Invalid("Confirm the Gmail sender rule first".into()));
    }
    let address = sender_rules::address(c.text("sender"))?;
    let _write = app.mutation.lock().await;
    let account = app
        .settings
        .lock()
        .unwrap()
        .accounts
        .iter()
        .find(|a| a.id == c.text("account"))
        .cloned()
        .ok_or_else(|| Error::Invalid("Account no longer exists".into()))?;
    if !account.sender_controls && !app.preview {
        return Err(Error::Invalid(
            "Connect Gmail sender controls in Settings first".into(),
        ));
    }
    let existing = account
        .blocked_senders
        .iter()
        .find(|b| b.address == address)
        .cloned();
    let mut blocked = account.blocked_senders.clone();
    match c.text("operation") {
        "block" if existing.is_none() => {
            if blocked.len() >= 1000 {
                return Err(Error::ResponseLimit);
            }
            let id = if app.preview {
                format!("preview_filter_{}", c.id)
            } else {
                app.client(&account.id)?
                    .create_sender_block(&address)
                    .await?
                    .id
            };
            blocked.push(BlockedSender {
                address: address.clone(),
                filter_id: id,
            });
        }
        "block" => (),
        "unblock" => {
            let rule = existing.ok_or_else(|| {
                Error::Invalid("Luma has no saved reference for this sender rule".into())
            })?;
            if !app.preview {
                app.client(&account.id)?
                    .remove_sender_block(&rule.filter_id, &rule.address)
                    .await?;
            }
            blocked.retain(|b| b.address != address);
        }
        _ => return Err(Error::Invalid("Unknown sender rule operation".into())),
    }
    let _preferences = app.preferences.lock().await;
    let settings = {
        let mut settings = app.settings.lock().unwrap();
        if let Some(a) = settings.accounts.iter_mut().find(|a| a.id == account.id) {
            a.blocked_senders = blocked;
        }
        settings.clone()
    };
    if !app.preview {
        app.paths.save(&settings).map_err(|_|Error::Invalid("The rule changed in Gmail, but its local reference could not be saved. Review it in Gmail Settings before retrying.".into()))?;
    }
    crate::accounts::publish(app, c.id).await;
    app.events
        .emit(
            c.id,
            "senderRuleSaved",
            json!({"account":account.id,"operation":c.text("operation")}),
        )
        .await;
    app.status(
        c.id,
        "idle",
        if c.text("operation") == "block" {
            "Sender blocked in Gmail · future mail goes to Trash"
        } else {
            "Gmail sender rule removed"
        },
    )
    .await;
    Ok(())
}
