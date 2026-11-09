//! Photo preparation: decode, downscale and re-encode as JPEG so the PDF stays
//! a sensible size regardless of what users uploaded.

use image::codecs::jpeg::JpegEncoder;
use image::imageops::FilterType;
use image::{DynamicImage, GenericImageView};

use crate::BookRecipe;

/// Longest edge of embedded photos, in pixels (~ 270 dpi across a full letter page width).
pub const MAX_EDGE: u32 = 1400;
const JPEG_QUALITY: u8 = 80;

#[derive(Debug, Clone)]
pub struct PreparedImage {
    pub jpeg: Vec<u8>,
    pub width: u32,
    pub height: u32,
}

/// Decodes, downsizes and re-encodes one image. Returns `None` for data that
/// cannot be decoded (or is degenerate); such photos are simply left out.
pub fn prepare(bytes: &[u8]) -> Option<PreparedImage> {
    let img = image::load_from_memory(bytes).ok()?;
    let (w, h) = img.dimensions();
    if w < 8 || h < 8 {
        return None;
    }
    let img = if w.max(h) > MAX_EDGE {
        let scale = MAX_EDGE as f64 / w.max(h) as f64;
        let (nw, nh) = (((w as f64 * scale).round() as u32).max(1), ((h as f64 * scale).round() as u32).max(1));
        img.resize_exact(nw, nh, FilterType::Triangle)
    } else {
        img
    };
    // Flatten any transparency onto white (JPEG has no alpha).
    let rgb = match img {
        DynamicImage::ImageRgb8(rgb) => rgb,
        other if other.color().has_alpha() => {
            let rgba = other.to_rgba8();
            let mut rgb = image::RgbImage::new(rgba.width(), rgba.height());
            for (dst, src) in rgb.pixels_mut().zip(rgba.pixels()) {
                let a = src[3] as u32;
                for c in 0..3 {
                    dst[c] = ((src[c] as u32 * a + 255 * (255 - a)) / 255) as u8;
                }
            }
            rgb
        }
        other => other.to_rgb8(),
    };
    let (width, height) = rgb.dimensions();
    let mut jpeg = Vec::with_capacity((width * height / 6) as usize);
    JpegEncoder::new_with_quality(&mut jpeg, JPEG_QUALITY).encode_image(&rgb).ok()?;
    Some(PreparedImage { jpeg, width, height })
}

/// Prepares all recipe photos, in parallel across the available cores.
pub fn prepare_all(recipes: &[BookRecipe]) -> Vec<Option<PreparedImage>> {
    let jobs: Vec<(usize, &[u8])> = recipes.iter().enumerate().filter_map(|(i, r)| r.image.as_deref().map(|b| (i, b))).collect();
    let mut out: Vec<Option<PreparedImage>> = vec![None; recipes.len()];
    if jobs.is_empty() {
        return out;
    }
    let threads = std::thread::available_parallelism().map(|n| n.get()).unwrap_or(1).clamp(1, 8).min(jobs.len());
    let chunk = jobs.len().div_ceil(threads);
    let results: Vec<(usize, Option<PreparedImage>)> = std::thread::scope(|s| {
        let handles: Vec<_> =
            jobs.chunks(chunk).map(|part| s.spawn(move || part.iter().map(|&(i, b)| (i, prepare(b))).collect::<Vec<_>>())).collect();
        handles.into_iter().flat_map(|h| h.join().unwrap_or_default()).collect()
    });
    for (i, img) in results {
        if img.is_none() {
            tracing::debug!(recipe = %recipes[i].id, "skipping cookbook photo that could not be decoded");
        }
        out[i] = img;
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn downsizes_and_reencodes() {
        let img = image::RgbaImage::from_fn(2400, 1200, |x, y| image::Rgba([(x % 256) as u8, (y % 256) as u8, 90, 200]));
        let mut png = vec![];
        DynamicImage::ImageRgba8(img).write_to(&mut std::io::Cursor::new(&mut png), image::ImageFormat::Png).unwrap();
        let p = prepare(&png).unwrap();
        assert_eq!((p.width, p.height), (1400, 700));
        assert_eq!(&p.jpeg[..2], &[0xFF, 0xD8]);
        assert!(prepare(b"not an image").is_none());
    }
}
