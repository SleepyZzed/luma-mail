//! Provider-neutral online signatures. No signature or image file is cached.
use crate::{Error, Result, gmail::Gmail, http::bounded_bytes, sender_rules};
use async_trait::async_trait;
use serde::Deserialize;
use std::{borrow::Cow, collections::HashSet};

pub const MAX_HTML: usize = 64 * 1024;
const START: &str = "<!--luma-signature-start-->";
const END: &str = "<!--luma-signature-end-->";
const WRAP: &str = "<div data-luma-signature=\"1\"><br>";
pub fn split(html: &str) -> Option<(String, String)> {
    let Some(start) = html.find(START) else {
        return Some((html.into(), String::new()));
    };
    let end = html[start + START.len()..].find(END)? + start + START.len();
    let signature = html[start + START.len()..end]
        .strip_prefix(WRAP)?
        .strip_suffix("</div>")?;
    if signature.len() > MAX_HTML
        || html.matches(START).count() != 1
        || html.matches(END).count() != 1
    {
        return None;
    }
    Some((
        format!("{}{}", &html[..start], &html[end + END.len()..]),
        signature.into(),
    ))
}
pub fn append(body: &str, html: &str) -> Result<String> {
    let safe = clean(html)?;
    if safe.trim().is_empty() {
        return Ok(body.into());
    }
    let suffix = "</div>";
    let block = format!("{START}{WRAP}{safe}</div>{END}");
    if body.starts_with("<div data-luma-editor=\"1\"")
        && let Some(prefix) = body.strip_suffix(suffix)
    {
        Ok(format!("{prefix}{block}{suffix}"))
    } else if let Some(at) = body.to_ascii_lowercase().rfind("</body>") {
        Ok(format!("{}{block}{}", &body[..at], &body[at..]))
    } else {
        Ok(format!("{body}{block}"))
    }
}
#[derive(Clone, Debug)]
pub struct Signature {
    pub identity: String,
    pub html: String,
    pub can_update: bool,
}
#[async_trait]
pub trait SignatureStore: Send + Sync {
    async fn signature(&self, identity: &str) -> Result<Signature>;
    async fn save_signature(&self, expected: &Signature, html: &str) -> Result<Signature>;
}
pub fn image_url(value: &str) -> Result<String> {
    let url = url::Url::parse(value.trim())
        .map_err(|_| Error::Invalid("Enter an HTTPS image URL".into()))?;
    if url.scheme() != "https"
        || !url.username().is_empty()
        || url.password().is_some()
        || url.host_str().is_none()
        || url.as_str().len() > 8192
    {
        return Err(Error::Invalid(
            "Signature pictures require a public HTTPS image URL without credentials".into(),
        ));
    }
    Ok(url.into())
}
pub fn clean(html: &str) -> Result<String> {
    if html.len() > MAX_HTML {
        return Err(Error::ResponseLimit);
    }
    {
        use html5ever::tokenizer::{
            BufferQueue, StartTag, Token, TokenSink, TokenSinkResult, Tokenizer,
        };
        struct Images(std::cell::Cell<usize>, std::cell::Cell<bool>);
        impl TokenSink for Images {
            type Handle = ();
            fn process_token(&self, token: Token, _: u64) -> TokenSinkResult<()> {
                if let Token::TagToken(tag) = token
                    && tag.kind == StartTag
                    && &*tag.name == "img"
                {
                    self.0.set(self.0.get() + 1);
                    if !tag
                        .attrs
                        .iter()
                        .any(|a| &*a.name.local == "src" && image_url(&a.value).is_ok())
                    {
                        self.1.set(true);
                    }
                }
                TokenSinkResult::Continue
            }
        }
        let input = BufferQueue::default();
        input.push_back(html.into());
        let parser = Tokenizer::new(
            Images(std::cell::Cell::new(0), std::cell::Cell::new(false)),
            Default::default(),
        );
        let _ = parser.feed(&input);
        parser.end();
        if parser.sink.0.get() > 8 || parser.sink.1.get() {
            return Err(Error::Invalid("Use at most eight hosted HTTPS pictures. Embedded/pasted image data must be uploaded with Insert picture before saving.".into()));
        }
    }
    let mut builder = ammonia::Builder::default();
    builder
        .add_tags(&["font", "center"])
        .add_generic_attributes(&["style", "align", "width", "height", "valign"])
        .add_tag_attributes("font", &["color", "face", "size"])
        .add_tag_attributes("table", &["cellpadding", "cellspacing", "border"])
        .url_relative(ammonia::UrlRelative::Deny)
        .url_schemes(HashSet::from(["https", "http", "mailto"]))
        .filter_style_properties(HashSet::from([
            "color",
            "background-color",
            "font-family",
            "font-size",
            "font-weight",
            "font-style",
            "line-height",
            "text-align",
            "text-decoration",
            "margin",
            "margin-left",
            "padding",
            "padding-left",
            "border",
            "border-collapse",
            "vertical-align",
            "width",
            "height",
            "max-width",
            "white-space",
        ]))
        .attribute_filter(|tag, name, value| {
            if tag == "img" && name == "src" {
                image_url(value).ok().map(Cow::Owned)
            } else {
                Some(Cow::Borrowed(value))
            }
        });
    let safe = builder.clean(html).to_string();
    if safe.len() > MAX_HTML {
        return Err(Error::ResponseLimit);
    }
    Ok(safe)
}
fn route(identity: &str) -> Result<String> {
    let email = sender_rules::address(identity)?;
    if !email.eq_ignore_ascii_case(identity) {
        return Err(Error::Invalid("Choose an exact sending address".into()));
    }
    Ok(format!(
        "/settings/sendAs/{}",
        url::form_urlencoded::byte_serialize(email.as_bytes()).collect::<String>()
    ))
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Remote {
    send_as_email: String,
    #[serde(default)]
    signature: String,
    #[serde(default)]
    is_primary: bool,
}
fn remote(bytes: &[u8], identity: &str) -> Result<Signature> {
    let value: Remote = serde_json::from_slice(bytes)?;
    if !value.send_as_email.eq_ignore_ascii_case(identity) || value.signature.len() > MAX_HTML {
        return Err(Error::Invalid(
            "Gmail returned a signature for a different sender or an oversized signature".into(),
        ));
    }
    Ok(Signature {
        identity: value.send_as_email,
        html: value.signature,
        can_update: value.is_primary,
    })
}
#[async_trait]
impl SignatureStore for Gmail {
    async fn signature(&self, identity: &str) -> Result<Signature> {
        let response = self
            .call(
                reqwest::Method::GET,
                &route(identity)?,
                &[("fields", "sendAsEmail,signature,isPrimary".into())],
                None,
            )
            .await?;
        remote(
            &bounded_bytes(response, MAX_HTML + 16 * 1024).await?,
            identity,
        )
    }
    async fn save_signature(&self, expected: &Signature, html: &str) -> Result<Signature> {
        if !expected.can_update {
            return Err(Error::Invalid("This Gmail integration edits the primary address's signature. Alias signatures remain readable.".into()));
        }
        let safe = clean(html)?;
        let current = self.signature(&expected.identity).await?;
        if current.html != expected.html {
            return Err(Error::DraftConflict);
        }
        let response = self
            .call(
                reqwest::Method::PATCH,
                &route(&expected.identity)?,
                &[("fields", "sendAsEmail,signature,isPrimary".into())],
                Some(&serde_json::json!({"signature":safe})),
            )
            .await?;
        remote(
            &bounded_bytes(response, MAX_HTML + 16 * 1024).await?,
            &expected.identity,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn signature_roundtrips_separately_from_reply_body_without_duplicate_wrappers() {
        let body = "<div data-luma-editor=\"1\"><strong>My reply</strong></div>";
        let signature = "<b>Name</b><img src=\"https://example.com/logo.png\">";
        let first = append(body, signature).unwrap();
        let mut current = first.clone();
        for _ in 0..12 {
            let rich = crate::rich_draft::parse(&current).unwrap();
            assert_eq!(rich.text, "My reply");
            assert!(rich.signature.contains("logo.png"));
            let (body, signature) = split(&current).unwrap();
            current = append(&body, &signature).unwrap();
            assert_eq!(current, first);
        }
    }
    #[test]
    fn signature_and_quote_survive_outgoing_mime_without_entering_editable_text() {
        let mut c = crate::composer::Compose::new("me@example.invalid".into());
        c.to = "you@example.invalid".into();
        c.html = true;
        let signature = "<table><tr><td><strong>Alex</strong></td><td><img src=\"https://example.com/logo.png\"></td></tr></table>";
        c.text = crate::rich_draft::with_quote(
            &append("<div data-luma-editor=\"1\">My answer</div>", signature).unwrap(),
            "Previous conversation",
        );
        let bytes = c.mime().unwrap();
        let mime = mailparse::parse_mail(&bytes).unwrap();
        let plain = mime.subparts[0].get_body().unwrap();
        let html = mime.subparts[1].get_body().unwrap();
        assert!(
            plain.contains("Alex")
                && plain.contains("My answer")
                && plain.contains("Previous conversation")
        );
        let rich = crate::rich_draft::parse(&html).unwrap();
        assert_eq!(rich.text, "My answer");
        assert_eq!(rich.quote, "Previous conversation");
        assert!(rich.signature.contains("<table>") && rich.signature.contains("logo.png"));
        assert_eq!(html.matches(START).count(), 1);
        assert!(clean("<img src=\"data:image/png;base64,abcdef\">").is_err());
        assert!(clean(&"<img src=\"https://example.com/a.png\">".repeat(9)).is_err());
    }
    #[tokio::test]
    async fn only_signature_is_patched_and_a_changed_server_copy_is_preserved() {
        use wiremock::{
            Mock, MockServer, ResponseTemplate,
            matchers::{body_json, method},
        };
        let server = MockServer::start().await;
        let client = crate::tests::gmail(&server);
        let previous = Signature {
            identity: "me@example.invalid".into(),
            html: "Old".into(),
            can_update: true,
        };
        Mock::given(method("GET")).respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({"sendAsEmail":"me@example.invalid","signature":"Old","isPrimary":true}))).expect(1).mount(&server).await;
        Mock::given(method("PATCH")).and(body_json(serde_json::json!({"signature":"<strong>New</strong>"})))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({"sendAsEmail":"me@example.invalid","signature":"<strong>New</strong>","isPrimary":true}))).expect(1).mount(&server).await;
        let saved = client
            .save_signature(&previous, "<strong>New</strong>")
            .await
            .unwrap();
        assert_eq!(saved.html, "<strong>New</strong>");
        let requests = server.received_requests().await.unwrap();
        assert!(requests.iter().all(|r| {
            r.url
                .query_pairs()
                .any(|(k, v)| k == "fields" && v == "sendAsEmail,signature,isPrimary")
        }));
        server.reset().await;
        Mock::given(method("GET")).respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({"sendAsEmail":"me@example.invalid","signature":"Changed elsewhere","isPrimary":true}))).expect(1).mount(&server).await;
        assert!(matches!(
            client.save_signature(&previous, "Mine").await,
            Err(Error::DraftConflict)
        ));
        assert_eq!(server.received_requests().await.unwrap().len(), 1);
    }
    #[test]
    fn keeps_rich_text_and_public_images_but_removes_active_content() {
        let html = clean(r#"<div style="text-align:center;color:#aabbcc"><strong>A</strong><em>B</em><u>C</u><ol><li>One</li></ol><a href="mailto:a@example.invalid">Mail</a><img src="https://example.com/logo.png" onerror="bad()"><script>bad()</script><iframe src="file:///private"></iframe></div>"#).unwrap();
        for part in [
            "text-align:center",
            "<strong>",
            "<em>",
            "<u>",
            "<ol>",
            "https://example.com/logo.png",
            "mailto:",
        ] {
            assert!(html.contains(part), "{part}");
        }
        for part in ["onerror", "<script", "<iframe", "file://"] {
            assert!(!html.contains(part));
        }
        assert!(image_url("data:image/png;base64,abc").is_err());
        assert!(image_url("https://user:secret@example.com/a").is_err());
        assert!(clean(&"x".repeat(MAX_HTML + 1)).is_err());
    }
}
