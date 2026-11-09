//! Small, defensive helpers shared by the importers: lenient JSON accessors,
//! text cleanup, yield/date parsing and image sniffing.

use std::sync::LazyLock;

use larder_core::text::{collapse_ws, decode_entities, strip_html};
use regex::Regex;
use serde_json::Value;

/// Returns the first present, non-null value among `keys`.
pub(crate) fn get<'a>(v: &'a Value, keys: &[&str]) -> Option<&'a Value> {
    let obj = v.as_object()?;
    keys.iter().filter_map(|k| obj.get(*k)).find(|v| !v.is_null())
}

/// Converts scalars (and single-element arrays / `{"name"}` / `{"@value"}`
/// objects) to a string. Returns `None` for empty strings.
pub(crate) fn as_text(v: &Value) -> Option<String> {
    let s = match v {
        Value::String(s) => s.clone(),
        Value::Number(n) => {
            let f = n.as_f64()?;
            if f.fract() == 0.0 && f.abs() < 1e15 {
                format!("{}", f as i64)
            } else {
                n.to_string()
            }
        }
        Value::Bool(b) => b.to_string(),
        Value::Array(a) => return a.iter().find_map(as_text),
        Value::Object(_) => return get(v, &["@value", "name", "text", "value", "url"]).and_then(as_text),
        Value::Null => return None,
    };
    let s = s.trim().to_string();
    (!s.is_empty()).then_some(s)
}

/// `as_text` of the first present key.
pub(crate) fn text_of(v: &Value, keys: &[&str]) -> Option<String> {
    keys.iter().filter_map(|k| v.get(*k)).find_map(as_text)
}

/// Text with HTML stripped, entities decoded and whitespace collapsed (single line).
pub(crate) fn clean_line(s: &str) -> String {
    collapse_ws(&clean_block(s))
}

/// Text with HTML stripped and entities decoded, keeping line structure.
pub(crate) fn clean_block(s: &str) -> String {
    let s = if s.contains('<') && s.contains('>') { strip_html(s) } else { decode_entities(s) };
    // Some sites double-encode ("&amp;amp;") — one more pass is harmless.
    let s = if s.contains('&') { decode_entities(&s) } else { s };
    let lines: Vec<String> =
        s.replace("\r\n", "\n").replace('\r', "\n").split('\n').map(|l| collapse_ws(&l.replace('\u{a0}', " "))).collect();
    // Collapse 3+ consecutive blank lines into one blank line.
    let mut out: Vec<String> = vec![];
    for l in lines {
        if l.is_empty() && out.last().is_some_and(|p: &String| p.is_empty()) {
            continue;
        }
        out.push(l);
    }
    out.join("\n").trim().to_string()
}

/// Numbers that may be encoded as strings ("4", "4.5", "4,5").
pub(crate) fn as_f64(v: &Value) -> Option<f64> {
    match v {
        Value::Number(n) => n.as_f64(),
        Value::String(s) => {
            let t = s.trim().replace(',', ".");
            t.parse::<f64>().ok().or_else(|| {
                // leading number of a string like "4 servings"
                LEADING_NUM.captures(&t).and_then(|c| c[1].parse().ok())
            })
        }
        Value::Bool(b) => Some(if *b { 1.0 } else { 0.0 }),
        Value::Array(a) => a.iter().find_map(as_f64),
        Value::Object(_) => get(v, &["@value", "value"]).and_then(as_f64),
        Value::Null => None,
    }
    .filter(|f| f.is_finite())
}

pub(crate) fn as_bool(v: &Value) -> bool {
    match v {
        Value::Bool(b) => *b,
        Value::Number(n) => n.as_f64().is_some_and(|f| f != 0.0),
        Value::String(s) => matches!(s.trim().to_ascii_lowercase().as_str(), "true" | "1" | "yes" | "y"),
        _ => false,
    }
}

