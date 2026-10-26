use crate::{
    auth::*, cache::Cache, composer::*, config::*, gmail::Gmail, http::Http, model::*,
    pagination::UnifiedPager, provider::*, *,
};
use async_trait::async_trait;
use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use serde_json::json;
use std::{
    collections::HashMap,
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
    time::Duration,
};
use wiremock::{
    Mock, MockServer, ResponseTemplate,
    matchers::{body_json, header, method, path, query_param},
};
#[tokio::test]
async fn deleting_drafts_preserves_other_mail_and_refuses_changed_drafts() {
    for changed in [false, true] {
        let server = MockServer::start().await;
        Mock::given(method("GET")).and(path("/threads/thread1"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({"id":"thread1","messages":[
                {"id":"received1","labelIds":["INBOX"]},
                {"id":"draftmessage1","labelIds":["DRAFT"],"payload":{"headers":[{"name":"Message-ID","value":"<exact@example.invalid>"}]}}
            ]}))).expect(1).mount(&server).await;
        Mock::given(method("GET"))
            .and(path("/drafts"))
            .and(query_param("q", "rfc822msgid:<exact@example.invalid>"))
            .respond_with(ResponseTemplate::new(200).set_body_json(
                json!({"drafts":[{"id":"draft1","message":{"id":"draftmessage1"}}]}),
            ))
            .expect(1)
            .mount(&server)
            .await;
        Mock::given(method("GET")).and(path("/drafts/draft1"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({"id":"draft1","message":{"id":if changed {"remote-revision"} else {"draftmessage1"}}})))
            .expect(if changed {1} else {2}).mount(&server).await;
        Mock::given(method("DELETE"))
            .and(path("/drafts/draft1"))
            .respond_with(ResponseTemplate::new(204))
            .expect(if changed { 0 } else { 1 })
            .mount(&server)
            .await;
        let result = gmail(&server).discard_thread_drafts("thread1").await;
        if changed {
            assert!(matches!(result, Err(Error::DraftConflict)));
        } else {
            assert_eq!(result.unwrap(), 1);
        }
        assert!(
            server
                .received_requests()
                .await
                .unwrap()
                .iter()
                .all(|r| r.method != "DELETE" || r.url.path() == "/drafts/draft1")
        );
    }
}
#[test]
fn rich_replies_include_plain_and_html_alternatives_and_keep_quoted_history() {
    let mut compose = Compose::new("sender@example.invalid".into());
    compose.to = "recipient@example.invalid".into();
    compose.html = true;
    compose.text = crate::rich_draft::with_quote(
        r#"<div data-luma-editor="1">Hello <strong>world</strong></div>"#,
        "On yesterday: Original words",
    );
    compose.retained.push(RetainedAttachment {
        name: "image.png".into(),
        mime_type: "image/png".into(),
        data: vec![1, 2, 3].into(),
        ..Default::default()
    });
    let bytes = compose.mime().unwrap();
    let mail = mailparse::parse_mail(&bytes).unwrap();
    assert_eq!(mail.ctype.mimetype, "multipart/mixed");
    let alternatives = &mail.subparts[0];
    assert_eq!(alternatives.ctype.mimetype, "multipart/alternative");
    assert_eq!(alternatives.subparts[0].ctype.mimetype, "text/plain");
    assert_eq!(alternatives.subparts[1].ctype.mimetype, "text/html");
    let plain = alternatives.subparts[0].get_body().unwrap();
    assert!(plain.contains("Original words") && plain.contains("world"));
    let rich = crate::rich_draft::parse(&alternatives.subparts[1].get_body().unwrap()).unwrap();
    assert!(!rich.text.contains("Original words"));
    assert_eq!(rich.quote, "On yesterday: Original words");
    assert_eq!(mail.subparts[1].get_body_raw().unwrap(), [1, 2, 3]);
}
#[tokio::test]
async fn repeated_attachment_saves_update_one_draft_and_send_that_same_draft_once() {
    let server = MockServer::start().await;
    let revision = Arc::new(AtomicUsize::new(0));
    Mock::given(method("POST"))
        .and(path("/drafts"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(json!({"id":"one-draft","message":{"id":"m0"}})),
        )
        .expect(1)
        .mount(&server)
        .await;
    let current = revision.clone();
    Mock::given(method("GET")).and(path("/drafts/one-draft"))
        .respond_with(move |_: &wiremock::Request| ResponseTemplate::new(200).set_body_json(json!({"id":"one-draft","message":{"id":format!("m{}",current.load(Ordering::SeqCst))}})))
        .expect(2).mount(&server).await;
    let current = revision.clone();
    Mock::given(method("PUT"))
        .and(path("/drafts/one-draft"))
        .respond_with(move |_: &wiremock::Request| {
            let n = current.fetch_add(1, Ordering::SeqCst) + 1;
            ResponseTemplate::new(200)
                .set_body_json(json!({"id":"one-draft","message":{"id":format!("m{n}")}}))
        })
        .expect(2)
        .mount(&server)
        .await;
    Mock::given(method("POST"))
        .and(path("/drafts/send"))
        .and(body_json(json!({"id":"one-draft"})))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(json!({"id":"sent-once","labelIds":["SENT"]})),
        )
        .expect(1)
        .mount(&server)
        .await;
    let gmail = gmail(&server);
    let mut compose = Compose::new("sender@example.invalid".into());
    compose.to = "recipient@example.invalid".into();
    let mut draft = gmail
        .save_draft(None, &compose.mime().unwrap(), None)
        .await
        .unwrap();
    compose.retained.push(RetainedAttachment {
        name: "pasted.png".into(),
        mime_type: "image/png".into(),
        data: vec![1, 2].into(),
        ..Default::default()
    });
    for body in [
        "With pasted image",
        "With image and Drive link https://drive.google.com/file/d/test/view",
    ] {
        compose.text = body.into();
        draft = gmail
            .save_draft(Some(&draft), &compose.mime().unwrap(), None)
            .await
            .unwrap();
        assert_eq!(draft.id, "one-draft");
    }
    assert_eq!(gmail.send_draft(&draft).await.unwrap().id, "sent-once");
}
#[test]
fn upgrade_keeps_gmail_drafts_and_custom_polling_preferences() {
    let root = tempfile::tempdir().unwrap();
    let paths = Paths {
        config: root.path().join("config"),
        cache: root.path().join("cache"),
    };
    std::fs::create_dir_all(&paths.config).unwrap();
    let settings = paths.config.join("settings.json");
    std::fs::write(&settings, r#"{"poll_seconds":120,"local_drafts":true}"#).unwrap();
    let migrated = paths.load().unwrap();
    assert_eq!(migrated.poll_seconds, 60);
    assert!(
        !migrated.local_drafts,
        "an old preference cannot enable new local draft copies"
    );
    std::fs::write(&settings, r#"{"poll_seconds":300}"#).unwrap();
    assert_eq!(paths.load().unwrap().poll_seconds, 300);
    std::fs::write(&settings, r#"{"poll_seconds":120,"poll_version":1}"#).unwrap();
    assert_eq!(paths.load().unwrap().poll_seconds, 120);
}

#[derive(Default)]
struct MemoryVault(
    tokio::sync::Mutex<HashMap<String, Credentials>>,
    AtomicUsize,
);
#[async_trait]
impl Vault for MemoryVault {
    async fn load(&self, a: &str) -> Result<Credentials> {
        self.1.fetch_add(1, Ordering::SeqCst);
        self.0
            .lock()
            .await
            .get(a)
            .cloned()
            .ok_or(Error::Reauthenticate)
    }
    async fn store(&self, a: &str, c: &Credentials) -> Result<()> {
        self.0.lock().await.insert(a.into(), c.clone());
        Ok(())
    }
    async fn delete(&self, a: &str) -> Result<()> {
        self.0.lock().await.remove(a);
        Ok(())
    }
}
fn config() -> ClientConfig {
    ClientConfig {
        client_id: "desktop-test".into(),
        client_secret: "public-client-secret".into(),
    }
}
#[test]
fn google_registration_requires_valid_build_time_fields() {
    assert!(
        ClientConfig::from_registration("", "")
            .err()
            .unwrap()
            .to_string()
            .contains("maintainer")
    );
    let valid = "1234567890-example.apps.googleusercontent.com";
    for (id, secret) in [
        ("", "public"),
        (valid, ""),
        ("invalid", "public"),
        (".apps.googleusercontent.com", "public"),
        ("injected\n.apps.googleusercontent.com", "public"),
        (valid, "contains\nnewline"),
    ] {
        assert!(ClientConfig::from_registration(id, secret).is_err());
    }
    let client = ClientConfig::from_registration(&format!(" {valid}\n"), "public").unwrap();
    assert_eq!(client.client_id, valid);
    assert!(ClientConfig::from_registration(valid, &"x".repeat(257)).is_err());
}
pub(crate) fn gmail(server: &MockServer) -> Gmail {
    let http = Http::new().unwrap();
    let mut tokens = TokenManager::new(
        "account-a".into(),
        config(),
        http.clone(),
        Arc::new(MemoryVault::default()),
    );
    tokens.test_access("test-access");
    let mut client = Gmail::new("account-a".into(), http, Arc::new(tokens));
    client.test_base(server.uri());
    client
}

#[tokio::test]
async fn sender_filter_is_server_side_and_unblock_checks_ownership() {
    use crate::sender_rules::address;
    assert_eq!(
        address("Spam Sender <Junk@Example.com>").unwrap(),
        "junk@example.com"
    );
    for invalid in [
        "",
        "a@example.com,b@example.com",
        "bad\r\nBcc: other@example.com",
    ] {
        assert!(address(invalid).is_err());
    }
    let server = MockServer::start().await;
    let client = gmail(&server);
    let rule = json!({"id":"filter_1","criteria":{"from":"junk@example.com"},"action":{"addLabelIds":["TRASH"],"removeLabelIds":["INBOX"]}});
    Mock::given(method("POST")).and(path("/settings/filters")).and(header("authorization","Bearer test-access"))
        .and(body_json(json!({"criteria":{"from":"junk@example.com"},"action":{"addLabelIds":["TRASH"],"removeLabelIds":["INBOX"]}})))
        .respond_with(ResponseTemplate::new(200).set_body_json(&rule)).expect(1).mount(&server).await;
    let filter = client
        .create_sender_block("Junk <junk@example.com>")
        .await
        .unwrap();
    assert_eq!(filter.id, "filter_1");
    Mock::given(method("GET"))
        .and(path("/settings/filters/filter_1"))
        .respond_with(ResponseTemplate::new(200).set_body_json(&rule))
        .expect(1)
        .mount(&server)
        .await;
    Mock::given(method("DELETE"))
        .and(path("/settings/filters/filter_1"))
        .respond_with(ResponseTemplate::new(204))
        .expect(1)
        .mount(&server)
        .await;
    client
        .remove_sender_block(&filter.id, "junk@example.com")
        .await
        .unwrap();
    let normalized = json!({"id":"filter_normalized","criteria":{"from":"junk@example.com"},"action":{"addLabelIds":["TRASH"]}});
    Mock::given(method("GET"))
        .and(path("/settings/filters/filter_normalized"))
        .respond_with(ResponseTemplate::new(200).set_body_json(normalized))
        .mount(&server)
        .await;
    Mock::given(method("DELETE"))
        .and(path("/settings/filters/filter_normalized"))
        .respond_with(ResponseTemplate::new(204))
        .expect(1)
        .mount(&server)
        .await;
    client
        .remove_sender_block("filter_normalized", "junk@example.com")
        .await
        .unwrap();
    let mut changed = rule;
    changed["id"] = json!("filter_2");
    changed["action"]["forward"] = json!("archive@example.com");
    Mock::given(method("GET"))
        .and(path("/settings/filters/filter_2"))
        .respond_with(ResponseTemplate::new(200).set_body_json(changed))
        .expect(1)
        .mount(&server)
        .await;
    Mock::given(method("DELETE"))
        .and(path("/settings/filters/filter_2"))
        .respond_with(ResponseTemplate::new(204))
        .expect(0)
        .mount(&server)
        .await;
    assert!(
        client
            .remove_sender_block("filter_2", "junk@example.com")
            .await
            .unwrap_err()
            .to_string()
            .contains("changed in Gmail")
    );
    let mut spam = message("spam", 0);
    for removed in MailAction::Spam.labels().1 {
        spam.label_ids.retain(|id| id != &removed);
    }
    spam.label_ids.extend(MailAction::Spam.labels().0);
    let thread = Thread {
        id: "thread-a".into(),
        messages: vec![spam],
        ..Default::default()
    };
    assert!(Folder::Spam.matches(&thread));
    assert!(!Folder::Inbox.matches(&thread));
    assert!(!Folder::AllMail.matches(&thread));
}

#[tokio::test]
async fn gmail_filter_permission_is_requested_only_when_needed() {
    let normal = Authorization::begin(&config(), false).await.unwrap();
    let scope = normal
        .url
        .query_pairs()
        .find(|(key, _)| key == "scope")
        .unwrap()
        .1
        .into_owned();
    assert_eq!(scope, GMAIL_SCOPE);
    let filters = Authorization::begin_with_settings(&config(), true, true)
        .await
        .unwrap();
    let scope = filters
        .url
        .query_pairs()
        .find(|(key, _)| key == "scope")
        .unwrap()
        .1
        .into_owned();
    assert!(scope.split_whitespace().any(|s| s == GMAIL_SCOPE));
    assert!(scope.split_whitespace().any(|s| s == DRIVE_SCOPE));
    assert!(scope.split_whitespace().any(|s| s == SETTINGS_SCOPE));
    assert!(
        filters
            .url
            .query_pairs()
            .any(|(key, value)| key == "include_granted_scopes" && value == "true")
    );
}
#[tokio::test]
async fn upload_progress_matches_real_json_transport_and_is_scoped() {
    let server = MockServer::start().await;
    let body = json!({"raw":"x".repeat(100000)});
    let bytes = serde_json::to_vec(&body).unwrap().len() as u64;
    Mock::given(method("POST"))
        .and(path("/draft"))
        .and(body_json(&body))
        .and(header("content-type", "application/json"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({"ok":true})))
        .expect(2)
        .mount(&server)
        .await;
    let http = Http::new().unwrap();
    let progress = Arc::new(crate::http::UploadProgress::default());
    let scoped = http.with_upload_progress(progress.clone());
    scoped
        .request(
            reqwest::Method::POST,
            &format!("{}/draft", server.uri()),
            "test",
            &[],
            Some(&body),
        )
        .await
        .unwrap();
    assert_eq!(progress.snapshot(), (bytes, bytes));
    http.request(
        reqwest::Method::POST,
        &format!("{}/draft", server.uri()),
        "test",
        &[],
        Some(&body),
    )
    .await
    .unwrap();
    assert_eq!(progress.snapshot(), (bytes, bytes));
}
fn message(id: &str, date: i64) -> Message {
    serde_json::from_value(json!({"id":id,"threadId":"thread-a","internalDate":date.to_string(),"labelIds":["INBOX","UNREAD"],"payload":{"mimeType":"text/plain","headers":[{"name":"Subject","value":"Hello"},{"name":"From","value":"Ann <ann@example.com>"}],"body":{"data":URL_SAFE_NO_PAD.encode("Hello world")}}})).unwrap()
}

#[test]
fn gmail_response_and_thread_identity() {
    let m = message("m", 123);
    let t = Thread {
        id: "same".into(),
        messages: vec![m],
        ..Default::default()
    };
    let a = Conversation::from_thread("a", &t).unwrap();
    let b = Conversation::from_thread("b", &t).unwrap();
    assert_ne!(a.key, b.key);
    assert!(a.unread);
    assert_eq!(a.subject, "Hello");
    assert_eq!(a.date, 123);
}
#[test]
fn all_mail_excludes_spam_and_trash() {
    assert_eq!(Folder::AllMail.query(), "-in:spam -in:trash");
    assert!(Folder::Archive.query().contains("-in:inbox"));
}
#[test]
fn archive_only_removes_inbox() {
    assert_eq!(MailAction::Archive.labels(), (vec![], vec!["INBOX".into()]));
}
#[test]
fn undo_restores_mixed_read_states() {
    let mut read = message("read", 0);
    read.label_ids.retain(|l| l != "UNREAD");
    let unread = message("unread", 0);
    assert!(MailAction::Read.undo_for(&read).is_empty());
    assert_eq!(
        MailAction::Read.undo_for(&unread),
        vec![MailAction::AddLabel("UNREAD".into())]
    );
    assert_eq!(
        MailAction::Archive.undo_for(&read),
        vec![MailAction::AddLabel("INBOX".into())]
    );
}
#[test]
fn label_changes_are_scoped() {
    assert_eq!(MailAction::Star.labels().0, vec!["STARRED"]);
    assert_eq!(MailAction::Read.labels().1, vec!["UNREAD"]);
    assert_eq!(
        MailAction::AddLabel("Label_1".into()).labels().0,
        vec!["Label_1"]
    );
}
#[tokio::test]
async fn cross_account_labels_resolve_by_name_and_reject_missing_matches() {
    let server = MockServer::start().await;
    Mock::given(path("/labels"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({"labels":[
            {"id":"Label_1", "name":"Unrelated", "type":"user"},
            {"id":"Label_9", "name":"Projects", "type":"user"}
        ]})))
        .mount(&server)
        .await;
    let client = gmail(&server);
    assert_eq!(client.matching_label("Projects").await.unwrap(), "Label_9");
    assert!(client.matching_label("Missing").await.is_err());
}

#[test]
fn search_builder_validates_dates_and_preserves_field_boundaries() {
    let query = search::Search {
        from: "Ann Example".into(),
        subject: "quarterly report".into(),
        after: "2025-01-01".into(),
        before: "2026-01-01".into(),
        unread: true,
        attachments: true,
        ..Default::default()
    }
    .query()
    .unwrap();
    assert_eq!(
        query,
        "from:\"Ann Example\" subject:\"quarterly report\" after:2025/01/01 before:2026/01/01 is:unread has:attachment"
    );
    assert!(
        search::Search {
            after: "2026-02-30".into(),
            ..Default::default()
        }
        .query()
        .is_err()
    );
    assert!(
        search::Search {
            after: "2026-02-01".into(),
            before: "2025-01-01".into(),
            ..Default::default()
        }
        .query()
        .is_err()
    );
    assert!(
        search::Search {
            from: "Ann\" OR is:unread".into(),
            ..Default::default()
        }
        .query()
        .is_err()
    );
}
#[test]
fn parse_plain_mime() {
    assert_eq!(readable(&message("m", 0).payload).unwrap(), "Hello world");
}
#[test]
fn html_does_not_execute_or_load() {
    let p:Part=serde_json::from_value(json!({"mimeType":"text/html","body":{"data":URL_SAFE_NO_PAD.encode("<script>alert('x')</script><p>Hello</p><img src='https://tracker.invalid/x'>")}})).unwrap();
    let text = readable(&p).unwrap();
    assert!(text.contains("Hello"));
    assert!(!text.contains("alert("));
}
#[test]
fn charset_decoding() {
    let p = Part {
        mime_type: "text/plain".into(),
        headers: vec![Header {
            name: "Content-Type".into(),
            value: "text/plain; charset=iso-8859-1".into(),
        }],
        body: PartBody {
            data: Some(URL_SAFE_NO_PAD.encode([0x63, 0x61, 0x66, 0xe9]).into()),
            ..Default::default()
        },
        ..Default::default()
    };
    assert_eq!(readable(&p).unwrap(), "café");
}
#[test]
fn attachment_is_not_body() {
    let p = Part {
        mime_type: "text/plain".into(),
        filename: "secret.txt".into(),
        body: PartBody {
            data: Some(URL_SAFE_NO_PAD.encode("secret").into()),
            ..Default::default()
        },
        ..Default::default()
    };
    assert!(!readable(&p).unwrap().contains("secret"));
}
#[test]
fn malformed_base64_is_rejected() {
    let mut m = message("a", 0);
    m.payload.body.data = Some("%%%".into());
    assert!(readable(&m.payload).is_err());
}
#[test]
fn header_injection_is_rejected() {
    let mut c = Compose::new("a@example.com".into());
    c.to = "b@example.com".into();
    c.subject = "Hi\r\nBcc: victim@example.com".into();
    assert!(c.mime().is_err());
}
#[test]
fn recipients_are_required_to_send() {
    let c = Compose::new("a@example.com".into());
    assert!(c.validate_send().is_err());
    assert!(recipients("invalid").is_err());
}
#[test]
fn quoted_display_names_are_single_recipients() {
    let r = recipients("\"Smith, Ann\" <ann@example.com>, Bob <bob@example.com>").unwrap();
    assert_eq!(r.len(), 2);
    assert!(recipients("a@@example.com").is_err());
    assert!(recipients("\"unclosed <a@example.com>").is_err());
}
#[test]
fn reply_all_uses_exact_mailboxes() {
    let mut m = message("m", 0);
    m.payload.headers.push(Header {
        name: "To".into(),
        value: "me@example.com, otherme@example.com, \"Smith, Ann\" <ann@example.com>".into(),
    });
    let c = Compose::reply("me@example.com".into(), &m, true);
    assert_eq!(c.cc, "otherme@example.com");
}
#[test]
fn long_unicode_subjects_are_folded_safely() {
    let mut c = Compose::new("a@example.com".into());
    c.subject = "🌒".repeat(200);
    let raw = String::from_utf8(c.mime().unwrap()).unwrap();
    assert!(raw.lines().all(|line| line.len() < 998));
    assert!(raw.contains("\r\n =?UTF-8?B?"));
}
#[test]
fn reply_headers_and_account_are_preserved() {
    let mut m = message("m", 0);
    m.payload.headers.push(Header {
        name: "Message-ID".into(),
        value: "<old@example.com>".into(),
    });
    m.payload.headers.push(Header {
        name: "References".into(),
        value: "<first@example.com>".into(),
    });
    let c = Compose::reply("me@example.com".into(), &m, false);
    let mime = String::from_utf8(c.mime().unwrap()).unwrap();
    assert!(mime.contains("In-Reply-To: <old@example.com>"));
    assert!(mime.contains("References: <first@example.com> <old@example.com>"));
    assert_eq!(c.thread.as_deref(), Some("thread-a"));
}
#[test]
fn attachments_have_encoded_filenames() {
    let d = tempfile::tempdir().unwrap();
    let p = d.path().join("a plus+.txt");
    std::fs::write(&p, b"data").unwrap();
    let mut c = Compose::new("a@example.com".into());
    c.attachments.push(p);
    let raw = String::from_utf8(c.mime().unwrap()).unwrap();
    assert!(raw.contains("filename*=UTF-8''a%20plus%2B.txt"));
    assert!(raw.contains("ZGF0YQ=="));
}
#[test]
fn reopened_draft_preserves_inline_and_regular_parts() {
    let mut c = Compose::new("a@example.com".into());
    c.html = true;
    c.text = "<img src=\"cid:photo\">".into();
    c.retained.push(RetainedAttachment {
        name: "photo.png".into(),
        mime_type: "image/png".into(),
        data: vec![1, 2, 3].into(),
        content_id: Some("<photo>".into()),
    });
    let raw = String::from_utf8(c.mime().unwrap()).unwrap();
    assert!(raw.contains("Content-ID: <photo>"));
    assert!(raw.contains("Content-Disposition: inline"));
    assert!(raw.contains("AQID"));
}
#[tokio::test]
async fn draft_attachments_download_only_when_editing() {
    let server = MockServer::start().await;
    Mock::given(method("GET")).and(path("/drafts/d")).respond_with(ResponseTemplate::new(200).set_body_json(json!({"id":"d","message":{"id":"m","payload":{"mimeType":"multipart/mixed","parts":[{"mimeType":"text/plain","body":{"data":URL_SAFE_NO_PAD.encode("body")}},{"mimeType":"application/octet-stream","filename":"one.bin","body":{"size":3,"attachmentId":"att"}}]}}}))).mount(&server).await;
    Mock::given(method("GET"))
        .and(path("/messages/m/attachments/att"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(json!({"size":3,"data":URL_SAFE_NO_PAD.encode([1,2,3])})),
        )
        .expect(1)
        .mount(&server)
        .await;
    let (_, parts) = gmail(&server).edit_draft("d").await.unwrap();
    assert_eq!(parts.len(), 1);
    assert_eq!(parts[0].data.as_slice(), [1, 2, 3]);
}
#[test]
fn oversize_attachment_rejected_before_read() {
    let d = tempfile::tempdir().unwrap();
    let p = d.path().join("big");
    std::fs::File::create(&p)
        .unwrap()
        .set_len(MAX_ATTACHMENT_BYTES + 1)
        .unwrap();
    let mut c = Compose::new("a@example.com".into());
    c.attachments.push(p);
    assert!(c.mime().is_err());
}
#[test]
fn cache_zero_does_not_store() {
    let d = tempfile::tempdir().unwrap();
    let mut c = Cache::new(d.path().join("bodies"), 0, Duration::from_secs(3600)).unwrap();
    assert!(!c.put("a", "m", b"private").unwrap());
    assert_eq!(c.usage().unwrap(), 0);
    assert!(!c.root().exists());
}
#[test]
fn cache_budget_enforced_under_many_messages() {
    let d = tempfile::tempdir().unwrap();
    let mut c = Cache::new(
        d.path().join("bodies"),
        64 * 1024,
        Duration::from_secs(3600),
    )
    .unwrap();
    for i in 0..1000 {
        c.put("a", &i.to_string(), &[1; 1000]).unwrap();
        assert!(c.usage().unwrap() <= 64 * 1024);
    }
    assert!(c.get("a", "0").unwrap().is_none());
    assert!(c.get("a", "999").unwrap().is_some());
}
#[test]
fn cache_lru_touches_reads() {
    let d = tempfile::tempdir().unwrap();
    let mut c = Cache::new(d.path().join("bodies"), 32768, Duration::from_secs(3600)).unwrap();
    c.put("a", "1", &[1; 1024]).unwrap();
    c.put("a", "2", &[2; 1024]).unwrap();
    let budget = c.usage().unwrap();
    c.configure(budget, Duration::from_secs(3600)).unwrap();
    for entry in std::fs::read_dir(c.root()).unwrap() {
        let path = entry.unwrap().path();
        std::fs::OpenOptions::new()
            .write(true)
            .open(path)
            .unwrap()
            .set_modified(std::time::UNIX_EPOCH + Duration::from_secs(10))
            .unwrap();
    }
    c.get("a", "1").unwrap();
    c.put("a", "3", &[3; 1024]).unwrap();
    assert!(c.get("a", "1").unwrap().is_some());
    assert!(c.get("a", "2").unwrap().is_none());
    assert!(c.get("a", "3").unwrap().is_some());
    assert!(c.usage().unwrap() <= budget);
}
#[test]
fn cache_clear_and_account_isolation() {
    let d = tempfile::tempdir().unwrap();
    let mut c = Cache::new(d.path().join("bodies"), 100000, Duration::from_secs(3600)).unwrap();
    c.put("a", "same", b"one").unwrap();
    c.put("b", "same", b"two").unwrap();
    assert_eq!(c.get("a", "same").unwrap().unwrap(), b"one");
    assert!(c.account_usage("a").unwrap() > 0);
    c.clear_account("a").unwrap();
    assert!(c.get("a", "same").unwrap().is_none());
    assert_eq!(c.get("b", "same").unwrap().unwrap(), b"two");
    c.clear().unwrap();
    assert_eq!(c.usage().unwrap(), 0);
}
#[test]
fn cache_shrink_to_zero() {
    let d = tempfile::tempdir().unwrap();
    let mut c = Cache::new(d.path().join("bodies"), 100000, Duration::from_secs(3600)).unwrap();
    c.put("a", "m", b"body").unwrap();
    c.configure(0, Duration::from_secs(3600)).unwrap();
    assert_eq!(c.usage().unwrap(), 0);
}
#[test]
fn cache_expiry_is_enforced() {
    let d = tempfile::tempdir().unwrap();
    let mut c = Cache::new(d.path().join("bodies"), 100000, Duration::from_secs(3600)).unwrap();
    c.put("a", "m", b"body").unwrap();
    c.configure(100000, Duration::ZERO).unwrap();
    assert!(c.get("a", "m").unwrap().is_none());
}
#[test]
fn settings_roundtrip_contains_no_tokens() {
    let d = tempfile::tempdir().unwrap();
    let p = Paths {
        config: d.path().join("config"),
        cache: d.path().join("cache"),
    };
    p.save(&Settings::default()).unwrap();
    assert_eq!(p.load().unwrap().cache_mib, 100);
    let data = std::fs::read_to_string(p.config.join("settings.json")).unwrap();
    assert!(!data.contains("token"));
}
#[test]
fn closing_hides_only_with_a_live_tray() {
    let mut s = Settings::default();
    assert!(s.hides_on_close(true, false));
    assert!(!s.hides_on_close(false, false));
    assert!(!s.hides_on_close(true, true));
    s.close_to_tray = false;
    assert!(!s.hides_on_close(true, false));
}
#[test]
fn csrf_comparison() {
    assert!(valid_state("secret", "secret"));
    assert!(!valid_state("secret", "other"));
    assert!(!valid_state("secret", ""));
}
#[tokio::test]
async fn pkce_loopback_flow_is_real() {
    let a = Authorization::begin(&config(), false).await.unwrap();
    let p: HashMap<_, _> = a.url.query_pairs().into_owned().collect();
    assert_eq!(p["code_challenge_method"], "S256");
    assert_eq!(p["code_challenge"].len(), 43);
    assert!(p["redirect_uri"].starts_with("http://127.0.0.1:"));
    assert_eq!(p["scope"], GMAIL_SCOPE);
    assert_eq!(p["access_type"], "offline");
}
#[tokio::test]
async fn oauth_refresh_is_serialized_and_not_plaintext() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/token"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(json!({"access_token":"fresh","expires_in":3600})),
        )
        .expect(1)
        .mount(&server)
        .await;
    let vault = Arc::new(MemoryVault::default());
    vault
        .store(
            "a",
            &Credentials {
                refresh_token: "refresh".into(),
                client_id: config().client_id,
            },
        )
        .await
        .unwrap();
    let mut t = TokenManager::new("a".into(), config(), Http::new().unwrap(), vault.clone());
    t.test_endpoint(format!("{}/token", server.uri()));
    let t = Arc::new(t);
    let results = futures_util::future::join_all((0..8).map(|_| t.access_token())).await;
    assert!(results.into_iter().all(|r| r.unwrap() == "fresh"));
    assert_eq!(vault.1.load(Ordering::SeqCst), 1);
}
#[tokio::test]
async fn changing_google_client_requires_consent_before_any_refresh_request() {
    let server = MockServer::start().await;
    let vault = Arc::new(MemoryVault::default());
    let legacy: Credentials = serde_json::from_value(json!({"refresh_token":"legacy"})).unwrap();
    for credential in [
        legacy,
        Credentials {
            refresh_token: "different-client".into(),
            client_id: "another-client".into(),
        },
    ] {
        vault.store("a", &credential).await.unwrap();
        let mut tokens =
            TokenManager::new("a".into(), config(), Http::new().unwrap(), vault.clone());
        tokens.test_endpoint(format!("{}/token", server.uri()));
        assert!(matches!(
            tokens.access_token().await,
            Err(Error::Reauthenticate)
        ));
    }
    assert!(server.received_requests().await.unwrap().is_empty());
}
#[tokio::test]
async fn gmail_archive_hits_correct_remote_resource() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/threads/thread-a/modify"))
        .and(header("authorization", "Bearer test-access"))
        .and(body_json(
            json!({"addLabelIds":[],"removeLabelIds":["INBOX"]}),
        ))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({})))
        .expect(1)
        .mount(&server)
        .await;
    gmail(&server)
        .act("thread-a", false, &MailAction::Archive)
        .await
        .unwrap();
}
#[tokio::test]
async fn gmail_trash_and_restore_use_endpoints() {
    let server = MockServer::start().await;
    for action in ["trash", "untrash"] {
        Mock::given(method("POST"))
            .and(path(format!("/messages/m/{action}")))
            .and(header("Content-Length", "0"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({})))
            .expect(1)
            .mount(&server)
            .await;
    }
    let g = gmail(&server);
    g.act("m", true, &MailAction::Trash).await.unwrap();
    g.act("m", true, &MailAction::Restore).await.unwrap();
    assert!(
        server
            .received_requests()
            .await
            .unwrap()
            .iter()
            .all(|r| r.body.is_empty()),
        "Trash/untrash endpoints require an empty request body"
    );
}
#[tokio::test]
async fn remote_search_is_paged_and_does_not_fetch_bodies() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/threads"))
        .and(query_param("maxResults", "25"))
        .and(query_param("pageToken", "next"))
        .and(query_param("q", "-in:spam -in:trash older:2020/01/01"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(json!({"threads":[{"id":"t"}],"nextPageToken":"later"})),
        )
        .expect(1)
        .mount(&server)
        .await;
    let mut m = message("m", 50);
    m.payload.headers.push(Header {
        name: "Message-ID".into(),
        value: "<search@example.com>".into(),
    });
    Mock::given(method("GET"))
        .and(path("/threads/t"))
        .and(query_param("format", "metadata"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({"id":"t","messages":[m]})))
        .expect(1)
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/threads"))
        .and(query_param(
            "q",
            "has:attachment {rfc822msgid:\"search@example.com\"}",
        ))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({"threads":[]})))
        .expect(1)
        .mount(&server)
        .await;
    let page = gmail(&server)
        .list(&Folder::AllMail, "older:2020/01/01", Some("next"))
        .await
        .unwrap();
    assert_eq!(page.items.len(), 1);
    assert_eq!(page.next.as_deref(), Some("later"));
    let requests = server.received_requests().await.unwrap();
    let get = requests
        .iter()
        .find(|r| r.url.path() == "/threads/t")
        .unwrap();
    let fields = get
        .url
        .query_pairs()
        .find(|(k, _)| k == "fields")
        .unwrap()
        .1;
    assert!(!fields.contains("data"));
}
#[tokio::test]
async fn expired_credentials_surface_reconnect() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .respond_with(ResponseTemplate::new(401))
        .mount(&server)
        .await;
    assert!(matches!(
        gmail(&server).thread("a").await,
        Err(Error::Reauthenticate)
    ));
}
#[tokio::test]
async fn mutating_requests_are_never_replayed() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .respond_with(ResponseTemplate::new(503))
        .expect(1)
        .mount(&server)
        .await;
    assert!(
        gmail(&server)
            .act("a", false, &MailAction::Trash)
            .await
            .is_err()
    );
}
#[tokio::test]
async fn rate_limited_reads_retry_boundedly() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .respond_with(ResponseTemplate::new(429).insert_header("Retry-After", "0"))
        .expect(4)
        .mount(&server)
        .await;
    assert!(gmail(&server).thread("a").await.is_err());
}
#[tokio::test]
async fn google_403_quota_uses_bounded_read_retries() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .respond_with(
            ResponseTemplate::new(403)
                .insert_header("Retry-After", "0")
                .set_body_json(json!({"error":{"errors":[{"reason":"userRateLimitExceeded"}]}})),
        )
        .expect(4)
        .mount(&server)
        .await;
    assert!(gmail(&server).thread("a").await.is_err());
}

