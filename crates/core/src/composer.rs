use crate::{
    Error, Result,
    model::{Message, Part},
};
use base64::{
    Engine,
    engine::general_purpose::{STANDARD, URL_SAFE_NO_PAD},
};
use std::path::PathBuf;

pub const MAX_RAW_SIZE: usize = 24 * 1024 * 1024;
pub const MAX_ATTACHMENT_BYTES: u64 = 16 * 1024 * 1024;
pub fn new_id() -> String {
    uuid::Uuid::new_v4().to_string()
}
pub fn decode_part(part: &Part) -> Result<Vec<u8>> {
    URL_SAFE_NO_PAD
        .decode(
            part.body
                .data
                .as_deref()
                .unwrap_or("")
                .trim_end_matches('='),
        )
        .map_err(|_| Error::Invalid("Attachment has invalid encoding".into()))
}

#[derive(Clone, Default)]
pub struct RetainedAttachment {
    pub name: String,
    pub mime_type: String,
    pub data: std::sync::Arc<Vec<u8>>,
    pub content_id: Option<String>,
}
#[derive(Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct Compose {
    #[serde(default)]
    pub account: Option<String>,
    pub from: String,
    #[serde(default)]
    pub sender_reply_to: String,
    pub to: String,
    pub cc: String,
    pub bcc: String,
    pub subject: String,
    pub text: String,
    pub html: bool,
    pub attachments: Vec<PathBuf>,
    #[serde(skip)] // Recovery stores references, never copies received attachments.
    pub retained: Vec<RetainedAttachment>,
    pub drive_files: Vec<crate::drive::DriveFile>,
    pub thread: Option<String>,
    pub reply_to: String,
    pub references: String,
    pub message_id: String,
}

pub fn recipients(value: &str) -> Result<Vec<String>> {
    clean_header(value)?;
    let mut inputs = Vec::new();
    let mut start = 0;
    let mut quoted = false;
    let mut escaped = false;
    let mut angle = false;
    for (index, ch) in value.char_indices() {
        if escaped {
            escaped = false;
            continue;
        }
        match ch {
            '\\' if quoted => escaped = true,
            '"' => quoted = !quoted,
            '<' if !quoted => {
                if angle {
                    return Err(Error::Invalid("Nested recipient brackets".into()));
                }
                angle = true;
            }
            '>' if !quoted => angle = false,
            ',' | ';' if !quoted && !angle => {
                inputs.push(&value[start..index]);
                start = index + ch.len_utf8();
            }
            _ => (),
        }
    }
    if quoted || angle {
        return Err(Error::Invalid("Unclosed recipient quote or bracket".into()));
    }
    inputs.push(&value[start..]);
    let mut out = Vec::new();
    for input in inputs.into_iter().map(str::trim).filter(|s| !s.is_empty()) {
        let email = if let Some(start) = input.find('<') {
            input
                .get(start + 1..)
                .and_then(|s| s.strip_suffix('>'))
                .unwrap_or("")
        } else {
            input
        };
        if email.len() > 254
            || email.matches('@').count() != 1
            || email.starts_with('@')
            || email.ends_with('@')
            || email
                .chars()
                .any(|c| c.is_whitespace() || c.is_control() || "<>\"\\,;".contains(c))
        {
            return Err(Error::Invalid(format!("Invalid recipient: {input}")));
        }
        out.push(input.to_owned());
    }
    Ok(out)
}

fn clean_header(value: &str) -> Result<()> {
    if value.len() > 8192 || value.chars().any(char::is_control) {
        return Err(Error::Invalid(
            "A mail header contains invalid control characters".into(),
        ));
    }
    Ok(())
}
fn encoded(value: &str) -> String {
    if value.is_ascii() && value.len() <= 78 {
        value.into()
    } else {
        let mut words = Vec::new();
        let mut chunk = String::new();
        for ch in value.chars() {
            if chunk.len() + ch.len_utf8() > 39 {
                words.push(format!("=?UTF-8?B?{}?=", STANDARD.encode(chunk.as_bytes())));
                chunk.clear();
            }
            chunk.push(ch);
        }
        if !chunk.is_empty() {
            words.push(format!("=?UTF-8?B?{}?=", STANDARD.encode(chunk.as_bytes())));
        }
        words.join("\r\n ")
    }
}
fn wrapped_base64(data: &[u8], out: &mut String) {
    for chunk in data.chunks(57) {
        out.push_str(&STANDARD.encode(chunk));
        out.push_str("\r\n");
    }
}