/// A list of names from a string (comma separated), an array of strings,
/// or an array of `{name: ..}` objects.
pub(crate) fn name_list(v: &Value) -> Vec<String> {
    match v {
        Value::String(s) => split_list(s),
        Value::Array(a) => a.iter().flat_map(name_list).collect(),
        Value::Object(_) => text_of(v, &["name", "title", "@value"]).map(|s| vec![clean_line(&s)]).unwrap_or_default(),
        Value::Number(_) => as_text(v).into_iter().collect(),
        _ => vec![],
    }
}

pub(crate) fn split_list(s: &str) -> Vec<String> {
    let sep = if s.contains(',') {
        ','
    } else if s.contains(';') {
        ';'
    } else {
        '\n'
    };
    s.split(sep).map(clean_line).filter(|s| !s.is_empty()).collect()
}

static LEADING_NUM: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^\s*(\d+(?:\.\d+)?)").unwrap());

static YIELD_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)(?P<num>\d+(?:[.,]\d+)?(?:\s+\d+/\d+)?|\d+/\d+|[½¼¾⅓⅔])(?:\s*(?:-|–|to|or|/)\s*\d+(?:[.,]\d+)?)?\s*(?P<unit>.*)$")
        .unwrap()
});

static ANY_NUM: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(\d+(?:\.\d+)?)").unwrap());

const WORD_NUMS: &[(&str, f64)] = &[
    ("one", 1.0),
    ("two", 2.0),
    ("three", 3.0),
    ("four", 4.0),
    ("five", 5.0),
    ("six", 6.0),
    ("seven", 7.0),
    ("eight", 8.0),
    ("nine", 9.0),
    ("ten", 10.0),
    ("twelve", 12.0),
    ("a dozen", 12.0),
    ("dozen", 12.0),
];

/// Parses a yield like "4", "Serves 4", "Makes 12 cookies", "4-6 servings",
/// "Yield: 1 loaf" into (quantity, unit). The unit defaults to "servings".
pub(crate) fn parse_yield_text(s: &str) -> (Option<f64>, String) {
    let s = clean_line(s);
    let mut t = s.as_str().trim();
    let lower = t.to_lowercase();
    for p in [
        "yield:",
        "yields:",
        "yield",
        "yields",
        "servings:",
        "serves:",
        "serves",
        "serving:",
        "makes:",
        "makes",
        "portions:",
        "portionen:",
        "for",
        "about",
        "approximately",
        "approx.",
        "approx",
    ] {
        let boundary = p.ends_with(':') || lower.get(p.len()..).and_then(|r| r.chars().next()).is_none_or(|c| !c.is_alphanumeric());
        if lower.starts_with(p) && t.is_char_boundary(p.len()) && boundary {
            t = t[p.len()..].trim_start_matches([':', ' ', '-']).trim();
            let l2 = t.to_lowercase();
            for p2 in ["about ", "approximately ", "approx. ", "up to "] {
                if l2.starts_with(p2) {
                    t = t[p2.len()..].trim();
                }
            }
            break;
        }
    }
    let norm = larder_core::parse::normalize_line(t);
    if let Some(c) = YIELD_RE.captures(&norm) {
        if c.get(0).unwrap().start() == 0 {
            let q = larder_core::parse::parse_number(&c["num"].replace(',', "."));
            let unit = normalize_yield_unit(c["unit"].trim());
            return (q, unit);
        }
    }
    let lower = norm.to_lowercase();
    for (w, n) in WORD_NUMS {
        if lower.starts_with(w) && lower[w.len()..].chars().next().is_none_or(|c| !c.is_alphanumeric()) {
            return (Some(*n), normalize_yield_unit(norm[w.len()..].trim()));
        }
    }
    // "Servings 4" style or unparseable text.
    if let Some(c) = ANY_NUM.captures(&norm) {
        let q: Option<f64> = c[1].parse().ok();
        let before = norm[..c.get(0).unwrap().start()].trim().to_lowercase();
        if before.contains("serv") || before.contains("portion") || before.contains("people") {
            return (q, "servings".into());
        }
    }
    (None, "servings".into())
}