#[tokio::test]
async fn forbidden_errors_explain_permissions_without_exposing_response_data() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .respond_with(ResponseTemplate::new(403).set_body_json(json!({"error":{
            "message":"PRIVATE-RESPONSE-CONTENT", "errors":[{"reason":"insufficientPermissions"}]
        }})))
        .expect(1)
        .mount(&server)
        .await;
    let error = gmail(&server).thread("a").await.unwrap_err().to_string();
    assert!(error.contains("permission missing"));
    assert!(!error.contains("PRIVATE-RESPONSE-CONTENT"));
}

#[tokio::test]
async fn visible_list_uses_metadata_and_exact_bounded_attachment_query() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/threads"))
        .and(query_param("q", "in:inbox "))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({"threads":[{"id":"a"}]})))
        .expect(1)
        .mount(&server)
        .await;
    Mock::given(method("GET")).and(path("/threads/a"))
        .and(query_param("format", "metadata"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({"id":"a","messages":[{
            "id":"m","threadId":"a","internalDate":"1","snippet":"preview", "payload":{"headers":[
                {"name":"Message-ID","value":"<one@example.com>"},
                {"name":"Subject","value":"visible subject"}
            ]}
        }]})))
        .expect(1).mount(&server).await;
    Mock::given(method("GET"))
        .and(path("/threads"))
        .and(query_param(
            "q",
            "has:attachment {rfc822msgid:\"one@example.com\"}",
        ))
        .and(query_param("maxResults", "100"))
        .and(query_param("includeSpamTrash", "true"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({"threads":[{"id":"a"}]})))
        .expect(1)
        .mount(&server)
        .await;
    let page = gmail(&server).list(&Folder::Inbox, "", None).await.unwrap();
    assert_eq!(page.items.len(), 1);
    assert_eq!(page.items[0].subject, "visible subject");
    assert_eq!(page.items[0].snippet, "preview");
    assert!(page.items[0].attachments);
    let requests = server.received_requests().await.unwrap();
    assert!(!requests.iter().any(|r| {
        r.url
            .query_pairs()
            .any(|(k, v)| k == "format" && v == "full")
    }));
    assert_eq!(requests.len(), 3);
}

