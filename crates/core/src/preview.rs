//! Disposable previews. No extraction to disk, active content or network access.
use crate::{Error, Result};
use std::io::{Cursor, Read};

pub const MAX_PREVIEW_BYTES: usize = 24 * 1024 * 1024;
pub const AUTO_IMAGE_BYTES: u64 = 8 * 1024 * 1024;
const XML_LIMIT: u64 = 2 * 1024 * 1024;
const TEXT_LIMIT: usize = 100_000;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Image,
    Document,
    Text,
    Audio,
    Video,
    Other,
}
impl Kind {
    pub fn of(name: &str, mime: &str) -> Self {
        let extension = name.rsplit('.').next().unwrap_or("").to_ascii_lowercase();
        match extension.as_str() {
            "png" | "jpg" | "jpeg" | "gif" | "webp" => Self::Image,
            "docx" => Self::Document,
            "txt" | "csv" | "md" | "log" | "json" => Self::Text,
            "mp3" | "wav" | "ogg" | "flac" | "m4a" | "aac" => Self::Audio,
            "mp4" | "webm" | "mkv" | "mov" | "ogv" => Self::Video,
            _ if mime.starts_with("image/") && !mime.contains("svg") => Self::Image,
            _ if mime.starts_with("audio/") => Self::Audio,
            _ if mime.starts_with("video/") => Self::Video,
            _ if mime.starts_with("text/plain") || mime.starts_with("text/csv") => Self::Text,
            _ => Self::Other,
        }
    }
    pub fn icon(self) -> &'static str {
        match self {
            Self::Image => "image-x-generic-symbolic",
            Self::Document | Self::Text => "x-office-document-symbolic",
            Self::Audio => "audio-x-generic-symbolic",
            Self::Video => "video-x-generic-symbolic",
            Self::Other => "text-x-generic-symbolic",
        }
    }
    pub fn label(self) -> &'static str {
        match self {
            Self::Image => "Image",
            Self::Document => "Word document",
            Self::Text => "Text",
            Self::Audio => "Audio",
            Self::Video => "Video",
            Self::Other => "File",
        }
    }
}
pub struct Thumbnail {
    pub rgba: Vec<u8>,
    pub width: u32,
    pub height: u32,
}
pub enum Preview {
    Image(Thumbnail),
    Text(String),
    Media(Vec<u8>),
}

