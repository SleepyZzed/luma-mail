//! Safe image discovery and explicit, anonymous external-image requests.
use crate::{Error, Result, composer::decode_part, http::bounded_bytes, model::Part};
use base64::{Engine, engine::general_purpose::STANDARD};
use html5ever::tokenizer::{BufferQueue, StartTag, Token, TokenSink, TokenSinkResult, Tokenizer};
use std::{
    cell::RefCell,
    net::{IpAddr, SocketAddr},
    time::Duration,
};

pub fn sources(payload: &Part) -> Vec<String> {
    let mut sources = Vec::new();
    payload.walk(&mut |part| {
        if part.mime_type != "text/html"
            || !part.filename.is_empty()
            || part.body.size > 2 * 1024 * 1024
        {
            return;
        }
        if part
            .body
            .data
            .as_ref()
            .is_some_and(|s| s.len() > 3 * 1024 * 1024)
        {
            return;
        }
        if let Ok(bytes) = decode_part(part) {
            let input = BufferQueue::default();
            input.push_back(String::from_utf8_lossy(&bytes).into_owned().into());
            let tokenizer = Tokenizer::new(Images(RefCell::new(Vec::new())), Default::default());
            let _ = tokenizer.feed(&input);
            tokenizer.end();
            for source in tokenizer.sink.0.into_inner() {
                if sources.len() < 12 && !sources.contains(&source) {
                    sources.push(source);
                }
            }
        }
    });
    sources
}
struct Images(RefCell<Vec<String>>);
impl TokenSink for Images {
    type Handle = ();
    fn process_token(&self, token: Token, _: u64) -> TokenSinkResult<()> {
        if let Token::TagToken(ref tag) = token
            && tag.kind == StartTag
        {
            if tag.name == html5ever::local_name!("script") {
                return TokenSinkResult::RawData(html5ever::tokenizer::states::RawKind::ScriptData);
            }
            if matches!(tag.name.as_ref(), "style" | "textarea" | "title") {
                return TokenSinkResult::RawData(html5ever::tokenizer::states::RawKind::Rawtext);
            }
        }
        if let Token::TagToken(tag) = token
            && tag.kind == StartTag
            && tag.name == html5ever::local_name!("img")
            && self.0.borrow().len() < 12
        {
            for attribute in tag.attrs {
                if attribute.name.local == html5ever::local_name!("src")
                    && attribute.value.len() < 1024 * 1024
                {
                    self.0.borrow_mut().push(attribute.value.to_string());
                }
            }
        }
        TokenSinkResult::Continue
    }
}
pub fn embedded(source: &str) -> Result<Vec<u8>> {
    let (kind, data) = source
        .split_once(',')
        .ok_or_else(|| Error::Invalid("Invalid embedded image".into()))?;
    if !matches!(
        kind.to_ascii_lowercase().as_str(),
        "data:image/png;base64"
            | "data:image/jpeg;base64"
            | "data:image/gif;base64"
            | "data:image/webp;base64"
    ) || data.len() > 1024 * 1024
    {
        return Err(Error::Invalid("Unsupported embedded image".into()));
    }
    STANDARD
        .decode(data)
        .map_err(|_| Error::Invalid("Invalid embedded image".into()))
}
fn public_ip(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(ip) => {
            let [a, b, c, _] = ip.octets();
            !ip.is_private()
                && !ip.is_loopback()
                && !ip.is_link_local()
                && !ip.is_documentation()
                && a != 0
                && a < 224
                && !(a == 100 && (64..128).contains(&b))
                && !(a == 198 && (b == 18 || b == 19))
                && !(a == 192 && b == 0 && c == 0)
        }
        IpAddr::V6(ip) => {
            if let Some(v4) = ip.to_ipv4_mapped() {
                return public_ip(IpAddr::V4(v4));
            }
            let segments = ip.segments();
            (segments[0] & 0xe000) == 0x2000
                && segments[0] != 0x2002
                && !(segments[0] == 0x2001 && (segments[1] < 0x0200 || segments[1] == 0x0db8))
                && !(segments[0] == 0x3fff && segments[1] < 0x1000)
        }
    }
}
fn external_url(value: &str) -> Result<url::Url> {
    if value.len() > 8192 {
        return Err(Error::ResponseLimit);
    }
    let url = url::Url::parse(value)
        .map_err(|_| Error::Invalid("Invalid external image address".into()))?;
    if url.scheme() != "https"
        || !url.username().is_empty()
        || url.password().is_some()
        || url.port_or_known_default() != Some(443)
        || url.host_str().is_none()
    {
        return Err(Error::Invalid(
            "Only public HTTPS images are allowed".into(),
        ));
    }
    Ok(url)
}
/// Each redirect is revalidated; DNS answers are pinned to the checked addresses.
/// No OAuth, cookies, referrer, proxy credentials or local-network requests.
pub async fn fetch_external(value: &str) -> Result<Vec<u8>> {
    let mut url = external_url(value)?;
    for _ in 0..4 {
        let host = url
            .host_str()
            .ok_or_else(|| Error::Invalid("Missing image host".into()))?;
        let addresses: Vec<SocketAddr> = match url.host() {
            Some(url::Host::Ipv4(ip)) => vec![SocketAddr::new(IpAddr::V4(ip), 443)],
            Some(url::Host::Ipv6(ip)) => vec![SocketAddr::new(IpAddr::V6(ip), 443)],
            _ => tokio::time::timeout(Duration::from_secs(5), tokio::net::lookup_host((host, 443)))
                .await
                .map_err(|_| Error::Timeout)??
                .take(16)
                .collect(),
        };
        if addresses.is_empty() || addresses.iter().any(|a| !public_ip(a.ip())) {
            return Err(Error::Invalid(
                "Private network image addresses are blocked".into(),
            ));
        }
        let client = reqwest::Client::builder()
            .no_proxy()
            .redirect(reqwest::redirect::Policy::none())
            .resolve_to_addrs(host, &addresses)
            .connect_timeout(Duration::from_secs(8))
            .timeout(Duration::from_secs(15))
            .user_agent("LumaMail image preview")
            .build()?;
        let response = client.get(url.clone()).send().await?;
        if response.status().is_redirection() {
            let location = response
                .headers()
                .get(reqwest::header::LOCATION)
                .and_then(|s| s.to_str().ok())
                .ok_or_else(|| Error::Invalid("Invalid image redirect".into()))?;
            let next = url
                .join(location)
                .map_err(|_| Error::Invalid("Invalid image redirect".into()))?;
            url = external_url(next.as_str())?;
            continue;
        }
        if !response.status().is_success() {
            return Err(Error::Invalid(
                "The sender’s image server could not load this image".into(),
            ));
        }
        return bounded_bytes(response, 4 * 1024 * 1024).await;
    }
    Err(Error::Invalid("Too many image redirects".into()))
}
#[cfg(test)]
mod tests {
    use super::*;
    use base64::engine::general_purpose::URL_SAFE_NO_PAD;
    #[test]
    fn discovery_is_bounded_and_does_not_execute_content() {
        let mut payload = Part {
            mime_type: "text/html".into(),
            ..Default::default()
        };
        payload.body.data=Some(URL_SAFE_NO_PAD.encode(r#"<img src="cid:photo"><IMG SRC='https://example.com/a?x=1&amp;y=2'><script>alert(1)</script><img src="cid:photo">"#));
        assert_eq!(
            sources(&payload),
            vec!["cid:photo", "https://example.com/a?x=1&y=2"]
        );
        assert!(embedded("data:image/svg+xml;base64,PHN2Zz4=").is_err());
    }
    #[tokio::test]
    async fn external_images_cannot_access_local_files_or_networks() {
        for address in [
            "http://example.com/a",
            "file:///etc/passwd",
            "https://127.0.0.1/a",
            "https://[::1]/a",
            "https://[::ffff:127.0.0.1]/a",
            "https://169.254.169.254/a",
            "https://user:secret@example.com/a",
            "https://example.com:8443/a",
        ] {
            assert!(fetch_external(address).await.is_err());
        }
        for ip in [
            "10.0.0.1",
            "172.16.0.1",
            "192.168.1.1",
            "100.64.0.1",
            "198.18.0.1",
            "224.0.0.1",
            "2001:db8::1",
            "2002:7f00:1::",
            "2001:10::1",
            "2001:20::1",
            "3fff::1",
        ] {
            assert!(!public_ip(ip.parse().unwrap()));
        }
        assert!(public_ip("8.8.8.8".parse().unwrap()));
    }
}
