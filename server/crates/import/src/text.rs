//! Heuristic plain-text recipe parser (no network, no LLM).

use std::sync::LazyLock;

use larder_core::{Instruction, RecipeData};
use regex::Regex;

use crate::schema_org::{ingredients_from_lines, parse_iso8601_duration};
use crate::util::{clean_block, has_step_number, parse_yield_text, resolve_url, strip_bullet, strip_step_number};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Mode {
    Intro,
    Ingredients,
    Instructions,
    Notes,
}

const INGREDIENT_HEADINGS: &[&str] = &[
    "ingredients",
    "ingredient",
    "ingredient list",
    "what you need",
    "what you'll need",
    "you will need",
    "you'll need",
    "shopping list",
    "zutaten",
    "ingrédients",
    "ingredientes",
    "ingredienti",
    "ingrediënten",
];
const INSTRUCTION_HEADINGS: &[&str] = &[
    "directions",
    "direction",
    "instructions",
    "instruction",
    "method",
    "methods",
    "preparation",
    "steps",
    "how to make",
    "how to make it",
    "procedure",
    "to make",
    "cooking instructions",
    "zubereitung",
    "anleitung",
    "préparation",
    "preparación",
    "preparazione",
    "bereiding",
    "instrucciones",
];
const NOTE_HEADINGS: &[&str] = &["notes", "note", "tips", "tip", "cook's notes", "chef's notes", "recipe notes", "notizen", "hinweise"];

/// Normalised form of a potential heading line ("## Ingredients:" -> "ingredients").
fn heading_key(line: &str) -> String {
    let t = line.trim().trim_start_matches('#').trim().trim_matches(|c: char| c == '*' || c == '_' || c == '=').trim();
    let t = t.trim_end_matches(':').trim();
    t.to_lowercase()
}

fn classify_heading(line: &str) -> Option<Mode> {
    let k = heading_key(line);
    if k.is_empty() || k.chars().count() > 40 {
        return None;
    }
    let starts = |list: &[&str]| {
        list.iter().any(|h| {
            k == *h
                || (k.starts_with(h) && {
                    // "Ingredients for 4 people", "Instructions (makes 2)"
                    let rest = &k[h.len()..];
                    rest.starts_with(' ') || rest.starts_with('(')
                })
        })
    };
    if starts(INGREDIENT_HEADINGS) {
        Some(Mode::Ingredients)
    } else if starts(INSTRUCTION_HEADINGS) {
        Some(Mode::Instructions)
    } else if starts(NOTE_HEADINGS) {
        Some(Mode::Notes)
    } else {
        None
    }
}

static META_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"(?i)^(?P<key>serves|servings|serving size|yields?|makes|portions?|prep(?:aration)?(?: time)?|cook(?:ing)?(?: time)?|bake time|baking time|total(?: time)?|ready in|time|source|from|url|category|categories|tags?|cuisine)\s*[:\-–]?\s*(?P<val>.+)$",
    )
    .unwrap()
});

/// Applies a metadata line ("Serves 4", "Prep time: 10 min"); returns true if consumed.
fn apply_meta(line: &str, d: &mut RecipeData) -> bool {
    let line = strip_bullet(line).trim().trim_matches('*').trim();
    if line.chars().count() > 80 {
        return false;
    }
    // Several "Prep: 10 min | Cook: 20 min" on one line.
    if line.contains('|') || line.contains(" · ") {
        let parts: Vec<&str> = line.split(['|', '·']).map(str::trim).filter(|p| !p.is_empty()).collect();
        if parts.len() > 1 {
            let mut any = false;
            for p in parts {
                any |= apply_meta(p, d);
            }
            return any;
        }
    }
    let Some(c) = META_RE.captures(line) else { return false };
    let key = c["key"].to_lowercase();
    let val = c["val"].trim();
    if key.starts_with("serv") || key.starts_with("yield") || key == "makes" || key.starts_with("portion") {
        let (q, u) = parse_yield_text(val);
        if q.is_none() {
            return false;
        }
        d.yield_quantity = q;
        d.yield_unit = u;
        return true;
    }
    if key.starts_with("prep")
        || key.starts_with("cook")
        || key.starts_with("bak")
        || key.starts_with("total")
        || key == "ready in"
        || key == "time"
    {
        let Some(m) = parse_iso8601_duration(val) else { return false };
        if key.starts_with("prep") {
            d.prep_time_minutes = Some(m);
        } else if key.starts_with("cook") || key.starts_with("bak") {
            d.cook_time_minutes = Some(m);
        } else {
            d.total_time_minutes = Some(m);
        }
        return true;
    }
    if key == "source" || key == "from" || key == "url" {
        match resolve_url(val, None) {
            Some(u) => d.source_url = Some(u),
            None => d.source_name = Some(val.to_string()),
        }
        return true;
    }
    if key.starts_with("categor") {
        d.categories.extend(crate::util::split_list(val));
        return true;
    }
    if key.starts_with("tag") || key == "cuisine" {
        d.tags.extend(crate::util::split_list(val));
        return true;
    }
    false
}