impl Compose {
    pub fn new(from: String) -> Self {
        Self {
            from,
            message_id: format!("<{}@luma.local>", uuid::Uuid::new_v4()),
            ..Self::default()
        }
    }
    pub fn reply(from: String, original: &Message, all: bool) -> Self {
        let mut draft = Self::new(from.clone());
        draft.to = original.payload.header("Reply-To").to_string();
        if draft.to.is_empty() {
            draft.to = original.payload.header("From").into();
        }
        if all {
            let candidates = format!(
                "{},{}",
                original.payload.header("To"),
                original.payload.header("Cc")
            );
            let own = mailbox(&from);
            let primary: std::collections::HashSet<_> = recipients(&draft.to)
                .unwrap_or_default()
                .iter()
                .map(|s| mailbox(s))
                .collect();
            let mut seen = std::collections::HashSet::new();
            draft.cc = recipients(&candidates)
                .unwrap_or_default()
                .into_iter()
                .filter(|s| mailbox(s) != own && !primary.contains(&mailbox(s)))
                .filter(|s| seen.insert(mailbox(s)))
                .collect::<Vec<_>>()
                .join(", ");
        }
        let subject = original.payload.header("Subject");
        draft.subject = if subject.to_ascii_lowercase().starts_with("re:") {
            subject.into()
        } else {
            format!("Re: {subject}")
        };
        draft.thread = Some(original.thread_id.clone());
        draft.reply_to = original.payload.header("Message-ID").into();
        draft.references = format!(
            "{} {}",
            original.payload.header("References"),
            draft.reply_to
        )
        .trim()
        .into();
        draft
    }
    pub fn validate_send(&self) -> Result<()> {
        if recipients(&format!("{},{},{}", self.to, self.cc, self.bcc))?.is_empty() {
            return Err(Error::Invalid("Add at least one recipient".into()));
        }
        Ok(())
    }
    pub fn mime(&self) -> Result<Vec<u8>> {
        for value in [
            &self.from,
            &self.to,
            &self.cc,
            &self.bcc,
            &self.subject,
            &self.reply_to,
            &self.references,
            &self.message_id,
        ] {
            clean_header(value)?;
        }
        for value in [&self.from, &self.to, &self.cc, &self.bcc] {
            recipients(value)?;
        }
        let mut attachment_bytes = self
            .retained
            .iter()
            .map(|a| a.data.len() as u64)
            .sum::<u64>();
        for path in &self.attachments {
            let metadata = std::fs::metadata(path)?;
            if !metadata.is_file() {
                return Err(Error::Invalid("Attachments must be regular files".into()));
            }
            attachment_bytes = attachment_bytes.saturating_add(metadata.len());
        }
        if attachment_bytes > MAX_ATTACHMENT_BYTES {
            return Err(Error::Invalid("Attachments exceed the 16 MiB in-memory safety limit. Use a Drive link for large files.".into()));
        }
        let mut out = format!(
            "From: {}\r\nTo: {}\r\nCc: {}\r\nBcc: {}\r\nSubject: {}\r\nDate: {}\r\nMessage-ID: {}\r\nMIME-Version: 1.0\r\n",
            self.from,
            self.to,
            self.cc,
            self.bcc,
            encoded(&self.subject),
            chrono::Utc::now().to_rfc2822(),
            self.message_id
        );
        if !self.reply_to.is_empty() {
            out.push_str(&format!(
                "In-Reply-To: {}\r\nReferences: {}\r\n",
                self.reply_to, self.references
            ));
        }
        if !self.sender_reply_to.is_empty() {
            recipients(&self.sender_reply_to)?;
            out.push_str(&format!("Reply-To: {}\r\n", self.sender_reply_to));
        }
        if !self.drive_files.is_empty() {
            if self.drive_files.len() > 32 {
                return Err(Error::ResponseLimit);
            }
            let references = serde_json::to_vec(&self.drive_files)?;
            if references.len() > 8192 {
                return Err(Error::ResponseLimit);
            }
            let encoded = URL_SAFE_NO_PAD.encode(references);
            out.push_str("X-Luma-Drive-Attachments: ");
            for (index, chunk) in encoded.as_bytes().chunks(76).enumerate() {
                if index > 0 {
                    out.push('\t');
                }
                out.push_str(std::str::from_utf8(chunk).expect("base64 ASCII"));
                out.push_str("\r\n");
            }
        }
        let boundary = format!("luma_{}", uuid::Uuid::new_v4());
        let multipart = !self.attachments.is_empty() || !self.retained.is_empty();
        if multipart {
            out.push_str(&format!(
                "Content-Type: multipart/mixed; boundary=\"{boundary}\"\r\n\r\n--{boundary}\r\n"
            ));
        }
        if self.html {
            let alternative = format!("luma_alt_{}", uuid::Uuid::new_v4());
            out.push_str(&format!("Content-Type: multipart/alternative; boundary=\"{alternative}\"\r\n\r\n--{alternative}\r\nContent-Type: text/plain; charset=UTF-8\r\nContent-Transfer-Encoding: base64\r\n\r\n"));
            wrapped_base64(html_text(&self.text)?.as_bytes(), &mut out);
            out.push_str(&format!("--{alternative}\r\nContent-Type: text/html; charset=UTF-8\r\nContent-Transfer-Encoding: base64\r\n\r\n"));
            wrapped_base64(self.text.as_bytes(), &mut out);
            out.push_str(&format!("--{alternative}--\r\n"));
        } else {
            out.push_str("Content-Type: text/plain; charset=UTF-8\r\nContent-Transfer-Encoding: base64\r\n\r\n");
            wrapped_base64(self.text.as_bytes(), &mut out);
        }
        for a in &self.retained {
            clean_header(&a.name)?;
            clean_header(&a.mime_type)?;
            if !a
                .mime_type
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b"/.-_+".contains(&b))
            {
                return Err(Error::Invalid("Invalid attachment content type".into()));
            }
            let name = url::form_urlencoded::byte_serialize(a.name.as_bytes())
                .collect::<String>()
                .replace('+', "%20");
            let disposition = if a.content_id.is_some() {
                "inline"
            } else {
                "attachment"
            };
            out.push_str(&format!("--{boundary}\r\nContent-Type: {}\r\nContent-Disposition: {disposition}; filename*=UTF-8''{name}\r\n",a.mime_type));
            if let Some(cid) = &a.content_id {
                clean_header(cid)?;
                out.push_str(&format!("Content-ID: {}\r\n", cid));
            }
            out.push_str("Content-Transfer-Encoding: base64\r\n\r\n");
            wrapped_base64(&a.data, &mut out);
        }
        for path in &self.attachments {
            let filename = path.file_name().unwrap_or_default().to_string_lossy();
            // RFC 2231 avoids quotes, path separators and Unicode header injection.
            let filename: String = url::form_urlencoded::byte_serialize(filename.as_bytes())
                .collect::<String>()
                .replace('+', "%20");
            out.push_str(&format!("--{boundary}\r\nContent-Type: application/octet-stream\r\nContent-Disposition: attachment; filename*=UTF-8''{filename}\r\nContent-Transfer-Encoding: base64\r\n\r\n"));
            // Read only the preflight-approved size; reject file growth during compose.
            use std::io::Read;
            let mut bytes = Vec::new();
            std::fs::File::open(path)?
                .take(MAX_ATTACHMENT_BYTES + 1)
                .read_to_end(&mut bytes)?;
            if bytes.len() as u64 > MAX_ATTACHMENT_BYTES {
                return Err(Error::Invalid(
                    "Attachment grew beyond its safety limit".into(),
                ));
            }
            wrapped_base64(&bytes, &mut out);
            if out.len() > MAX_RAW_SIZE {
                return Err(Error::Invalid(
                    "Encoded message exceeds the safe Gmail size limit".into(),
                ));
            }
        }
        if multipart {
            out.push_str(&format!("--{boundary}--\r\n"));
        }
        if out.len() > MAX_RAW_SIZE {
            return Err(Error::Invalid("Encoded message is too large".into()));
        }
        Ok(out.into_bytes())
    }
}
fn mailbox(value: &str) -> String {
    value
        .find('<')
        .and_then(|i| value.get(i + 1..))
        .unwrap_or(value)
        .trim_end_matches('>')
        .trim()
        .to_ascii_lowercase()
}

