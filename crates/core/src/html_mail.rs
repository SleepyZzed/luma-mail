//! HTML mail is display-only. No remote resources reach the renderer: approved
//! images are decoded, resized and re-encoded as inert PNG data URLs first.
use crate::{Error, Result, composer::decode_part, model::Part, preview};
use base64::{Engine, engine::general_purpose::STANDARD};
use std::{
    borrow::Cow,
    collections::{HashMap, HashSet},
    io::Cursor,
};

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
pub fn document(html: &str, images: HashMap<String, String>) -> String {
    let mut cleaner = ammonia::Builder::default();
    cleaner
        .add_tags(&["font", "center"])
        .add_generic_attributes(&["style", "align", "bgcolor", "width", "height", "valign"])
        .add_tag_attributes("table", &["cellpadding", "cellspacing", "border"])
        .add_tag_attributes("font", &["color", "face", "size"])
        .url_schemes(HashSet::from(["http", "https", "mailto", "data", "cid"]))
        .url_relative(ammonia::UrlRelative::Deny)
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
            "border",
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
                Some(Cow::Owned(normalize_backgrounds(value)))
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
    format!(
        r#"<!doctype html><html><head><meta charset="utf-8"><meta http-equiv="Content-Security-Policy" content="default-src 'none'; img-src data:; style-src 'unsafe-inline'; script-src 'none'; connect-src 'none'; font-src 'none'; frame-src 'none'; object-src 'none'; base-uri 'none'; form-action 'none';"><meta name="viewport" content="width=device-width, initial-scale=1"><style>html{{color-scheme:light}}body{{margin:0;padding:16px;background:#fff;color:#202124;font:16px/1.5 system-ui,sans-serif;overflow-wrap:anywhere}}img{{max-width:100%;height:auto}}table{{max-width:100%}}pre{{white-space:pre-wrap}}a{{color:#6254c7}}</style></head><body>{safe}</body></html>"#
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
