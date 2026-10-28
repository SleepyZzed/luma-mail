//! A bounded visible header window. Complete bodies remain explicit/on demand.
use luma_core::{Error, Result, model::Message};
use serde_json::{Value, json};
use std::ops::Range;
pub const HEADERS: usize = 32;
#[derive(Default)]
pub struct Window {
    end: Option<String>,
    pub revision: u64,
}
impl Window {
    pub fn reset(&mut self, revision: u64) {
        self.end = None;
        self.revision = revision;
    }
    pub fn range(&self, messages: &[Message]) -> Range<usize> {
        let end = self
            .end
            .as_ref()
            .and_then(|id| messages.iter().position(|m| m.id == *id))
            .map_or(messages.len(), |i| i + 1);
        end.saturating_sub(HEADERS)..end
    }
    pub fn focus(&mut self, messages: &[Message], id: &str) {
        if self.end.is_none() && messages.last().is_some_and(|last| last.id != id) {
            self.end = messages.last().map(|m| m.id.clone());
        }
    }
    pub fn change(&mut self, messages: &[Message], direction: &str, revision: u64) -> Result<()> {
        if revision <= self.revision {
            return Ok(());
        }
        let range = self.range(messages);
        let end = match direction {
            "earlier" => range.start.max(HEADERS.min(messages.len())),
            "later" => (range.end + HEADERS).min(messages.len()),
            "latest" => messages.len(),
            _ => {
                return Err(Error::Invalid(
                    "Choose Earlier, Later or Latest messages".into(),
                ));
            }
        };
        self.end = if end == messages.len() {
            None
        } else {
            messages.get(end.saturating_sub(1)).map(|m| m.id.clone())
        };
        self.revision = revision;
        Ok(())
    }
    pub fn describe(&self, messages: &[Message]) -> Value {
        let range = self.range(messages);
        json!({"start":if range.is_empty(){0}else{range.start+1},"end":range.end,"total":messages.len(),"earlier":range.start>0,"later":range.end<messages.len(),"revision":self.revision})
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    fn messages(n: usize) -> Vec<Message> {
        (0..n)
            .map(|i| Message {
                id: format!("message-{i}"),
                ..Default::default()
            })
            .collect()
    }
    #[test]
    fn huge_chains_start_at_the_latest_and_every_header_remains_accessible() {
        let all = messages(2503);
        let mut w = Window::default();
        w.reset(1);
        let mut seen = std::collections::HashSet::new();
        loop {
            let r = w.range(&all);
            assert!(r.len() <= HEADERS);
            seen.extend(r.clone());
            if r.start == 0 {
                break;
            }
            w.change(&all, "earlier", w.revision + 1).unwrap();
        }
        assert_eq!(seen.len(), all.len());
        w.change(&all, "latest", w.revision + 1).unwrap();
        assert_eq!(w.range(&all), 2471..2503);
    }
    #[test]
    fn reading_an_older_message_pins_the_window_until_explicit_navigation() {
        let mut all = messages(40);
        let mut w = Window::default();
        w.focus(&all, "message-8");
        all.push(Message {
            id: "new".into(),
            ..Default::default()
        });
        assert_eq!(w.range(&all), 8..40);
        assert_eq!(w.describe(&all)["later"], true);
    }
    #[test]
    fn historical_windows_stay_anchored_when_new_replies_arrive() {
        let mut all = messages(70);
        let mut w = Window::default();
        w.change(&all, "earlier", 1).unwrap();
        assert_eq!(w.range(&all), 6..38);
        all.extend(messages(3).into_iter().enumerate().map(|(i, mut m)| {
            m.id = format!("new-{i}");
            m
        }));
        assert_eq!(w.range(&all), 6..38);
        w.change(&all, "later", 2).unwrap();
        assert_eq!(w.range(&all), 38..70);
        w.change(&all, "later", 3).unwrap();
        assert_eq!(w.range(&all), 41..73);
        w.change(&all, "earlier", 2).unwrap();
        assert_eq!(w.range(&all), 41..73);
    }
}