fn looks_like_ingredient(line: &str) -> bool {
    let l = strip_bullet(line);
    if l.chars().count() > 80 || l.ends_with('.') && l.split_whitespace().count() > 6 {
        return false;
    }
    let ing = larder_core::parse::parse_ingredient(l);
    ing.quantity.is_some() || line.trim_start().starts_with(['-', '*', '•'])
}

/// Turns instruction lines into steps: numbered lines start new steps and
/// unnumbered lines continue them; without numbering, every line is a step.
fn build_steps(lines: &[String]) -> Vec<Instruction> {
    let numbered = lines.iter().filter(|l| has_step_number(l)).count() >= 2;
    let mut out: Vec<Instruction> = vec![];
    let mut section: Option<String> = None;
    let mut cur: Option<String> = None;
    for raw in lines {
        let line = raw.trim();
        if line.is_empty() {
            if !numbered {
                if let Some(t) = cur.take() {
                    out.push(Instruction { section: section.clone(), text: t });
                }
            }
            continue;
        }
        let heading =
            (line.ends_with(':') && line.chars().count() < 50 && !line.contains(". ") && !has_step_number(line)) || line.starts_with('#');
        if heading {
            if let Some(t) = cur.take() {
                out.push(Instruction { section: section.clone(), text: t });
            }
            section = Some(line.trim_start_matches('#').trim().trim_end_matches(':').trim().to_string()).filter(|s| !s.is_empty());
            continue;
        }
        if numbered && !has_step_number(line) {
            if let Some(c) = cur.as_mut() {
                c.push(' ');
                c.push_str(strip_bullet(line));
                continue;
            }
        }
        if let Some(t) = cur.take() {
            out.push(Instruction { section: section.clone(), text: t });
        }
        cur = Some(strip_step_number(strip_bullet(line)));
    }
    if let Some(t) = cur.take() {
        out.push(Instruction { section, text: t });
    }
    out.retain(|i| !i.text.is_empty());
    out
}

