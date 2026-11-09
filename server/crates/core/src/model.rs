use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

/// A single ingredient line. `original` always carries the text as entered;
/// the structured fields are best-effort parses of it.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Ingredient {
    pub section: Option<String>,
    pub quantity: Option<f64>,
    pub quantity_max: Option<f64>,
    pub unit: Option<String>,
    pub food: Option<String>,
    pub note: Option<String>,
    pub original: String,
}

impl Ingredient {
    /// Builds an ingredient by parsing a free-text line.
    pub fn parse(line: &str) -> Self {
        crate::parse::parse_ingredient(line)
    }

    /// Fills in structured fields from `original` when they are all missing.
    pub fn ensure_parsed(&mut self) {
        if self.quantity.is_none() && self.unit.is_none() && self.food.is_none() && !self.original.trim().is_empty() {
            let parsed = crate::parse::parse_ingredient(&self.original);
            self.quantity = parsed.quantity;
            self.quantity_max = parsed.quantity_max;
            self.unit = parsed.unit;
            self.food = parsed.food;
            if self.note.is_none() {
                self.note = parsed.note;
            }
        }
        if self.original.trim().is_empty() {
            self.original = crate::format::ingredient_display(self, 1.0, crate::units::UnitSystem::Original);
        }
    }

    /// Tidies a parsed ingredient: normalises the food name (spelling,
    /// synonyms, capitalisation; preparation and size words move to the
    /// note) and, when the line was understood, rewrites `original` in one
    /// standard form ("2 tbsp butter, melted") so every recipe spells units
    /// and foods the same way. Lines the parser could not structure are left
    /// exactly as written.
    pub fn canonicalize(&mut self) {
        let Some(food) = self.food.clone() else { return };
        // Lines like "egg wash: 1 egg beaten with milk" are sub-recipes or
        // instructions, not a single ingredient; leave them as written.
        if food.contains(':') || (self.quantity.is_none() && food.chars().any(|c| c.is_ascii_digit())) {
            return;
        }
        let Some(nf) = crate::food::normalize_food(&food) else { return };
        self.food = Some(nf.name);
        let mut parts: Vec<String> = vec![];
        for extra in [nf.descriptors, self.note.take(), nf.remark].into_iter().flatten() {
            let extra = extra.trim().trim_matches(',').trim().to_string();
            if extra.is_empty() || parts.iter().any(|p| p.eq_ignore_ascii_case(&extra)) {
                continue;
            }
            parts.push(extra);
        }
        self.note = if parts.is_empty() { None } else { Some(parts.join(", ")) };
        // A unit without a quantity ("cup of joy") or free-text unit means the
        // parse is uncertain; keep the author's wording then.
        let unit_ok = self.unit.as_deref().is_none_or(|u| crate::units::unit_def(u).is_some());
        if unit_ok && (self.quantity.is_some() || self.unit.is_none()) {
            self.original = crate::format::ingredient_display(self, 1.0, crate::units::UnitSystem::Original);
        }
    }

    /// Singular, accent-folded key identifying the food (for merging/search).
    pub fn food_key(&self) -> Option<String> {
        self.food.as_deref().map(crate::food::food_key)
    }

    /// Human readable form, optionally scaled and converted.
    pub fn display(&self, scale: f64, system: crate::units::UnitSystem) -> String {
        crate::format::ingredient_display(self, scale, system)
    }
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Instruction {
    pub section: Option<String>,
    pub text: String,
}

/// The user-editable content of a recipe (the API's `RecipeInput`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct RecipeData {
    pub name: String,
    pub description: String,
    pub yield_quantity: Option<f64>,
    pub yield_unit: String,
    pub prep_time_minutes: Option<i64>,
    pub cook_time_minutes: Option<i64>,
    pub total_time_minutes: Option<i64>,
    pub source_url: Option<String>,
    pub source_name: Option<String>,
    pub notes: String,
    pub rating: Option<i64>,
    pub ingredients: Vec<Ingredient>,
    pub instructions: Vec<Instruction>,
    pub tags: Vec<String>,
    pub categories: Vec<String>,
    pub nutrition: BTreeMap<String, String>,
}

impl Default for RecipeData {
    fn default() -> Self {
        Self {
            name: String::new(),
            description: String::new(),
            yield_quantity: None,
            yield_unit: "servings".into(),
            prep_time_minutes: None,
            cook_time_minutes: None,
            total_time_minutes: None,
            source_url: None,
            source_name: None,
            notes: String::new(),
            rating: None,
            ingredients: vec![],
            instructions: vec![],
            tags: vec![],
            categories: vec![],
            nutrition: BTreeMap::new(),
        }
    }
}

