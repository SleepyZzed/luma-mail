use crate::{
    app::{self, App},
    protocol::Command,
};
use luma_core::{Error, Result, search::Search};
use serde_json::json;
pub async fn build(app: &App, c: &Command) -> Result<()> {
    let query = Search {
        from: c.text("from").into(),
        to: c.text("to").into(),
        subject: c.text("subject").into(),
        words: c.text("words").into(),
        after: c.text("after").into(),
        before: c.text("before").into(),
        unread: c.flag("unread"),
        starred: c.flag("starred"),
        attachments: c.flag("attachments"),
    }
    .query()?;
    let folder = app::folder(c.text("folder"))?;
    let accounts = if c.flag("allAccounts") {
        None
    } else {
        let ids = c.data["accounts"]
            .as_array()
            .filter(|ids| !ids.is_empty() && ids.len() <= 1000)
            .ok_or_else(|| Error::Invalid("Choose at least one account to search".into()))?;
        let settings = app.settings.lock().unwrap();
        let mut selected = Vec::new();
        for id in ids {
            let id = id
                .as_str()
                .ok_or_else(|| Error::Invalid("Invalid search account".into()))?;
            if !settings.accounts.iter().any(|a| a.id == id && a.enabled) {
                return Err(Error::Invalid(
                    "Choose enabled accounts for this search".into(),
                ));
            }
            if !selected.iter().any(|a| a == id) {
                selected.push(id.to_owned());
            }
        }
        Some(selected)
    };
    if matches!(folder, luma_core::model::Folder::Label(_)) {
        let view = app.view.lock().unwrap();
        if accounts.as_ref().is_none_or(|ids| {
            ids.len() != 1
                || view
                    .accounts
                    .as_ref()
                    .is_none_or(|current| !current.contains(&ids[0]))
        }) {
            return Err(Error::Invalid(
                "A label-scoped search must use the account that owns that label".into(),
            ));
        }
    }
    app.events
        .emit(
            c.id,
            "searchBuilt",
            json!({"query":query,"accounts":accounts,"folder":c.text("folder")}),
        )
        .await;
    Ok(())
}
