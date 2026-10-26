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
    chars: i32,
    valid: bool,
}
impl Parser {
    fn push(&mut self, text: &str) {
        if self.draft.text.len() + text.len() > 512 * 1024 {
            self.valid = false;
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
                    "p" => {
                        p.newline();
                        vec![]
                    }
                    "ul" => {
                        p.depth += 1;
                        vec![]
                    }
                    "li" => {
                        p.newline();
                        let indent = "  ".repeat(p.depth.saturating_sub(1).min(8));
                        p.push(&format!("{indent}• "));
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
                    if frame.name == "ul" {
                        p.depth = p.depth.saturating_sub(1);
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
    let sink = RefCell::new(Parser {
        draft: RichDraft::default(),
        stack: vec![],
        depth: 0,
        chars: 0,
        valid: true,
    });
    let input = BufferQueue::default();
    input.push_back(html.into());
    let tokenizer = Tokenizer::new(Sink(sink), Default::default());
    let _ = tokenizer.feed(&input);
    tokenizer.end();
    let p = tokenizer.sink.0.into_inner();
    (p.valid && p.stack.is_empty()).then_some(p.draft)
}
#[cfg(test)]
mod tests {
    use super::*;
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