fn normalize_yield_unit(u: &str) -> String {
    let u = u.trim().trim_start_matches(['(', ':']).trim_end_matches([')', '.']).trim();
    let lower = u.to_lowercase();
    let first = lower.split_whitespace().next().unwrap_or("");
    if u.is_empty()
        || first.starts_with("serving")
        || matches!(
            first,
            "serving"
                | "servings"
                | "people"
                | "persons"
                | "person"
                | "portion"
                | "portions"
                | "personen"
                | "portionen"
                | "pers"
                | "pers."
                | "porciones"
                | "portions."
                | "serves"
                | "pax"
        )
    {
        return "servings".into();
    }
    u.chars().take(60).collect()
}

/// Recipe yield from a schema.org-ish value: number, string or array of them
/// (e.g. `["4", "4 servings"]` — the most descriptive entry wins).
pub(crate) fn parse_yield_value(v: &Value) -> (Option<f64>, String) {
    match v {
        Value::Number(_) => (as_f64(v).filter(|f| *f > 0.0), "servings".into()),
        Value::String(s) => parse_yield_text(s),
        Value::Array(a) => {
            let parsed: Vec<_> = a.iter().map(parse_yield_value).filter(|(q, _)| q.is_some()).collect();
            parsed.iter().find(|(_, u)| u != "servings").or_else(|| parsed.first()).cloned().unwrap_or((None, "servings".into()))
        }
        Value::Object(_) => get(v, &["value", "@value", "name"]).map(parse_yield_value).unwrap_or((None, "servings".into())),
        _ => (None, "servings".into()),
    }
}

/// Best-effort conversion of a date/time string to RFC3339 (UTC assumed when
/// no offset is present).
pub(crate) fn to_rfc3339(s: &str) -> Option<String> {
    // Exporters write epoch 0 / year 1 for "unknown".
    to_rfc3339_inner(s).filter(|d| d.as_str() >= "1980")
}

fn to_rfc3339_inner(s: &str) -> Option<String> {
    use chrono::{DateTime, NaiveDate, NaiveDateTime, SecondsFormat, Utc};
    let s = s.trim();
    if s.is_empty() {
        return None;
    }
    if let Ok(d) = DateTime::parse_from_rfc3339(s) {
        return Some(d.with_timezone(&Utc).to_rfc3339_opts(SecondsFormat::Secs, true));
    }
    for fmt in ["%Y-%m-%dT%H:%M:%S%.f%z", "%Y-%m-%d %H:%M:%S%.f%z", "%Y-%m-%dT%H:%M%z"] {
        if let Ok(d) = DateTime::parse_from_str(s, fmt) {
            return Some(d.with_timezone(&Utc).to_rfc3339_opts(SecondsFormat::Secs, true));
        }
    }
    for fmt in ["%Y-%m-%dT%H:%M:%S%.f", "%Y-%m-%d %H:%M:%S%.f", "%Y-%m-%dT%H:%M", "%Y-%m-%d %H:%M"] {
        if let Ok(d) = NaiveDateTime::parse_from_str(s, fmt) {
            return Some(d.and_utc().to_rfc3339_opts(SecondsFormat::Secs, true));
        }
    }
    if let Ok(d) = NaiveDate::parse_from_str(s.get(..10).unwrap_or(s), "%Y-%m-%d") {
        return Some(d.and_hms_opt(0, 0, 0)?.and_utc().to_rfc3339_opts(SecondsFormat::Secs, true));
    }
    if let Ok(ts) = s.parse::<i64>() {
        let ts = if ts > 100_000_000_000 { ts / 1000 } else { ts };
        return DateTime::<Utc>::from_timestamp(ts, 0).map(|d| d.to_rfc3339_opts(SecondsFormat::Secs, true));
    }
    None
}

/// Host name without a leading "www.".
pub(crate) fn host_of(url: &str) -> Option<String> {
    let u = url::Url::parse(url).ok()?;
    let h = u.host_str()?;
    Some(h.strip_prefix("www.").unwrap_or(h).to_string())
}