pub fn create(kind: Kind, bytes: Vec<u8>) -> Result<Preview> {
    if bytes.len() > MAX_PREVIEW_BYTES {
        return Err(Error::ResponseLimit);
    }
    match kind {
        Kind::Image => thumbnail(&bytes, 1280, 960).map(Preview::Image),
        Kind::Document => docx(&bytes).map(Preview::Text),
        Kind::Text => {
            let text = String::from_utf8_lossy(&bytes[..bytes.len().min(TEXT_LIMIT)]).into_owned();
            Ok(Preview::Text(format!(
                "{text}{}",
                if bytes.len() > TEXT_LIMIT {
                    "\n\n[Preview shortened]"
                } else {
                    ""
                }
            )))
        }
        Kind::Audio | Kind::Video => Ok(Preview::Media(bytes)),
        Kind::Other => Err(Error::Invalid(
            "Save this file to open it in its associated application.".into(),
        )),
    }
}
pub fn thumbnail(bytes: &[u8], width: u32, height: u32) -> Result<Thumbnail> {
    if bytes.len() > MAX_PREVIEW_BYTES {
        return Err(Error::ResponseLimit);
    }
    let invalid = || {
        Error::Invalid("This image cannot be previewed safely. Save it to open externally.".into())
    };
    let reader = image::ImageReader::new(Cursor::new(bytes))
        .with_guessed_format()
        .map_err(|_| invalid())?;
    let (w, h) = reader.into_dimensions().map_err(|_| invalid())?;
    if w == 0 || h == 0 || w > 16384 || h > 16384 || u64::from(w) * u64::from(h) > 32_000_000 {
        return Err(Error::ResponseLimit);
    }
    let mut reader = image::ImageReader::new(Cursor::new(bytes))
        .with_guessed_format()
        .map_err(|_| invalid())?;
    let mut limits = image::Limits::default();
    limits.max_image_width = Some(16384);
    limits.max_image_height = Some(16384);
    limits.max_alloc = Some(128 * 1024 * 1024);
    reader.limits(limits);
    let image = reader
        .decode()
        .map_err(|_| invalid())?
        .thumbnail(width.max(1), height.max(1))
        .to_rgba8();
    Ok(Thumbnail {
        width: image.width(),
        height: image.height(),
        rgba: image.into_raw(),
    })
}
fn docx(bytes: &[u8]) -> Result<String> {
    let invalid = |_| {
        Error::Invalid("This Word document cannot be previewed. Save it to open externally.".into())
    };
    let mut archive = zip::ZipArchive::new(Cursor::new(bytes)).map_err(invalid)?;
    if archive.len() > 4096 {
        return Err(Error::ResponseLimit);
    }
    let file = archive.by_name("word/document.xml").map_err(invalid)?;
    if file.size() > XML_LIMIT {
        return Err(Error::ResponseLimit);
    }
    let mut xml = String::new();
    file.take(XML_LIMIT + 1).read_to_string(&mut xml)?;
    if xml.len() as u64 > XML_LIMIT {
        return Err(Error::ResponseLimit);
    }
    let options = roxmltree::ParsingOptions {
        allow_dtd: false,
        nodes_limit: 100_000,
        ..Default::default()
    };
    let doc = roxmltree::Document::parse_with_options(&xml, options)
        .map_err(|_| Error::Invalid("Invalid or unsupported Word document XML".into()))?;
    let mut text = String::new();
    const WORD: &str = "http://schemas.openxmlformats.org/wordprocessingml/2006/main";
    for node in doc
        .descendants()
        .filter(|n| n.is_element() && n.tag_name().namespace() == Some(WORD))
    {
        match node.tag_name().name() {
            "t" => text.push_str(node.text().unwrap_or("")),
            "p" if !text.is_empty() => text.push('\n'),
            "tab" => text.push('\t'),
            "br" => text.push('\n'),
            _ => (),
        }
        if text.len() > TEXT_LIMIT {
            let mut end = TEXT_LIMIT;
            while !text.is_char_boundary(end) {
                end -= 1;
            }
            text.truncate(end);
            text.push_str("\n\n[Preview shortened]");
            break;
        }
    }
    Ok(text)
}
pub fn size(bytes: u64) -> String {
    if bytes >= 1024 * 1024 {
        format!("{:.1} MB", bytes as f64 / (1024. * 1024.))
    } else if bytes >= 1024 {
        format!("{:.0} KB", bytes as f64 / 1024.)
    } else {
        format!("{bytes} B")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    fn word(xml: &str) -> Vec<u8> {
        let mut archive = zip::ZipWriter::new(Cursor::new(Vec::new()));
        archive
            .start_file(
                "word/document.xml",
                zip::write::SimpleFileOptions::default(),
            )
            .unwrap();
        archive.write_all(xml.as_bytes()).unwrap();
        archive.finish().unwrap().into_inner()
    }
    #[test]
    fn word_text_ignores_relationships_and_active_content() {
        let data = word(
            r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:p><w:r><w:t>Fish &amp; chips</w:t></w:r></w:p><w:p><w:r><w:t>Hello</w:t></w:r></w:p></w:body></w:document>"#,
        );
        assert_eq!(docx(&data).unwrap(), "Fish & chips\nHello");
        assert!(
            docx(&word(
                "<!DOCTYPE a [<!ENTITY x SYSTEM 'file:///private'>]><a>&x;</a>"
            ))
            .is_err()
        );
        assert!(docx(&word(&"x".repeat(XML_LIMIT as usize + 1))).is_err());
    }
    #[test]
    fn image_is_scaled_and_bad_data_rejected() {
        let source = image::RgbaImage::new(3000, 2000);
        let mut encoded = Cursor::new(Vec::new());
        source
            .write_to(&mut encoded, image::ImageFormat::Png)
            .unwrap();
        let out = thumbnail(encoded.get_ref(), 320, 200).unwrap();
        assert!(out.width <= 320 && out.height <= 200);
        assert_eq!(out.rgba.len(), out.width as usize * out.height as usize * 4);
        assert!(thumbnail(b"not an image", 320, 200).is_err());
        assert_eq!(
            Kind::of("photo.JPG", "application/octet-stream"),
            Kind::Image
        );
        assert_eq!(Kind::of("payload.svg", "image/svg+xml"), Kind::Other);
    }
}
