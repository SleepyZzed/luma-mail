//! Portable composer envelope. Authored text, online signature and read-only
//! quoted history round-trip in the Gmail draft itself, never a local sidecar.
use crate::{Error, Result};
use html5ever::tokenizer::{BufferQueue, Token, TokenSink, TokenSinkResult, Tokenizer};
use std::{cell::RefCell, collections::HashSet};

pub const MAX_BODY: usize = 1024 * 1024;
pub const MAX_QUOTE: usize = 128 * 1024;
const ROOT: &str = "<div data-luma-editor=\"2\">";
const START: &str = "<!--luma-authored-start-->";
const END: &str = "<!--luma-authored-end-->";

pub struct Sections {
    pub body: String,
    pub quote: String,
    pub signature: String,
}
pub fn escape(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}
pub fn inline_sources(html: &str) -> HashSet<String> {
    struct Images(RefCell<HashSet<String>>);
    impl TokenSink for Images {
        type Handle = ();
        fn process_token(&self, token: Token, _: u64) -> TokenSinkResult<()> {
            if let Token::TagToken(tag) = token
                && &*tag.name == "img"
            {
                for attribute in tag.attrs {
                    if &*attribute.name.local == "src"
                        && attribute.value.starts_with("cid:luma-inline-")
                    {
                        let mut images = self.0.borrow_mut();
                        if images.len() < 64 {
                            images.insert(attribute.value[4..].to_string());
                        }
                    }
                }
            }
            TokenSinkResult::Continue
        }
    }
    let sink = Images(RefCell::new(HashSet::new()));
    let queue = BufferQueue::default();
    queue.push_back(html.into());
    let tokenizer = Tokenizer::new(sink, Default::default());
    let _ = tokenizer.feed(&queue);
    tokenizer.end();
    tokenizer.sink.0.into_inner()
}
pub fn clean(html: &str) -> Result<String> {
    if html.len() > MAX_BODY {
        return Err(Error::ResponseLimit);
    }
    let mut builder = ammonia::Builder::default();
    builder
        .add_tags(&["font", "center"])
        .add_generic_attributes(&["style", "align", "width", "height", "valign"])
        .add_tag_attributes("font", &["color", "face", "size"])
        .add_tag_attributes("table", &["cellpadding", "cellspacing", "border"])
        .set_tag_attribute_value("img", "style", "max-width:100%;height:auto;")
        .url_relative(ammonia::UrlRelative::Deny)
        .url_schemes(HashSet::from(["https", "http", "mailto", "cid"]))
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
            "margin-right",
            "margin-top",
            "margin-bottom",
            "padding",
            "padding-left",
            "border",
            "border-collapse",
            "vertical-align",
            "width",
            "height",
            "max-width",
            "white-space",
            "overflow-wrap",
            "text-indent",
            "list-style-type",
        ]));
    let safe = builder.clean(html).to_string();
    if safe.len() > MAX_BODY {
        return Err(Error::ResponseLimit);
    }
    Ok(safe)
}
pub fn envelope(text: &str, html: bool, quote: &str, signature: &str) -> Result<String> {
    if text.len() > MAX_BODY || quote.len() > MAX_QUOTE {
        return Err(Error::ResponseLimit);
    }
    let body = if html {
        clean(text)?
    } else {
        format!("<div style=\"white-space:pre-wrap\">{}</div>", escape(text))
    };
    // Use the existing signature/quote emitters; only the authored section is
    // new. Their version-1 container support is retained for old clients.
    let root = format!("<div data-luma-editor=\"1\">{START}{body}{END}</div>");
    let signed = crate::signatures::append(&root, signature)?;
    Ok(crate::rich_draft::with_quote(&signed, quote).replacen(
        "<div data-luma-editor=\"1\">",
        ROOT,
        1,
    ))
}
pub fn sections(html: &str) -> Option<Sections> {
    if html.len() > MAX_BODY + 256 * 1024
        || !html.starts_with(ROOT)
        || html.matches(START).count() != 1
        || html.matches(END).count() != 1
    {
        return None;
    }
    let (without_signature, signature) = crate::signatures::split(html)?;
    let start = without_signature.find(START)? + START.len();
    let end = without_signature[start..].find(END)? + start;
    let body = clean(&without_signature[start..end]).ok()?;
    Some(Sections {
        body,
        quote: quote(&without_signature),
        signature,
    })
}
fn quote(html: &str) -> String {
    struct State {
        depth: usize,
        capture: usize,
        text: String,
    }
    struct Sink(RefCell<State>);
    impl TokenSink for Sink {
        type Handle = ();
        fn process_token(&self, token: Token, _: u64) -> TokenSinkResult<()> {
            use html5ever::tokenizer::{EndTag, StartTag};
            let mut s = self.0.borrow_mut();
            match token {
                Token::TagToken(t) if t.kind == StartTag => {
                    let void = matches!(
                        t.name.as_ref(),
                        "br" | "img" | "hr" | "meta" | "link" | "input"
                    );
                    if !void {
                        s.depth += 1;
                    }
                    if &*t.name == "blockquote"
                        && t.attrs
                            .iter()
                            .any(|a| &*a.name.local == "data-luma-quote" && a.value.as_ref() == "1")
                    {
                        s.capture = s.depth;
                    }
                    if s.capture > 0 && &*t.name == "br" && s.text.len() < MAX_QUOTE {
                        s.text.push('\n');
                    }
                }
                Token::TagToken(t) if t.kind == EndTag => {
                    if s.capture == s.depth {
                        s.capture = 0;
                    }
                    s.depth = s.depth.saturating_sub(1);
                }
                Token::CharacterTokens(t) if s.capture > 0 => {
                    for c in t.chars() {
                        if s.text.len() + c.len_utf8() > MAX_QUOTE {
                            break;
                        }
                        s.text.push(c);
                    }
                }
                _ => {}
            }
            TokenSinkResult::Continue
        }
    }
    let input = BufferQueue::default();
    input.push_back(html.into());
    let tokenizer = Tokenizer::new(
        Sink(RefCell::new(State {
            depth: 0,
            capture: 0,
            text: String::new(),
        })),
        Default::default(),
    );
    let _ = tokenizer.feed(&input);
    tokenizer.end();
    tokenizer.sink.0.into_inner().text
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn quoted_history_and_signature_round_trip_outside_editable_body() {
        let html = envelope(
            "<p><b>My reply</b> <a href=\"https://example.com\">Link</a></p>",
            true,
            "On Tuesday, someone wrote:\n<original> & text",
            "<p>My signature</p>",
        )
        .unwrap();
        let parts = sections(&html).unwrap();
        assert!(parts.body.contains("<b>My reply</b>"));
        assert!(!parts.body.contains("original"));
        assert_eq!(parts.quote, "On Tuesday, someone wrote:\n<original> & text");
        assert_eq!(parts.signature, "<p>My signature</p>");
        let again = envelope(&parts.body, true, &parts.quote, &parts.signature).unwrap();
        assert_eq!(again.matches("luma-signature-start").count(), 1);
        assert_eq!(again.matches("data-luma-quote").count(), 1);
    }
    #[test]
    fn native_formatting_survives_but_active_or_local_content_does_not() {
        let safe=clean("<p style=\"color:#ff0000;font-size:14pt;text-align:center\"><font face=\"Arial\">Hello</font></p><ul><li>One</li></ul><script>bad()</script><img src=\"file:///private\"><a href=\"javascript:bad()\">bad</a><!--luma-authored-end-->").unwrap();
        assert!(safe.contains("color:#ff0000"));
        assert!(safe.contains("<li>One</li>"));
        for bad in ["bad()", "file:", "javascript:", END] {
            assert!(!safe.contains(bad), "{safe}");
        }
    }
    #[test]
    fn marker_injection_and_oversize_bodies_cannot_spoof_authored_content() {
        assert!(sections("<div data-luma-editor=\"2\">unmarked</div>").is_none());
        assert!(envelope(&"x".repeat(MAX_BODY + 1), false, "", "").is_err());
        let html = envelope("<!--luma-authored-end--><p>Safe</p>", true, "", "").unwrap();
        assert_eq!(html.matches(END).count(), 1);
        assert_eq!(sections(&html).unwrap().body, "<p>Safe</p>");
    }
    #[test]
    fn forwarded_history_round_trips_up_to_the_existing_quote_budget() {
        let history = "Forwarded message\n".repeat(5000);
        let html = envelope("New introduction", false, &history, "").unwrap();
        assert_eq!(sections(&html).unwrap().quote, history);
        assert!(envelope("", false, &"x".repeat(MAX_QUOTE + 1), "").is_err());
    }
}
