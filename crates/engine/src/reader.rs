//! Display documents are regenerated from the selected message. Only one
//! message's inert image data is retained, with a four-MiB aggregate RAM limit.
use crate::{
    app::{self, App},
    protocol::Command,
};
use luma_core::{
    Error, Result, composer, html_mail, mail_images, model::Message, provider::MailProvider,
};
use serde_json::json;
use std::{
    collections::HashMap,
    sync::{Arc, atomic::Ordering},
};
const IMAGE_BUDGET: usize = 4 * 1024 * 1024;
pub fn links(text: &str) -> Vec<serde_json::Value> {
    let mut byte = 0;
    let mut utf16 = 0;
    html_mail::links(text)
        .into_iter()
        .map(|(start, end, url)| {
            utf16 += text[byte..start].encode_utf16().count();
            let from = utf16;
            utf16 += text[start..end].encode_utf16().count();
            byte = end;
            json!({"start":from,"end":utf16,"url":url})
        })
        .collect()
}
#[derive(Default)]
pub struct Assets {
    pub reading: u64,
    pub id: String,
    images: HashMap<String, String>,
    embedded: bool,
    external: bool,
}
impl Assets {
    pub fn clear(&mut self) {
        *self = Self::default();
    }
}
fn appearance(c: &Command) -> Result<Option<html_mail::Appearance>> {
    if c.flag("original") {
        return Ok(None);
    }
    fn channels(c: &Command, key: &str) -> Result<[u8; 3]> {
        let v = c.data[key]
            .as_array()
            .filter(|a| a.len() == 3)
            .ok_or_else(|| Error::Invalid("Invalid reader colour".into()))?;
        let mut rgb = [0; 3];
        for (i, ch) in v.iter().enumerate() {
            rgb[i] = ch
                .as_u64()
                .and_then(|v| u8::try_from(v).ok())
                .ok_or_else(|| Error::Invalid("Invalid reader colour".into()))?;
        }
        Ok(rgb)
    }
    Ok(Some(html_mail::Appearance {
        background: channels(c, "background")?,
        foreground: channels(c, "foreground")?,
        dark: c.flag("dark"),
    }))
}
pub fn fixture_html() -> String {
    "<html><head><title>Hidden title</title></head><body><table width='600' style='background:#fff'><tr><td style='width:40px'>Status</td><td style='padding:16px'><h2>A calmer inbox</h2><p>Synthetic newsletter with a table, a long paragraph and a <a href='https://example.com/review'>safe link</a>.</p><img src='https://example.invalid/tracker.png' alt='Private image'></td></tr></table><p style='margin-top:600px'>End of the complete message</p><script>window.senderRan=true</script><iframe src='file:///private'></iframe></body></html>".into()
}
pub async fn render(app: &Arc<App>, c: &Command) -> Result<()> {
    let key = app::key(&c.data)?;
    let reading = app.reading.load(Ordering::SeqCst);
    let appearance = appearance(c)?;
    let _lock = app.render_lock.lock().await;
    if app.preview {
        app.preview_read_failure(4)?;
    }
    let mut assets = {
        let mut current = app.reader_assets.lock().unwrap();
        if current.reading == reading && current.id == key.id {
            std::mem::take(&mut *current)
        } else {
            Assets {
                reading,
                id: key.id.clone(),
                ..Default::default()
            }
        }
    };
    let current = || {
        let v = app.view.lock().unwrap();
        app.reading.load(Ordering::SeqCst) == reading
            && v.selected
                .as_ref()
                .is_some_and(|k| k.account == key.account)
            && if app.preview {
                v.selected.as_ref().is_some_and(|k| k.id == key.id)
            } else {
                v.thread
                    .as_ref()
                    .is_some_and(|t| t.messages.iter().any(|m| m.id == key.id))
            }
    };
    if !current() {
        return Ok(());
    }
    let (html, message) =
        if app.preview && std::env::var("LUMA_PREVIEW_HTML").is_ok_and(|v| v == "1") {
            if app.reading.load(Ordering::SeqCst) != reading {
                return Ok(());
            }
            (fixture_html(), None)
        } else {
            if !current() {
                return Ok(());
            }
            let resident = app.view.lock().unwrap().bodies.get(&key.id);
            let m = match resident {
                Some(m) => m,
                None => Arc::new(app.client(&key.account)?.message(&key.id).await?),
            };
            let html = html_mail::body(&m.payload)?
                .ok_or_else(|| Error::Invalid("This message has no HTML document".into()))?;
            (html, Some(m))
        };
    let mut failed = 0;
    if let Some(message) = message {
        let sources = mail_images::sources(&message.payload);
        for source in sources.iter().take(8) {
            let external = source.starts_with("https://");
            if assets.images.contains_key(source)
                || if external {
                    !c.flag("external") || assets.external
                } else {
                    assets.embedded
                }
            {
                continue;
            }
            if !current() {
                return Ok(());
            }
            let result = image(app, &key.account, &message, source, external).await;
            match result {
                Ok(data)
                    if assets
                        .images
                        .iter()
                        .map(|(k, v)| k.len() + v.len())
                        .sum::<usize>()
                        + source.len()
                        + data.len()
                        <= IMAGE_BUDGET =>
                {
                    assets.images.insert(source.clone(), data);
                }
                _ => failed += 1,
            }
        }
        assets.embedded = true;
        if c.flag("external") {
            assets.external = true;
        }
    }
    let document = html_mail::styled_document(&html, &assets.images, appearance);
    if app.reading.load(Ordering::SeqCst) != reading {
        return Ok(());
    }
    let external = assets.external;
    *app.reader_assets.lock().unwrap() = assets;
    app.events.emit(c.id,"readerDocument",json!({"account":key.account,"reading":reading,"id":key.id,"html":document,"externalLoaded":external,"failedImages":failed,"request":c.id})).await;
    Ok(())
}
async fn image(
    app: &App,
    account: &str,
    message: &Message,
    source: &str,
    external: bool,
) -> Result<String> {
    let bytes = if external {
        mail_images::fetch_external(source).await?
    } else if source.starts_with("data:image/") {
        mail_images::embedded(source)?
    } else if let Some(cid) = source.strip_prefix("cid:") {
        let part = message
            .payload
            .attachments()
            .into_iter()
            .find(|p| p.header("Content-ID").trim_matches(['<', '>']) == cid)
            .ok_or_else(|| Error::Invalid("Inline image missing".into()))?;
        if part.body.size > 8 * 1024 * 1024 {
            return Err(Error::ResponseLimit);
        }
        if let Some(id) = &part.body.attachment_id {
            app.client(account)?
                .attachment_limited(&message.id, id, 8 * 1024 * 1024)
                .await?
        } else {
            composer::decode_part(part)?
        }
    } else {
        return Err(Error::Invalid("Unsupported image source".into()));
    };
    tokio::task::spawn_blocking(move || html_mail::image_data(&bytes))
        .await
        .map_err(|_| Error::Invalid("Image conversion failed".into()))?
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn received_links_use_native_utf16_offsets_and_safe_schemes() {
        let text = "🙂 see https://example.com/é and person@example.com; file:///private";
        let spans = links(text);
        assert_eq!(spans.len(), 2);
        assert_eq!(spans[0]["start"], 7);
        let units: Vec<_> = text.encode_utf16().collect();
        let range = spans[1]["start"].as_u64().unwrap() as usize
            ..spans[1]["end"].as_u64().unwrap() as usize;
        assert_eq!(
            String::from_utf16(&units[range]).unwrap(),
            "person@example.com"
        );
        assert_eq!(spans[1]["url"], "mailto:person@example.com");
    }
    #[test]
    fn colour_channels_are_numeric_and_original_is_explicit() {
        let mut c = Command {
            id: 1,
            action: "renderMessage".into(),
            data: json!({"background":[25,24,31],"foreground":[242,239,247],"dark":true}),
        };
        assert_eq!(appearance(&c).unwrap().unwrap().background, [25, 24, 31]);
        c.data["background"] = json!([0, 256, 0]);
        assert!(appearance(&c).is_err());
        c.data["original"] = json!(true);
        assert!(appearance(&c).unwrap().is_none());
    }
}
