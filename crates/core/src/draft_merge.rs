//! Three-way, field-level draft resolution. Never writes or sends a message.
use crate::{Error, Result, composer::Compose};
pub const NAMES: [&str; 6] = ["From", "To", "Cc", "Bcc", "Subject", "Message body"];
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Value {
    pub text: String,
    pub html: bool,
}
#[derive(Clone, Debug)]
pub struct Fields(pub [Value; 6]);
impl Fields {
    pub fn of(c: &Compose) -> Self {
        let mut values = [&c.from, &c.to, &c.cc, &c.bcc, &c.subject, &c.text].map(|v| Value {
            text: v.to_string(),
            html: false,
        });
        values[5].html = c.html;
        Self(values)
    }
    pub fn apply(&self, c: &mut Compose) {
        c.from = self.0[0].text.clone();
        c.to = self.0[1].text.clone();
        c.cc = self.0[2].text.clone();
        c.bcc = self.0[3].text.clone();
        c.subject = self.0[4].text.clone();
        c.text = self.0[5].text.clone();
        c.html = self.0[5].html;
    }
}
fn same(a: &Value, b: &Value) -> bool {
    if a == b {
        return true;
    }
    if !a.html || !b.html {
        return false;
    }
    let (Some(a), Some(b)) = (
        crate::rich_draft::parse(&a.text),
        crate::rich_draft::parse(&b.text),
    ) else {
        return false;
    };
    let spans = |r: crate::rich_draft::RichDraft| {
        let mut spans: Vec<_> = r
            .spans
            .into_iter()
            .map(|s| (s.start, s.end, s.tag))
            .collect();
        spans.sort();
        (r.text, r.quote, spans)
    };
    spans(a) == spans(b)
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Resolution {
    Same,
    Mine,
    Gmail,
    Choose,
}
pub struct Plan {
    pub local: Fields,
    pub remote: Fields,
    pub resolution: [Resolution; 6],
}
impl Plan {
    pub fn is_resolved(&self, choices: [Option<bool>; 6]) -> bool {
        (0..6).all(|i| self.resolution[i] != Resolution::Choose || choices[i].is_some())
    }
    pub fn new(base: &Fields, local: Fields, remote: Fields) -> Self {
        let resolution = std::array::from_fn(|i| {
            if same(&local.0[i], &remote.0[i]) {
                Resolution::Same
            } else if same(&local.0[i], &base.0[i]) {
                Resolution::Gmail
            } else if same(&remote.0[i], &base.0[i]) {
                Resolution::Mine
            } else {
                Resolution::Choose
            }
        });
        Self {
            local,
            remote,
            resolution,
        }
    }
    pub fn resolve(&self, choices: [Option<bool>; 6]) -> Result<Fields> {
        let mut use_remote = [false; 6];
        for i in 0..6 {
            let remote = match self.resolution[i] {
                Resolution::Same | Resolution::Mine => false,
                Resolution::Gmail => true,
                Resolution::Choose => choices[i].ok_or_else(|| {
                    Error::Invalid(format!("Choose which {} to keep", NAMES[i].to_lowercase()))
                })?,
            };
            use_remote[i] = remote;
        }
        Ok(Fields(std::array::from_fn(|i| {
            if use_remote[i] {
                self.remote.0[i].clone()
            } else {
                self.local.0[i].clone()
            }
        })))
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn unrelated_changes_merge_and_conflicting_bodies_require_an_explicit_choice() {
        let base = Compose {
            subject: "Original".into(),
            text: "Base body".into(),
            ..Default::default()
        };
        let mut mine = base.clone();
        mine.subject = "My subject".into();
        mine.text = "My body".into();
        let mut remote = base.clone();
        remote.to = "new@example.invalid".into();
        remote.text = "Remote body".into();
        let plan = Plan::new(&Fields::of(&base), Fields::of(&mine), Fields::of(&remote));
        assert_eq!(plan.resolution[4], Resolution::Mine);
        assert_eq!(plan.resolution[1], Resolution::Gmail);
        assert_eq!(plan.resolution[5], Resolution::Choose);
        assert!(plan.resolve([None; 6]).is_err());
        let mut choice = [None; 6];
        choice[5] = Some(true);
        let fields = plan.resolve(choice).unwrap();
        fields.apply(&mut mine);
        assert_eq!(mine.subject, "My subject");
        assert_eq!(mine.to, "new@example.invalid");
        assert_eq!(mine.text, "Remote body");
    }
    #[test]
    fn rich_markup_is_preserved_and_equal_supported_formatting_is_not_a_conflict() {
        let a = Value {
            text: "<div data-luma-editor=\"1\"><strong><em>Hello</em></strong></div>".into(),
            html: true,
        };
        let b = Value {
            text: "<div data-luma-editor=\"1\"><em><strong>Hello</strong></em></div>".into(),
            html: true,
        };
        assert!(same(&a, &b));
        let c = Value {
            text: a.text.clone(),
            html: false,
        };
        assert!(!same(&a, &c));
        let unknown = Value {
            text: "<table><tr><td>Brand mail</td></tr></table>".into(),
            html: true,
        };
        assert!(!same(&a, &unknown));
    }
}
