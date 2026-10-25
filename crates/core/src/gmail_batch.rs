use super::*;

impl Gmail {
    pub(super) async fn metadata_page(&self, ids: Vec<String>) -> Result<Vec<Thread>> {
        if ids.len() > 25 {
            return Err(Error::ResponseLimit);
        }
        for id in &ids {
            check_id(id)?;
        }
        if ids.len() < 2 {
            return self.individual_metadata(&ids).await;
        }
        let boundary = format!("luma_{}", uuid::Uuid::new_v4().simple());
        let mut body = String::new();
        for (index, id) in ids.iter().enumerate() {
            let query=url::form_urlencoded::Serializer::new(String::new())
                .append_pair("format","metadata")
                .append_pair("fields","id,messages(id,threadId,labelIds,snippet,internalDate,payload(headers(name,value)))")
                .append_pair("metadataHeaders","From").append_pair("metadataHeaders","Subject").append_pair("metadataHeaders","Message-ID").finish();
            body.push_str(&format!("--{boundary}\r\nContent-Type: application/http\r\nContent-ID: <luma-{index}>\r\n\r\nGET /gmail/v1/users/me/threads/{id}?{query} HTTP/1.1\r\n\r\n"));
        }
        body.push_str(&format!("--{boundary}--\r\n"));
        self.rate.acquire(ids.len() as u32 * 40).await;
        let response = self
            .http
            .read_batch(
                &self.batch_url,
                &self.tokens.access_token().await?,
                &boundary,
                &body,
            )
            .await?;
        let content_type = response
            .headers()
            .get(reqwest::header::CONTENT_TYPE)
            .and_then(|s| s.to_str().ok())
            .unwrap_or("")
            .to_owned();
        let bytes = bounded_bytes(response, 8 * 1024 * 1024).await?;
        let parts = parse(&content_type, &bytes, &ids)?;
        let mut threads = Vec::new();
        for (id, part) in ids.iter().zip(parts) {
            match part {
                Read::Thread(thread) => threads.push(thread),
                Read::Missing => (),
                Read::Retry => threads.push(self.thread_metadata(id).await?),
            }
        }
        Ok(threads)
    }
    async fn individual_metadata(&self, ids: &[String]) -> Result<Vec<Thread>> {
        use futures_util::FutureExt;
        let jobs: Vec<_> = ids
            .iter()
            .map(|id| {
                let id = id.clone();
                let client = self.clone();
                async move { client.thread_metadata(&id).await }.boxed()
            })
            .collect();
        let items: Vec<Result<Thread>> = stream::iter(jobs).buffer_unordered(4).collect().await;
        items.into_iter().collect()
    }
}
enum Read {
    Thread(Thread),
    Missing,
    Retry,
}
fn invalid() -> Error {
    Error::Invalid("Gmail returned an incomplete metadata batch; refresh to retry".into())
}
fn parse(content_type: &str, bytes: &[u8], ids: &[String]) -> Result<Vec<Read>> {
    if !content_type
        .to_ascii_lowercase()
        .starts_with("multipart/mixed")
    {
        return Err(invalid());
    }
    let boundary = content_type
        .split(';')
        .skip(1)
        .filter_map(|s| s.trim().split_once('='))
        .find(|(k, _)| k.eq_ignore_ascii_case("boundary"))
        .map(|(_, v)| v.trim().trim_matches('"'))
        .ok_or_else(invalid)?;
    if boundary.is_empty()
        || boundary.len() > 100
        || boundary.bytes().any(|b| !(33..127).contains(&b))
    {
        return Err(invalid());
    }
    let text = std::str::from_utf8(bytes)
        .map_err(|_| invalid())?
        .trim_start_matches("\r\n");
    let text = text
        .strip_prefix(&format!("--{boundary}"))
        .ok_or_else(invalid)?;
    let chunks: Vec<_> = text.split(&format!("\r\n--{boundary}")).collect();
    if chunks.len() != ids.len() + 1 || !chunks.last().is_some_and(|s| s.starts_with("--")) {
        return Err(invalid());
    }
    let mut out = Vec::new();
    for (index, (part, id)) in chunks[..ids.len()].iter().zip(ids).enumerate() {
        let (headers, nested) = part
            .trim_start_matches("\r\n")
            .split_once("\r\n\r\n")
            .ok_or_else(invalid)?;
        let content_id = headers
            .lines()
            .find_map(|line| {
                line.split_once(':')
                    .filter(|(k, _)| k.eq_ignore_ascii_case("Content-ID"))
            })
            .map(|(_, v)| v.trim());
        if content_id.is_some_and(|v| v != format!("<response-luma-{index}>")) {
            return Err(invalid());
        }
        let (status_line, rest) = nested.split_once("\r\n").ok_or_else(invalid)?;
        let status = status_line
            .split_whitespace()
            .nth(1)
            .and_then(|s| s.parse::<u16>().ok())
            .ok_or_else(invalid)?;
        let (_, json) = rest.split_once("\r\n\r\n").ok_or_else(invalid)?;
        out.push(match status {
            200 => {
                let thread: Thread = serde_json::from_str(json.trim_end())?;
                if thread.id != *id {
                    return Err(invalid());
                }
                Read::Thread(thread)
            }
            404 | 410 => Read::Missing,
            401 => return Err(Error::Reauthenticate),
            403 | 429 | 500..=599 => Read::Retry,
            _ => {
                return Err(Error::Api {
                    status,
                    reason: "Metadata read failed".into(),
                });
            }
        });
    }
    Ok(out)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn batch_parser_preserves_identity_and_partial_failures() {
        let bytes=b"--batch\r\nContent-Type: application/http\r\nContent-ID: <response-luma-0>\r\n\r\nHTTP/1.1 200 OK\r\nContent-Type: application/json\r\n\r\n{\"id\":\"a\",\"messages\":[]}\r\n--batch\r\nContent-Type: application/http\r\n\r\nHTTP/1.1 429 Too Many Requests\r\nContent-Type: application/json\r\n\r\n{}\r\n--batch--\r\n";
        let ids = vec!["a".into(), "b".into()];
        let result = parse("multipart/mixed; boundary=\"batch\"", bytes, &ids).unwrap();
        assert!(matches!(&result[0],Read::Thread(t) if t.id=="a"));
        assert!(matches!(result[1], Read::Retry));
        assert!(
            parse(
                "multipart/mixed; boundary=batch",
                bytes,
                &["other".into(), "b".into()]
            )
            .is_err()
        );
        assert!(
            parse(
                "multipart/mixed; boundary=batch",
                &bytes[..bytes.len() - 20],
                &ids
            )
            .is_err()
        );
    }
}
