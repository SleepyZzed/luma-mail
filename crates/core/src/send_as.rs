//! Projected, read-only sender identities. No alias creation or SMTP credentials.
use crate::{Error, Result, composer, gmail::Gmail, http::bounded_bytes, sender_rules};
use serde::Deserialize;

#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SendAs {
    pub send_as_email: String,
    #[serde(default)]
    pub display_name: String,
    #[serde(default)]
    pub reply_to_address: String,
    #[serde(default)]
    pub is_primary: bool,
    #[serde(default)]
    pub is_default: bool,
    #[serde(default)]
    pub verification_status: String,
}
impl SendAs {
    pub fn primary(email: &str) -> Self {
        Self {
            send_as_email: email.into(),
            display_name: String::new(),
            reply_to_address: String::new(),
            is_primary: true,
            is_default: true,
            verification_status: "accepted".into(),
        }
    }
    pub fn header(&self) -> String {
        if self.display_name.is_empty() {
            self.send_as_email.clone()
        } else {
            format!(
                "\"{}\" <{}>",
                self.display_name.replace('\\', "\\\\").replace('"', "\\\""),
                self.send_as_email
            )
        }
    }
}
fn usable(rows: Vec<SendAs>, primary: &str) -> Result<Vec<SendAs>> {
    if rows.len() > 100 {
        return Err(Error::ResponseLimit);
    }
    let mut out = Vec::<SendAs>::new();
    for mut row in rows {
        if !row.is_primary && row.verification_status != "accepted" {
            continue;
        }
        let address = sender_rules::address(&row.send_as_email)?;
        if !address.eq_ignore_ascii_case(&row.send_as_email)
            || (row.is_primary && !address.eq_ignore_ascii_case(primary))
            || row.display_name.len() > 512
            || row.display_name.chars().any(char::is_control)
        {
            return Err(Error::Invalid(
                "Google returned an invalid sender identity".into(),
            ));
        }
        if !row.reply_to_address.is_empty() {
            row.reply_to_address = sender_rules::address(&row.reply_to_address)?;
        }
        row.send_as_email = address;
        composer::recipients(&row.header())?;
        if !out
            .iter()
            .any(|a| a.send_as_email.eq_ignore_ascii_case(&row.send_as_email))
        {
            out.push(row);
        }
    }
    if !out.iter().any(|a| a.is_primary) {
        out.insert(0, SendAs::primary(primary));
    }
    out.sort_by_key(|a| (!a.is_primary, !a.is_default, a.send_as_email.clone()));
    Ok(out)
}
impl Gmail {
    pub async fn send_as(&self, primary: &str) -> Result<Vec<SendAs>> {
        #[derive(Deserialize)]
        #[serde(rename_all = "camelCase")]
        struct Response {
            #[serde(default)]
            send_as: Vec<SendAs>,
        }
        let response = self.call(reqwest::Method::GET, "/settings/sendAs", &[("fields", "sendAs(sendAsEmail,displayName,replyToAddress,isPrimary,isDefault,verificationStatus)".into())], None).await?;
        let bytes = bounded_bytes(response, 256 * 1024).await?;
        usable(serde_json::from_slice::<Response>(&bytes)?.send_as, primary)
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn only_verified_exact_addresses_are_sender_choices() {
        let mut alias = SendAs::primary("alias@example.invalid");
        alias.is_primary = false;
        alias.display_name = "Last, \"First\"".into();
        alias.is_default = true;
        let mut pending = alias.clone();
        pending.send_as_email = "pending@example.invalid".into();
        pending.verification_status = "pending".into();
        let rows = usable(
            vec![alias.clone(), pending, alias.clone()],
            "me@example.invalid",
        )
        .unwrap();
        assert_eq!(rows.len(), 2);
        assert!(rows[0].is_primary);
        assert_eq!(composer::recipients(&rows[1].header()).unwrap().len(), 1);
        alias.display_name = "Name\r\nBcc: other@example.invalid".into();
        assert!(usable(vec![alias], "me@example.invalid").is_err());
        assert!(
            usable(
                vec![SendAs::primary("other@example.invalid")],
                "me@example.invalid"
            )
            .is_err()
        );
    }
    #[tokio::test]
    async fn sender_listing_is_one_projected_read_without_signatures_or_smtp_secrets() {
        use wiremock::{
            Mock, MockServer, ResponseTemplate,
            matchers::{method, path, query_param},
        };
        let server = MockServer::start().await;
        Mock::given(method("GET")).and(path("/settings/sendAs"))
            .and(query_param("fields","sendAs(sendAsEmail,displayName,replyToAddress,isPrimary,isDefault,verificationStatus)"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({"sendAs":[{"sendAsEmail":"alias@example.invalid","verificationStatus":"accepted"}]}))).expect(1).mount(&server).await;
        let rows = crate::tests::gmail(&server)
            .send_as("me@example.invalid")
            .await
            .unwrap();
        assert_eq!(rows.len(), 2);
        assert_eq!(server.received_requests().await.unwrap().len(), 1);
    }
}
