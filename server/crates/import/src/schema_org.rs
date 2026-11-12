//! schema.org `Recipe` (JSON-LD) mapping.

use std::sync::LazyLock;

use larder_core::{Ingredient, Instruction, RecipeData};
use regex::Regex;
use serde_json::Value;

use crate::util::{
    as_f64, as_text, clean_block, clean_line, get, name_list, nutrition_key, parse_yield_value, resolve_url, strip_bullet,
    strip_step_number, text_of, to_rfc3339,
};
use crate::{ImageSource, ImportedRecipe};

/// True if the JSON-LD `@type` (string or array, possibly prefixed) contains `ty`.
pub(crate) fn has_type(v: &Value, ty: &str) -> bool {
    let matches = |s: &str| {
        let s = s.trim();
        let s = s.rsplit(['/', ':', '#']).next().unwrap_or(s);
        s.eq_ignore_ascii_case(ty)
    };
    match v.get("@type") {
        Some(Value::String(s)) => matches(s),
        Some(Value::Array(a)) => a.iter().any(|t| t.as_str().is_some_and(matches)),
        _ => false,
    }
}

/// Finds all schema.org Recipe objects anywhere in a JSON-LD document
/// (top-level object, arrays, `@graph`, `mainEntity`, nested values).
pub fn find_recipes(v: &Value) -> Vec<&Value> {
    let mut out = vec![];
    walk(v, 0, &mut out);
    out
}

fn walk<'a>(v: &'a Value, depth: usize, out: &mut Vec<&'a Value>) {
    if depth > 12 {
        return;
    }
    match v {
        Value::Array(a) => a.iter().for_each(|x| walk(x, depth + 1, out)),
        Value::Object(o) => {
            if has_type(v, "Recipe") {
                out.push(v);
                return;
            }
            for (k, x) in o {
                if matches!(x, Value::Object(_) | Value::Array(_)) && k != "@context" {
                    walk(x, depth + 1, out);
                }
            }
        }
        _ => {}
    }
}

static ISO_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"(?i)^P(?:(?P<y>\d+(?:[.,]\d+)?)Y)?(?:(?P<mo>\d+(?:[.,]\d+)?)M)?(?:(?P<w>\d+(?:[.,]\d+)?)W)?(?:(?P<d>\d+(?:[.,]\d+)?)D)?(?:T(?:(?P<h>\d+(?:[.,]\d+)?)H)?(?:(?P<m>\d+(?:[.,]\d+)?)M)?(?:(?P<s>\d+(?:[.,]\d+)?)S)?)?$",
    )
    .unwrap()
});

static TEXT_DUR_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"(?i)(?P<n>\d+\s+\d+/\d+|\d+/\d+|\d+(?:[.,]\d+)?|\b(?:an?|one|half(?:\s+an?)?))\s*(?:(?:-|–|to)\s*\d+(?:[.,]\d+)?\s*)?(?P<u>days?|hours?|hrs?|stunden?|std\.?|heures?|horas?|ore|minutes?|minuten?|minutos?|minuti|mins?|sec(?:ond)?s?|sekunden?|d|h|m|s)",
    )
    .unwrap()
});

static CLOCK_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^(\d{1,2}):(\d{2})(?::(\d{2}))?$").unwrap());

/// Parses a duration into minutes. Accepts ISO 8601 ("PT1H20M", "P0DT1H",
/// "PT90M", "PT0.5H") and plain text ("1 hour 20 mins", "1h20m", "90 minutes",
/// "1:30", "45"). Returns `None` for unparseable or zero durations.
pub fn parse_iso8601_duration(s: &str) -> Option<i64> {
    let s = s.trim();
    if s.is_empty() {
        return None;
    }
    let num = |m: Option<regex::Match>| m.and_then(|m| m.as_str().replace(',', ".").parse::<f64>().ok()).unwrap_or(0.0);
    if let Some(c) = ISO_RE.captures(s) {
        let mins = num(c.name("y")) * 525_600.0
            + num(c.name("mo")) * 43_200.0
            + num(c.name("w")) * 10_080.0
            + num(c.name("d")) * 1440.0
            + num(c.name("h")) * 60.0
            + num(c.name("m"))
            + num(c.name("s")) / 60.0;
        return to_minutes(mins);
    }
    if let Some(c) = CLOCK_RE.captures(s) {
        return to_minutes(num(c.get(1)) * 60.0 + num(c.get(2)));
    }
    if let Ok(n) = s.replace(',', ".").parse::<f64>() {
        return to_minutes(n);
    }
    let mut total = 0.0;
    let mut any = false;
    for c in TEXT_DUR_RE.captures_iter(s) {
        let n_str = c["n"].to_lowercase();
        let n = match n_str.as_str() {
            "a" | "an" | "one" => 1.0,
            x if x.starts_with("half") => 0.5,
            x => larder_core::parse::parse_number(x).unwrap_or(0.0),
        };
        let u = c["u"].to_lowercase();
        let factor = if u.starts_with('d') {
            1440.0
        } else if u.starts_with('h') || u.starts_with("st") || u.starts_with("or") {
            60.0
        } else if u.starts_with("se") || u == "s" {
            1.0 / 60.0
        } else {
            1.0
        };
        total += n * factor;
        any = true;
    }
    if any {
        return to_minutes(total);
    }
    None
}

