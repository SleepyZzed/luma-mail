//! One bounded, ordered clipboard transfer per composer. No spool or temp file.
use base64::{Engine, engine::general_purpose::STANDARD};
use luma_core::{Error, Result, composer::MAX_ATTACHMENT_BYTES};
pub const CHUNK: usize = 256 * 1024;
pub struct Input {
    pub bytes: Vec<u8>,
    pub total: Option<usize>,
    pub limit: usize,
}
impl Input {
    pub fn new(token: &str, retained: usize) -> Result<Self> {
        if token.is_empty()
            || token.len() > 64
            || !token
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b == b'-')
        {
            return Err(Error::Invalid("Invalid clipboard transfer".into()));
        }
        let limit = (MAX_ATTACHMENT_BYTES as usize)
            .checked_sub(retained)
            .filter(|n| *n > 0)
            .ok_or(Error::ResponseLimit)?;
        Ok(Self {
            bytes: Vec::new(),
            total: None,
            limit,
        })
    }
    pub fn append(&mut self, total: usize, offset: usize, encoded: &str) -> Result<usize> {
        if total == 0
            || total > self.limit
            || offset != self.bytes.len()
            || self.total.is_some_and(|n| n != total)
            || encoded.len() > 4 * CHUNK.div_ceil(3)
        {
            return Err(Error::Invalid(
                "The clipboard transfer changed or exceeded its memory limit".into(),
            ));
        }
        let bytes = STANDARD
            .decode(encoded)
            .map_err(|_| Error::Invalid("Invalid clipboard image data".into()))?;
        if bytes.is_empty() || bytes.len() > CHUNK || bytes.len() > total.saturating_sub(offset) {
            return Err(Error::ResponseLimit);
        }
        if self.total.is_none() {
            self.bytes.reserve_exact(total);
            self.total = Some(total);
        }
        self.bytes.extend_from_slice(&bytes);
        Ok(self.bytes.len())
    }
    pub fn complete(&self) -> bool {
        self.total == Some(self.bytes.len()) && !self.bytes.is_empty()
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn ordered_chunks_cannot_replay_change_size_or_exceed_aggregate_budget() {
        let mut i = Input::new("token", 0).unwrap();
        let encoded = STANDARD.encode(b"abc");
        assert_eq!(i.append(6, 0, &encoded).unwrap(), 3);
        assert!(i.append(6, 0, &encoded).is_err());
        assert!(i.append(7, 3, &encoded).is_err());
        assert_eq!(i.append(6, 3, &encoded).unwrap(), 6);
        assert!(i.complete());
        assert_eq!(i.bytes, b"abcabc");
        let mut tight = Input::new("token", MAX_ATTACHMENT_BYTES as usize - 2).unwrap();
        assert!(tight.append(3, 0, &encoded).is_err());
        assert!(tight.bytes.is_empty());
        assert!(Input::new("token", MAX_ATTACHMENT_BYTES as usize).is_err());
    }
}
