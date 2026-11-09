//! Image normalisation for on-device storage (mirrors the server's sizes).

use std::io::Cursor;
use std::path::{Path, PathBuf};

use anyhow::Context;
use image::imageops::FilterType;
use image::{DynamicImage, ImageReader};

const MAX_PIXELS: u64 = 80_000_000;

pub struct Paths {
    pub thumb: PathBuf,
    pub medium: PathBuf,
    pub full: PathBuf,
}

fn decode(bytes: &[u8]) -> anyhow::Result<DynamicImage> {
    let reader = ImageReader::new(Cursor::new(bytes)).with_guessed_format().context("reading image")?;
    if reader.format().is_none() {
        anyhow::bail!("unsupported image format (use JPEG, PNG, WebP or GIF)");
    }
    let (w, h) = reader.into_dimensions().context("reading image dimensions")?;
    if (w as u64) * (h as u64) > MAX_PIXELS {
        anyhow::bail!("image is too large ({w}x{h})");
    }
    ImageReader::new(Cursor::new(bytes)).with_guessed_format()?.decode().context("decoding image")
}

fn encode(img: &DynamicImage, max_edge: u32, quality: u8) -> anyhow::Result<Vec<u8>> {
    let resized =
        if img.width() > max_edge || img.height() > max_edge { img.resize(max_edge, max_edge, FilterType::Lanczos3) } else { img.clone() };
    let mut out = Vec::new();
    image::codecs::jpeg::JpegEncoder::new_with_quality(&mut out, quality).encode_image(&DynamicImage::ImageRgb8(resized.to_rgb8()))?;
    Ok(out)
}

/// Re-encodes any supported image as a JPEG no larger than `max_edge`.
pub fn to_jpeg(bytes: &[u8], max_edge: u32) -> anyhow::Result<Vec<u8>> {
    encode(&decode(bytes)?, max_edge, 86)
}

pub fn write_jpeg(bytes: &[u8], path: &Path, max_edge: u32) -> anyhow::Result<()> {
    std::fs::write(path, to_jpeg(bytes, max_edge)?)?;
    Ok(())
}

/// Writes `<dir>/<id>-{thumb,medium,full}.jpg`.
pub fn process(bytes: &[u8], dir: &Path, id: &str) -> anyhow::Result<Paths> {
    let safe: String = id.chars().filter(|c| c.is_ascii_alphanumeric() || *c == '-').collect();
    if safe.is_empty() {
        anyhow::bail!("invalid image id");
    }
    let img = decode(bytes)?;
    std::fs::create_dir_all(dir)?;
    let p = |s: &str| dir.join(format!("{safe}-{s}.jpg"));
    let paths = Paths { thumb: p("thumb"), medium: p("medium"), full: p("full") };
    std::fs::write(&paths.full, encode(&img, 2400, 86)?)?;
    std::fs::write(&paths.medium, encode(&img, 960, 86)?)?;
    std::fs::write(&paths.thumb, encode(&img, 320, 78)?)?;
    Ok(paths)
}