#[tokio::test]
async fn invalid_rfc_id_uses_projected_metadata_without_query_injection() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/threads"))
        .and(query_param("q", "in:inbox "))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({"threads":[{"id":"a"}]})))
        .expect(1)
        .mount(&server)
        .await;
    Mock::given(method("GET")).and(path("/threads/a"))
        .and(query_param("format", "metadata"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({"id":"a","messages":[{
            "id":"m", "payload":{"headers":[{"name":"Message-ID","value":"bad\"} OR in:anywhere"}]}
        }]})))
        .expect(1).mount(&server).await;
    Mock::given(method("GET")).and(path("/threads/a"))
        .and(query_param("format", "full"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({"id":"a","messages":[{
            "id":"m", "payload":{"parts":[{"filename":"one.txt", "body":{"size":12,"attachmentId":"att"}}]}
        }]})))
        .expect(1).mount(&server).await;
    let page = gmail(&server).list(&Folder::Inbox, "", None).await.unwrap();
    assert!(page.items[0].attachments);
    let requests = server.received_requests().await.unwrap();
    assert_eq!(requests.len(), 3);
    assert!(
        requests
            .iter()
            .filter_map(|r| r.url.query_pairs().find(|(k, _)| k == "fields"))
            .all(|(_, v)| !v.contains("body(data"))
    );
}