/// Resolves `href` against `base` (if given); only http(s) and data: URLs are kept.
pub(crate) fn resolve_url(href: &str, base: Option<&str>) -> Option<String> {
    let href = decode_entities(href.trim());
    if href.is_empty() {
        return None;
    }
    let parsed = match url::Url::parse(&href) {
        Ok(u) => u,
        Err(url::ParseError::RelativeUrlWithoutBase) => url::Url::parse(base?).ok()?.join(&href).ok()?,
        Err(_) => return None,
    };
    matches!(parsed.scheme(), "http" | "https").then(|| parsed.to_string())
}

/// Detects an image format from magic bytes; returns a file extension.
pub fn image_extension(bytes: &[u8]) -> Option<&'static str> {
    if bytes.starts_with(&[0xFF, 0xD8, 0xFF]) {
        Some("jpg")
    } else if bytes.starts_with(b"\x89PNG\r\n\x1a\n") {
        Some("png")
    } else if bytes.len() >= 12 && &bytes[..4] == b"RIFF" && &bytes[8..12] == b"WEBP" {
        Some("webp")
    } else if bytes.starts_with(b"GIF87a") || bytes.starts_with(b"GIF89a") {
        Some("gif")
    } else if bytes.len() >= 12 && &bytes[4..8] == b"ftyp" && matches!(&bytes[8..12], b"avif" | b"avis" | b"heic" | b"heix" | b"mif1") {
        Some(if &bytes[8..12] == b"avif" || &bytes[8..12] == b"avis" { "avif" } else { "heic" })
    } else if bytes.starts_with(b"BM") && bytes.len() > 14 {
        Some("bmp")
    } else {
        None
    }
}

/// Canonical (schema.org style) nutrition key for a loose name such as
/// "fat_content", "TotalFat", "Calories", "carbohydrates".
pub(crate) fn nutrition_key(k: &str) -> Option<String> {
    let norm: String = k.chars().filter(|c| c.is_ascii_alphanumeric()).collect::<String>().to_ascii_lowercase();
    let norm = norm.strip_prefix("recipenut").unwrap_or(&norm).to_string();
    let key = match norm.as_str() {
        "" | "type" | "id" | "context" | "recipeid" | "source" => return None,
        "calories" | "energy" | "kcal" | "caloriescontent" => "calories",
        "fat" | "fats" | "totalfat" | "fatcontent" => "fatContent",
        "saturatedfat" | "saturatedfatcontent" => "saturatedFatContent",
        "transfat" | "transfatcontent" => "transFatContent",
        "unsaturatedfat" | "unsaturatedfatcontent" => "unsaturatedFatContent",
        "carbohydrate" | "carbohydrates" | "carbs" | "totalcarbohydrate" | "carbohydratecontent" => "carbohydrateContent",
        "sugar" | "sugars" | "sugarcontent" => "sugarContent",
        "fiber" | "fibre" | "dietaryfiber" | "fibercontent" => "fiberContent",
        "protein" | "proteins" | "proteincontent" => "proteinContent",
        "sodium" | "sodiumcontent" | "salt" => "sodiumContent",
        "cholesterol" | "cholesterolcontent" => "cholesterolContent",
        "servingsize" => "servingSize",
        _ => return Some(k.trim().to_string()),
    };
    Some(key.to_string())
}

/// Parses free-text nutrition info ("Calories: 200\nFat: 10 g") into pairs.
pub(crate) fn parse_nutrition_text(s: &str) -> Vec<(String, String)> {
    let mut out = vec![];
    for line in clean_block(s).lines().flat_map(|l| l.split(';')) {
        if let Some((k, v)) = line.split_once(':') {
            let (k, v) = (k.trim(), v.trim());
            if !k.is_empty() && !v.is_empty() && k.len() < 40 {
                if let Some(key) = nutrition_key(k) {
                    out.push((key, v.to_string()));
                }
            }
        }
    }
    out
}

static STEP_NUM_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)^(?:step\s*)?\d{1,3}\s*[.):]\s+|^(?:step\s*)\d{1,3}\s*[-–]?\s*").unwrap());