/// Return plain text only. HTML is converted without a browser engine, JS,
/// CSS, remote image fetches or local file access.
pub fn readable(part: &Part) -> Result<String> {
    let mut plain = None;
    let mut html = None;
    part.walk(&mut |p| {
        if !p.filename.is_empty() {
            return;
        }
        if p.mime_type == "text/plain" && plain.is_none() {
            plain = Some(p);
        }
        if p.mime_type == "text/html" && html.is_none() {
            html = Some(p);
        }
    });
    let Some(selected) = plain.or(html) else {
        return Ok("No text body is available.".into());
    };
    let data = URL_SAFE_NO_PAD
        .decode(
            selected
                .body
                .data
                .as_deref()
                .unwrap_or("")
                .trim_end_matches('='),
        )
        .map_err(|_| Error::Invalid("Message body has invalid encoding".into()))?;
    let content_type = selected.header("Content-Type").to_ascii_lowercase();
    let charset = content_type
        .split("charset=")
        .nth(1)
        .and_then(|s| s.split(';').next())
        .unwrap_or("utf-8")
        .trim_matches(['\'', '"', ' ']);
    let encoding =
        encoding_rs::Encoding::for_label(charset.as_bytes()).unwrap_or(encoding_rs::UTF_8);
    let (text, _, _) = encoding.decode(&data);
    if selected.mime_type == "text/html" {
        html_text(&text)
    } else {
        Ok(text.into_owned())
    }
}
pub fn html_text(html: &str) -> Result<String> {
    html2text::from_read(html.as_bytes(), 90)
        .map_err(|_| Error::Invalid("Cannot display malformed HTML email".into()))
}
