//! Portable, validated formatting metadata. No browser, font download or disk state.
/// Return the canonical attribute and CSS for an internal formatting tag.
pub fn properties(tag: &str) -> Option<(&'static str, String, String)> {
    let (group, value) = tag.rsplit_once(':')?;
    let colour = || {
        let v = value.strip_prefix('#').unwrap_or(value);
        (v.len() == 6 && v.bytes().all(|b| b.is_ascii_hexdigit()))
            .then(|| format!("#{}", v.to_ascii_lowercase()))
    };
    Some(match group {
        "style:color" => {
            let c = colour()?;
            ("data-luma-color", c.clone(), format!("color:{c}"))
        }
        "style:highlight" => {
            let c = colour()?;
            (
                "data-luma-highlight",
                c.clone(),
                format!("background-color:{c}"),
            )
        }
        "style:script" if matches!(value, "super" | "sub") => (
            "data-luma-script",
            value.into(),
            format!("vertical-align:{value};font-size:0.8em"),
        ),
        "style:code" if value == "1" => (
            "data-luma-code",
            value.into(),
            "font-family:monospace".into(),
        ),
        "paragraph:align" if matches!(value, "left" | "center" | "right" | "justify") => (
            "data-luma-align",
            value.into(),
            format!("text-align:{value}"),
        ),
        "paragraph:indent" => {
            let n = value.parse::<u8>().ok().filter(|n| *n <= 8)?;
            (
                "data-luma-indent",
                n.to_string(),
                format!("padding-left:{}px", n as usize * 24),
            )
        }
        "paragraph:spacing" => {
            let n = value
                .parse::<u16>()
                .ok()
                .filter(|n| [100, 115, 150, 200].contains(n))?;
            (
                "data-luma-spacing",
                n.to_string(),
                format!("line-height:{}", n as f64 / 100.),
            )
        }
        _ => return None,
    })
}
pub fn from_attribute(name: &str, value: &str) -> Option<String> {
    let kind = match name {
        "data-luma-color" => "style:color",
        "data-luma-highlight" => "style:highlight",
        "data-luma-script" => "style:script",
        "data-luma-code" => "style:code",
        "data-luma-align" => "paragraph:align",
        "data-luma-indent" => "paragraph:indent",
        "data-luma-spacing" => "paragraph:spacing",
        _ => return None,
    };
    let raw = format!("{kind}:{value}");
    let (_, canonical, _) = properties(&raw)?;
    Some(format!("{kind}:{canonical}"))
}
pub fn open(tag: &str) -> Option<String> {
    if !tag.starts_with("style:") {
        return None;
    }
    let (name, value, css) = properties(tag)?;
    Some(format!("<span {name}=\"{value}\" style=\"{css}\">"))
}
pub fn paragraph(tags: impl Iterator<Item = String>, body: &str) -> String {
    let mut attributes = String::new();
    let mut css = String::new();
    for tag in tags.filter(|t| t.starts_with("paragraph:")) {
        if let Some((name, value, style)) = properties(&tag) {
            attributes.push_str(&format!(" {name}=\"{value}\""));
            css.push_str(&style);
            css.push(';');
        }
    }
    if attributes.is_empty() {
        return body.into();
    }
    format!(
        "<span{attributes} style=\"display:inline-block;width:100%;box-sizing:border-box;{css}\">{body}</span>"
    )
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn metadata_rejects_css_injection_and_keeps_supported_styles() {
        for input in [
            "style:color:javascript:alert(1)",
            "style:highlight:#fff;url(x)",
            "paragraph:indent:999",
            "paragraph:align:inherit;bad()",
            "paragraph:spacing:999",
        ] {
            assert!(properties(input).is_none());
        }
        assert_eq!(
            from_attribute("data-luma-color", "#AABBCC").unwrap(),
            "style:color:#aabbcc"
        );
        let html = paragraph(
            ["paragraph:align:center".into(), "paragraph:indent:2".into()].into_iter(),
            "Hi",
        );
        assert!(html.contains("text-align:center") && html.contains("padding-left:48px"));
    }
}
