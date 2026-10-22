use std::io::Cursor;
use windows::Win32::System::Com::{IStream, STREAM_SEEK_SET};

pub fn same_document(first: &str, second: &str) -> bool {
    match (url::Url::parse(first), url::Url::parse(second)) {
        (Ok(mut first), Ok(mut second)) => {
            first.set_fragment(None);
            second.set_fragment(None);
            first == second
        }
        _ => first == second,
    }
}

pub fn read_png(stream: &IStream) -> Option<Vec<u8>> {
    // Some streams expose an advanced cursor or do not support Stat. Read to EOF,
    // accepting short reads, rather than assuming one Read returns the entire icon.
    let mut bytes = Vec::new();
    unsafe {
        let _ = stream.Seek(0, STREAM_SEEK_SET, None);
        loop {
            let mut buffer = [0u8; 8192];
            let mut read = 0;
            if stream
                .Read(
                    buffer.as_mut_ptr().cast(),
                    buffer.len() as u32,
                    Some(&mut read),
                )
                .is_err()
            {
                return None;
            }
            if read == 0 {
                break;
            }
            if read as usize > buffer.len() || bytes.len() + read as usize > 4 * 1024 * 1024 {
                return None;
            }
            bytes.extend_from_slice(&buffer[..read as usize]);
        }
    }
    if !bytes.starts_with(b"\x89PNG\r\n\x1a\n") {
        return None;
    }
    let mut reader = image::ImageReader::with_format(Cursor::new(&bytes), image::ImageFormat::Png);
    let mut limits = image::Limits::default();
    limits.max_image_width = Some(4096);
    limits.max_image_height = Some(4096);
    limits.max_alloc = Some(64 * 1024 * 1024);
    reader.limits(limits);
    let image = reader.decode().ok()?;
    if image.width() <= 64 && image.height() <= 64 {
        return Some(bytes);
    }
    let mut output = Cursor::new(Vec::new());
    image
        .thumbnail(64, 64)
        .write_to(&mut output, image::ImageFormat::Png)
        .ok()?;
    Some(output.into_inner())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn hash_routes_share_an_icon_but_other_documents_do_not() {
        assert!(same_document(
            "https://example.com/dashboard/#one",
            "https://example.com/dashboard/#two"
        ));
        assert!(!same_document(
            "https://example.com/dashboard/",
            "https://example.com/login/"
        ));
        assert!(!same_document(
            "https://example.com/?one",
            "https://example.com/?two"
        ));
        assert!(!same_document(
            "https://example.com/",
            "https://other.example.com/"
        ));
    }
    #[test]
    fn reads_an_advanced_stream_and_reduces_large_icons() {
        let image = image::DynamicImage::new_rgba8(1356, 1356);
        let mut png = Cursor::new(Vec::new());
        image.write_to(&mut png, image::ImageFormat::Png).unwrap();
        let bytes = png.into_inner();
        unsafe {
            let stream = windows::Win32::UI::Shell::SHCreateMemStream(Some(&bytes)).unwrap();
            stream.Seek(16, STREAM_SEEK_SET, None).unwrap();
            let icon = read_png(&stream).unwrap();
            let decoded = image::load_from_memory(&icon).unwrap();
            assert_eq!((decoded.width(), decoded.height()), (64, 64));
        }
    }
}
