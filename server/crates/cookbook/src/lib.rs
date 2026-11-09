//! Printable cookbook rendering for Larder.
//!
//! A set of recipes is turned into a traditionally typeset, book-like PDF
//! (title page, dedication, table of contents, chapter openers, one recipe per
//! page, running heads, folios and an index) using [Typst] embedded as a
//! library.
//!
//! User text is never interpolated into Typst markup: the book content is
//! serialised to JSON and served to the template as the virtual file
//! `/data.json`, images are served as `/img/<n>.jpg`. Strings coming from JSON
//! are always rendered literally by Typst.
//!
//! [Typst]: https://typst.app

mod book;
mod fonts;
mod images;
mod text;
mod world;

use std::time::Instant;

use serde::{Deserialize, Serialize};

pub use book::group_recipes;

/// Options controlling the generated cookbook (the body of `POST /api/export/cookbook`).
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(default)]
pub struct CookbookOptions {
    pub title: String,
    pub subtitle: Option<String>,
    pub author: Option<String>,
    pub dedication: Option<String>,
    /// `"classic"`, `"modern"` or `"rustic"`; anything else falls back to classic.
    pub theme: String,
    /// `"letter"`, `"a4"` or `"a5"`; anything else falls back to letter.
    pub paper: String,
    pub group_by: GroupBy,
    /// Chapters listed here come first, in this order (case-insensitive);
    /// the remaining chapters follow alphabetically.
    pub chapter_order: Vec<String>,
    pub include_images: bool,
    pub include_toc: bool,
    pub include_index: bool,
    pub include_notes: bool,
    pub include_nutrition: bool,
    pub units: larder_core::units::UnitSystem,
}

impl Default for CookbookOptions {
    fn default() -> Self {
        Self {
            title: "Our Family Cookbook".into(),
            subtitle: None,
            author: None,
            dedication: None,
            theme: "classic".into(),
            paper: "letter".into(),
            group_by: GroupBy::Category,
            chapter_order: vec![],
            include_images: true,
            include_toc: true,
            include_index: true,
            include_notes: true,
            include_nutrition: false,
            units: larder_core::units::UnitSystem::Original,
        }
    }
}

/// How recipes are split into chapters.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum GroupBy {
    /// One chapter per (first) category.
    #[default]
    Category,
    /// One chapter per (first) tag.
    Tag,
    /// A single "Recipes" chapter.
    None,
}

/// A recipe to be printed, with its optional photo (JPEG/PNG/WebP/GIF bytes, any size).
#[derive(Debug, Clone)]
pub struct BookRecipe {
    pub id: String,
    pub data: larder_core::RecipeData,
    pub image: Option<Vec<u8>>,
}

/// A selectable cookbook theme (`GET /api/export/cookbook/themes`).
#[derive(Debug, Clone, Serialize)]
pub struct ThemeInfo {
    pub id: &'static str,
    pub name: &'static str,
    pub description: &'static str,
}

/// The available themes. The first one is the default.
pub fn themes() -> Vec<ThemeInfo> {
    vec![
        ThemeInfo {
            id: "classic",
            name: "Classic",
            description: "A traditional heirloom cookbook: EB Garamond, centred headings, small capitals, \
                          fleurons and fine rules with a burgundy accent.",
        },
        ThemeInfo {
            id: "modern",
            name: "Modern",
            description: "Clean and airy: DM Serif Display headings over Source Sans text, left-aligned \
                          layout, generous whitespace, bold terracotta accents and edge-to-edge photos.",
        },
        ThemeInfo {
            id: "rustic",
            name: "Rustic",
            description: "Farmhouse kitchen warmth: hand-lettered Caveat headings, Lora text, a sepia and \
                          brown palette, double rules and framed snapshot-style photos.",
        },
    ]
}

/// Normalised theme id (unknown themes fall back to `"classic"`).
pub(crate) fn theme_id(theme: &str) -> &'static str {
    match theme.trim().to_ascii_lowercase().as_str() {
        "modern" => "modern",
        "rustic" => "rustic",
        _ => "classic",
    }
}

/// Normalised paper id (unknown sizes fall back to `"letter"`).
pub(crate) fn paper_id(paper: &str) -> &'static str {
    match paper.trim().to_ascii_lowercase().as_str() {
        "a4" => "a4",
        "a5" => "a5",
        _ => "letter",
    }
}

/// Builds the JSON document the Typst template consumes (useful for debugging).
pub fn render_data_json(opts: &CookbookOptions, recipes: &[BookRecipe]) -> String {
    let images = vec![None; recipes.len()];
    let data = book::build(opts, recipes, &images);
    serde_json::to_string_pretty(&data).unwrap_or_default()
}

/// The Typst source of the main template for the given options (for debugging).
/// The book content itself lives in `/data.json` (see [`render_data_json`]).
pub fn render_typst_source(_opts: &CookbookOptions) -> String {
    world::MAIN_TEMPLATE.to_string()
}

/// Renders the cookbook to PDF bytes.
///
/// This is CPU-bound (image re-encoding plus typesetting) and may take a few
/// seconds for large books, so call it from a blocking context.
pub fn render_pdf(opts: &CookbookOptions, recipes: Vec<BookRecipe>) -> anyhow::Result<Vec<u8>> {
    let started = Instant::now();
    let prepared = if opts.include_images { images::prepare_all(&recipes) } else { vec![None; recipes.len()] };
    let image_time = started.elapsed();

    let data = book::build(opts, &recipes, &prepared);
    let json = serde_json::to_vec(&data)?;
    let image_files: Vec<(String, Vec<u8>)> =
        prepared.into_iter().enumerate().filter_map(|(i, img)| img.map(|img| (book::image_path(i), img.jpeg))).collect();

    let world = world::BookWorld::new(json, image_files);
    let pdf = world.compile_pdf(&opts.title)?;
    tracing::info!(
        recipes = recipes.len(),
        images_ms = image_time.as_millis() as u64,
        total_ms = started.elapsed().as_millis() as u64,
        bytes = pdf.len(),
        "rendered cookbook PDF"
    );
    Ok(pdf)
}
