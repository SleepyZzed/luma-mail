use crate::{Error, Result};
use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub struct Palette {
    pub mode: String,
    pub background: String,
    pub surface: String,
    pub text: String,
    pub muted: String,
    pub accent: String,
    pub on_accent: String,
    pub outline: String,
    pub hover: String,
}
impl Palette {
    pub fn parse(input: &str) -> Result<Self> {
        if input.len() > 8192 {
            return Err(Error::Invalid("Theme file is too large".into()));
        }
        let value: Self = toml::from_str(input)
            .map_err(|_| Error::Invalid("Invalid Noctalia palette file".into()))?;
        if !["dark", "light"].contains(&value.mode.as_str()) {
            return Err(Error::Invalid("Invalid theme mode".into()));
        }
        for color in [
            &value.background,
            &value.surface,
            &value.text,
            &value.muted,
            &value.accent,
            &value.on_accent,
            &value.outline,
            &value.hover,
        ] {
            if color.len() != 7
                || !color.starts_with('#')
                || !color[1..].bytes().all(|c| c.is_ascii_hexdigit())
            {
                return Err(Error::Invalid(
                    "Theme values must be #RRGGBB colours".into(),
                ));
            }
        }
        if contrast(&value.background, &value.text) < 4.5
            || contrast(&value.surface, &value.text) < 4.5
            || contrast(&value.accent, &value.on_accent) < 4.5
        {
            return Err(Error::Invalid(
                "Theme text contrast is below WCAG AA; using the system theme".into(),
            ));
        }
        Ok(value)
    }
    pub fn css(&self) -> String {
        format!(
            "@define-color window_bg_color {}; @define-color window_fg_color {}; @define-color view_bg_color {}; @define-color view_fg_color {}; @define-color sidebar_bg_color {}; @define-color sidebar_fg_color {}; @define-color headerbar_bg_color {}; @define-color headerbar_fg_color {}; @define-color accent_bg_color {}; @define-color accent_fg_color {}; @define-color accent_color {}; .luma-sidebar {{ background: {}; }} .mail-card {{ border-color: {}; }} row:hover {{ background: {}; }} .dim-label {{ color: {}; }}",
            self.background,
            self.text,
            self.surface,
            self.text,
            self.background,
            self.text,
            self.background,
            self.text,
            self.accent,
            self.on_accent,
            self.accent,
            self.background,
            self.outline,
            self.hover,
            self.muted
        )
    }
}
pub fn contrast(a: &str, b: &str) -> f64 {
    fn luminance(s: &str) -> f64 {
        let channels: Vec<f64> = [1, 3, 5]
            .into_iter()
            .map(|i| u8::from_str_radix(&s[i..i + 2], 16).unwrap_or(0) as f64 / 255.)
            .map(|v| {
                if v <= 0.04045 {
                    v / 12.92
                } else {
                    ((v + 0.055) / 1.055).powf(2.4)
                }
            })
            .collect();
        channels[0] * 0.2126 + channels[1] * 0.7152 + channels[2] * 0.0722
    }
    let (a, b) = (luminance(a), luminance(b));
    (a.max(b) + 0.05) / (a.min(b) + 0.05)
}
