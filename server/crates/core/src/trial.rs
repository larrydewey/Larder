//! Trial recipes: recipes the household wants to try but has not vetted yet.
//!
//! A recipe's [`RecipeStatus`] is either `cookbook` (the default, the main
//! collection) or `trial`. Trial recipes also count how often they were made
//! ([`TrialState::tried_count`], [`TrialState::last_tried_at`]). The status
//! and counters are household-wide and live next to the recipe content
//! ([`crate::RecipeData`]) rather than inside it, like favourites.

use serde::{Deserialize, Deserializer, Serialize};

/// Where a recipe lives: the main cookbook or the trial list.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum RecipeStatus {
    #[default]
    Cookbook,
    Trial,
}

impl RecipeStatus {
    /// The API / database name.
    pub fn as_str(self) -> &'static str {
        match self {
            RecipeStatus::Cookbook => "cookbook",
            RecipeStatus::Trial => "trial",
        }
    }

    /// Parses an API / database name (case-insensitive, surrounding space ignored).
    pub fn parse(s: &str) -> Option<RecipeStatus> {
        match s.trim().to_ascii_lowercase().as_str() {
            "cookbook" => Some(RecipeStatus::Cookbook),
            "trial" => Some(RecipeStatus::Trial),
            _ => None,
        }
    }

    pub fn is_trial(self) -> bool {
        self == RecipeStatus::Trial
    }
}

impl<'de> Deserialize<'de> for RecipeStatus {
    /// Accepts `"cookbook"` / `"trial"` in any case; `null` means the default.
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        match Option::<String>::deserialize(d)? {
            None => Ok(RecipeStatus::default()),
            Some(s) => RecipeStatus::parse(&s)
                .ok_or_else(|| serde::de::Error::custom(format!("unknown recipe status '{s}' (expected cookbook or trial)"))),
        }
    }
}

/// Household-wide trial bookkeeping for one recipe.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct TrialState {
    pub status: RecipeStatus,
    /// How often the household said it made the recipe.
    pub tried_count: i64,
    /// When it was last made (RFC 3339 UTC), if ever.
    pub last_tried_at: Option<String>,
}

/// Upper bound for `tried_count` (guards against nonsense in imported files).
const MAX_TRIED: i64 = 100_000;

impl TrialState {
    pub fn new(status: RecipeStatus) -> Self {
        Self { status, ..Default::default() }
    }

    /// Clamps the counter and rewrites `last_tried_at` in the fixed-width
    /// form the server stores (dropping it when it is not a timestamp).
    pub fn normalize(&mut self) {
        self.tried_count = self.tried_count.clamp(0, MAX_TRIED);
        self.last_tried_at = self.last_tried_at.as_deref().and_then(normalize_timestamp);
    }
}

/// Parses an RFC 3339 timestamp and formats it as fixed-width UTC with
/// microseconds (`2026-11-12T18:30:00.000000Z`), the server's sortable form.
pub fn normalize_timestamp(s: &str) -> Option<String> {
    let t = chrono::DateTime::parse_from_rfc3339(s.trim()).ok()?;
    Some(t.with_timezone(&chrono::Utc).format("%Y-%m-%dT%H:%M:%S%.6fZ").to_string())
}

/// The dated line a verdict note adds to the recipe notes, e.g.
/// `Nov 12, 2026: too salty, cut soy by half`. `None` for an empty note.
pub fn note_line(date: chrono::NaiveDate, note: &str) -> Option<String> {
    let note = crate::text::collapse_ws(note);
    if note.is_empty() {
        return None;
    }
    Some(format!("{}: {note}", date.format("%b %-d, %Y")))
}

/// Appends `line` to `notes` on a line of its own.
pub fn append_note(notes: &str, line: &str) -> String {
    let notes = notes.trim_end();
    if notes.is_empty() {
        line.to_string()
    } else {
        format!("{notes}\n{line}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn status_parses_leniently_and_serializes_lowercase() {
        assert_eq!(RecipeStatus::parse(" Trial "), Some(RecipeStatus::Trial));
        assert_eq!(RecipeStatus::parse("COOKBOOK"), Some(RecipeStatus::Cookbook));
        assert_eq!(RecipeStatus::parse("trail"), None);
        assert_eq!(serde_json::to_string(&RecipeStatus::Trial).unwrap(), "\"trial\"");
        let s: RecipeStatus = serde_json::from_str("\"Trial\"").unwrap();
        assert_eq!(s, RecipeStatus::Trial);
        let s: RecipeStatus = serde_json::from_str("null").unwrap();
        assert_eq!(s, RecipeStatus::Cookbook);
        assert!(serde_json::from_str::<RecipeStatus>("\"maybe\"").is_err());
    }

    #[test]
    fn missing_fields_default_to_an_untried_cookbook_recipe() {
        let t: TrialState = serde_json::from_str("{}").unwrap();
        assert_eq!(t, TrialState::default());
        assert_eq!(t.status, RecipeStatus::Cookbook);
        assert_eq!(t.tried_count, 0);
        assert_eq!(t.last_tried_at, None);
    }

    #[test]
    fn normalize_clamps_and_fixes_timestamps() {
        let mut t = TrialState { status: RecipeStatus::Trial, tried_count: -3, last_tried_at: Some("2026-11-12T10:00:00+02:00".into()) };
        t.normalize();
        assert_eq!(t.tried_count, 0);
        assert_eq!(t.last_tried_at.as_deref(), Some("2026-11-12T08:00:00.000000Z"));

        let mut t = TrialState { tried_count: i64::MAX, last_tried_at: Some("last Tuesday".into()), ..Default::default() };
        t.normalize();
        assert_eq!(t.tried_count, MAX_TRIED);
        assert_eq!(t.last_tried_at, None);
    }

    #[test]
    fn note_lines_are_dated_and_appended() {
        let d = chrono::NaiveDate::from_ymd_opt(2026, 11, 2).unwrap();
        assert_eq!(note_line(d, "  too salty,\n cut soy by half "), Some("Nov 2, 2026: too salty, cut soy by half".into()));
        assert_eq!(note_line(d, "   "), None);
        assert_eq!(append_note("", "Nov 2, 2026: good"), "Nov 2, 2026: good");
        assert_eq!(append_note("Use fresh basil.\n\n", "Nov 2, 2026: good"), "Use fresh basil.\nNov 2, 2026: good");
    }
}
