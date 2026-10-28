//! RFC 6068 compose input. It never reads attachments, sends, or creates a draft.
use crate::{Error, Result, composer};
#[derive(Default, Debug, PartialEq, Eq)]
pub struct Mailto {
    pub to: String,
    pub cc: String,
    pub bcc: String,
    pub subject: String,
    pub body: String,
}
fn decode(value: &str) -> Result<String> {
    let mut output = Vec::with_capacity(value.len());
    let mut input = value.as_bytes().iter().copied();
    while let Some(byte) = input.next() {
        if byte == b'%' {
            let hex = |b: u8| (b as char).to_digit(16).map(|v| v as u8);
            let a = input.next().and_then(hex);
            let b = input.next().and_then(hex);
            output.push(
                a.zip(b).map(|(a, b)| a * 16 + b).ok_or_else(|| {
                    Error::Invalid("Invalid percent encoding in email link".into())
                })?,
            );
        } else {
            output.push(byte);
        }
    }
    String::from_utf8(output)
        .map_err(|_| Error::Invalid("The email link is not valid UTF-8".into()))
}
fn recipients(target: &mut String, value: &str) -> Result<()> {
    if value.is_empty() {
        return Ok(());
    }
    if value.len() + target.len() > 16384 || value.chars().any(char::is_control) {
        return Err(Error::Invalid(
            "Invalid recipient header in email link".into(),
        ));
    }
    composer::recipients(value)?;
    if !target.is_empty() {
        target.push_str(", ");
    }
    target.push_str(value);
    Ok(())
}
pub fn parse(uri: &str) -> Result<Mailto> {
    if uri.len() > 65536 {
        return Err(Error::ResponseLimit);
    }
    let url = url::Url::parse(uri).map_err(|_| Error::Invalid("Invalid email link".into()))?;
    if url.scheme() != "mailto"
        || url.has_host()
        || url.fragment().is_some()
        || uri.chars().any(char::is_control)
    {
        return Err(Error::Invalid("Invalid mailto link".into()));
    }
    let mut mail = Mailto::default();
    recipients(&mut mail.to, &decode(url.path())?)?;
    let mut subject = false;
    let mut body = false;
    for part in url
        .query()
        .unwrap_or("")
        .split('&')
        .filter(|p| !p.is_empty())
    {
        let (key, value) = part.split_once('=').unwrap_or((part, ""));
        let key = decode(key)?.to_ascii_lowercase();
        let value = decode(value)?;
        match key.as_str() {
            "to" => recipients(&mut mail.to, &value)?,
            "cc" => recipients(&mut mail.cc, &value)?,
            "bcc" => recipients(&mut mail.bcc, &value)?,
            "subject" => {
                if subject || value.len() > 4096 || value.chars().any(char::is_control) {
                    return Err(Error::Invalid("Invalid subject in email link".into()));
                }
                subject = true;
                mail.subject = value;
            }
            "body" => {
                if body || value.contains('\0') {
                    return Err(Error::Invalid("Invalid body in email link".into()));
                }
                body = true;
                mail.body = value;
            }
            // In particular: never accept attachment/file paths, From, token,
            // Reply-To, send, or any other sender-controlled instruction.
            _ => (),
        }
    }
    Ok(mail)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn decodes_unicode_and_preserves_plus_tags_and_body_line_breaks() {
        let mail=parse("mailto:alex+tag%40example.com?cc=Jamie%20%3Cjamie%40example.com%3E&subject=Tea+plans%20%E2%98%95&body=Hi%0D%0Athere").unwrap();
        assert_eq!(mail.to, "alex+tag@example.com");
        assert_eq!(mail.subject, "Tea+plans ☕");
        assert_eq!(mail.body, "Hi\r\nthere");
        assert_eq!(mail.cc, "Jamie <jamie@example.com>");
    }
    #[test]
    fn rejects_header_injection_and_never_imports_attachment_or_from_instructions() {
        assert!(parse("mailto:a@example.com?subject=x%0ABcc:other@example.com").is_err());
        assert!(parse("mailto:a%0D%0ABcc:x@example.com").is_err());
        assert!(parse("mailto:a@example.com?body=%GG").is_err());
        assert!(parse("mailto:a@example.com?subject=a&subject=b").is_err());
        assert!(parse("mailto://host/a@example.com").is_err());
        assert_eq!(
            parse("mailto:a@example.com?attach=file:///private&from=other@example.com&send=true")
                .unwrap(),
            Mailto {
                to: "a@example.com".into(),
                ..Default::default()
            }
        );
    }
}