fn to_minutes(m: f64) -> Option<i64> {
    if !m.is_finite() || m <= 0.0 || m > 525_600.0 * 2.0 {
        return None;
    }
    Some((m.round() as i64).max(1))
}

fn duration_of(v: &Value, keys: &[&str]) -> Option<i64> {
    let x = get(v, keys)?;
    match x {
        Value::Number(_) => as_f64(x).and_then(to_minutes),
        _ => as_text(x).and_then(|s| parse_iso8601_duration(&s)),
    }
}

/// Collects ingredient lines from a string / array / objects.
pub(crate) fn ingredient_lines(v: &Value, out: &mut Vec<String>) {
    match v {
        Value::String(s) => {
            for line in clean_block(s).lines() {
                let l = line.trim();
                if !l.is_empty() {
                    out.push(l.to_string());
                }
            }
        }
        Value::Array(a) => a.iter().for_each(|x| ingredient_lines(x, out)),
        Value::Object(_) => {
            if let Some(list) = get(v, &["itemListElement"]) {
                ingredient_lines(list, out);
            } else if let Some(t) = text_of(v, &["text", "name", "description", "value"]) {
                let amount = text_of(v, &["requiredQuantity", "amount"]).unwrap_or_default();
                let line = if amount.is_empty() { t } else { format!("{amount} {t}") };
                out.push(clean_line(&line));
            }
        }
        Value::Number(_) => out.extend(as_text(v)),
        _ => {}
    }
}

/// Turns ingredient text lines into ingredients, honouring section headings.
pub(crate) fn ingredients_from_lines(lines: &[String]) -> Vec<Ingredient> {
    let mut out = vec![];
    let mut section: Option<String> = None;
    for raw in lines {
        let line = strip_bullet(raw).trim();
        if line.is_empty() {
            continue;
        }
        if let Some(h) = larder_core::parse::section_heading(line) {
            section = Some(h);
            continue;
        }
        let mut ing = larder_core::parse::parse_ingredient(line);
        ing.section = section.clone();
        out.push(ing);
    }
    out
}

/// Flattens `recipeInstructions` (string, array of strings, HowToStep,
/// HowToSection with nested itemListElement, ItemList, ...).
pub(crate) fn instructions_from(v: &Value) -> Vec<Instruction> {
    let mut out = vec![];
    collect_steps(v, None, &mut out, 0);
    out
}

fn push_text_steps(text: &str, section: &Option<String>, out: &mut Vec<Instruction>) {
    let cleaned = clean_block(text);
    let mut section = section.clone();
    let paragraphs: Vec<&str> = cleaned.lines().map(str::trim).filter(|l| !l.is_empty()).collect();
    for p in paragraphs {
        let t = strip_step_number(strip_bullet(p));
        if t.is_empty() {
            continue;
        }
        if let Some(h) = t.strip_prefix('#') {
            section = Some(h.trim_start_matches('#').trim().to_string()).filter(|s| !s.is_empty());
            continue;
        }
        if t.ends_with(':') && t.chars().count() < 50 && !t.contains(". ") {
            section = Some(t.trim_end_matches(':').trim().to_string());
            continue;
        }
        out.push(Instruction { section: section.clone(), text: t });
    }
}

