//! Recipe importers for Larder.
//!
//! * [`parse_file`] — export files from Mealie, Tandoor, Paprika, Nextcloud
//!   Cookbook, Recipe Keeper, Larder itself and plain schema.org JSON-LD.
//! * [`parse_html`] / [`scrape_url`] — recipe web pages (JSON-LD, microdata).
//! * [`text::parse_plain_text`] — heuristic plain-text parser.
//! * [`mealie_api::MealieClient`] — live import from a Mealie server.
//! * [`larder::write_export`] — Larder's own round-trippable export.

mod archive;
mod detect;
mod html;
pub mod larder;
pub mod mealie;
pub mod mealie_api;
mod net;
mod nextcloud;
mod paprika;
mod recipekeeper;
pub mod schema_org;
mod tandoor;
pub mod text;
mod util;

pub use html::parse_html;
pub use larder_core::{RecipeData, RecipeStatus, TrialState};
pub use net::{fetch_image, scrape_url, USER_AGENT};
pub use util::image_extension;

/// Where a recipe's image comes from.
#[derive(Debug, Clone)]
pub enum ImageSource {
    /// Image bytes that were embedded in the import file.
    Bytes(Vec<u8>),
    /// An absolute http(s) URL that still has to be downloaded (see [`fetch_image`]).
    Url(String),
}

/// One recipe produced by an importer.
#[derive(Debug, Clone)]
pub struct ImportedRecipe {
    /// Recipe content, already normalised.
    pub data: RecipeData,
    pub image: Option<ImageSource>,
    /// RFC3339 creation time, if the source records one.
    pub created_at: Option<String>,
    /// Whether the source marks the recipe as a favourite.
    pub favorite: bool,
    /// Status and trial counters, when the source records them (Larder
    /// archives). `None`: the importer's default applies, see [`ImportedRecipe::trial_state`].
    pub trial: Option<TrialState>,
}

impl ImportedRecipe {
    /// The trial state to save the recipe with. `status` (the user's choice
    /// on the import screen) wins; otherwise what the source recorded, and
    /// failing that `default` (trial for single-recipe imports, cookbook for
    /// bulk imports).
    pub fn trial_state(&self, status: Option<RecipeStatus>, default: RecipeStatus) -> TrialState {
        let mut t = self.trial.clone().unwrap_or_else(|| TrialState::new(default));
        if let Some(s) = status {
            t.status = s;
        }
        t.normalize();
        t
    }
}

/// Import file formats.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Source {
    Auto,
    Mealie,
    Tandoor,
    Paprika,
    Nextcloud,
    RecipeKeeper,
    Larder,
    JsonLd,
}

impl Source {
    pub const ALL: [Source; 8] = [
        Source::Auto,
        Source::Mealie,
        Source::Tandoor,
        Source::Paprika,
        Source::Nextcloud,
        Source::RecipeKeeper,
        Source::Larder,
        Source::JsonLd,
    ];

    /// The serde / API name.
    pub fn as_str(&self) -> &'static str {
        match self {
            Source::Auto => "auto",
            Source::Mealie => "mealie",
            Source::Tandoor => "tandoor",
            Source::Paprika => "paprika",
            Source::Nextcloud => "nextcloud",
            Source::RecipeKeeper => "recipekeeper",
            Source::Larder => "larder",
            Source::JsonLd => "jsonld",
        }
    }

    /// Parses an API name (case-insensitive; a few aliases accepted).
    pub fn parse(s: &str) -> Option<Source> {
        let s: String = s.trim().to_ascii_lowercase().chars().filter(|c| !matches!(c, ' ' | '-' | '_')).collect();
        Some(match s.as_str() {
            "" | "auto" => Source::Auto,
            "mealie" => Source::Mealie,
            "tandoor" => Source::Tandoor,
            "paprika" => Source::Paprika,
            "nextcloud" | "nextcloudcookbook" => Source::Nextcloud,
            "recipekeeper" => Source::RecipeKeeper,
            "larder" | "recipers" => Source::Larder,
            "jsonld" | "json" | "schemaorg" => Source::JsonLd,
            _ => return None,
        })
    }

    /// Human readable name, used as `source_name` fallback.
    pub fn label(&self) -> &'static str {
        match self {
            Source::Auto => "Import",
            Source::Mealie => "Mealie",
            Source::Tandoor => "Tandoor",
            Source::Paprika => "Paprika",
            Source::Nextcloud => "Nextcloud Cookbook",
            Source::RecipeKeeper => "Recipe Keeper",
            Source::Larder => "Larder",
            Source::JsonLd => "JSON-LD",
        }
    }
}

