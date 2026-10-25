//! Gmail-side future-mail rules. No client-only block list or history crawler.
use crate::{
    Error, Result,
    composer::recipients,
    gmail::{Gmail, check_id},
    http::json,
};
use reqwest::Method;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Filter {
    pub id: String,
    pub criteria: Value,
    pub action: Value,
}
pub fn address(sender: &str) -> Result<String> {
    let addresses = recipients(sender)?;
    if addresses.len() != 1 {
        return Err(Error::Invalid("Choose one sender address".into()));
    }
    let value = &addresses[0];
    let email = value
        .rsplit_once('<')
        .map(|(_, email)| email.trim_end_matches('>'))
        .unwrap_or(value)
        .trim();
    Ok(email.to_ascii_lowercase())
}
impl Gmail {
    pub async fn create_sender_block(&self, sender: &str) -> Result<Filter> {
        let sender = address(sender)?;
        let body = json!({"criteria":{"from":sender},"action":{"addLabelIds":["TRASH"],"removeLabelIds":["INBOX"]}});
        json(
            self.call(Method::POST, "/settings/filters", &[], Some(&body))
                .await?,
        )
        .await
    }
    pub async fn sender_filters(&self) -> Result<Vec<Filter>> {
        #[derive(Deserialize)]
        struct Response {
            #[serde(default)]
            filter: Vec<Filter>,
        }
        let response: Response = json(
            self.call(Method::GET, "/settings/filters", &[], None)
                .await?,
        )
        .await?;
        Ok(response.filter)
    }
    /// Unblock only the exact filter the app created. If it was edited in Gmail,
    /// do not delete unrelated criteria, forwarding or other user-owned actions.
    pub async fn remove_sender_block(&self, id: &str, sender: &str) -> Result<()> {
        check_id(id)?;
        let sender = address(sender)?;
        let filter: Filter = json(
            self.call(Method::GET, &format!("/settings/filters/{id}"), &[], None)
                .await?,
        )
        .await?;
        if filter.id != id
            || filter.criteria != json!({"from":sender})
            // Gmail normalizes the redundant INBOX removal away when TRASH is
            // added. Accept exactly those two equivalent server representations.
            || (filter.action != json!({"addLabelIds":["TRASH"],"removeLabelIds":["INBOX"]})
                && filter.action != json!({"addLabelIds":["TRASH"]}))
        {
            return Err(Error::Invalid(
                "This filter was changed in Gmail. Review it in Gmail Settings before removing it."
                    .into(),
            ));
        }
        self.call(
            Method::DELETE,
            &format!("/settings/filters/{id}"),
            &[],
            None,
        )
        .await?;
        Ok(())
    }
}