fn collect_steps(v: &Value, section: Option<String>, out: &mut Vec<Instruction>, depth: usize) {
    if depth > 8 {
        return;
    }
    match v {
        Value::String(s) => push_text_steps(s, &section, out),
        Value::Array(a) => a.iter().for_each(|x| collect_steps(x, section.clone(), out, depth + 1)),
        Value::Object(_) => {
            let items = get(v, &["itemListElement", "steps", "step"]);
            if has_type(v, "HowToSection") || (items.is_some() && !has_type(v, "HowToStep")) {
                let name = text_of(v, &["name", "headline"]).map(|s| clean_line(&s)).filter(|s| !s.is_empty());
                if let Some(items) = items {
                    collect_steps(items, name.or(section), out, depth + 1);
                } else if let Some(t) = text_of(v, &["text", "description"]) {
                    push_text_steps(&t, &name.or(section), out);
                }
                return;
            }
            // HowToStep / HowToDirection / HowToTip: prefer text over name
            // (name is usually a truncated copy of the text).
            let text = text_of(v, &["text", "description"]).or_else(|| text_of(v, &["name"]));
            if let Some(t) = text {
                push_text_steps(&t, &section, out);
            } else if let Some(items) = items {
                collect_steps(items, section, out, depth + 1);
            }
        }
        _ => {}
    }
}

/// First image URL from string / array / ImageObject.
pub(crate) fn image_url(v: &Value, base: Option<&str>) -> Option<String> {
    match v {
        Value::String(s) => resolve_url(s, base),
        Value::Array(a) => {
            // Prefer the largest declared ImageObject when sizes are given.
            let mut best: Option<(f64, String)> = None;
            for x in a {
                if let Some(u) = image_url(x, base) {
                    let w = x.get("width").and_then(as_f64).unwrap_or(0.0);
                    if best.as_ref().is_none_or(|(bw, _)| w > *bw) {
                        best = Some((w, u));
                    }
                }
            }
            best.map(|(_, u)| u)
        }
        Value::Object(_) => get(v, &["url", "contentUrl", "@id", "src"]).and_then(|u| image_url(u, base)),
        _ => None,
    }
}

/// Nutrition map from a NutritionInformation object (or a loose map).
pub(crate) fn nutrition_from(v: &Value, out: &mut std::collections::BTreeMap<String, String>) {
    let Some(o) = v.as_object() else { return };
    for (k, val) in o {
        if k.starts_with('@') {
            continue;
        }
        if let (Some(key), Some(text)) = (nutrition_key(k), as_text(val)) {
            let text = clean_line(&text);
            if !text.is_empty() && text.chars().count() <= 100 {
                out.insert(key, text);
            }
        }
    }
}

fn name_of_org(v: &Value) -> Option<String> {
    match v {
        Value::Array(a) => a.iter().find_map(name_of_org),
        Value::Object(_) => text_of(v, &["name"]).map(|s| clean_line(&s)),
        Value::String(s) if !s.starts_with("http") => Some(clean_line(s)),
        _ => None,
    }
}

/// Maps a schema.org Recipe object to `RecipeData` (not normalised).
pub(crate) fn recipe_data_from_json(v: &Value, base_url: Option<&str>) -> RecipeData {
    let mut d = RecipeData {
        name: text_of(v, &["name", "headline", "title"]).map(|s| clean_line(&s)).unwrap_or_default(),
        description: text_of(v, &["description"]).map(|s| clean_block(&s)).unwrap_or_default(),
        ..Default::default()
    };
    if let Some(y) = get(v, &["recipeYield", "yield", "servings"]) {
        let (q, u) = parse_yield_value(y);
        d.yield_quantity = q;
        d.yield_unit = u;
    }
    d.prep_time_minutes = duration_of(v, &["prepTime"]);
    d.cook_time_minutes = duration_of(v, &["cookTime", "performTime"]);
    d.total_time_minutes = duration_of(v, &["totalTime"]);

    let mut lines = vec![];
    if let Some(ing) = get(v, &["recipeIngredient", "ingredients", "recipeIngredients"]) {
        ingredient_lines(ing, &mut lines);
    }
    d.ingredients = ingredients_from_lines(&lines);
    if let Some(ins) = get(v, &["recipeInstructions", "instructions", "recipeDirections", "directions", "step"]) {
        d.instructions = instructions_from(ins);
    }

    d.source_url = text_of(v, &["url", "mainEntityOfPage"])
        .and_then(|u| resolve_url(&u, base_url))
        .or_else(|| base_url.and_then(|b| resolve_url(b, None)));
    d.source_name = get(v, &["publisher"]).and_then(name_of_org).filter(|s| !s.is_empty());

    if let Some(r) = get(v, &["aggregateRating"]) {
        let val = get(r, &["ratingValue"]).and_then(as_f64).or_else(|| as_f64(r));
        let best = get(r, &["bestRating"]).and_then(as_f64).filter(|b| *b > 0.0).unwrap_or(5.0);
        if let Some(val) = val {
            d.rating = Some((val / best * 5.0).round() as i64);
        }
    } else if let Some(r) = get(v, &["rating"]).and_then(|r| get(r, &["ratingValue"]).or(Some(r))).and_then(as_f64) {
        d.rating = Some(r.round() as i64);
    }

    if let Some(k) = get(v, &["keywords"]) {
        d.tags.extend(name_list(k));
    }
    if let Some(c) = get(v, &["recipeCuisine"]) {
        d.tags.extend(name_list(c));
    }
    if let Some(c) = get(v, &["recipeCategory"]) {
        d.categories.extend(name_list(c));
    }
    if let Some(n) = get(v, &["nutrition"]) {
        nutrition_from(n, &mut d.nutrition);
    }
    if let Some(n) = text_of(v, &["notes", "comment"]) {
        d.notes = clean_block(&n);
    }
    d
}