#[tokio::test]
async fn empty_projected_attachment_search_keeps_visible_mail() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/threads"))
        .and(query_param("q", "in:inbox "))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({"threads":[{"id":"a"}]})))
        .expect(1)
        .mount(&server)
        .await;
    Mock::given(method("GET")).and(path("/threads/a")).and(query_param("format","metadata"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({"id":"a","messages":[{"id":"m","threadId":"a","payload":{"headers":[{"name":"Message-ID","value":"<one@example.com>"}]}}]})))
        .expect(1).mount(&server).await;
    Mock::given(method("GET"))
        .and(path("/threads"))
        .and(query_param(
            "q",
            "has:attachment {rfc822msgid:\"one@example.com\"}",
        ))
        .respond_with(ResponseTemplate::new(204))
        .expect(1)
        .mount(&server)
        .await;
    let page = gmail(&server).list(&Folder::Inbox, "", None).await.unwrap();
    assert_eq!(page.items.len(), 1);
    assert!(!page.items[0].attachments);
}
#[test]
fn only_uncertain_writes_require_reconciliation() {
    assert!(!Error::Invalid("Invalid recipient".into()).write_may_have_succeeded());
    assert!(
        !Error::Api {
            status: 403,
            reason: "permission".into()
        }
        .write_may_have_succeeded()
    );
    assert!(
        Error::Api {
            status: 503,
            reason: "unavailable".into()
        }
        .write_may_have_succeeded()
    );
    assert!(Error::ResponseLimit.write_may_have_succeeded());
}
#[tokio::test]
async fn draft_conflict_prevents_overwrite() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/drafts/d"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(json!({"id":"d","message":{"id":"newer"}})),
        )
        .expect(1)
        .mount(&server)
        .await;
    let d = gmail::Draft {
        id: "d".into(),
        message: message("old", 0),
    };
    assert!(matches!(
        gmail(&server).save_draft(Some(&d), b"mail", None).await,
        Err(Error::DraftConflict)
    ));
    Mock::given(method("POST"))
        .and(path("/drafts"))
        .and(body_json(
            json!({"message":{"raw":URL_SAFE_NO_PAD.encode(b"my unsaved version")}}),
        ))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(json!({"id":"separate","message":{"id":"copy"}})),
        )
        .expect(1)
        .mount(&server)
        .await;
    let separate = gmail(&server)
        .save_draft(None, b"my unsaved version", None)
        .await
        .unwrap();
    assert_eq!(separate.id, "separate");
    assert!(
        server
            .received_requests()
            .await
            .unwrap()
            .iter()
            .all(|r| r.method != "PUT" && r.method != "DELETE")
    );
}
#[tokio::test]
async fn draft_create_contains_thread_and_encoded_mime() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/drafts"))
        .and(body_json(
            json!({"message":{"raw":URL_SAFE_NO_PAD.encode(b"MIME"),"threadId":"t"}}),
        ))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(json!({"id":"d","message":{"id":"m"}})),
        )
        .expect(1)
        .mount(&server)
        .await;
    assert_eq!(
        gmail(&server)
            .save_draft(None, b"MIME", Some("t"))
            .await
            .unwrap()
            .id,
        "d"
    );
}
#[tokio::test]
async fn notification_baseline_never_alerts_old_mail() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/profile"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(json!({"emailAddress":"a@example.com","historyId":"100"})),
        )
        .expect(1)
        .mount(&server)
        .await;
    let (items, changed) = notifications::Monitor::default()
        .poll(&gmail(&server))
        .await
        .unwrap();
    assert!(items.is_empty());
    assert!(!changed);
}
#[tokio::test]
async fn notification_burst_resumes_inside_history_records_without_losing_mail() {
    let server = MockServer::start().await;
    Mock::given(path("/profile"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(json!({"emailAddress":"a@example.com","historyId":"100"})),
        )
        .mount(&server)
        .await;
    let added: Vec<_> = (0..30)
        .map(|i| json!({"message":{"id":format!("m{i}")}}))
        .collect();
    Mock::given(path("/history"))
        .and(query_param("startHistoryId", "100"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "history":[{"messagesAdded":added}], "historyId":"200"
        })))
        .expect(2)
        .mount(&server)
        .await;
    for i in 0..30 {
        Mock::given(path(format!("/messages/m{i}")))
            .respond_with(ResponseTemplate::new(200).set_body_json(message(&format!("m{i}"), i)))
            .expect(1)
            .mount(&server)
            .await;
    }
    let client = gmail(&server);
    let mut monitor = notifications::Monitor::default();
    monitor.poll(&client).await.unwrap();
    let first = monitor.poll(&client).await.unwrap().0;
    let second = monitor.poll(&client).await.unwrap().0;
    assert_eq!(first.len(), 25);
    assert_eq!(second.len(), 5);
    assert_eq!(second[0].id, "m25");
    assert_eq!(second[4].id, "m29");
}

