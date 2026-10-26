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
use std::{collections::HashSet, sync::Arc};
#[path = "gmail_batch.rs"]
mod batch;

#[derive(Clone)]
pub struct Gmail {
    pub account: String,
    pub http: Http,
    pub tokens: Arc<TokenManager>,
    base: String,
    rate: Arc<crate::rate::Rate>,
    batch_url: String,
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
            rate: Arc::new(crate::rate::Rate::new()),
            batch_url: "https://gmail.googleapis.com/batch".into(),
        }
    }
    #[cfg(test)]
    pub(crate) fn test_base(&mut self, base: String) {
        self.batch_url = format!("{base}/batch");
        self.base = base;
    }
    pub async fn call(
        &self,
        method: Method,
        path: &str,
        query: &[(&str, String)],
        body: Option<&serde_json::Value>,
    ) -> Result<crate::http::HttpResponse> {
        self.rate.acquire(quota_cost(&method, path, query)).await;
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
    /// Label IDs belong to one account. Resolve a user-selected label by name
    /// before using it in a different account; never reuse an unrelated ID.
    pub async fn matching_label(&self, name: &str) -> Result<String> {
        self.labels()
            .await?
            .into_iter()
            .find(|label| label.kind == "user" && label.name == name)
            .map(|label| label.id)
            .ok_or_else(|| {
                Error::Invalid(format!(
                    "The label ‘{name}’ does not exist in this account. No label change was made."
                ))
            })
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
    pub async fn thread_metadata(&self, id: &str) -> Result<Thread> {
        check_id(id)?;
        json(self.call(Method::GET, &format!("/threads/{id}"), &[
            ("format", "metadata".into()),
            ("fields", "id,messages(id,threadId,labelIds,snippet,internalDate,payload(headers(name,value)))".into()),
            ("metadataHeaders", "From".into()), ("metadataHeaders", "Subject".into()),
            ("metadataHeaders", "Message-ID".into()),
        ], None).await?).await
    }
    /// Consume Gmail's mutation response for immediate label reconciliation.
    /// Header/body projection avoids downloading the conversation again.
    pub async fn act_confirmed(
        &self,
        id: &str,
        message: bool,
        action: &MailAction,
    ) -> Result<Vec<Message>> {
        check_id(id)?;
        let resource = if message { "messages" } else { "threads" };
        let endpoint = match action {
            MailAction::Trash => "trash",
            MailAction::Restore => "untrash",
            _ => "modify",
        };
        let (add, remove) = action.labels();
        let body = json!({"addLabelIds":add,"removeLabelIds":remove});
        let fields = if message {
            "id,threadId,labelIds"
        } else {
            "id,messages(id,threadId,labelIds)"
        };
        let response = self
            .call(
                Method::POST,
                &format!("/{resource}/{id}/{endpoint}"),
                &[("fields", fields.into())],
                if endpoint == "modify" {
                    Some(&body)
                } else {
                    None
                },
            )
            .await?;
        if message {
            let result: Message = json(response).await?;
            if result.id != id {
                return Err(Error::Invalid(
                    "Gmail returned an unexpected message after the change".into(),
                ));
            }
            Ok(vec![result])
        } else {
            let result: Thread = json(response).await?;
            if result.id != id {
                return Err(Error::Invalid(
                    "Gmail returned an unexpected conversation after the change".into(),
                ));
            }
            Ok(result.messages)
        }
    }

    /// Ask Gmail for attachment flags only for this visible page's RFC message
    /// IDs. This avoids a 120-unit `format=full` request per conversation merely
    /// to draw a paperclip. No historical mailbox scan or attachment fetch.
    async fn attachment_flags(
        &self,
        threads: &[Thread],
    ) -> Result<(HashSet<String>, HashSet<String>)> {
        #[derive(Deserialize)]
        #[serde(rename_all = "camelCase")]
        struct Response {
            #[serde(default)]
            threads: Vec<Thread>,
            next_page_token: Option<String>,
        }
        let mut tokens = Vec::new();
        let mut fallback = HashSet::new();
        for thread in threads {
            for message in &thread.messages {
                if let Some(token) = message_id_query(message.payload.header("Message-ID")) {
                    tokens.push((thread.id.clone(), token));
                } else {
                    fallback.insert(thread.id.clone());
                }
            }
        }
        let mut flags = HashSet::new();
        let mut batch = Vec::new();
        let mut bytes = 0;
        for token in tokens
            .into_iter()
            .chain(std::iter::once((String::new(), String::new())))
        {
            if !batch.is_empty()
                && (batch.len() >= 25 || bytes + token.1.len() > 3000 || token.1.is_empty())
            {
                let q = format!(
                    "has:attachment {{{}}}",
                    batch
                        .iter()
                        .map(|(_, q): &(String, String)| q.as_str())
                        .collect::<Vec<_>>()
                        .join(" ")
                );
                let response: Response = json(
                    self.call(
                        Method::GET,
                        "/threads",
                        &[
                            ("q", q),
                            ("maxResults", "100".into()),
                            ("includeSpamTrash", "true".into()),
                            ("fields", "threads(id),nextPageToken".into()),
                        ],
                        None,
                    )
                    .await?,
                )
                .await?;
                if response.next_page_token.is_some() {
                    // Duplicate RFC IDs can have many matches. Do not walk a
                    // history-sized result or silently claim a missing flag.
                    fallback.extend(batch.iter().map(|(id, _)| id.clone()));
                }
                flags.extend(response.threads.into_iter().map(|t| t.id));
                batch.clear();
                bytes = 0;
            }
            if !token.1.is_empty() {
                bytes += token.1.len();
                batch.push(token);
            }
        }
        Ok((flags, fallback))
    }
    pub async fn matches_view(
        &self,
        thread: &Thread,
        folder: &Folder,
        search: &str,
    ) -> Result<Option<bool>> {
        if search.trim().is_empty() {
            return Ok(Some(folder.matches(thread)));
        }
        let Some(tokens) = thread
            .messages
            .iter()
            .map(|m| message_id_query(m.payload.header("Message-ID")))
            .collect::<Option<Vec<_>>>()
        else {
            return Ok(None);
        };
        #[derive(Deserialize)]
        #[serde(rename_all = "camelCase")]
        struct Matches {
            #[serde(default)]
            threads: Vec<Thread>,
            next_page_token: Option<String>,
        }
        if tokens.is_empty() {
            return Ok(Some(false));
        }
        let mut batch = Vec::new();
        let mut bytes = 0;
        for token in tokens.into_iter().chain(std::iter::once(String::new())) {
            if !batch.is_empty()
                && (batch.len() >= 25 || bytes + token.len() > 3000 || token.is_empty())
            {
                let result: Matches = json(
                    self.call(
                        Method::GET,
                        "/threads",
                        &[
                            (
                                "q",
                                format!("({} {search}) {{{}}}", folder.query(), batch.join(" ")),
                            ),
                            ("maxResults", "100".into()),
                            (
                                "includeSpamTrash",
                                matches!(folder, Folder::Trash).to_string(),
                            ),
                            ("fields", "threads(id),nextPageToken".into()),
                        ],
                        None,
                    )
                    .await?,
                )
                .await?;
                if result.threads.iter().any(|t| t.id == thread.id) {
                    return Ok(Some(true));
                }
                if result.next_page_token.is_some() {
                    return Ok(None);
                }
                batch.clear();
                bytes = 0;
            }
            if !token.is_empty() {
                bytes += token.len();
                batch.push(token);
            }
        }
        Ok(Some(false))
    }
    pub async fn attachment(&self, message: &str, attachment: &str) -> Result<Vec<u8>> {
        self.attachment_limited(message, attachment, 26 * 1024 * 1024)
            .await
    }
    pub async fn attachment_limited(
        &self,
        message: &str,
        attachment: &str,
        max: usize,
    ) -> Result<Vec<u8>> {
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
        let max = max.min(26 * 1024 * 1024);
        let body: PartBody =
            serde_json::from_slice(&bounded_bytes(response, max.div_ceil(3) * 4 + 4096).await?)?;
        if body.size > max as u64 {
            return Err(Error::ResponseLimit);
        }
        let data = URL_SAFE_NO_PAD
            .decode(body.data.as_deref().unwrap_or("").trim_end_matches('='))
            .map_err(|_| Error::Invalid("Attachment has invalid encoding".into()))?;
        if data.len() > max {
            return Err(Error::ResponseLimit);
        }
        Ok(data)
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
    async fn draft_revision(&self, id: &str) -> Result<String> {
        #[derive(Deserialize)]
        struct Revision {
            message: Id,
        }
        #[derive(Deserialize)]
        struct Id {
            id: String,
        }
        check_id(id)?;
        let current: Revision = json(
            self.call(
                Method::GET,
                &format!("/drafts/{id}"),
                &[
                    ("format", "minimal".into()),
                    ("fields", "message(id)".into()),
                ],
                None,
            )
            .await?,
        )
        .await?;
        Ok(current.message.id)
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
                data: data.into(),
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
            let current = self.draft_revision(&draft.id).await?;
            if current != draft.message.id {
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
    /// Delete only verified draft resources in one explicitly selected thread.
    /// Sent/received messages in the same conversation are never deleted.
    pub async fn discard_thread_drafts(&self, thread_id: &str) -> Result<usize> {
        let thread = self.thread_metadata(thread_id).await?;
        let mut drafts = Vec::new();
        for message in thread.messages.iter().filter(|m| m.has("DRAFT")) {
            drafts.push(
                self.find_draft(&message.id, message.payload.header("Message-ID"))
                    .await?,
            );
        }
        if drafts.is_empty() {
            return Err(Error::Invalid(
                "This conversation no longer contains a draft. Refresh Drafts.".into(),
            ));
        }
        // Preflight the whole selection before the first deletion. Gmail does
        // not provide compare-and-swap; recheck immediately before each DELETE.
        for draft in &drafts {
            if self.draft_revision(&draft.id).await? != draft.message.id {
                return Err(Error::DraftConflict);
            }
        }
        for draft in &drafts {
            if self.draft_revision(&draft.id).await? != draft.message.id {
                return Err(Error::DraftConflict);
            }
            self.discard_draft(&draft.id).await?;
        }
        Ok(drafts.len())
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

fn message_id_query(value: &str) -> Option<String> {
    let id = value.trim().trim_start_matches('<').trim_end_matches('>');
    if id.is_empty()
        || id.len() > 500
        || !id
            .bytes()
            .all(|b| b.is_ascii_graphic() && !b"\"\\{}<>".contains(&b))
    {
        return None;
    }
    Some(format!("rfc822msgid:\"{id}\""))
}

fn quota_cost(method: &Method, path: &str, query: &[(&str, String)]) -> u32 {
    let full = query
        .iter()
        .any(|(k, v)| *k == "format" && matches!(v.as_str(), "full" | "raw" | "html"));
    let parts: Vec<_> = path.trim_matches('/').split('/').collect();
    match parts.as_slice() {
        ["profile"] => 1,
        ["history"] => 2,
        ["settings", "filters"] | ["settings", "filters", _] => {
            if *method == Method::GET {
                1
            } else {
                5
            }
        }
        ["labels"] | ["labels", _] => {
            if *method == Method::GET {
                1
            } else {
                5
            }
        }
        ["threads"] => 10,
        ["threads", _] if *method == Method::GET => {
            if full {
                120
            } else {
                40
            }
        }
        ["threads", _, "trash"] => 20,
        ["threads", _, _] => 10,
        ["messages"] => 5,
        ["messages", _, "attachments", _] => 20,
        ["messages", _] if *method == Method::GET => {
            if full {
                60
            } else {
                20
            }
        }
        ["messages", _, "trash"] => 20,
        ["messages", _, _] => 5,
        ["drafts"] => {
            if *method == Method::POST {
                10
            } else {
                5
            }
        }
        ["drafts", "send"] => 100,
        ["drafts", _] => match *method {
            Method::GET => {
                if full {
                    60
                } else {
                    20
                }
            }
            Method::PUT => 15,
            _ => 10,
        },
        _ => 100,
    }
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
                matches!(folder, Folder::Trash | Folder::Spam).to_string(),
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
        let threads = self
            .metadata_page(response.threads.into_iter().map(|t| t.id).collect())
            .await?;
        let (flags, fallback) = self.attachment_flags(&threads).await?;
        let mut conversations = Vec::new();
        for thread in threads {
            let thread = if fallback.contains(&thread.id) {
                self.metadata(&thread.id).await?
            } else {
                thread
            };
            if let Some(mut item) = Conversation::from_thread(&self.account, &thread) {
                item.attachments |= flags.contains(&thread.id);
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
