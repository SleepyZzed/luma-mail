//! HTML mail is display-only. No remote resources reach the renderer: approved
//! images are decoded, resized and re-encoded as inert PNG data URLs first.
use crate::{Error, Result, composer::decode_part, model::Part, preview};
use base64::{Engine, engine::general_purpose::STANDARD};
use std::{
    borrow::Cow,
    collections::{HashMap, HashSet},
    io::Cursor,
};
#[path = "html_appearance.rs"]
mod appearance;
pub use appearance::Appearance;

pub fn body(payload: &Part) -> Result<Option<String>> {
    let mut html = None;
    payload.walk(&mut |part| {
        if html.is_none() && part.mime_type == "text/html" && part.filename.is_empty() {
            html = Some(part);
        }
    });
    let Some(part) = html else { return Ok(None) };
    if part.body.size > 2 * 1024 * 1024
        || part
            .body
            .data
            .as_ref()
            .is_some_and(|s| s.len() > 3 * 1024 * 1024)
    {
        return Err(Error::ResponseLimit);
    }
    let bytes = decode_part(part)?;
    if bytes.len() > 2 * 1024 * 1024 {
        return Err(Error::ResponseLimit);
    }
    let content_type = part.header("Content-Type").to_ascii_lowercase();
    let charset = content_type
        .split("charset=")
        .nth(1)
        .and_then(|s| s.split(';').next())
        .unwrap_or("utf-8")
        .trim_matches(['\'', '"', ' ']);
    let encoding =
        encoding_rs::Encoding::for_label(charset.as_bytes()).unwrap_or(encoding_rs::UTF_8);
    Ok(Some(encoding.decode(&bytes).0.into_owned()))
}
pub fn image_data(bytes: &[u8]) -> Result<String> {
    let thumb = preview::thumbnail(bytes, 1280, 960)?;
    let image = image::RgbaImage::from_raw(thumb.width, thumb.height, thumb.rgba)
        .ok_or(Error::ResponseLimit)?;
    let mut png = Cursor::new(Vec::new());
    image
        .write_to(&mut png, image::ImageFormat::Png)
        .map_err(|_| Error::Invalid("Image conversion failed".into()))?;
    Ok(format!(
        "data:image/png;base64,{}",
        STANDARD.encode(png.into_inner())
    ))
}
/// Ordinary phone mail often contains only div/p/br wrappers. Avoid creating a
/// browser surface for it. Keep layout, images, links and rich text in HTML.
pub fn needs_layout(html: &str) -> bool {
    use html5ever::tokenizer::{
        BufferQueue, StartTag, Token, TokenSink, TokenSinkResult, Tokenizer,
    };
    struct Layout(std::cell::Cell<bool>);
    impl TokenSink for Layout {
        type Handle = ();
        fn process_token(&self, token: Token, _: u64) -> TokenSinkResult<()> {
            if let Token::TagToken(tag) = token
                && tag.kind == StartTag
                && (!matches!(
                    tag.name.as_ref(),
                    "html"
                        | "head"
                        | "meta"
                        | "title"
                        | "body"
                        | "div"
                        | "span"
                        | "p"
                        | "br"
                        | "wbr"
                        | "blockquote"
                ) || tag.attrs.iter().any(|a| {
                    a.name.local == html5ever::local_name!("bgcolor")
                        || (a.name.local == html5ever::local_name!("style")
                            && (a.value.to_ascii_lowercase().contains("background")
                                || (tag.name == html5ever::local_name!("span")
                                    && (a.value.contains("font-family")
                                        || a.value.contains("font-size")))))
                }))
            {
                self.0.set(true);
            }
            TokenSinkResult::Continue
        }
    }
    let input = BufferQueue::default();
    input.push_back(html.into());
    let tokenizer = Tokenizer::new(Layout(std::cell::Cell::new(false)), Default::default());
    let _ = tokenizer.feed(&input);
    tokenizer.end();
    tokenizer.sink.0.get()
}
pub fn document(html: &str, images: HashMap<String, String>) -> String {
    styled_document(html, &images, None)
}
pub fn styled_document(
    html: &str,
    images: &HashMap<String, String>,
    appearance: Option<Appearance>,
) -> String {
    let images = images.clone();
    let mut cleaner = ammonia::Builder::default();
    cleaner
        .add_clean_content_tags(&["title"])
        .add_tags(&["font", "center"])
        .add_generic_attributes(&["style", "align", "bgcolor", "width", "height", "valign"])
        .add_tag_attributes("table", &["cellpadding", "cellspacing", "border"])
        .add_tag_attributes("font", &["color", "face", "size"])
        .url_schemes(HashSet::from(["http", "https", "mailto", "data", "cid"]))
        .url_relative(ammonia::UrlRelative::Deny)
        .filter_style_properties(HashSet::from([
            "color",
            "box-sizing",
            "background-color",
            "font-family",
            "font-size",
            "font-weight",
            "font-style",
            "line-height",
            "text-align",
            "text-decoration",
            "border",
            "border-color",
            "border-top",
            "border-bottom",
            "border-left",
            "border-right",
            "border-collapse",
            "border-spacing",
            "border-radius",
            "padding",
            "padding-top",
            "padding-bottom",
            "padding-left",
            "padding-right",
            "margin",
            "margin-top",
            "margin-bottom",
            "margin-left",
            "margin-right",
            "max-width",
            "min-width",
            "width",
            "height",
            "max-height",
            "overflow",
            "opacity",
            "display",
            "vertical-align",
            "table-layout",
            "letter-spacing",
            "white-space",
            "word-break",
            "overflow-wrap",
        ]))
        .attribute_filter(move |tag, attribute, value| {
            if attribute == "style" {
                let style = normalize_backgrounds(value);
                Some(Cow::Owned(
                    appearance.map(|a| a.style(&style)).unwrap_or(style),
                ))
            } else if appearance.is_some_and(|a| a.neutral_attribute(value))
                && matches!(attribute, "bgcolor" | "color")
            {
                None
            } else if tag == "img" && attribute == "src" {
                images
                    .get(value)
                    .filter(|s| s.starts_with("data:image/png;base64,"))
                    .map(|s| Cow::Owned(s.clone()))
            } else if tag == "a" && attribute == "href" {
                url::Url::parse(value)
                    .ok()
                    .filter(|u| matches!(u.scheme(), "http" | "https" | "mailto"))
                    .map(|u| Cow::Owned(u.to_string()))
            } else {
                Some(Cow::Borrowed(value))
            }
        });
    let safe = cleaner.clean(html).to_string();
    let appearance = appearance.map(Appearance::css).unwrap_or_else(|| {
        "html{color-scheme:light}body{background:#fff;color:#202124}a{color:#1a73e8}".into()
    });
    format!(
        r#"<!doctype html><html><head><meta charset="utf-8"><meta http-equiv="Content-Security-Policy" content="default-src 'none'; img-src data:; style-src 'unsafe-inline'; script-src 'none'; connect-src 'none'; font-src 'none'; frame-src 'none'; object-src 'none'; base-uri 'none'; form-action 'none';"><meta name="viewport" content="width=device-width, initial-scale=1"><style>body{{margin:0;padding:0;font:16px/1.5 system-ui,sans-serif;overflow-wrap:break-word;word-break:normal}}img{{max-width:100%;max-height:480px;height:auto;object-fit:contain}}table{{max-width:100%}}th,td{{overflow-wrap:normal;word-break:normal}}pre{{white-space:pre-wrap}}a{{overflow-wrap:anywhere}}{appearance}</style></head><body>{safe}</body></html>"#
    )
}
// Newsletter templates commonly use background:#fff / background:#9147ff.
// Normalize only inert colour shorthand before the CSS allowlist. Background
// images, imports and arbitrary shorthand never become renderer requests.
fn normalize_backgrounds(style: &str) -> String {
    style
        .split(';')
        .filter_map(|declaration| {
            let Some((name, value)) = declaration.split_once(':') else {
                return Some(declaration.to_owned());
            };
            if !name.trim().eq_ignore_ascii_case("background") {
                return Some(declaration.to_owned());
            }
            let value = value.trim().to_ascii_lowercase();
            let (colour, priority) = value
                .strip_suffix("!important")
                .map(|v| (v.trim(), " !important"))
                .unwrap_or((&value, ""));
            let hex = colour.strip_prefix('#').is_some_and(|v| {
                matches!(v.len(), 3 | 4 | 6 | 8) && v.bytes().all(|b| b.is_ascii_hexdigit())
            });
            let named = matches!(
                colour,
                "white"
                    | "black"
                    | "transparent"
                    | "currentcolor"
                    | "red"
                    | "green"
                    | "blue"
                    | "gray"
                    | "grey"
                    | "yellow"
                    | "purple"
                    | "orange"
            );
            let numeric = ["rgb(", "rgba(", "hsl(", "hsla("].iter().any(|prefix| {
                colour
                    .strip_prefix(prefix)
                    .and_then(|s| s.strip_suffix(')'))
                    .is_some_and(|s| {
                        !s.is_empty()
                            && s.len() < 128
                            && s.bytes()
                                .all(|b| b.is_ascii_digit() || b".,% /+-".contains(&b))
                    })
            });
            (hex || named || numeric).then(|| format!("background-color:{colour}{priority}"))
        })
        .collect::<Vec<_>>()
        .join(";")
}
pub fn links(text: &str) -> Vec<(usize, usize, String)> {
    let mut finder = linkify::LinkFinder::new();
    finder.kinds(&[linkify::LinkKind::Url, linkify::LinkKind::Email]);
    finder
        .links(text)
        .filter_map(|link| {
            let target = if link.kind() == &linkify::LinkKind::Email {
                format!("mailto:{}", link.as_str())
            } else if link.as_str().starts_with("www.") {
                format!("https://{}", link.as_str())
            } else {
                link.as_str().into()
            };
            url::Url::parse(&target)
                .ok()
                .filter(|u| matches!(u.scheme(), "http" | "https" | "mailto"))
                .map(|u| (link.start(), link.end(), u.to_string()))
        })
        .take(1000)
        .collect()
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn themed_mail_keeps_layout_and_images_without_white_surfaces_or_title_text() {
        let html = r##"<html><head><title>Duplicate subject</title></head><body bgcolor="#ffffff"><table bgcolor="#ffffff" style="background:rgb(248,249,250);width:100%"><tr><th style="width:24px;box-sizing:border-box;padding:16px">Status</th><th style="width:100%">Job</th><th style="white-space:nowrap">Annotations</th></tr></table><p style="color:#202124">Hi</p><img src="cid:photo"><a href="https://example.com" style="background:#238636;color:white;padding:12px">Open</a><script>evil()</script></body></html>"##;
        let images = HashMap::from([("cid:photo".into(), "data:image/png;base64,AA==".into())]);
        for dark in [false, true] {
            let page = styled_document(
                html,
                &images,
                Some(Appearance {
                    background: if dark { [40, 24, 34] } else { [250, 250, 250] },
                    foreground: if dark { [245, 235, 240] } else { [30, 30, 30] },
                    dark,
                }),
            );
            assert!(!page.contains("Duplicate subject"));
            assert!(!page.contains("bgcolor="));
            assert!(!page.contains("<script"));
            assert!(page.contains("box-sizing:border-box"));
            assert!(page.contains("white-space:nowrap"));
            assert!(page.contains("background-color:transparent"));
            assert!(page.contains("background-color:#238636"));
            assert!(page.contains("color:white"));
            assert!(page.contains("data:image/png;base64,AA=="));
        }
        let original = styled_document(html, &images, None);
        assert!(original.contains("bgcolor=\"#ffffff\""));
        assert!(
            original
                .replace(" ", "")
                .contains("background-color:rgb(248,249,250)"),
            "{original}"
        );
        assert!(!original.contains("Duplicate subject"));
    }
    #[test]
    fn simple_phone_text_uses_native_body_but_layout_and_links_keep_html() {
        assert!(!needs_layout(
            "<html><head><meta charset='utf-8'></head><body><div dir='auto'>Hi<br>From my phone</div></body></html>"
        ));
        assert!(needs_layout("<table><tr><td>Newsletter</td></tr></table>"));
        assert!(needs_layout(
            "<div><a href='https://example.com'>Project notes</a></div>"
        ));
        assert!(needs_layout("<p><img src='cid:photo'></p>"));
        assert!(needs_layout("<div style='background:#9147ff'>News</div>"));
    }
    #[test]
    fn newsletter_background_shorthand_keeps_cards_and_buttons_without_urls() {
        let html = r##"<table style="background:#E5E3E8"><tr><td style="background: #fff; padding:24px"><a href="https://example.com" style="background:#9147ff !important;color:#fff;display:inline-block;padding:12px 32px">Watch now</a><div style="background:rgb(239, 238, 241)">Footer</div><div style="background:url(https://tracker.invalid/x)">No remote background</div></td></tr></table>"##;
        let safe = document(html, HashMap::new());
        for expected in [
            "background-color:#e5e3e8",
            "background-color:#fff",
            "background-color:#9147ff",
            "background-color:rgb(239, 238, 241)",
            "padding:12px 32px",
        ] {
            assert!(safe.contains(expected), "{expected}: {safe}");
        }
        assert!(!safe.contains("tracker.invalid"));
        assert!(
            !normalize_backgrounds(
                "background:expression(alert(1));background:url(file:///private)"
            )
            .contains("expression")
        );
    }
    #[test]
    fn keeps_newsletter_layout_but_blocks_active_content_and_networks() {
        let html = r##"<table bgcolor="#eee" style="width:600px;background-image:url(https://tracker.invalid/x)"><tr><td style="color:#9146ff;padding:20px"><a href="https://example.com">Watch now</a><img src="https://tracker.invalid/pixel"><script>evil()</script><iframe src="file:///private"></iframe><form><input></form><img src="cid:photo" onerror="evil()"></td></tr></table>"##;
        let page = document(
            html,
            HashMap::from([("cid:photo".into(), "data:image/png;base64,AA==".into())]),
        );
        assert!(page.contains("<table"));
        assert!(page.contains("#9146ff"));
        assert!(page.contains("padding:20px"));
        assert!(page.contains("href=\"https://example.com/\""));
        assert!(page.contains("src=\"data:image/png;base64,AA==\""));
        for forbidden in [
            "tracker.invalid",
            "<script",
            "<iframe",
            "onerror",
            "file:///",
            "<form",
            "<input",
        ] {
            assert!(!page.contains(forbidden), "{forbidden}");
        }
        assert!(page.contains("default-src 'none'"));
    }
    #[test]
    fn plain_links_are_clickable_without_file_or_script_protocols() {
        let found = links(
            "Read https://example.com/a?b=1&c=2 or write hello@example.com. file:///private javascript:evil()",
        );
        assert_eq!(found.len(), 2);
        assert_eq!(found[1].2, "mailto:hello@example.com");
    }
}