/// Removes a leading step number ("1. ", "2) ", "Step 3: ").
pub(crate) fn strip_step_number(s: &str) -> String {
    STEP_NUM_RE.replace(s.trim(), "").trim().to_string()
}

pub(crate) fn has_step_number(s: &str) -> bool {
    STEP_NUM_RE.is_match(s.trim())
}

/// Removes a leading list bullet.
pub(crate) fn strip_bullet(s: &str) -> &str {
    let t = s.trim_start();
    for b in ["- ", "* ", "• ", "· ", "▢ ", "□ ", "☐ ", "– ", "+ "] {
        if let Some(r) = t.strip_prefix(b) {
            return r.trim_start();
        }
    }
    t
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn yields() {
        assert_eq!(parse_yield_text("4"), (Some(4.0), "servings".into()));
        assert_eq!(parse_yield_text("Serves 4"), (Some(4.0), "servings".into()));
        assert_eq!(parse_yield_text("Makes 12 cookies"), (Some(12.0), "cookies".into()));
        assert_eq!(parse_yield_text("4-6 servings"), (Some(4.0), "servings".into()));
        assert_eq!(parse_yield_text("Yield: 1 loaf"), (Some(1.0), "loaf".into()));
        assert_eq!(parse_yield_text("4 Portionen"), (Some(4.0), "servings".into()));
        assert_eq!(parse_yield_text("one 9-inch pie"), (Some(1.0), "9-inch pie".into()));
        assert_eq!(parse_yield_text("Servings 6"), (Some(6.0), "servings".into()));
        assert_eq!(parse_yield_text("lots"), (None, "servings".into()));
        assert_eq!(parse_yield_text("4 serving(s)"), (Some(4.0), "servings".into()));
        assert_eq!(parse_yield_value(&json!(["4", "4 servings"])), (Some(4.0), "servings".into()));
        assert_eq!(parse_yield_value(&json!(["24", "24 cookies"])), (Some(24.0), "cookies".into()));
        assert_eq!(parse_yield_value(&json!(6)), (Some(6.0), "servings".into()));
    }

    #[test]
    fn dates() {
        assert_eq!(to_rfc3339("2012-09-05 13:03:36").as_deref(), Some("2012-09-05T13:03:36Z"));
        assert_eq!(to_rfc3339("2022-12-05T22:33:16.445964").as_deref(), Some("2022-12-05T22:33:16Z"));
        assert_eq!(to_rfc3339("2020-07-13T16:48:25+02:00").as_deref(), Some("2020-07-13T14:48:25Z"));
        assert_eq!(to_rfc3339("2021-10-28").as_deref(), Some("2021-10-28T00:00:00Z"));
        assert_eq!(to_rfc3339("garbage"), None);
        assert_eq!(to_rfc3339("1970-01-01T00:00:00+0000"), None);
    }

    #[test]
    fn misc() {
        assert_eq!(as_f64(&json!("4,5")), Some(4.5));
        assert_eq!(as_text(&json!(4.0)).as_deref(), Some("4"));
        assert_eq!(name_list(&json!("a, b ,c")), vec!["a", "b", "c"]);
        assert_eq!(name_list(&json!([{"name": "x"}, "y"])), vec!["x", "y"]);
        assert_eq!(strip_step_number("1. Mix"), "Mix");
        assert_eq!(strip_step_number("Step 2: Bake"), "Bake");
        assert_eq!(strip_step_number("350 degrees"), "350 degrees");
        assert_eq!(nutrition_key("fat_content").as_deref(), Some("fatContent"));
        assert_eq!(nutrition_key("recipeNutTotalFat").as_deref(), Some("fatContent"));
        assert_eq!(resolve_url("/img/a.jpg", Some("https://ex.com/r/1")).as_deref(), Some("https://ex.com/img/a.jpg"));
        assert_eq!(resolve_url("javascript:alert(1)", None), None);
        assert_eq!(image_extension(&[0xFF, 0xD8, 0xFF, 0xE0]), Some("jpg"));
        assert_eq!(clean_line("<b>Hi</b> &amp;amp; there"), "Hi & there");
    }
}
