//! On-disk image storage. Every upload is decoded, normalised to JPEG and
//! stored in three sizes under `<data>/images/<xx>/<id>/{thumb,medium,full}.jpg`.

use std::io::Cursor;
use std::path::PathBuf;

use anyhow::Context;
use image::imageops::FilterType;
use image::{DynamicImage, ImageReader};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ImageSize {
    Thumb,
    Medium,
    Full,
}

impl ImageSize {
    pub fn parse(s: Option<&str>) -> Self {
        match s {
            Some("thumb") | Some("small") => Self::Thumb,
            Some("full") | Some("original") | Some("large") => Self::Full,
            _ => Self::Medium,
        }
    }
    fn name(&self) -> &'static str {
        match self {
            Self::Thumb => "thumb",
            Self::Medium => "medium",
            Self::Full => "full",
        }
    }
    fn max_edge(&self) -> u32 {
        match self {
            Self::Thumb => 320,
            Self::Medium => 960,
            Self::Full => 2400,
        }
    }
}

#[derive(Clone)]
pub struct ImageStore {
    root: PathBuf,
}

const MAX_PIXELS: u64 = 80_000_000;

impl ImageStore {
    pub fn new(root: PathBuf) -> anyhow::Result<Self> {
        std::fs::create_dir_all(&root).with_context(|| format!("creating {}", root.display()))?;
        Ok(Self { root })
    }

    fn dir(&self, id: &str) -> Option<PathBuf> {
        // ids are UUIDs; reject anything else to avoid path traversal
        let uuid = uuid::Uuid::parse_str(id).ok()?;
        let s = uuid.to_string();
        Some(self.root.join(&s[..2]).join(s))
    }

    pub fn path(&self, id: &str, size: ImageSize) -> Option<PathBuf> {
        self.dir(id).map(|d| d.join(format!("{}.jpg", size.name())))
    }

    /// Decodes and stores an image, returning its new id. CPU heavy: runs on a blocking thread.
    pub async fn save(&self, bytes: Vec<u8>) -> anyhow::Result<String> {
        let store = self.clone();
        tokio::task::spawn_blocking(move || store.save_blocking(&bytes)).await?
    }

    pub fn save_blocking(&self, bytes: &[u8]) -> anyhow::Result<String> {
        let img = decode(bytes)?;
        let id = uuid::Uuid::new_v4().to_string();
        let dir = self.dir(&id).expect("valid uuid");
        std::fs::create_dir_all(&dir)?;
        for size in [ImageSize::Full, ImageSize::Medium, ImageSize::Thumb] {
            let max = size.max_edge();
            let resized = if img.width() > max || img.height() > max { img.resize(max, max, FilterType::Lanczos3) } else { img.clone() };
            let rgb = DynamicImage::ImageRgb8(resized.to_rgb8());
            let mut out = Vec::new();
            let quality = if size == ImageSize::Thumb { 78 } else { 86 };
            let mut enc = image::codecs::jpeg::JpegEncoder::new_with_quality(&mut out, quality);
            enc.encode_image(&rgb).context("encoding jpeg")?;
            std::fs::write(dir.join(format!("{}.jpg", size.name())), out)?;
        }
        Ok(id)
    }

    pub async fn read(&self, id: &str, size: ImageSize) -> Option<Vec<u8>> {
        let p = self.path(id, size)?;
        tokio::fs::read(p).await.ok()
    }

    pub async fn delete(&self, id: &str) {
        if let Some(d) = self.dir(id) {
            let _ = tokio::fs::remove_dir_all(d).await;
        }
    }
}

pub fn decode(bytes: &[u8]) -> anyhow::Result<DynamicImage> {
    let reader = ImageReader::new(Cursor::new(bytes)).with_guessed_format().context("reading image")?;
    if reader.format().is_none() {
        anyhow::bail!("unsupported image format (use JPEG, PNG, WebP or GIF)");
    }
    let (w, h) = reader.into_dimensions().context("reading image dimensions")?;
    if (w as u64) * (h as u64) > MAX_PIXELS {
        anyhow::bail!("image is too large ({w}x{h})");
    }
    let reader = ImageReader::new(Cursor::new(bytes)).with_guessed_format()?;
    reader.decode().context("decoding image")
}

/// Public URL for an image id.
pub fn image_url(id: &Option<String>) -> Option<String> {
    id.as_ref().map(|i| format!("/api/images/{i}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn save_and_read() {
        let dir = tempfile::tempdir().unwrap();
        let store = ImageStore::new(dir.path().to_path_buf()).unwrap();
        let img = image::RgbImage::from_fn(1200, 800, |x, y| image::Rgb([(x % 255) as u8, (y % 255) as u8, 128]));
        let mut png = Vec::new();
        DynamicImage::ImageRgb8(img).write_to(&mut Cursor::new(&mut png), image::ImageFormat::Png).unwrap();
        let id = store.save_blocking(&png).unwrap();
        let thumb = std::fs::read(store.path(&id, ImageSize::Thumb).unwrap()).unwrap();
        let decoded = image::load_from_memory(&thumb).unwrap();
        assert_eq!(decoded.width(), 320);
        assert!(store.path("../../etc/passwd", ImageSize::Full).is_none());
        assert!(store.save_blocking(b"not an image").is_err());
    }
}