/// Converts one schema.org Recipe JSON object into an [`ImportedRecipe`].
/// Returns `None` if `v` is not a Recipe.
pub fn recipe_from_json(v: &Value, base_url: Option<&str>) -> Option<ImportedRecipe> {
    if !has_type(v, "Recipe") {
        return None;
    }
    let mut data = recipe_data_from_json(v, base_url);
    data.normalize();
    let image = get(v, &["image", "thumbnailUrl", "photo"]).and_then(|i| image_url(i, base_url)).map(ImageSource::Url);
    let created_at = text_of(v, &["dateCreated"]).and_then(|s| to_rfc3339(&s));
    Some(ImportedRecipe { data, image, created_at, favorite: false, trial: None })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn durations() {
        assert_eq!(parse_iso8601_duration("PT1H20M"), Some(80));
        assert_eq!(parse_iso8601_duration("PT90M"), Some(90));
        assert_eq!(parse_iso8601_duration("P0DT1H"), Some(60));
        assert_eq!(parse_iso8601_duration("PT0H15M"), Some(15));
        assert_eq!(parse_iso8601_duration("pt0.5h"), Some(30));
        assert_eq!(parse_iso8601_duration("P1D"), Some(1440));
        assert_eq!(parse_iso8601_duration("PT30S"), Some(1));
        assert_eq!(parse_iso8601_duration("PT0S"), None);
        assert_eq!(parse_iso8601_duration("1 hour 20 mins"), Some(80));
        assert_eq!(parse_iso8601_duration("1 Hour 20 Minutes"), Some(80));
        assert_eq!(parse_iso8601_duration("1h20m"), Some(80));
        assert_eq!(parse_iso8601_duration("1 1/2 hours"), Some(90));
        assert_eq!(parse_iso8601_duration("an hour"), Some(60));
        assert_eq!(parse_iso8601_duration("10-15 minutes"), Some(10));
        assert_eq!(parse_iso8601_duration("90"), Some(90));
        assert_eq!(parse_iso8601_duration("1:30"), Some(90));
        assert_eq!(parse_iso8601_duration("2 Stunden"), Some(120));
        assert_eq!(parse_iso8601_duration("soon"), None);
    }

    #[test]
    fn graph_and_nested_types() {
        let doc = json!({
            "@context": "https://schema.org",
            "@graph": [
                {"@type": "Organization", "name": "Site"},
                {"@type": "WebPage", "mainEntity": {"@type": ["Recipe", "NewsArticle"], "name": "Inner"}},
                {"@type": "http://schema.org/Recipe", "name": "Second"}
            ]
        });
        let names: Vec<_> = find_recipes(&doc).iter().map(|r| r["name"].as_str().unwrap()).collect();
        assert_eq!(names, vec!["Inner", "Second"]);
        assert!(recipe_from_json(&json!({"@type": "Article"}), None).is_none());
    }

    #[test]
    fn full_recipe_mapping() {
        let v = json!({
            "@type": "Recipe",
            "name": "Mom&#39;s <b>Chili</b>",
            "description": "<p>Hearty &amp; warm.</p>",
            "image": [{"@type": "ImageObject", "url": "/img/small.jpg", "width": 100},
                      {"@type": "ImageObject", "url": "/img/big.jpg", "width": 1200}],
            "recipeYield": ["6", "6 bowls"],
            "prepTime": "PT15M",
            "cookTime": "PT1H",
            "keywords": "beef, spicy, Beef",
            "recipeCuisine": ["Tex-Mex"],
            "recipeCategory": "Main Course",
            "aggregateRating": {"ratingValue": "4.6", "ratingCount": "120"},
            "nutrition": {"@type": "NutritionInformation", "calories": "450 kcal", "fatContent": "20 g"},
            "recipeIngredient": ["2 lbs ground beef", "1 (15 oz) can kidney beans", "For the topping:", "1 cup shredded cheddar"],
            "recipeInstructions": [
                {"@type": "HowToSection", "name": "Chili", "itemListElement": [
                    {"@type": "HowToStep", "name": "Brown", "text": "Brown the beef."},
                    {"@type": "HowToStep", "text": "Add beans &amp; simmer."}
                ]},
                {"@type": "HowToSection", "name": "Serve", "itemListElement": [{"@type": "HowToStep", "name": "Top with cheese."}]}
            ],
            "publisher": {"@type": "Organization", "name": "Chili Co"}
        });
        let r = recipe_from_json(&v, Some("https://chili.example/recipes/1")).unwrap();
        let d = &r.data;
        assert_eq!(d.name, "Mom's Chili");
        assert_eq!(d.description, "Hearty & warm.");
        assert_eq!(d.yield_quantity, Some(6.0));
        assert_eq!(d.yield_unit, "bowls");
        assert_eq!((d.prep_time_minutes, d.cook_time_minutes, d.total_time_minutes), (Some(15), Some(60), Some(75)));
        assert_eq!(d.tags, vec!["beef", "spicy", "Tex-Mex"]);
        assert_eq!(d.categories, vec!["Main Course"]);
        assert_eq!(d.rating, Some(5));
        assert_eq!(d.nutrition.get("calories").map(String::as_str), Some("450 kcal"));
        assert_eq!(d.ingredients.len(), 3);
        assert_eq!(d.ingredients[0].unit.as_deref(), Some("lb"));
        assert_eq!(d.ingredients[2].section.as_deref(), Some("For the topping"));
        assert_eq!(d.instructions.len(), 3);
        assert_eq!(d.instructions[0].text, "Brown the beef.");
        assert_eq!(d.instructions[0].section.as_deref(), Some("Chili"));
        assert_eq!(d.instructions[1].text, "Add beans & simmer.");
        assert_eq!(d.instructions[2].section.as_deref(), Some("Serve"));
        assert_eq!(d.source_url.as_deref(), Some("https://chili.example/recipes/1"));
        assert_eq!(d.source_name.as_deref(), Some("Chili Co"));
        match r.image {
            Some(ImageSource::Url(u)) => assert_eq!(u, "https://chili.example/img/big.jpg"),
            other => panic!("unexpected image {other:?}"),
        }
    }

    #[test]
    fn loose_shapes() {
        let v = json!({
            "@type": "Recipe",
            "name": "Toast",
            "recipeYield": 2,
            "totalTime": "5 minutes",
            "ingredients": "2 slices bread\n1 tbsp butter",
            "recipeInstructions": "1. Toast the bread.\n2. Butter it.",
            "image": {"url": "https://x.example/t.jpg"},
            "keywords": ["breakfast", "quick"]
        });
        let r = recipe_from_json(&v, None).unwrap();
        assert_eq!(r.data.yield_quantity, Some(2.0));
        assert_eq!(r.data.total_time_minutes, Some(5));
        assert_eq!(r.data.ingredients.len(), 2);
        assert_eq!(r.data.instructions.iter().map(|i| i.text.as_str()).collect::<Vec<_>>(), vec!["Toast the bread.", "Butter it."]);
        assert!(matches!(r.image, Some(ImageSource::Url(ref u)) if u == "https://x.example/t.jpg"));
        assert_eq!(r.data.tags, vec!["breakfast", "quick"]);

        // strings in arrays of a single HTML blob with <li> items
        let v = json!({"@type": "Recipe", "name": "X", "recipeInstructions": ["<ol><li>One</li><li>Two</li></ol>"]});
        let r = recipe_from_json(&v, None).unwrap();
        assert_eq!(r.data.instructions.len(), 2);
    }
}
