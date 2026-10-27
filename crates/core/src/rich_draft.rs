//! Bounded, inert re-editing of Luma's own generated HTML. Unknown markup stays
//! in source mode rather than silently losing formatting or inline attachments.
use html5ever::tokenizer::{
    BufferQueue, CharacterTokens, EndTag, StartTag, TagToken, Token, TokenSink, TokenSinkResult,
    Tokenizer,
};
use std::cell::RefCell;

#[derive(Default)]
pub struct RichDraft {
    pub text: String,
    pub spans: Vec<Span>,
    pub quote: String,
    pub signature: String,
}
pub struct Span {
    pub start: i32,
    pub end: i32,
    pub tag: String,
}
struct Frame {
    name: String,
    tags: Vec<String>,
    start: i32,
}
struct Parser {
    draft: RichDraft,
    stack: Vec<Frame>,
    depth: usize,
    numbers: Vec<Option<u32>>,
    chars: i32,
    valid: bool,
}
impl Parser {
    fn push(&mut self, text: &str) {
        if self.draft.text.len() + self.draft.quote.len() + text.len() > 512 * 1024 {
            self.valid = false;
            return;
        }
        if self.stack.iter().any(|f| f.name == "blockquote") {
            self.draft.quote.push_str(text);
            return;
        }
        self.draft.text.push_str(text);
        self.chars += text.chars().count() as i32;
    }
    fn newline(&mut self) {
        if !self.draft.text.is_empty() && !self.draft.text.ends_with('\n') {
            self.push("\n");
        }
    }
}
struct Sink(RefCell<Parser>);
impl TokenSink for Sink {
    type Handle = ();
    fn process_token(&self, token: Token, _: u64) -> TokenSinkResult<()> {
        let mut p = self.0.borrow_mut();
        if !p.valid {
            return TokenSinkResult::Continue;
        }
        match token {
            CharacterTokens(text) => p.push(&text),
            TagToken(tag) if tag.kind == StartTag => {
                let name = tag.name.to_string();
                let attr = |name: &str| {
                    tag.attrs
                        .iter()
                        .find(|a| &*a.name.local == name)
                        .map(|a| a.value.to_string())
                };
                let tags = match name.as_str() {
                    "br" => {
                        p.push("\n");
                        return TokenSinkResult::Continue;
                    }
                    "div" => vec![],
                    "blockquote" if attr("data-luma-quote").as_deref() == Some("1") => vec![],
                    "p" => {
                        p.newline();
                        vec![]
                    }
                    "ul" | "ol" => {
                        p.depth += 1;
                        p.numbers.push(if name == "ol" {
                            Some(
                                attr("start")
                                    .and_then(|v| v.parse::<u32>().ok())
                                    .filter(|n| (1..=999999).contains(n))
                                    .unwrap_or(1),
                            )
                        } else {
                            None
                        });
                        vec![]
                    }
                    "li" => {
                        p.newline();
                        let indent = "  ".repeat(p.depth.saturating_sub(1).min(8));
                        let number = if let Some(Some(next)) = p.numbers.last_mut() {
                            let number = attr("value")
                                .and_then(|v| v.parse::<u32>().ok())
                                .filter(|n| (1..=999999).contains(n))
                                .unwrap_or(*next);
                            *next = number.saturating_add(1).min(999999);
                            Some(number)
                        } else {
                            None
                        };
                        p.push(&format!(
                            "{indent}{}",
                            number.map_or_else(|| "• ".into(), |n| format!("{n}. "))
                        ));
                        vec![]
                    }
                    "strong" | "em" | "u" | "s" => vec![name.clone()],
                    "a" => match attr("href").and_then(|s| url::Url::parse(&s).ok()) {
                        Some(url)
                            if matches!(url.scheme(), "http" | "https" | "mailto")
                                && url.as_str().len() <= 8192 =>
                        {
                            vec![format!("link:{url}")]
                        }
                        _ => {
                            p.valid = false;
                            vec![]
                        }
                    },
                    "span" => {
                        let mut tags = Vec::new();
                        if let Some(family) = attr("data-luma-family").filter(|v| {
                            !v.is_empty() && v.len() <= 128 && !v.chars().any(char::is_control)
                        }) {
                            tags.push(format!("font:family:{family}"));
                        }
                        if let Some(size) = attr("data-luma-size")
                            .and_then(|v| v.parse::<u16>().ok())
                            .filter(|v| (8..=72).contains(v))
                        {
                            tags.push(format!("font:size:{size}"));
                        }
                        for attribute in &tag.attrs {
                            if let Some(style) = crate::rich_style::from_attribute(
                                &attribute.name.local,
                                &attribute.value,
                            ) {
                                tags.push(style);
                            }
                        }
                        if tags.is_empty() {
                            p.valid = false;
                        }
                        tags
                    }
                    _ => {
                        p.valid = false;
                        vec![]
                    }
                };
                if p.stack.len() >= 32 {
                    p.valid = false;
                }
                let start = p.chars;
                p.stack.push(Frame { name, tags, start });
            }
            TagToken(tag) if tag.kind == EndTag => {
                if let Some(frame) = p.stack.pop().filter(|f| f.name == *tag.name) {
                    let end = p.chars;
                    for tag in frame.tags {
                        if end > frame.start {
                            p.draft.spans.push(Span {
                                start: frame.start,
                                end,
                                tag,
                            });
                        }
                    }
                    if matches!(frame.name.as_str(), "li" | "p") {
                        p.newline();
                    }
                    if matches!(frame.name.as_str(), "ul" | "ol") {
                        p.depth = p.depth.saturating_sub(1);
                        p.numbers.pop();
                    }
                    if p.draft.spans.len() > 4096 {
                        p.valid = false;
                    }
                } else {
                    p.valid = false;
                }
            }
            _ => (),
        }
        TokenSinkResult::Continue
    }
}
pub fn parse(html: &str) -> Option<RichDraft> {
    if html.len() > 2 * 1024 * 1024 || !html.starts_with("<div data-luma-editor=\"1\"") {
        return None;
    }
    let (html, signature) = crate::signatures::split(html)?;
    let sink = RefCell::new(Parser {
        draft: RichDraft::default(),
        stack: vec![],
        depth: 0,
        numbers: vec![],
        chars: 0,
        valid: true,
    });
    let input = BufferQueue::default();
    input.push_back(html.into());
    let tokenizer = Tokenizer::new(Sink(sink), Default::default());
    let _ = tokenizer.feed(&input);
    tokenizer.end();
    let mut p = tokenizer.sink.0.into_inner();
    p.draft.signature = signature;
    (p.valid && p.stack.is_empty()).then_some(p.draft)
}
/// Keep quoted history outside the editable reply, without a local sidecar.
pub fn with_quote(body: &str, quote: &str) -> String {
    if quote.is_empty() {
        return body.into();
    }
    let escaped = quote
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;");
    let quoted = format!(
        "<blockquote data-luma-quote=\"1\" style=\"border-left:2px solid #888;padding-left:12px;white-space:pre-wrap\">{escaped}</blockquote>"
    );
    if body.starts_with("<div data-luma-editor=\"1\"")
        && let Some(prefix) = body.strip_suffix("</div>")
    {
        format!("{prefix}{quoted}</div>")
    } else {
        format!("{body}{quoted}")
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn numbered_lists_keep_start_values_and_nested_bullets() {
        let html = r#"<div data-luma-editor="1"><ol><li value="4">Four<ul><li>Child</li></ul></li><li value="5">Five</li></ol></div>"#;
        assert_eq!(parse(html).unwrap().text, "4. Four\n  • Child\n5. Five\n");
    }
    #[test]
    fn quoted_history_round_trips_outside_editable_text_with_safe_markup() {
        let body = r#"<div data-luma-editor="1"><strong>My reply</strong></div>"#;
        let history = "On yesterday, Person wrote:\n<script>not executable</script> & original";
        let html = with_quote(body, history);
        assert!(!html.contains("<script>"));
        let parsed = parse(&html).unwrap();
        assert_eq!(parsed.text, "My reply");
        assert_eq!(parsed.quote, history);
        assert_eq!(parsed.spans[0].tag, "strong");
        let reopened = with_quote(body, &parsed.quote);
        assert_eq!(reopened.matches("data-luma-quote").count(), 1);
    }
    #[test]
    fn fonts_links_unicode_and_lists_restore_without_a_browser_or_disk() {
        let html = r#"<div data-luma-editor="1"><strong>Hi &amp; 你好</strong><br><span style="font-family:Serif" data-luma-family="Serif"><span data-luma-size="16">Next</span></span><ul><li>One</li><li>Two</li></ul><a href="https://example.com">Notes</a></div>"#;
        let d = parse(html).unwrap();
        assert!(d.text.starts_with("Hi & 你好\nNext\n• One\n• Two\nNotes"));
        assert!(d.spans.iter().any(|s| s.tag == "font:family:Serif"));
        let s = d.spans.iter().find(|s| s.tag == "font:size:16").unwrap();
        assert_eq!(
            d.text
                .chars()
                .skip(s.start as usize)
                .take((s.end - s.start) as usize)
                .collect::<String>(),
            "Next"
        );
        assert!(parse("<div data-luma-editor=\"1\"><img src='cid:original'></div>").is_none());
        assert!(parse("<div data-luma-editor=\"1\"><script>alert(1)</script></div>").is_none());
        assert!(
            parse("<div data-luma-editor=\"1\"><a href='file:///private'>bad</a></div>").is_none()
        );
        assert!(parse("<p>Third-party draft stays in source mode</p>").is_none());
    }
}
