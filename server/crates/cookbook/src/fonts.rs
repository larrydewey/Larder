//! Font loading: the bundled OFL theme fonts plus Typst's own default fonts
//! (Libertinus Serif, New Computer Modern, DejaVu Sans Mono) as fallbacks.
//! Additional fonts (e.g. CJK coverage) can be supplied at runtime through the
//! `LARDER_COOKBOOK_FONT_DIRS` environment variable (a `:`-separated list of
//! directories, scanned non-recursively for .ttf/.otf/.ttc files).

use std::sync::LazyLock;

use typst::foundations::Bytes;
use typst::text::{Font, FontBook};
use typst::utils::LazyHash;

static BUNDLED: &[&[u8]] = &[
    include_bytes!("../fonts/EBGaramond.ttf"),
    include_bytes!("../fonts/EBGaramond-Italic.ttf"),
    include_bytes!("../fonts/DMSerifDisplay-Regular.ttf"),
    include_bytes!("../fonts/DMSerifDisplay-Italic.ttf"),
    include_bytes!("../fonts/SourceSans3.ttf"),
    include_bytes!("../fonts/SourceSans3-Italic.ttf"),
    include_bytes!("../fonts/Caveat.ttf"),
    include_bytes!("../fonts/Lora.ttf"),
    include_bytes!("../fonts/Lora-Italic.ttf"),
];

pub struct FontSet {
    pub book: LazyHash<FontBook>,
    pub fonts: Vec<Font>,
}

pub static FONTS: LazyLock<FontSet> = LazyLock::new(|| {
    let mut fonts: Vec<Font> = vec![];
    for data in BUNDLED.iter().copied().chain(typst_assets::fonts()) {
        fonts.extend(Font::iter(Bytes::new(data)));
    }
    if let Ok(dirs) = std::env::var("LARDER_COOKBOOK_FONT_DIRS") {
        for dir in dirs.split(':').filter(|d| !d.trim().is_empty()) {
            let Ok(entries) = std::fs::read_dir(dir.trim()) else { continue };
            for entry in entries.flatten() {
                let path = entry.path();
                let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("").to_ascii_lowercase();
                if !matches!(ext.as_str(), "ttf" | "otf" | "ttc" | "otc") {
                    continue;
                }
                if let Ok(data) = std::fs::read(&path) {
                    let before = fonts.len();
                    fonts.extend(Font::iter(Bytes::new(data)));
                    tracing::debug!(path = %path.display(), faces = fonts.len() - before, "loaded extra cookbook font");
                }
            }
        }
    }
    let book = LazyHash::new(FontBook::from_fonts(&fonts));
    FontSet { book, fonts }
});