impl std::fmt::Display for Source {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Result of parsing an export file.
#[derive(Debug, Clone)]
pub struct ImportBatch {
    /// The detected (or requested) format.
    pub source: Source,
    pub recipes: Vec<ImportedRecipe>,
    /// Per-recipe problems, e.g. `"Recipe 'X': missing name"`.
    pub errors: Vec<String>,
}

#[derive(Debug, thiserror::Error)]
pub enum ImportError {
    #[error("unrecognised import file: {0}")]
    UnsupportedFormat(String),
    #[error("invalid file: {0}")]
    InvalidFormat(String),
    #[error("invalid archive: {0}")]
    InvalidArchive(String),
    #[error("too large: {0}")]
    TooLarge(String),
    #[error("no recipe found")]
    NoRecipeFound,
    #[error("invalid URL: {0}")]
    InvalidUrl(String),
    #[error("request failed: {0}")]
    Network(String),
    #[error("HTTP {status} from {url}")]
    Http { status: u16, url: String },
    #[error("unauthorized: the server rejected the API token")]
    Unauthorized,
    #[error("not an image: {0}")]
    NotAnImage(String),
}

impl From<reqwest::Error> for ImportError {
    fn from(e: reqwest::Error) -> Self {
        if e.is_timeout() {
            ImportError::Network("timed out".into())
        } else {
            // Without the URL: it may carry credentials or tokens in query strings.
            ImportError::Network(e.without_url().to_string())
        }
    }
}

impl From<serde_json::Error> for ImportError {
    fn from(e: serde_json::Error) -> Self {
        ImportError::InvalidFormat(format!("invalid JSON: {e}"))
    }
}

/// Parses an uploaded export file. With [`Source::Auto`] the format is
/// detected from the file name and contents. Per-recipe failures are
/// reported in [`ImportBatch::errors`]; only whole-file failures return `Err`.
pub fn parse_file(source: Source, filename: &str, bytes: &[u8]) -> Result<ImportBatch, ImportError> {
    detect::parse_file(source, filename, bytes)
}

/// Accumulates recipes and errors while parsing a file.
pub(crate) struct Collector {
    pub source: Source,
    pub recipes: Vec<ImportedRecipe>,
    pub errors: Vec<String>,
    /// Fill an empty `source_name` (from the source URL's host, else the app label).
    pub fill_source_name: bool,
}

impl Collector {
    pub fn new(source: Source) -> Self {
        Collector { source, recipes: vec![], errors: vec![], fill_source_name: source != Source::Larder }
    }

    /// Normalises, validates and stores a recipe.
    pub fn push(&mut self, mut r: ImportedRecipe, context: &str) {
        r.data.normalize();
        if self.fill_source_name && r.data.source_name.is_none() {
            r.data.source_name = r.data.source_url.as_deref().and_then(util::host_of).or_else(|| Some(self.source.label().to_string()));
        }
        if let Err(e) = r.data.validate() {
            let label = if r.data.name.trim().is_empty() { context } else { r.data.name.as_str() };
            self.errors.push(format!("Recipe '{}': {e}", truncate(label, 80)));
            return;
        }
        if let Some(ImageSource::Bytes(b)) = &r.image {
            if image_extension(b).is_none() {
                tracing::debug!(recipe = %r.data.name, "dropping embedded file that is not a recognised image");
                r.image = None;
            }
        }
        self.recipes.push(r);
    }

    pub fn error(&mut self, context: &str, e: impl std::fmt::Display) {
        self.errors.push(format!("Recipe '{}': {e}", truncate(context, 80)));
    }

    pub fn finish(self) -> ImportBatch {
        ImportBatch { source: self.source, recipes: self.recipes, errors: self.errors }
    }
}

pub(crate) fn truncate(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        s.to_string()
    } else {
        let mut t: String = s.chars().take(max).collect();
        t.push('…');
        t
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn source_names() {
        for s in Source::ALL {
            assert_eq!(Source::parse(s.as_str()), Some(s));
            let json = serde_json::to_string(&s).unwrap();
            assert_eq!(json, format!("\"{}\"", s.as_str()));
            assert_eq!(serde_json::from_str::<Source>(&json).unwrap(), s);
        }
        assert_eq!(Source::parse("Recipe Keeper"), Some(Source::RecipeKeeper));
        assert_eq!(Source::parse("nope"), None);
        assert_eq!(Source::Tandoor.label(), "Tandoor");
    }

    #[test]
    fn error_is_send_sync() {
        fn assert_bounds<T: Send + Sync + 'static + std::fmt::Display>() {}
        assert_bounds::<ImportError>();
    }
}

/// Smoke test against real export files: set `LARDER_IMPORT_SAMPLES` to a
/// directory of exports and run with `--ignored --nocapture`.
#[cfg(test)]
mod sample_files {
    #[test]
    #[ignore]
    fn parse_sample_directory() {
        let Ok(dir) = std::env::var("LARDER_IMPORT_SAMPLES") else { return };
        for entry in std::fs::read_dir(dir).unwrap().flatten() {
            let path = entry.path();
            let name = path.file_name().unwrap().to_string_lossy().to_string();
            let bytes = std::fs::read(&path).unwrap();
            match crate::parse_file(crate::Source::Auto, &name, &bytes) {
                Ok(b) => {
                    println!("{name}: {:?} {} recipes, {} errors {:?}", b.source, b.recipes.len(), b.errors.len(), b.errors);
                    for r in b.recipes.iter().take(3) {
                        let d = &r.data;
                        println!(
                            "  - {} | yield {:?} {} | times {:?}/{:?}/{:?} | {} ings, {} steps | tags {:?} cats {:?} | src {:?} {:?} | img {} | fav {} | {:?}",
                            d.name, d.yield_quantity, d.yield_unit, d.prep_time_minutes, d.cook_time_minutes, d.total_time_minutes,
                            d.ingredients.len(), d.instructions.len(), d.tags, d.categories, d.source_name, d.source_url,
                            r.image.is_some(), r.favorite, r.created_at
                        );
                        if let Some(i) = d.ingredients.first() {
                            println!("    first ing: {:?}", i);
                        }
                    }
                }
                Err(e) => println!("{name}: ERROR {e}"),
            }
        }
    }
}
