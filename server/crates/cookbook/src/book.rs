//! Turns recipes + options into the JSON book model consumed by the template:
//! chapter grouping/ordering, pre-formatted ingredients and metadata, and the
//! index structure (page numbers are resolved by Typst at layout time).

use std::collections::{BTreeMap, HashMap};

use larder_core::format::{format_minutes, format_quantity_unicode, scaled_parts, unit_label};
use larder_core::units::UnitSystem;
use larder_core::{Ingredient, RecipeData};
use serde::Serialize;

use crate::images::PreparedImage;
use crate::text::{
    capitalize_first, index_letter, note_items, nutrition_label, nutrition_rank, paragraphs, smart_quotes, sort_key, tidy,
    unicode_fractions, url_host,
};
use crate::{BookRecipe, CookbookOptions, GroupBy};

/// Name of the chapter collecting recipes without a category/tag.
pub const OTHER_CHAPTER: &str = "Other Recipes";
/// Name of the single chapter when grouping is disabled.
pub const ALL_CHAPTER: &str = "Recipes";

#[derive(Debug, Serialize)]
pub struct Book {
    pub theme: &'static str,
    pub paper: &'static str,
    pub title: String,
    pub subtitle: Option<String>,
    pub author: Option<String>,
    pub dedication: Vec<String>,
    pub toc: bool,
    pub index: bool,
    pub grouped: bool,
    pub recipe_count: usize,
    pub chapters: Vec<Chapter>,
    pub index_groups: Vec<IndexGroup>,
}

#[derive(Debug, Serialize)]
pub struct Chapter {
    pub key: String,
    pub number: usize,
    pub title: String,
    pub recipes: Vec<Recipe>,
}

#[derive(Debug, Serialize)]
pub struct Recipe {
    pub key: String,
    pub name: String,
    pub description: Vec<String>,
    pub meta: Vec<Meta>,
    pub image: Option<ImageRef>,
    pub ingredients: Vec<IngredientGroup>,
    pub steps: Vec<StepGroup>,
    pub notes: Vec<String>,
    pub nutrition: Vec<Meta>,
    pub source: Option<Source>,
}

#[derive(Debug, Serialize)]
pub struct Meta {
    pub label: String,
    pub value: String,
}

#[derive(Debug, Serialize)]
pub struct ImageRef {
    pub path: String,
    /// width / height
    pub aspect: f64,
}

#[derive(Debug, Serialize)]
pub struct IngredientGroup {
    pub title: Option<String>,
    pub items: Vec<IngredientLine>,
}

#[derive(Debug, Serialize)]
pub struct IngredientLine {
    /// Quantity and unit ("1½ cups"), may be empty.
    pub amount: String,
    /// The rest of the line ("flour, sifted").
    pub item: String,
}

#[derive(Debug, Serialize)]
pub struct StepGroup {
    pub title: Option<String>,
    pub steps: Vec<String>,
}

