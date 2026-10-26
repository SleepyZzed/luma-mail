//! A pasted raster image becomes an in-memory MIME attachment, never a temp file.
use crate::{Error, Result, composer::RetainedAttachment};

pub fn attachment(data: Vec<u8>, number: usize) -> Result<RetainedAttachment> {
    if data.len() as u64 > crate::composer::MAX_ATTACHMENT_BYTES {
        return Err(Error::ResponseLimit);
    }
    let (extension, mime) = match image::guess_format(&data) {
        Ok(image::ImageFormat::Png) => ("png", "image/png"),
        Ok(image::ImageFormat::Jpeg) => ("jpg", "image/jpeg"),
        Ok(image::ImageFormat::Gif) => ("gif", "image/gif"),
        Ok(image::ImageFormat::WebP) => ("webp", "image/webp"),
        _ => {
            return Err(Error::Invalid(
                "Paste a PNG, JPEG, GIF or WebP image.".into(),
            ));
        }
    };
    // The shared decoder enforces dimensions, pixel count and allocation limits.
    crate::preview::thumbnail(&data, 160, 120)?;
    Ok(RetainedAttachment {
        name: format!("pasted-image-{number}.{extension}"),
        mime_type: mime.into(),
        data: data.into(),
        content_id: None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn raster_is_bounded_and_keeps_original_bytes_without_an_inline_content_id() {
        let mut png = std::io::Cursor::new(Vec::new());
        image::DynamicImage::new_rgb8(2, 2)
            .write_to(&mut png, image::ImageFormat::Png)
            .unwrap();
        let bytes = png.into_inner();
        let pasted = attachment(bytes.clone(), 2).unwrap();
        assert_eq!(pasted.data.as_slice(), bytes.as_slice());
        assert_eq!(pasted.name, "pasted-image-2.png");
        assert!(pasted.content_id.is_none());
        assert!(attachment(b"<svg onload='alert(1)'/>".to_vec(), 1).is_err());
        assert!(
            attachment(
                vec![0; crate::composer::MAX_ATTACHMENT_BYTES as usize + 1],
                1
            )
            .is_err()
        );
    }
}