/// Parses a recipe from free text: the first non-empty line is the title;
/// "Ingredients" / "Directions" (and similar) headings split the sections;
/// lines such as "Serves 4" or "Prep time: 10 min" fill in metadata.
pub fn parse_plain_text(text: &str) -> RecipeData {
    let text = if text.contains('<') && text.contains('>') { clean_block(text) } else { text.replace("\r\n", "\n") };
    let mut d = RecipeData::default();
    let mut lines = text.lines().peekable();
    while let Some(l) = lines.peek() {
        if l.trim().is_empty() {
            lines.next();
        } else {
            break;
        }
    }
    if let Some(title) = lines.next() {
        let t = title.trim().trim_start_matches('#').trim();
        let t = t.strip_prefix("Title:").or_else(|| t.strip_prefix("title:")).unwrap_or(t).trim().trim_matches('*').trim();
        if classify_heading(t).is_some() {
            // No title at all: start parsing from this heading.
            d.name = String::new();
        } else {
            d.name = t.to_string();
        }
    }
    let rest: Vec<String> =
        text.lines().skip_while(|l| l.trim().is_empty()).skip(if d.name.is_empty() { 0 } else { 1 }).map(str::to_string).collect();

    let mut mode = Mode::Intro;
    let mut intro: Vec<String> = vec![];
    let mut ing_lines: Vec<String> = vec![];
    let mut step_lines: Vec<String> = vec![];
    let mut notes: Vec<String> = vec![];
    let mut saw_heading = false;
    for raw in &rest {
        let line = raw.trim();
        // Metadata first, so "Preparation time: 10 min" isn't taken for a heading.
        // Not inside steps/notes, where "Cook for 10 minutes" is an instruction.
        if !matches!(mode, Mode::Instructions | Mode::Notes) && !line.is_empty() && apply_meta(line, &mut d) {
            continue;
        }
        if let Some(m) = classify_heading(line) {
            mode = m;
            saw_heading = true;
            continue;
        }
        match mode {
            Mode::Intro => intro.push(line.to_string()),
            Mode::Ingredients => {
                if !line.is_empty() {
                    ing_lines.push(line.to_string());
                }
            }
            Mode::Instructions => step_lines.push(line.to_string()),
            Mode::Notes => notes.push(line.to_string()),
        }
    }

    if !saw_heading {
        // No headings: ingredient-looking lines become ingredients, the rest steps.
        let mut desc: Vec<String> = vec![];
        let mut in_ings = false;
        for l in intro.drain(..) {
            let t = l.trim();
            if t.is_empty() {
                if !step_lines.is_empty() {
                    step_lines.push(String::new());
                }
                continue;
            }
            let short = t.split_whitespace().count() <= 4 && !t.ends_with('.');
            if step_lines.is_empty() && (looks_like_ingredient(t) || (in_ings && short)) {
                in_ings = true;
                ing_lines.push(t.to_string());
            } else if in_ings || has_step_number(t) || !step_lines.is_empty() {
                step_lines.push(t.to_string());
            } else {
                desc.push(t.to_string());
            }
        }
        intro = desc;
    }

    d.description = intro.join("\n").trim().to_string();
    d.ingredients = ingredients_from_lines(&ing_lines);
    d.instructions = build_steps(&step_lines);
    d.notes = notes.join("\n").trim().to_string();
    d.normalize();
    d
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classic_layout() {
        let t = "\n\nBest Banana Bread\n\nMoist and easy.\nServes 8\nPrep time: 15 min\nCook time: 1 hour\n\nIngredients:\n- 3 ripe bananas\n- 1/3 cup melted butter\n\nFor the topping:\n* 1 tbsp sugar\n\nDirections\n1. Preheat the oven to 350°F.\n2. Mash the bananas\n   in a bowl.\n3. Bake for 1 hour.\n\nNotes:\nFreezes well.";
        let d = parse_plain_text(t);
        assert_eq!(d.name, "Best Banana Bread");
        assert_eq!(d.description, "Moist and easy.");
        assert_eq!(d.yield_quantity, Some(8.0));
        assert_eq!((d.prep_time_minutes, d.cook_time_minutes, d.total_time_minutes), (Some(15), Some(60), Some(75)));
        assert_eq!(d.ingredients.len(), 3);
        assert_eq!(d.ingredients[0].original, "3 ripe bananas");
        assert_eq!(d.ingredients[1].unit.as_deref(), Some("cup"));
        assert_eq!(d.ingredients[2].section.as_deref(), Some("For the topping"));
        assert_eq!(d.instructions.len(), 3);
        assert_eq!(d.instructions[1].text, "Mash the bananas in a bowl.");
        assert_eq!(d.notes, "Freezes well.");
    }

    #[test]
    fn markdown_and_yield_variants() {
        let t = "# Cookies\nYield: 24 cookies\nTotal time: 1 hr 20 mins\n\n## Ingredients\n2 cups flour\n1 cup sugar\n\n## Method\nMix everything.\n\nBake 12 minutes.";
        let d = parse_plain_text(t);
        assert_eq!(d.name, "Cookies");
        assert_eq!((d.yield_quantity, d.yield_unit.as_str()), (Some(24.0), "cookies"));
        assert_eq!(d.total_time_minutes, Some(80));
        assert_eq!(d.ingredients.len(), 2);
        assert_eq!(d.instructions.len(), 2);
    }

    #[test]
    fn no_headings() {
        let t = "Quick Omelette\n2 eggs\n1 tbsp butter\nSalt\nBeat the eggs.\nMelt butter and cook the eggs.";
        let d = parse_plain_text(t);
        assert_eq!(d.name, "Quick Omelette");
        assert_eq!(d.ingredients.len(), 3);
        assert_eq!(d.instructions.len(), 2, "{:?}", d.instructions);
    }

    #[test]
    fn german_headings() {
        let t = "Pfannkuchen\nZutaten\n200 g Mehl\n2 Eier\nZubereitung\nAlles verrühren.\nAusbacken.";
        let d = parse_plain_text(t);
        assert_eq!(d.ingredients.len(), 2);
        assert_eq!(d.instructions.len(), 2);
    }
}
