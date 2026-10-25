use crate::{Error, Result};

#[derive(Default)]
pub struct Search {
    pub from: String,
    pub to: String,
    pub subject: String,
    pub words: String,
    pub after: String,
    pub before: String,
    pub unread: bool,
    pub starred: bool,
    pub attachments: bool,
}
impl Search {
    pub fn query(&self) -> Result<String> {
        let mut terms = Vec::new();
        for (operator, value) in [
            ("from", &self.from),
            ("to", &self.to),
            ("subject", &self.subject),
        ] {
            let value = value.trim();
            if value.is_empty() {
                continue;
            }
            if value.len() > 1024 || value.contains(['"', '\\', '\n', '\r']) {
                return Err(Error::Invalid(format!(
                    "The {operator} filter cannot contain quotes, backslashes or line breaks. Use Gmail operators in the query field for more complex searches."
                )));
            }
            terms.push(format!("{operator}:\"{value}\""));
        }
        let date = |value: &str| -> Result<Option<chrono::NaiveDate>> {
            if value.trim().is_empty() {
                return Ok(None);
            }
            chrono::NaiveDate::parse_from_str(value.trim(), "%Y-%m-%d")
                .map(Some)
                .map_err(|_| Error::Invalid("Use YYYY-MM-DD for search dates".into()))
        };
        let after = date(&self.after)?;
        let before = date(&self.before)?;
        if after.zip(before).is_some_and(|(a, b)| a >= b) {
            return Err(Error::Invalid(
                "The before date must be later than the after date".into(),
            ));
        }
        for (name, value) in [("after", after), ("before", before)] {
            if let Some(value) = value {
                terms.push(format!("{name}:{}", value.format("%Y/%m/%d")));
            }
        }
        if self.unread {
            terms.push("is:unread".into());
        }
        if self.starred {
            terms.push("is:starred".into());
        }
        if self.attachments {
            terms.push("has:attachment".into());
        }
        if !self.words.trim().is_empty() {
            terms.push(self.words.trim().into());
        }
        let query = terms.join(" ");
        if query.len() > 8192 {
            return Err(Error::Invalid("This search query is too long".into()));
        }
        Ok(query)
    }
}