#[tokio::test]
async fn network_limit_covers_response_bodies_and_releases_on_drop() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .respond_with(ResponseTemplate::new(200).set_body_string("body"))
        .mount(&server)
        .await;
    let http = Http::new().unwrap();
    let mut responses = Vec::new();
    for _ in 0..4 {
        responses.push(
            http.request(reqwest::Method::GET, &server.uri(), "t", &[], None)
                .await
                .unwrap(),
        );
    }
    let task = {
        let http = http.clone();
        let url = server.uri();
        tokio::spawn(async move {
            http.request(reqwest::Method::GET, &url, "t", &[], None)
                .await
        })
    };
    tokio::time::sleep(Duration::from_millis(30)).await;
    assert!(
        !task.is_finished(),
        "a fifth body cannot bypass the transfer limit"
    );
    drop(responses.pop());
    let response = tokio::time::timeout(Duration::from_secs(2), task)
        .await
        .unwrap()
        .unwrap()
        .unwrap();
    assert_eq!(http::bounded_bytes(response, 16).await.unwrap(), b"body");
}

#[tokio::test]
async fn rate_limit_backoff_does_not_starve_other_requests() {
    let server = MockServer::start().await;
    Mock::given(path("/limited"))
        .respond_with(ResponseTemplate::new(429).insert_header("Retry-After", "2"))
        .mount(&server)
        .await;
    Mock::given(path("/ready"))
        .respond_with(ResponseTemplate::new(200))
        .mount(&server)
        .await;
    let http = Http::new().unwrap();
    let mut tasks = Vec::new();
    for _ in 0..4 {
        let http = http.clone();
        let url = format!("{}/limited", server.uri());
        tasks.push(tokio::spawn(async move {
            http.request(reqwest::Method::GET, &url, "t", &[], None)
                .await
        }));
    }
    tokio::time::timeout(Duration::from_secs(2), async {
        while server.received_requests().await.unwrap().len() < 4 {
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap();
    tokio::time::timeout(
        Duration::from_millis(500),
        http.request(
            reqwest::Method::GET,
            &format!("{}/ready", server.uri()),
            "t",
            &[],
            None,
        ),
    )
    .await
    .expect("sleeping retries must release their slots")
    .unwrap();
    for task in tasks {
        task.abort();
    }
}
#[test]
fn notifications_are_deduplicated() {
    let mut d = notifications::Dedup::default();
    assert!(d.fresh("a"));
    assert!(!d.fresh("a"));
    for i in 0..1000 {
        d.fresh(&i.to_string());
    }
    assert!(d.fresh("a"));
}
fn palette() -> String {
    "mode='dark'\nbackground='#101010'\nsurface='#202020'\ntext='#ffffff'\nmuted='#bbbbbb'\naccent='#aabbff'\non_accent='#000000'\noutline='#555555'\nhover='#333333'".into()
}
#[test]
fn theme_parses_and_reloads_new_values() {
    let a = theme::Palette::parse(&palette()).unwrap();
    let b = theme::Palette::parse(&palette().replace("#aabbff", "#bbffaa")).unwrap();
    assert_ne!(a.css(), b.css());
    assert_eq!(a.mode, "dark");
}
#[test]
fn theme_rejects_css_injection_and_bad_contrast() {
    assert!(
        theme::Palette::parse(&palette().replace("#101010", "url(file:///etc/passwd)")).is_err()
    );
    assert!(theme::Palette::parse(&palette().replace("#ffffff", "#111111")).is_err());
}
#[test]
fn invalid_resource_id_cannot_change_path() {
    assert!(gmail::check_id("../threads").is_err());
    assert!(gmail::check_id("a?x=1").is_err());
}

#[derive(Default)]
struct FakeProvider {
    account: String,
    pages: Vec<Vec<i64>>,
    calls: AtomicUsize,
    behavior: AtomicUsize,
}
#[async_trait]
impl MailProvider for FakeProvider {
    fn account_id(&self) -> &str {
        &self.account
    }
    async fn list(&self, _: &Folder, _: &str, page: Option<&str>) -> Result<Page<Conversation>> {
        if page.is_some() {
            match self.behavior.swap(0, Ordering::SeqCst) {
                1 => return Err(Error::Timeout),
                2 => tokio::time::sleep(Duration::from_secs(1)).await,
                _ => (),
            }
        }
        self.calls.fetch_add(1, Ordering::SeqCst);
        let n = page.unwrap_or("0").parse::<usize>().unwrap();
        Ok(Page {
            items: self.pages[n]
                .iter()
                .map(|date| Conversation {
                    key: ThreadKey {
                        account: self.account.clone(),
                        id: date.to_string(),
                    },
                    subject: String::new(),
                    sender: String::new(),
                    snippet: String::new(),
                    date: *date,
                    unread: false,
                    starred: false,
                    important: false,
                    attachments: false,
                    count: 1,
                })
                .collect(),
            next: if n + 1 < self.pages.len() {
                Some((n + 1).to_string())
            } else {
                None
            },
        })
    }
    async fn thread(&self, _: &str) -> Result<Thread> {
        unreachable!()
    }
    async fn message(&self, _: &str) -> Result<Message> {
        unreachable!()
    }
    async fn act(&self, _: &str, _: bool, _: &MailAction) -> Result<()> {
        unreachable!()
    }
    async fn labels(&self) -> Result<Vec<Label>> {
        unreachable!()
    }
}
#[tokio::test]
async fn unified_merge_refills_frontiers_and_preserves_accounts() {
    let a = Arc::new(FakeProvider {
        account: "a".into(),
        pages: vec![vec![100, 90], vec![80, 70]],
        calls: AtomicUsize::new(0),
        behavior: AtomicUsize::new(0),
    });
    let b = Arc::new(FakeProvider {
        account: "b".into(),
        pages: vec![vec![95, 85], vec![80, 60]],
        calls: AtomicUsize::new(0),
        behavior: AtomicUsize::new(0),
    });
    let mut p = UnifiedPager::new(vec![a, b], Folder::Inbox, String::new());
    let rows = p.next_page().await.unwrap();
    assert_eq!(
        rows.iter().map(|r| r.date).collect::<Vec<_>>(),
        vec![100, 95, 90, 85, 80, 80, 70, 60]
    );
    assert_ne!(rows[4].key, rows[5].key);
    assert!(!p.has_more());
}
#[tokio::test]
async fn large_mailbox_fetches_only_required_pages() {
    let a = Arc::new(FakeProvider {
        account: "a".into(),
        pages: (0..4000)
            .map(|p| (0..25).map(|i| 100000 - p * 25 - i).collect())
            .collect(),
        calls: AtomicUsize::new(0),
        behavior: AtomicUsize::new(0),
    });
    let mut p = UnifiedPager::new(vec![a.clone()], Folder::Inbox, String::new());
    assert_eq!(p.next_page().await.unwrap().len(), 25);
    assert_eq!(a.calls.load(Ordering::SeqCst), 1);
    assert!(p.has_more());
}
#[tokio::test]
async fn pagination_failure_does_not_consume_unshown_rows() {
    let provider = Arc::new(FakeProvider {
        account: "a".into(),
        pages: vec![vec![5, 4, 3], vec![2, 1]],
        behavior: AtomicUsize::new(1),
        ..Default::default()
    });
    let mut p = UnifiedPager::new(vec![provider], Folder::Inbox, String::new());
    assert!(p.next_page().await.is_err());
    let rows = p.next_page().await.unwrap();
    assert_eq!(
        rows.iter().map(|r| r.date).collect::<Vec<_>>(),
        vec![5, 4, 3, 2, 1]
    );
}
#[tokio::test]
async fn pagination_cancellation_does_not_consume_unshown_rows() {
    let provider = Arc::new(FakeProvider {
        account: "a".into(),
        pages: vec![vec![5, 4, 3], vec![2, 1]],
        behavior: AtomicUsize::new(2),
        ..Default::default()
    });
    let mut p = UnifiedPager::new(vec![provider], Folder::Inbox, String::new());
    assert!(
        tokio::time::timeout(Duration::from_millis(10), p.next_page())
            .await
            .is_err()
    );
    assert_eq!(p.next_page().await.unwrap().len(), 5);
}

#[test]
fn appearance_preferences_upgrade_without_new_retention_or_unsafe_scaling() {
    let dir = tempfile::tempdir().unwrap();
    let paths = crate::config::Paths {
        config: dir.path().join("config"),
        cache: dir.path().join("cache"),
    };
    std::fs::create_dir_all(&paths.config).unwrap();
    std::fs::write(
        paths.config.join("settings.json"),
        r#"{"cache_mib":0,"ui_scale_percent":65535}"#,
    )
    .unwrap();
    let settings = paths.load().unwrap();
    assert_eq!(settings.ui_scale_percent, 200);
    assert_eq!(settings.cache_mib, 0);
    assert!(!settings.local_drafts);
    assert!(!settings.window_chrome);
    paths.save(&settings).unwrap();
    assert!(!paths.cache.exists());
}

#[tokio::test]
async fn contact_photo_scope_is_opt_in_and_preserves_existing_permissions() {
    let basic = Authorization::begin(&config(), false).await.unwrap();
    assert!(
        !basic
            .url
            .query_pairs()
            .any(|(k, v)| k == "scope" && v.contains(auth::CONTACTS_SCOPE))
    );
    let photos = Authorization::begin_with_photos(&config(), true, true, true)
        .await
        .unwrap();
    let scope = photos
        .url
        .query_pairs()
        .find(|(k, _)| k == "scope")
        .unwrap()
        .1
        .into_owned();
    for required in [
        GMAIL_SCOPE,
        DRIVE_SCOPE,
        SETTINGS_SCOPE,
        auth::CONTACTS_SCOPE,
    ] {
        assert!(scope.split_whitespace().any(|s| s == required));
    }
}
#[tokio::test]
async fn contact_photo_search_is_on_demand_projected_and_warms_only_once() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/people:searchContacts"))
        .and(query_param("query", ""))
        .and(query_param("readMask", "emailAddresses,photos"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({"results":[]})))
        .expect(1)
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/people:searchContacts"))
        .and(query_param("query", "sender@example.invalid"))
        .and(query_param("pageSize", "30"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({"results":[]})))
        .expect(2)
        .mount(&server)
        .await;
    let mut photos = crate::contact_photos::Photos::new(Arc::new(gmail(&server)));
    photos.test_endpoint(format!("{}/people:searchContacts", server.uri()));
    assert!(
        photos
            .get("\"Sender, Example\" <SENDER@example.invalid>")
            .await
            .unwrap()
            .is_none()
    );
    assert!(
        photos
            .get("sender@example.invalid")
            .await
            .unwrap()
            .is_none()
    );
    assert_eq!(server.received_requests().await.unwrap().len(), 3);
    for request in server.received_requests().await.unwrap() {
        let sources: Vec<_> = request
            .url
            .query_pairs()
            .filter(|(key, _)| key == "sources")
            .map(|(_, value)| value.into_owned())
            .collect();
        assert_eq!(
            sources,
            ["READ_SOURCE_TYPE_CONTACT", "READ_SOURCE_TYPE_PROFILE"]
        );
    }
}
#[tokio::test]
async fn public_contact_photo_transport_does_not_forward_authorization_or_cookies() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/photo"))
        .respond_with(ResponseTemplate::new(200).set_body_bytes(b"fixture"))
        .expect(1)
        .mount(&server)
        .await;
    let response = Http::new()
        .unwrap()
        .public_get(&format!("{}/photo", server.uri()))
        .await
        .unwrap();
    assert_eq!(
        crate::http::bounded_bytes(response, 32).await.unwrap(),
        b"fixture"
    );
    let requests = server.received_requests().await.unwrap();
    assert!(!requests[0].headers.contains_key("authorization"));
    assert!(!requests[0].headers.contains_key("cookie"));
    assert!(!requests[0].headers.contains_key("referer"));
}

#[test]
fn send_as_reply_address_is_separate_from_thread_headers_and_account_metadata() {
    use mailparse::MailHeaderMap;
    let compose = Compose {
        account: Some("private-account-key".into()),
        from: "Alias <alias@example.invalid>".into(),
        to: "self@example.invalid".into(),
        sender_reply_to: "reply@example.invalid".into(),
        reply_to: "<original@example.invalid>".into(),
        references: "<original@example.invalid>".into(),
        text: "Test".into(),
        ..Compose::new("alias@example.invalid".into())
    };
    let raw = compose.mime().unwrap();
    let mail = mailparse::parse_mail(&raw).unwrap();
    assert_eq!(
        mail.headers.get_first_value("Reply-To").as_deref(),
        Some("reply@example.invalid")
    );
    assert_eq!(
        mail.headers.get_first_value("In-Reply-To").as_deref(),
        Some("<original@example.invalid>")
    );
    assert!(!String::from_utf8_lossy(&raw).contains("private-account-key"));
}

#[test]
fn message_actions_and_composer_snapshots_share_large_payloads_in_ram() {
    let body: Arc<str> = "x".repeat(2 * 1024 * 1024).into();
    let message = Message {
        payload: Part {
            body: PartBody {
                data: Some(body.clone()),
                ..Default::default()
            },
            ..Default::default()
        },
        ..Default::default()
    };
    let actions: Vec<_> = (0..32).map(|_| message.clone()).collect();
    assert!(
        actions
            .iter()
            .all(|m| Arc::ptr_eq(m.payload.body.data.as_ref().unwrap(), &body))
    );
    let wire = serde_json::to_vec(&message).unwrap();
    let restored: Message = serde_json::from_slice(&wire).unwrap();
    assert_eq!(restored.payload.body.data.as_deref(), Some(body.as_ref()));
    let bytes = Arc::new(vec![0u8; 8 * 1024 * 1024]);
    let mut compose = Compose::new("sender@example.invalid".into());
    compose.retained.push(RetainedAttachment {
        data: bytes.clone(),
        ..Default::default()
    });
    let snapshots: Vec<_> = (0..32).map(|_| compose.clone()).collect();
    assert!(
        snapshots
            .iter()
            .all(|c| Arc::ptr_eq(&c.retained[0].data, &bytes))
    );
}