impl RecipeData {
    /// Normalises user/imported data: trims strings, removes empty entries,
    /// dedupes tags, parses ingredients, clamps rating and computes total time.
    pub fn normalize(&mut self) {
        self.name = crate::text::collapse_ws(&self.name);
        self.description = self.description.trim().to_string();
        self.notes = self.notes.trim().to_string();
        if self.yield_unit.trim().is_empty() {
            self.yield_unit = "servings".into();
        } else {
            self.yield_unit = self.yield_unit.trim().to_string();
        }
        if let Some(q) = self.yield_quantity {
            if !q.is_finite() || q <= 0.0 {
                self.yield_quantity = None;
            }
        }
        for t in [&mut self.prep_time_minutes, &mut self.cook_time_minutes, &mut self.total_time_minutes] {
            if matches!(t, Some(v) if *v <= 0) {
                *t = None;
            }
        }
        if self.total_time_minutes.is_none() {
            let sum = self.prep_time_minutes.unwrap_or(0) + self.cook_time_minutes.unwrap_or(0);
            if sum > 0 {
                self.total_time_minutes = Some(sum);
            }
        }
        self.rating = self.rating.map(|r| r.clamp(0, 5)).filter(|r| *r > 0);
        self.source_url = clean_opt(self.source_url.take());
        self.source_name = clean_opt(self.source_name.take());
        self.ingredients.retain(|i| !i.original.trim().is_empty() || i.food.as_deref().is_some_and(|f| !f.trim().is_empty()));
        for ing in &mut self.ingredients {
            ing.original = crate::text::collapse_ws(&ing.original);
            ing.section = clean_opt(ing.section.take());
            ing.unit = clean_opt(ing.unit.take()).map(|u| crate::units::canonical_unit(&u).map(|c| c.to_string()).unwrap_or(u));
            ing.food = clean_opt(ing.food.take());
            ing.note = clean_opt(ing.note.take());
            ing.ensure_parsed();
            ing.canonicalize();
        }
        self.instructions.retain(|i| !i.text.trim().is_empty());
        for ins in &mut self.instructions {
            ins.text = ins.text.trim().to_string();
            ins.section = clean_opt(ins.section.take());
        }
        self.tags = dedupe(std::mem::take(&mut self.tags));
        self.categories = dedupe(std::mem::take(&mut self.categories));
        self.nutrition.retain(|k, v| !k.trim().is_empty() && !v.trim().is_empty());
    }

    /// Validation errors, if any.
    pub fn validate(&self) -> Result<(), String> {
        if self.name.trim().is_empty() {
            return Err("recipe name is required".into());
        }
        if self.name.chars().count() > 300 {
            return Err("recipe name is too long".into());
        }
        Ok(())
    }

    /// Lower-cased text used for search.
    pub fn search_text(&self) -> String {
        let mut s = String::new();
        s.push_str(&self.name);
        s.push(' ');
        s.push_str(&self.description);
        for i in &self.ingredients {
            s.push(' ');
            s.push_str(i.food.as_deref().unwrap_or(&i.original));
        }
        for t in self.tags.iter().chain(&self.categories) {
            s.push(' ');
            s.push_str(t);
        }
        if let Some(src) = &self.source_name {
            s.push(' ');
            s.push_str(src);
        }
        crate::text::fold(&s)
    }
}

fn clean_opt(s: Option<String>) -> Option<String> {
    s.map(|s| s.trim().to_string()).filter(|s| !s.is_empty())
}

fn dedupe(items: Vec<String>) -> Vec<String> {
    let mut seen = std::collections::HashSet::new();
    let mut out = vec![];
    for t in items {
        let t = crate::text::collapse_ws(&t);
        if t.is_empty() || t.chars().count() > 80 {
            continue;
        }
        if seen.insert(t.to_lowercase()) {
            out.push(t);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalize_fills_fields() {
        let mut r = RecipeData {
            name: "  Pancakes  ".into(),
            prep_time_minutes: Some(10),
            cook_time_minutes: Some(15),
            rating: Some(9),
            tags: vec!["Quick".into(), "quick".into(), " ".into()],
            ingredients: vec![
                Ingredient { original: "2 cups flour".into(), ..Default::default() },
                Ingredient { original: "  ".into(), ..Default::default() },
            ],
            ..Default::default()
        };
        r.normalize();
        assert_eq!(r.name, "Pancakes");
        assert_eq!(r.total_time_minutes, Some(25));
        assert_eq!(r.rating, Some(5));
        assert_eq!(r.tags, vec!["Quick"]);
        assert_eq!(r.ingredients.len(), 1);
        assert_eq!(r.ingredients[0].unit.as_deref(), Some("cup"));
        assert_eq!(r.ingredients[0].food.as_deref(), Some("flour"));
    }

    #[test]
    fn ingredient_without_original_gets_display() {
        let mut i = Ingredient { quantity: Some(2.0), unit: Some("cup".into()), food: Some("milk".into()), ..Default::default() };
        i.ensure_parsed();
        assert_eq!(i.original, "2 cups milk");
    }
}
