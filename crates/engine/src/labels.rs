//! On-demand account label metadata. No disk mirror or polling task.
use crate::{app::App, protocol::Command};
use luma_core::{Error, Result, model::Label, provider::MailProvider};
use serde_json::json;
pub async fn run(app: &App, c: &Command) -> Result<()> {
    let account = c.text("account");
    if !app
        .settings
        .lock()
        .unwrap()
        .accounts
        .iter()
        .any(|a| a.id == account)
    {
        return Err(Error::Invalid("Choose an account first".into()));
    }
    let _write = app.mutation.lock().await;
    let operation = c.text("operation");
    if !["", "list", "create", "rename", "delete"].contains(&operation) {
        return Err(Error::Invalid("Unknown label operation".into()));
    }
    let name = c.text("name").trim();
    if ["create", "rename"].contains(&operation)
        && (name.is_empty() || name.len() > 225 || name.chars().any(char::is_control))
    {
        return Err(Error::Invalid(
            "Enter a label name of up to 225 bytes".into(),
        ));
    }
    if operation == "delete" && !c.flag("confirmed") {
        return Err(Error::Invalid("Confirm label deletion first".into()));
    }
    let items = if app.preview {
        let mut all = app.preview_labels.lock().unwrap();
        let items = all.entry(account.into()).or_insert_with(|| {
            vec![
                Label {
                    id: "Label_work".into(),
                    name: "Work".into(),
                    kind: "user".into(),
                    ..Default::default()
                },
                Label {
                    id: "Label_personal".into(),
                    name: "Personal".into(),
                    kind: "user".into(),
                    ..Default::default()
                },
            ]
        });
        match operation {
            "create" => items.push(Label {
                id: format!("Label_preview_{}", c.id),
                name: name.into(),
                kind: "user".into(),
                ..Default::default()
            }),
            "rename" => {
                items
                    .iter_mut()
                    .find(|l| l.id == c.text("label") && l.kind == "user")
                    .ok_or_else(|| Error::Invalid("Label no longer exists".into()))?
                    .name = name.into()
            }
            "delete" => {
                if !items
                    .iter()
                    .any(|l| l.id == c.text("label") && l.kind == "user")
                {
                    return Err(Error::Invalid("Only your own labels can be deleted".into()));
                }
                items.retain(|l| l.id != c.text("label"));
            }
            _ => (),
        }
        items.clone()
    } else {
        let client = app.client(account)?;
        if ["rename", "delete"].contains(&operation)
            && !client
                .labels()
                .await?
                .iter()
                .any(|l| l.id == c.text("label") && l.kind == "user")
        {
            return Err(Error::Invalid("Only your own labels can be changed".into()));
        }
        match operation {
            "create" => {
                client.create_label(name).await?;
            }
            "rename" => {
                client.rename_label(c.text("label"), name).await?;
            }
            "delete" => client.delete_label(c.text("label")).await?,
            _ => (),
        }
        client.labels().await?
    };
    let mut users: Vec<_> = items
        .iter()
        .filter(|l| l.kind == "user")
        .map(|l| json!({"id":l.id,"name":l.name,"type":l.kind}))
        .collect();
    users.sort_by(|a, b| a["name"].as_str().cmp(&b["name"].as_str()));
    app.events
        .emit(
            c.id,
            "labels",
            json!({"account":account,"items":users,"operation":operation,"label":c.text("label")}),
        )
        .await;
    Ok(())
}
