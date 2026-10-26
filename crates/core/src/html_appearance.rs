//! Reader colours are supplied as numeric channels, never executable sender CSS.
use csscolorparser::Color;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Appearance {
    pub background: [u8; 3],
    pub foreground: [u8; 3],
    pub dark: bool,
}

fn neutral(colour: Color) -> bool {
    let [r, g, b, _] = colour.to_rgba8();
    r.max(g).max(b) - r.min(g).min(b) <= 36
}
fn value(raw: &str) -> (&str, &str) {
    let (colour, priority) = raw.split_once('!').unwrap_or((raw, ""));
    (
        colour.trim(),
        if priority.trim().eq_ignore_ascii_case("important") {
            " !important"
        } else {
            ""
        },
    )
}
fn hex([r, g, b]: [u8; 3]) -> String {
    format!("#{r:02x}{g:02x}{b:02x}")
}
fn readable_background(colour: Color) -> &'static str {
    let [r, g, b, _] = colour.to_rgba8();
    if crate::theme::contrast(&hex([r, g, b]), "#ffffff") >= 4.5 {
        "#ffffff"
    } else {
        "#101010"
    }
}
impl Appearance {
    pub(crate) fn css(self) -> String {
        format!(
            "html{{color-scheme:{};background:{}}}body{{background:transparent;color:{}}}a{{color:{}}}",
            if self.dark { "dark" } else { "light" },
            hex(self.background),
            hex(self.foreground),
            if self.dark { "#75adff" } else { "#165fcb" }
        )
    }
    pub(crate) fn neutral_attribute(self, value: &str) -> bool {
        value.parse::<Color>().is_ok_and(neutral)
    }
    pub(crate) fn style(self, style: &str) -> String {
        let declarations: Vec<_> = style.split(';').filter_map(|d| d.split_once(':')).collect();
        let background = declarations
            .iter()
            .find_map(|(name, raw)| {
                name.trim()
                    .eq_ignore_ascii_case("background-color")
                    .then(|| value(raw).0.parse::<Color>().ok())
                    .flatten()
            })
            .filter(|c| c.a > 0.9 && !neutral(*c));
        let mut foreground = false;
        let mut result = Vec::new();
        for (name, raw) in declarations {
            let name = name.trim().to_ascii_lowercase();
            let (colour, priority) = value(raw);
            let parsed = colour.parse::<Color>().ok();
            let adapted = match name.as_str() {
                "background-color" if parsed.is_some_and(neutral) => Some("transparent".into()),
                "color" => {
                    foreground = true;
                    if let Some(bg) = background {
                        let safe = parsed.filter(|fg| {
                            let [r, g, b, _] = fg.to_rgba8();
                            let [br, bg, bb, _] = bg.to_rgba8();
                            crate::theme::contrast(&hex([r, g, b]), &hex([br, bg, bb])) >= 4.5
                        });
                        Some(
                            safe.map(|_| colour.to_owned())
                                .unwrap_or_else(|| readable_background(bg).into()),
                        )
                    } else if parsed.is_some_and(neutral) {
                        Some("inherit".into())
                    } else {
                        None
                    }
                }
                "border-color" if parsed.is_some_and(neutral) => {
                    let colour = std::array::from_fn(|i| {
                        ((self.foreground[i] as u16 + self.background[i] as u16 * 3) / 4) as u8
                    });
                    Some(hex(colour))
                }
                _ => None,
            };
            result.push(match adapted {
                Some(v) => format!("{name}:{v}{priority}"),
                None => format!("{name}:{raw}"),
            });
        }
        if let Some(bg) = background
            && !foreground
        {
            result.push(format!("color:{}", readable_background(bg)));
        }
        result.join(";")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    const DARK: Appearance = Appearance {
        background: [40, 24, 34],
        foreground: [245, 235, 240],
        dark: true,
    };
    #[test]
    fn neutral_surfaces_follow_theme_and_brand_buttons_stay_readable() {
        assert_eq!(
            DARK.style("background-color:#fff!important;color:rgb(30,30,30)"),
            "background-color:transparent !important;color:inherit"
        );
        assert_eq!(
            DARK.style("background-color:#9147ff;color:#fff"),
            "background-color:#9147ff;color:#fff"
        );
        assert_eq!(
            DARK.style("background-color:#ffd400;color:#eee"),
            "background-color:#ffd400;color:#101010"
        );
        assert_eq!(
            DARK.style("background-color:#238636"),
            "background-color:#238636;color:#ffffff"
        );
        assert!(DARK.neutral_attribute("#f6f8fa"));
        assert!(!DARK.neutral_attribute("#9147ff"));
    }
}
