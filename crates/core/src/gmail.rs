use crate::{
    Error, Result,
    auth::TokenManager,
    http::{Http, bounded_bytes, json},
    model::*,
    provider::*,
};
use async_trait::async_trait;
use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use futures_util::{StreamExt, stream};
use reqwest::Method;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::sync::Arc;

#[derive(Clone)]
pub struct Gmail {
    pub account: String,
    pub http: Http,
    pub tokens: Arc<TokenManager>,
    base: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Profile {
    pub email_address: String,
    pub history_id: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Draft {
    pub id: String,
    pub message: Message,
}

impl Gmail {
    pub fn new(account: String, http: Http, tokens: Arc<TokenManager>) -> Self {
        Self {
            account,
            http,
            tokens,
            base: "https://gmail.googleapis.com/gmail/v1/users/me".into(),
        }
    }
    #[cfg(test)]
    pub(crate) fn test_base(&mut self, base: String) {
        self.base = base;
    }
    pub async fn call(
        &self,
        method: Method,
        path: &str,
        query: &[(&str, String)],
        body: Option<&serde_json::Value>,
    ) -> Result<crate::http::HttpResponse> {
        self.http
            .request(
                method,
                &format!("{}{path}", self.base),
                &self.tokens.access_token().await?,
                query,
                body,
            )
            .await
    }
    pub async fn profile_with_token(http: &Http, access: &str) -> Result<Profile> {
        json(
            http.request(
                Method::GET,
                "https://gmail.googleapis.com/gmail/v1/users/me/profile",
                access,
                &[],
                None,
            )
            .await?,
        )
        .await
    }
    pub async fn unread(&self) -> Result<u64> {
        let label: Label = json(self.call(Method::GET, "/labels/INBOX", &[], None).await?).await?;
        Ok(label.messages_unread)
    }
    pub async fn profile(&self) -> Result<Profile> {
        json(self.call(Method::GET, "/profile", &[], None).await?).await
    }
    pub async fn message_metadata(&self, id: &str) -> Result<Message> {
        check_id(id)?;
        json(
            self.call(
                Method::GET,
                &format!("/messages/{id}"),
                &[
                    ("format", "metadata".into()),
                    ("metadataHeaders", "From".into()),
                    ("metadataHeaders", "Subject".into()),
                    (
                        "fields",
                        "id,threadId,labelIds,internalDate,payload(headers)".into(),
                    ),
                ],
                None,
            )
            .await?,
        )
        .await
    }
    pub async fn create_label(&self, name: &str) -> Result<Label> {
        json(self.call(Method::POST, "/labels", &[], Some(&json!({"name":name,"labelListVisibility":"labelShow","messageListVisibility":"show"}))).await?).await
    }
    pub async fn rename_label(&self, id: &str, name: &str) -> Result<Label> {
        check_id(id)?;
        json(
            self.call(
                Method::PATCH,
                &format!("/labels/{id}"),
                &[],
                Some(&json!({"name":name})),
            )
            .await?,
        )
        .await
    }
    pub async fn delete_label(&self, id: &str) -> Result<()> {
        check_id(id)?;
        self.call(Method::DELETE, &format!("/labels/{id}"), &[], None)
            .await?;
        Ok(())
    }
    /// Metadata only: opening a conversation never fetches all its bodies.
    async fn metadata(&self, id: &str) -> Result<Thread> {
        check_id(id)?;
        json(self.call(Method::GET, &format!("/threads/{id}"), &[
            // Full MIME structure projected to metadata fields: no body data.
            ("format", "full".into()),
            ("fields", "id,snippet,messages(id,threadId,labelIds,snippet,internalDate,historyId,payload(mimeType,filename,headers(name,value),parts(mimeType,filename,body(size,attachmentId))))".into()),
            ("metadataHeaders", "From".into()), ("metadataHeaders", "To".into()),
            ("metadataHeaders", "Cc".into()), ("metadataHeaders", "Subject".into()),
            ("metadataHeaders", "Message-ID".into()), ("metadataHeaders", "References".into()),
            ("metadataHeaders", "Reply-To".into()),
        ], None).await?).await
    }
    pub async fn attachment(&self, message: &str, attachment: &str) -> Result<Vec<u8>> {
        check_id(message)?;
        check_id(attachment)?;
        let response = self
            .call(
                Method::GET,
                &format!("/messages/{message}/attachments/{attachment}"),
                &[],
                None,
            )
            .await?;
        let body: PartBody =
            serde_json::from_slice(&bounded_bytes(response, 36 * 1024 * 1024).await?)?;
        URL_SAFE_NO_PAD
            .decode(body.data.as_deref().unwrap_or("").trim_end_matches('='))
            .map_err(|_| Error::Invalid("Attachment has invalid encoding".into()))
    }
    pub async fn draft(&self, id: &str) -> Result<Draft> {
        check_id(id)?;
        json(
            self.call(
                Method::GET,
                &format!("/drafts/{id}"),
                &[("format", "full".into())],
                None,
            )
            .await?,
        )
        .await
    }
    /// Explicit edit action only. Preserve attachments in bounded memory; never
    /// spool private draft attachments into an unbudgeted temporary directory.
    pub async fn edit_draft(
        &self,
        id: &str,
    ) -> Result<(Draft, Vec<crate::composer::RetainedAttachment>)> {
        let draft = self.draft(id).await?;
        let retained = self.message_attachments(&draft.message).await?;
        Ok((draft, retained))
    }
    pub async fn message_attachments(
        &self,
        message: &Message,
    ) -> Result<Vec<crate::composer::RetainedAttachment>> {
        let mut retained = Vec::new();
        let mut bytes = 0u64;
        for part in message.payload.attachments() {
            if bytes.saturating_add(part.body.size) > crate::composer::MAX_ATTACHMENT_BYTES {
                return Err(Error::Invalid(
                    "This draft's attachments exceed the 16 MiB editing limit; edit it in Gmail."
                        .into(),
                ));
            }
            let data = if let Some(id) = &part.body.attachment_id {
                self.attachment(&message.id, id).await?
            } else {
                crate::composer::decode_part(part)?
            };
            bytes = bytes.saturating_add(data.len() as u64);
            if bytes > crate::composer::MAX_ATTACHMENT_BYTES {
                return Err(Error::Invalid(
                    "Draft attachments exceed the editing limit".into(),
                ));
            }
            let cid = part.header("Content-ID");
            retained.push(crate::composer::RetainedAttachment {
                name: if part.filename.is_empty() {
                    "attachment".into()
                } else {
                    part.filename.clone()
                },
                mime_type: part.mime_type.clone(),
                data,
                content_id: if cid.is_empty() {
                    None
                } else {
                    Some(cid.into())
                },
            });
        }
        Ok(retained)
    }
    pub async fn list_drafts(&self, page: Option<&str>) -> Result<Page<Draft>> {
        #[derive(Deserialize)]
        #[serde(rename_all = "camelCase")]
        struct Response {
            #[serde(default)]
            drafts: Vec<Draft>,
            next_page_token: Option<String>,
        }
        let mut q = vec![("maxResults", "25".into())];
        if let Some(token) = page {
            q.push(("pageToken", token.into()));
        }
        let response: Response = json(self.call(Method::GET, "/drafts", &q, None).await?).await?;
        Ok(Page {
            items: response.drafts,
            next: response.next_page_token,
        })
    }
    pub async fn find_draft(&self, message: &str, rfc_id: &str) -> Result<Draft> {
        #[derive(Deserialize)]
        struct Response {
            #[serde(default)]
            drafts: Vec<Draft>,
        }
        if rfc_id.is_empty() {
            return Err(Error::Invalid("This draft has no Message-ID. Edit it in Gmail to avoid scanning the draft mailbox.".into()));
        }
        let response: Response = json(
            self.call(
                Method::GET,
                "/drafts",
                &[
                    ("maxResults", "25".into()),
                    ("q", format!("rfc822msgid:{rfc_id}")),
                ],
                None,
            )
            .await?,
        )
        .await?;
        response
            .drafts
            .into_iter()
            .find(|d| d.message.id == message)
            .ok_or_else(|| {
                Error::Invalid("Draft changed or was deleted. Refresh Gmail drafts.".into())
            })
    }
    /// Gmail has no compare-and-swap for drafts. Check current message ID first;
    /// callers serialize saves. A remaining cross-client race is documented.
    pub async fn save_draft(
        &self,
        existing: Option<&Draft>,
        raw: &[u8],
        thread: Option<&str>,
    ) -> Result<Draft> {
        if let Some(draft) = existing {
            let current = self.draft(&draft.id).await?;
            if current.message.id != draft.message.id {
                return Err(Error::DraftConflict);
            }
        }
        let mut message = json!({"raw": URL_SAFE_NO_PAD.encode(raw)});
        if let Some(id) = thread {
            message["threadId"] = json!(id);
        }
        let (method, path) = if let Some(draft) = existing {
            check_id(&draft.id)?;
            (Method::PUT, format!("/drafts/{}", draft.id))
        } else {
            (Method::POST, "/drafts".into())
        };
        json(
            self.call(method, &path, &[], Some(&json!({"message":message})))
                .await?,
        )
        .await
    }
    pub async fn discard_draft(&self, id: &str) -> Result<()> {
        check_id(id)?;
        self.call(Method::DELETE, &format!("/drafts/{id}"), &[], None)
            .await?;
        Ok(())
    }
    pub async fn send_draft(&self, draft: &Draft) -> Result<Message> {
        json(
            self.call(
                Method::POST,
                "/drafts/send",
                &[],
                Some(&json!({"id":draft.id})),
            )
            .await?,
        )
        .await
    }
}

pub fn check_id(id: &str) -> Result<()> {
    if id.is_empty()
        || !id
            .bytes()
            .all(|c| c.is_ascii_alphanumeric() || b"_-".contains(&c))
    {
        return Err(Error::Invalid("Invalid Google resource identifier".into()));
    }
    Ok(())
}

#[async_trait]
impl MailProvider for Gmail {
    fn account_id(&self) -> &str {
        &self.account
    }
    async fn list(
        &self,
        folder: &Folder,
        search: &str,
        page: Option<&str>,
    ) -> Result<Page<Conversation>> {
        #[derive(Deserialize)]
        #[serde(rename_all = "camelCase")]
        struct Response {
            #[serde(default)]
            threads: Vec<Thread>,
            next_page_token: Option<String>,
        }
        let mut query = vec![
            ("maxResults", "25".into()),
            ("q", format!("{} {search}", folder.query())),
            (
                "includeSpamTrash",
                matches!(folder, Folder::Trash).to_string(),
            ),
        ];
        if let Some(page) = page {
            query.push(("pageToken", page.into()));
        }
        if let Folder::Label(id) = folder {
            query.push(("labelIds", id.clone()));
        }
        let response: Response =
            json(self.call(Method::GET, "/threads", &query, None).await?).await?;
        let items: Vec<Result<Option<Conversation>>> =
            stream::iter(response.threads.into_iter().map(|t| async move {
                Ok(Conversation::from_thread(
                    &self.account,
                    &self.metadata(&t.id).await?,
                ))
            }))
            .buffer_unordered(4)
            .collect()
            .await;
        let mut conversations = Vec::new();
        for item in items {
            if let Some(item) = item? {
                conversations.push(item);
            }
        }
        conversations.sort_by(|a, b| b.date.cmp(&a.date).then(a.key.cmp(&b.key)));
        Ok(Page {
            items: conversations,
            next: response.next_page_token,
        })
    }
    async fn thread(&self, id: &str) -> Result<Thread> {
        self.metadata(id).await
    }
    async fn message(&self, id: &str) -> Result<Message> {
        check_id(id)?;
        json(
            self.call(
                Method::GET,
                &format!("/messages/{id}"),
                &[("format", "full".into())],
                None,
            )
            .await?,
        )
        .await
    }
    async fn act(&self, id: &str, message: bool, action: &MailAction) -> Result<()> {
        check_id(id)?;
        let resource = if message { "messages" } else { "threads" };
        let endpoint = match action {
            MailAction::Trash => "trash",
            MailAction::Restore => "untrash",
            _ => "modify",
        };
        let (add, remove) = action.labels();
        let body = json!({"addLabelIds": add, "removeLabelIds": remove});
        self.call(
            Method::POST,
            &format!("/{resource}/{id}/{endpoint}"),
            &[],
            if endpoint == "modify" {
                Some(&body)
            } else {
                None
            },
        )
        .await?;
        Ok(())
    }
    async fn labels(&self) -> Result<Vec<Label>> {
        #[derive(Deserialize)]
        struct Response {
            #[serde(default)]
            labels: Vec<Label>,
        }
        let response: Response = json(self.call(Method::GET, "/labels", &[], None).await?).await?;
        Ok(response.labels)
    }
}