#[derive(Debug, Serialize)]
pub struct Source {
    pub name: Option<String>,
    pub host: Option<String>,
    pub url: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct IndexGroup {
    pub letter: String,
    pub entries: Vec<IndexEntry>,
}

/// Either a recipe (`key` set) or a headword (ingredient/tag) with sub-entries.
#[derive(Debug, Serialize)]
pub struct IndexEntry {
    pub term: String,
    pub key: Option<String>,
    pub subs: Vec<IndexSub>,
}

#[derive(Debug, Serialize)]
pub struct IndexSub {
    pub name: String,
    pub key: String,
}

pub fn recipe_key(i: usize) -> String {
    format!("r{i}")
}

pub fn image_path(i: usize) -> String {
    format!("/img/{i}.jpg")
}

fn display_name(r: &RecipeData) -> String {
    let n = larder_core::text::collapse_ws(&r.name);
    if n.is_empty() {
        "Untitled Recipe".into()
    } else {
        smart_quotes(&n)
    }
}

fn chapter_label(s: &str) -> String {
    let s = larder_core::text::collapse_ws(s);
    if s.chars().any(|c| c.is_uppercase()) {
        s
    } else {
        capitalize_first(&s)
    }
}

/// Splits recipes into chapters: returns `(chapter title, recipe indices)` in
/// book order. Recipes are sorted alphabetically within a chapter; chapters
/// named in `opts.chapter_order` come first (in that order, case-insensitive),
/// then the rest alphabetically, then the catch-all chapter for recipes
/// without a category/tag (unless `chapter_order` places it explicitly).
pub fn group_recipes(opts: &CookbookOptions, recipes: &[BookRecipe]) -> Vec<(String, Vec<usize>)> {
    // lower-cased chapter key -> (display title, indices)
    let mut groups: HashMap<String, (String, Vec<usize>)> = HashMap::new();
    let mut other: Vec<usize> = vec![];
    for (i, r) in recipes.iter().enumerate() {
        let first = match opts.group_by {
            GroupBy::Category => r.data.categories.iter().find(|c| !c.trim().is_empty()),
            GroupBy::Tag => r.data.tags.iter().find(|c| !c.trim().is_empty()),
            GroupBy::None => None,
        };
        match first {
            Some(name) => {
                let title = chapter_label(name);
                groups.entry(title.to_lowercase()).or_insert_with(|| (title, vec![])).1.push(i);
            }
            None => other.push(i),
        }
    }
    // Nothing to group by (e.g. a collection without categories): one plain
    // "Recipes" chapter rather than a lone "Other Recipes" catch-all.
    if opts.group_by == GroupBy::None || groups.is_empty() {
        groups.clear();
        let title = ALL_CHAPTER.to_string();
        groups.insert(title.to_lowercase(), (title, std::mem::take(&mut other)));
    } else if !other.is_empty() {
        let key = OTHER_CHAPTER.to_lowercase();
        match groups.get_mut(&key) {
            Some(g) => g.1.append(&mut other),
            None => {
                groups.insert(key, (OTHER_CHAPTER.to_string(), std::mem::take(&mut other)));
            }
        }
    }

    let mut out: Vec<(String, Vec<usize>)> = vec![];
    for wanted in &opts.chapter_order {
        let key = larder_core::text::collapse_ws(wanted).to_lowercase();
        if let Some(g) = groups.remove(&key) {
            out.push(g);
        }
    }
    let catch_all = groups.remove(&OTHER_CHAPTER.to_lowercase());
    let mut rest: Vec<(String, Vec<usize>)> = groups.into_values().collect();
    // Larder's suggested categories come in meal order (Breakfast … Desserts,
    // Drinks); any other chapter names follow alphabetically.
    let meal_rank = |t: &str| larder_core::categorize::CATEGORIES.iter().position(|c| c.eq_ignore_ascii_case(t)).unwrap_or(usize::MAX);
    rest.sort_by(|a, b| {
        meal_rank(&a.0).cmp(&meal_rank(&b.0)).then_with(|| sort_key(&a.0).cmp(&sort_key(&b.0))).then_with(|| a.0.cmp(&b.0))
    });
    out.extend(rest);
    out.extend(catch_all);

    for (_, idx) in &mut out {
        idx.sort_by(|&a, &b| {
            let (na, nb) = (display_name(&recipes[a].data), display_name(&recipes[b].data));
            sort_key(&na).cmp(&sort_key(&nb)).then_with(|| na.cmp(&nb)).then(a.cmp(&b))
        });
    }
    out.retain(|(_, idx)| !idx.is_empty());
    out
}

/// Splits a formatted ingredient into amount ("1½ cups") and item ("flour, sifted").
fn ingredient_line(ing: &Ingredient, units: UnitSystem) -> IngredientLine {
    let full = larder_core::format::ingredient_display(ing, 1.0, units);
    let full = larder_core::text::collapse_ws(&full);
    let mut amount = String::new();
    if ing.quantity.is_some() && (ing.food.is_some() || ing.unit.is_some()) {
        let (q, qmax, unit) = scaled_parts(ing, 1.0, units);
        let mut parts: Vec<String> = vec![];
        if let Some(q) = q {
            match qmax {
                Some(m) if m > q => {
                    parts.push(format!("{}–{}", larder_core::format::format_quantity(q), larder_core::format::format_quantity(m)))
                }
                _ => parts.push(larder_core::format::format_quantity(q)),
            }
        }
        if let Some(u) = unit {
            parts.push(unit_label(&u, qmax.or(q)));
        }
        let candidate = parts.join(" ");
        if !candidate.is_empty() && full.starts_with(&candidate) {
            amount = candidate;
        }
    }
    let item = full[amount.len()..].trim().trim_start_matches(',').trim().to_string();
    let (amount, item) = if item.is_empty() { (String::new(), full.clone()) } else { (amount, item) };
    IngredientLine { amount: unicode_fractions(&amount), item: smart_quotes(&unicode_fractions(&item)) }
}

fn meta(r: &RecipeData) -> Vec<Meta> {
    let mut out = vec![];
    if let Some(q) = r.yield_quantity.filter(|q| q.is_finite() && *q > 0.0) {
        let unit = r.yield_unit.trim();
        let lower = unit.to_lowercase();
        let qs = format_quantity_unicode(q);
        if unit.is_empty() || ["serving", "servings", "people", "persons", "portions", "portion"].contains(&lower.as_str()) {
            out.push(Meta { label: "Serves".into(), value: qs });
        } else {
            out.push(Meta { label: "Makes".into(), value: format!("{qs} {}", smart_quotes(unit)) });
        }
    }
    let total = r.total_time_minutes.or_else(|| {
        let s = r.prep_time_minutes.unwrap_or(0) + r.cook_time_minutes.unwrap_or(0);
        (s > 0).then_some(s)
    });
    for (label, v) in [("Prep", r.prep_time_minutes), ("Cook", r.cook_time_minutes), ("Total", total)] {
        if let Some(m) = v.filter(|m| *m > 0) {
            out.push(Meta { label: label.into(), value: format_minutes(m) });
        }
    }
    out
}

fn ingredient_groups(r: &RecipeData, units: UnitSystem) -> Vec<IngredientGroup> {
    let mut groups: Vec<IngredientGroup> = vec![];
    for ing in &r.ingredients {
        if ing.original.trim().is_empty() && ing.food.as_deref().is_none_or(|f| f.trim().is_empty()) {
            continue;
        }
        let section = ing.section.as_deref().map(str::trim).filter(|s| !s.is_empty()).map(tidy);
        let line = ingredient_line(ing, units);
        if line.item.is_empty() && line.amount.is_empty() {
            continue;
        }
        match groups.last_mut() {
            Some(g) if g.title == section => g.items.push(line),
            _ => groups.push(IngredientGroup { title: section, items: vec![line] }),
        }
    }
    groups
}

fn step_groups(r: &RecipeData) -> Vec<StepGroup> {
    let mut groups: Vec<StepGroup> = vec![];
    for ins in &r.instructions {
        let text = unicode_fractions(&tidy(&larder_core::text::collapse_ws(&ins.text)));
        if text.is_empty() {
            continue;
        }
        let section = ins.section.as_deref().map(str::trim).filter(|s| !s.is_empty()).map(tidy);
        match groups.last_mut() {
            Some(g) if g.title == section => g.steps.push(text),
            _ => groups.push(StepGroup { title: section, steps: vec![text] }),
        }
    }
    groups
}

fn source(r: &RecipeData) -> Option<Source> {
    let name = r.source_name.as_deref().map(str::trim).filter(|s| !s.is_empty()).map(tidy);
    let url = r.source_url.as_deref().map(str::trim).filter(|s| !s.is_empty());
    let host = url.and_then(url_host);
    let url = if host.is_some() { url.map(str::to_string) } else { None };
    if name.is_none() && host.is_none() {
        return None;
    }
    Some(Source { name, host, url })
}

fn nutrition(r: &RecipeData) -> Vec<Meta> {
    let mut items: Vec<(&String, &String)> = r.nutrition.iter().filter(|(k, v)| !k.trim().is_empty() && !v.trim().is_empty()).collect();
    items.sort_by_key(|(k, _)| nutrition_rank(k));
    items.into_iter().map(|(k, v)| Meta { label: nutrition_label(k), value: tidy(v) }).collect()
}

const STAPLES: &[&str] = &[
    "salt",
    "kosher salt",
    "sea salt",
    "table salt",
    "fine salt",
    "flaky salt",
    "flaky sea salt",
    "pepper",
    "black pepper",
    "ground black pepper",
    "freshly ground black pepper",
    "ground pepper",
    "white pepper",
    "salt and pepper",
    "salt and black pepper",
    "water",
    "cold water",
    "warm water",
    "hot water",
    "boiling water",
    "ice water",
    "ice",
    "ice cubes",
    "oil",
    "olive oil",
    "extra-virgin olive oil",
    "extra virgin olive oil",
    "vegetable oil",
    "canola oil",
    "neutral oil",
    "cooking oil",
    "cooking spray",
    "nonstick cooking spray",
    "sugar",
    "granulated sugar",
    "white sugar",
    "flour",
    "all-purpose flour",
    "all purpose flour",
    "plain flour",
    "butter",
    "unsalted butter",
    "salted butter",
    "baking soda",
    "baking powder",
];

/// Leading words dropped from index headwords ("large eggs" → "eggs").
const DESCRIPTORS: &[&str] = &[
    "large",
    "small",
    "medium",
    "big",
    "fresh",
    "freshly",
    "dried",
    "chopped",
    "finely",
    "roughly",
    "coarsely",
    "whole",
    "ripe",
    "thick-cut",
    "thin",
    "thinly",
    "sliced",
    "diced",
    "minced",
    "grated",
    "shredded",
    "softened",
    "melted",
    "canned",
    "tinned",
    "frozen",
    "cooked",
    "raw",
    "boneless",
    "skinless",
    "extra-virgin",
    "organic",
    "good-quality",
    "good",
    "quality",
    "cold",
    "warm",
    "hot",
    "unsalted",
    "salted",
    "peeled",
    "lean",
    "crushed",
    "a",
    "an",
    "some",
    "few",
    "handful",
    "pinch",
];

/// Normalised index headword for an ingredient food, or `None` for staples/noise.
fn ingredient_term(food: &str) -> Option<String> {
    let f = larder_core::text::collapse_ws(food).to_lowercase();
    let f = f.trim_matches(|c: char| !c.is_alphanumeric());
    let mut words: Vec<&str> = f.split(' ').collect();
    while words.len() > 1 && DESCRIPTORS.contains(&words[0]) {
        words.remove(0);
    }
    let f = words.join(" ");
    if f.chars().count() < 2 || f.chars().count() > 32 || f.split(' ').count() > 4 {
        return None;
    }
    if STAPLES.contains(&f.as_str()) || f.chars().any(|c| c.is_ascii_digit()) {
        return None;
    }
    Some(f)
}

fn build_index(book_order: &[(usize, &RecipeData, String)]) -> Vec<IndexGroup> {
    // headword key -> (display, [(name, key)])
    let mut heads: BTreeMap<String, (String, Vec<(String, String)>)> = BTreeMap::new();
    let mut entries: Vec<IndexEntry> = vec![];
    for (i, r, name) in book_order {
        let key = recipe_key(*i);
        entries.push(IndexEntry { term: name.clone(), key: Some(key.clone()), subs: vec![] });
        let mut terms: Vec<String> = vec![];
        for ing in &r.ingredients {
            if let Some(t) = ing.food.as_deref().and_then(ingredient_term) {
                if !terms.contains(&t) && terms.len() < 8 {
                    terms.push(t);
                }
            }
        }
        for tag in &r.tags {
            let t = larder_core::text::collapse_ws(tag).to_lowercase();
            if !t.is_empty() && t.chars().count() <= 40 && !terms.contains(&t) {
                terms.push(t);
            }
        }
        for t in terms {
            let e = heads.entry(t.clone()).or_insert_with(|| (capitalize_first(&smart_quotes(&t)), vec![]));
            if !e.1.iter().any(|(_, k)| *k == key) {
                e.1.push((name.clone(), key.clone()));
            }
        }
    }
    // Merge simple singular/plural pairs ("egg" into "eggs").
    let keys: Vec<String> = heads.keys().cloned().collect();
    for k in keys {
        for plural in [format!("{k}s"), format!("{k}es")] {
            if heads.contains_key(&plural) {
                if let Some((_, subs)) = heads.remove(&k) {
                    let target = heads.get_mut(&plural).expect("present");
                    for s in subs {
                        if !target.1.iter().any(|(_, k)| *k == s.1) {
                            target.1.push(s);
                        }
                    }
                }
                break;
            }
        }
    }
    for (_, (display, mut subs)) in heads {
        subs.sort_by_cached_key(|s| sort_key(&s.0));
        // A headword identical to a single recipe's name adds nothing.
        if subs.len() == 1 && sort_key(&subs[0].0) == sort_key(&display) {
            continue;
        }
        entries.push(IndexEntry { term: display, key: None, subs: subs.into_iter().map(|(name, key)| IndexSub { name, key }).collect() });
    }
    entries.sort_by(|a, b| {
        sort_key(&a.term).cmp(&sort_key(&b.term)).then_with(|| a.key.is_none().cmp(&b.key.is_none())).then_with(|| a.term.cmp(&b.term))
    });
    let mut groups: Vec<IndexGroup> = vec![];
    for e in entries {
        let letter = index_letter(&e.term);
        match groups.last_mut() {
            Some(g) if g.letter == letter => g.entries.push(e),
            _ => groups.push(IndexGroup { letter, entries: vec![e] }),
        }
    }
    // Symbols/digits first.
    groups.sort_by_key(|g| g.letter != "#");
    groups
}

pub fn build(opts: &CookbookOptions, recipes: &[BookRecipe], images: &[Option<PreparedImage>]) -> Book {
    let grouped = group_recipes(opts, recipes);
    let mut chapters = vec![];
    let mut order: Vec<(usize, &RecipeData, String)> = vec![];
    for (n, (title, idx)) in grouped.into_iter().enumerate() {
        let mut out = vec![];
        for i in idx {
            let r = &recipes[i].data;
            let name = display_name(r);
            order.push((i, r, name.clone()));
            let image = images
                .get(i)
                .and_then(|im| im.as_ref())
                .map(|im| ImageRef { path: image_path(i), aspect: im.width as f64 / im.height.max(1) as f64 });
            out.push(Recipe {
                key: recipe_key(i),
                name,
                description: paragraphs(&r.description),
                meta: meta(r),
                image,
                ingredients: ingredient_groups(r, opts.units),
                steps: step_groups(r),
                notes: if opts.include_notes { note_items(&r.notes).iter().map(|n| unicode_fractions(n)).collect() } else { vec![] },
                nutrition: if opts.include_nutrition { nutrition(r) } else { vec![] },
                source: source(r),
            });
        }
        chapters.push(Chapter { key: format!("c{n}"), number: n + 1, title: smart_quotes(&title), recipes: out });
    }
    let opt_text = |s: &Option<String>| s.as_deref().map(str::trim).filter(|s| !s.is_empty()).map(tidy);
    let title = {
        let t = larder_core::text::collapse_ws(&opts.title);
        if t.is_empty() {
            "Our Family Cookbook".to_string()
        } else {
            smart_quotes(&t)
        }
    };
    Book {
        theme: crate::theme_id(&opts.theme),
        paper: crate::paper_id(&opts.paper),
        title,
        subtitle: opt_text(&opts.subtitle),
        author: opt_text(&opts.author),
        dedication: opts
            .dedication
            .as_deref()
            .map(|d| d.trim().lines().map(|l| tidy(&larder_core::text::collapse_ws(l))).collect())
            .unwrap_or_default(),
        toc: opts.include_toc && !recipes.is_empty(),
        index: opts.include_index && !recipes.is_empty(),
        grouped: opts.group_by != GroupBy::None,
        recipe_count: recipes.len(),
        index_groups: if opts.include_index { build_index(&order) } else { vec![] },
        chapters,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn recipe(name: &str, cats: &[&str], tags: &[&str]) -> BookRecipe {
        BookRecipe {
            id: name.to_lowercase(),
            data: RecipeData {
                name: name.into(),
                categories: cats.iter().map(|s| s.to_string()).collect(),
                tags: tags.iter().map(|s| s.to_string()).collect(),
                ..Default::default()
            },
            image: None,
        }
    }

    fn titles(ch: &[(String, Vec<usize>)], rs: &[BookRecipe]) -> Vec<(String, Vec<String>)> {
        ch.iter().map(|(t, idx)| (t.clone(), idx.iter().map(|&i| rs[i].data.name.clone()).collect())).collect()
    }

    #[test]
    fn groups_by_first_category_with_other_last() {
        let rs = vec![
            recipe("Zucchini Bread", &["Breads", "Breakfast"], &[]),
            recipe("Apple Pie", &["desserts"], &[]),
            recipe("Pancakes", &["Breakfast"], &[]),
            recipe("Mystery Stew", &[], &[]),
            recipe("Brownies", &["Desserts"], &[]),
            recipe("éclairs", &["Desserts"], &[]),
        ];
        let opts = CookbookOptions::default();
        let got = titles(&group_recipes(&opts, &rs), &rs);
        assert_eq!(
            got,
            vec![
                ("Breakfast".to_string(), vec!["Pancakes".to_string()]),
                ("Breads".to_string(), vec!["Zucchini Bread".to_string()]),
                ("Desserts".to_string(), vec!["Apple Pie".into(), "Brownies".into(), "éclairs".into()]),
                (OTHER_CHAPTER.to_string(), vec!["Mystery Stew".to_string()]),
            ]
        );
    }

    #[test]
    fn respects_chapter_order() {
        let rs = vec![
            recipe("A", &["Mains"], &[]),
            recipe("B", &["Breakfast"], &[]),
            recipe("C", &["Desserts"], &[]),
            recipe("D", &["Appetizers"], &[]),
            recipe("E", &[], &[]),
        ];
        let opts =
            CookbookOptions { chapter_order: vec!["desserts".into(), "Nonexistent".into(), "BREAKFAST".into()], ..Default::default() };
        let got: Vec<String> = group_recipes(&opts, &rs).into_iter().map(|c| c.0).collect();
        assert_eq!(got, vec!["Desserts", "Breakfast", "Appetizers", "Mains", OTHER_CHAPTER]);

        let opts = CookbookOptions { chapter_order: vec!["other recipes".into(), "Mains".into()], ..Default::default() };
        let got: Vec<String> = group_recipes(&opts, &rs).into_iter().map(|c| c.0).collect();
        // Remaining chapters: Larder categories in meal order, then the rest alphabetically.
        assert_eq!(got, vec![OTHER_CHAPTER, "Mains", "Breakfast", "Desserts", "Appetizers"]);
    }

    #[test]
    fn suggested_categories_use_meal_order() {
        let rs = vec![
            recipe("Cake", &["Desserts"], &[]),
            recipe("Pancakes", &["Breakfast"], &[]),
            recipe("Tea", &["Drinks"], &[]),
            recipe("Odd", &["Zebra Food"], &[]),
            recipe("Soup", &["Soups & Stews"], &[]),
            recipe("Apple Thing", &["Apples"], &[]),
        ];
        let got: Vec<String> = group_recipes(&CookbookOptions::default(), &rs).into_iter().map(|c| c.0).collect();
        assert_eq!(got, vec!["Breakfast", "Soups & Stews", "Desserts", "Drinks", "Apples", "Zebra Food"]);
    }

    #[test]
    fn no_categories_anywhere_gives_one_recipes_chapter() {
        let rs = vec![recipe("Soup", &[], &[]), recipe("Bread", &[], &[])];
        let got = group_recipes(&CookbookOptions::default(), &rs);
        assert_eq!(got.len(), 1);
        assert_eq!(got[0].0, ALL_CHAPTER);
    }

    #[test]
    fn groups_by_tag_and_none() {
        let rs =
            vec![recipe("Soup", &["Mains"], &["winter", "quick"]), recipe("Salad", &["Sides"], &["Summer"]), recipe("Toast", &[], &[])];
        let opts = CookbookOptions { group_by: GroupBy::Tag, ..Default::default() };
        let got: Vec<String> = group_recipes(&opts, &rs).into_iter().map(|c| c.0).collect();
        assert_eq!(got, vec!["Summer", "Winter", OTHER_CHAPTER]);

        let opts = CookbookOptions { group_by: GroupBy::None, ..Default::default() };
        let got = titles(&group_recipes(&opts, &rs), &rs);
        assert_eq!(got, vec![(ALL_CHAPTER.to_string(), vec!["Salad".into(), "Soup".into(), "Toast".into()])]);

        assert!(group_recipes(&opts, &[]).is_empty());
    }

    #[test]
    fn ingredient_lines_split_amount() {
        let ing = Ingredient::parse("1 1/2 cups flour, sifted");
        let l = ingredient_line(&ing, UnitSystem::Original);
        assert_eq!((l.amount.as_str(), l.item.as_str()), ("1½ cups", "flour, sifted"));
        let ing = Ingredient::parse("Salt to taste");
        let l = ingredient_line(&ing, UnitSystem::Original);
        assert!(l.amount.is_empty());
        assert!(!l.item.is_empty());
        let ing = Ingredient::parse("2 eggs");
        let l = ingredient_line(&ing, UnitSystem::Original);
        assert_eq!((l.amount.as_str(), l.item.as_str()), ("2", "eggs"));
    }

    #[test]
    fn index_merges_plurals_and_skips_staples() {
        let mut a = recipe("Omelette", &[], &["Quick"]);
        a.data.ingredients = vec![Ingredient::parse("2 eggs"), Ingredient::parse("salt")];
        let mut b = recipe("Egg Salad", &[], &[]);
        b.data.ingredients = vec![Ingredient::parse("1 egg"), Ingredient::parse("1 tbsp olive oil")];
        let opts = CookbookOptions::default();
        let book = build(&opts, &[a, b], &[None, None]);
        let terms: Vec<&str> = book.index_groups.iter().flat_map(|g| g.entries.iter().map(|e| e.term.as_str())).collect();
        assert!(terms.contains(&"Eggs"), "{terms:?}");
        assert!(!terms.contains(&"Egg"), "{terms:?}");
        assert!(!terms.iter().any(|t| t.eq_ignore_ascii_case("salt")), "{terms:?}");
        assert!(terms.contains(&"Quick"));
        let eggs = book.index_groups.iter().flat_map(|g| &g.entries).find(|e| e.term == "Eggs").unwrap();
        assert_eq!(eggs.subs.len(), 2);
    }
}
