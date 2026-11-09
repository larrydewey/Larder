//! Rule-based ingredient line parser.
//!
//! Handles quantities written as integers, decimals (with `.` or `,`),
//! fractions, mixed numbers, unicode vulgar fractions, ranges ("2-3",
//! "2 to 3") and number words, followed by an optional unit (any alias in
//! [`crate::units`]), the food, and a trailing note after a comma or in
//! parentheses.

use std::sync::LazyLock;

use regex::Regex;

use crate::model::Ingredient;
use crate::units;

static NUM: &str = r"(?:\d+\s+\d+/\d+|\d+/\d+|\d+(?:[.,]\d+)?)";

static QTY_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(&format!(r"^(?P<a>{NUM})(?:\s*(?:-|to|or)\s*(?P<b>{NUM}))?")).unwrap());

static SIZE_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(&format!(r"^(?P<n>{NUM})\s*-?\s*")).unwrap());

static ALIASES: LazyLock<Vec<(&'static str, &'static str, bool)>> = LazyLock::new(units::alias_table);

const WORD_NUMBERS: &[(&str, f64)] = &[
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
    ("eleven", 11.0),
    ("twelve", 12.0),
    ("dozen", 12.0),
    ("half", 0.5),
    ("a half", 0.5),
    ("one half", 0.5),
    ("a quarter", 0.25),
];

fn sup_sub_digit(c: char) -> Option<char> {
    const SUP: &str = "⁰¹²³⁴⁵⁶⁷⁸⁹";
    const SUB: &str = "₀₁₂₃₄₅₆₇₈₉";
    SUP.chars().position(|x| x == c).or_else(|| SUB.chars().position(|x| x == c)).map(|i| (b'0' + i as u8) as char)
}

/// Rewrites typographic fractions built from superscript/subscript digits
/// ("¹/₂", "²¹⁄₃₂") as ASCII fractions and drops footnote markers such as
/// the "²" in "cream cheese²".
fn expand_script_digits(s: &str) -> String {
    let chars: Vec<char> = s.chars().collect();
    let mut out = String::with_capacity(s.len());
    let mut i = 0;
    while i < chars.len() {
        if sup_sub_digit(chars[i]).is_none() {
            out.push(chars[i]);
            i += 1;
            continue;
        }
        let start = i;
        while i < chars.len() && sup_sub_digit(chars[i]).is_some() {
            i += 1;
        }
        let num: String = chars[start..i].iter().filter_map(|c| sup_sub_digit(*c)).collect();
        if i < chars.len() && matches!(chars[i], '/' | '⁄') && chars.get(i + 1).is_some_and(|c| sup_sub_digit(*c).is_some()) {
            let dstart = i + 1;
            let mut j = dstart;
            while j < chars.len() && sup_sub_digit(chars[j]).is_some() {
                j += 1;
            }
            let den: String = chars[dstart..j].iter().filter_map(|c| sup_sub_digit(*c)).collect();
            if out.chars().next_back().is_some_and(|c| c.is_ascii_digit()) {
                out.push(' ');
            }
            out.push_str(&num);
            out.push('/');
            out.push_str(&den);
            i = j;
        }
        // otherwise: a footnote/exponent marker, dropped
    }
    out
}

/// Replaces unicode fractions and dashes with ASCII equivalents.
pub fn normalize_line(s: &str) -> String {
    let s = &expand_script_digits(s);
    let mut out = String::with_capacity(s.len() + 8);
    let mut prev_digit = false;
    for c in s.chars() {
        let frac = match c {
            '½' => Some("1/2"),
            '⅓' => Some("1/3"),
            '⅔' => Some("2/3"),
            '¼' => Some("1/4"),
            '¾' => Some("3/4"),
            '⅕' => Some("1/5"),
            '⅖' => Some("2/5"),
            '⅗' => Some("3/5"),
            '⅘' => Some("4/5"),
            '⅙' => Some("1/6"),
            '⅚' => Some("5/6"),
            '⅛' => Some("1/8"),
            '⅜' => Some("3/8"),
            '⅝' => Some("5/8"),
            '⅞' => Some("7/8"),
            _ => None,
        };
        if let Some(f) = frac {
            if prev_digit {
                out.push(' ');
            }
            out.push_str(f);
            prev_digit = false;
            continue;
        }
        let c = match c {
            '–' | '—' | '‒' | '−' => '-',
            '⁄' => '/',
            '\u{a0}' | '\t' => ' ',
            c => c,
        };
        prev_digit = c.is_ascii_digit();
        out.push(c);
    }
    crate::text::collapse_ws(&out)
}

/// Parses a numeric token such as "1 1/2", "3/4", "2,5".
pub fn parse_number(s: &str) -> Option<f64> {
    let s = s.trim();
    if let Some((whole, frac)) = s.split_once(' ') {
        return Some(parse_number(whole)? + parse_number(frac)?);
    }
    if let Some((n, d)) = s.split_once('/') {
        let (n, d): (f64, f64) = (n.trim().parse().ok()?, d.trim().parse().ok()?);
        return if d == 0.0 { None } else { Some(n / d) };
    }
    s.replace(',', ".").parse().ok()
}

fn is_boundary(rest: &str) -> bool {
    match rest.chars().next() {
        None => true,
        Some(c) => !c.is_alphanumeric(),
    }
}

/// Tries to match a unit at the start of `s`; returns (canonical key, bytes consumed).
fn match_unit(s: &str) -> Option<(&'static str, usize)> {
    let lower = s.to_lowercase();
    for (alias, key, cs) in ALIASES.iter() {
        let len = alias.len();
        if s.len() < len || !s.is_char_boundary(len) {
            continue;
        }
        let hit = if *cs { &s[..len] == *alias } else { lower.get(..len) == Some(*alias) };
        if hit && is_boundary(&s[len..]) {
            let mut consumed = len;
            // swallow a trailing period ("tbsp.", "oz.")
            if s[consumed..].starts_with('.') {
                consumed += 1;
            }
            return Some((key, consumed));
        }
    }
    None
}

fn strip_bullet(s: &str) -> &str {
    let t = s.trim_start();
    for b in ["- ", "* ", "• ", "· ", "▢ ", "□ ", "☐ "] {
        if let Some(r) = t.strip_prefix(b) {
            return r.trim_start();
        }
    }
    t
}

/// Parses a single ingredient line into structured parts.
pub fn parse_ingredient(line: &str) -> Ingredient {
    let original = crate::text::collapse_ws(line);
    let norm = normalize_line(strip_bullet(&original));
    let mut rest: &str = &norm;
    let mut quantity = None;
    let mut quantity_max = None;
    let mut notes: Vec<String> = vec![];

    if let Some(c) = QTY_RE.captures(rest) {
        let a = parse_number(&c["a"]);
        // Don't treat "2/3" where the regex could be a date-like thing; fine for recipes.
        if a.is_some() {
            quantity = a;
            quantity_max = c.name("b").and_then(|b| parse_number(b.as_str())).filter(|b| Some(*b) > a);
            rest = rest[c.get(0).unwrap().end()..].trim_start();
        }
    } else {
        let lower = rest.to_lowercase();
        for (w, v) in WORD_NUMBERS.iter().rev() {
            if lower.starts_with(w) && is_boundary(&rest[w.len()..]) {
                quantity = Some(*v);
                rest = rest[w.len()..].trim_start();
                break;
            }
        }
        if quantity.is_none() {
            // "a pinch of salt", "an onion" -> 1 only when followed by a unit
            for art in ["a ", "an "] {
                if lower.starts_with(art) {
                    let after = rest[art.len()..].trim_start();
                    if match_unit(after).is_some() {
                        quantity = Some(1.0);
                        rest = after;
                    }
                    break;
                }
            }
        }
    }

    // "x" multiplier: "2 x 400g tins" -> keep simple: drop the "x"
    if quantity.is_some() {
        // "4-ounces cheese": a dash between number and unit
        if let Some(r) = rest.strip_prefix('-') {
            if match_unit(r.trim_start()).is_some() {
                rest = r.trim_start();
            }
        }
        if let Some(r) = rest.strip_prefix("x ").or_else(|| rest.strip_prefix("× ")) {
            rest = r.trim_start();
        }
    }

    // Parenthetical size right after the quantity: "1 (14 oz) can tomatoes"
    if quantity.is_some() && rest.starts_with('(') {
        if let Some(end) = rest.find(')') {
            notes.push(rest[1..end].trim().to_string());
            rest = rest[end + 1..].trim_start();
        }
    }

    let mut unit = None;
    // "1 - 8oz package", "1 3-ounce package", "2 8-ounce packages": a count,
    // then a sized container. Becomes count × container with the size as note.
    if quantity.is_some() {
        let r = rest.trim_start_matches(['-', ' ']);
        if let Some(c) = SIZE_RE.captures(r) {
            let size = parse_number(&c["n"]);
            let after_num = &r[c.get(0).unwrap().end()..];
            if let (Some(size), Some((size_unit, used))) = (size, match_unit(after_num)) {
                let after_unit = after_num[used..].trim_start();
                if let Some((container, used2)) = match_unit(after_unit) {
                    let size_is_measure = units::unit_def(size_unit).is_some_and(|d| d.dim != units::Dimension::Count);
                    if size_is_measure && matches!(container, "can" | "jar" | "package" | "stick" | "bunch" | "head") {
                        let label =
                            units::unit_def(size_unit).map(|d| if size == 1.0 { d.singular } else { d.plural }).unwrap_or(size_unit);
                        notes.insert(0, format!("{} {label}", crate::format::format_quantity(size)));
                        unit = Some(container.to_string());
                        rest = after_unit[used2..].trim_start();
                        for of in ["of ", "Of "] {
                            if let Some(r2) = rest.strip_prefix(of) {
                                rest = r2.trim_start();
                            }
                        }
                    }
                }
            }
        }
    }
    if unit.is_none() {
        if let Some((key, used)) = match_unit(rest) {
            // Units are only recognised after a quantity; "Head of lettuce" or
            // "Can opener" without a number stay as plain food text.
            let after = rest[used..].trim_start();
            if quantity.is_some() {
                unit = Some(key.to_string());
                rest = after;
                // "10 ounce can pineapple" / "2 (8 oz) packages": a size followed by
                // a container is one container of that size.
                if let Some((container, used2)) = match_unit(rest) {
                    if matches!(container, "can" | "jar" | "package" | "stick" | "bunch" | "head")
                        && units::unit_def(key).is_some_and(|d| d.dim != units::Dimension::Count)
                    {
                        // "1 - 8oz package" was read as the range 1–8: count 1, size 8.
                        let (count, q) = match quantity_max.take() {
                            Some(size) => (quantity.unwrap_or(1.0), size),
                            None => (1.0, quantity.unwrap_or(1.0)),
                        };
                        let size_unit = units::unit_def(key).map(|d| if q == 1.0 { d.singular } else { d.plural }).unwrap_or(key);
                        notes.insert(0, format!("{} {size_unit}", crate::format::format_quantity(q)));
                        quantity = Some(count);
                        unit = Some(container.to_string());
                        rest = rest[used2..].trim_start();
                    }
                }
                if rest.starts_with('(') {
                    if let Some(end) = rest.find(')') {
                        notes.push(rest[1..end].trim().to_string());
                        rest = rest[end + 1..].trim_start();
                    }
                }
                for of in ["of ", "Of "] {
                    if let Some(r) = rest.strip_prefix(of) {
                        rest = r.trim_start();
                    }
                }
            }
        }
    }
    if unit.is_none() && quantity.is_none() {
        rest = norm.as_str();
    }

    // Split food from note at the first comma outside parentheses, so
    // "butter (cold, cubed)" keeps its parenthetical together.
    let split_at = {
        let mut depth = 0i32;
        rest.char_indices().find_map(|(i, c)| {
            match c {
                '(' => depth += 1,
                ')' => depth -= 1,
                ',' if depth <= 0 => return Some(i),
                _ => {}
            }
            None
        })
    };
    let (mut food, mut note) = match split_at {
        Some(i) => (rest[..i].trim().to_string(), Some(rest[i + 1..].trim().to_string())),
        None => (rest.trim().to_string(), None),
    };
    // trailing parenthetical on the food
    if food.ends_with(')') {
        if let Some(start) = food.rfind('(') {
            let inner = food[start + 1..food.len() - 1].trim().to_string();
            food = food[..start].trim().to_string();
            notes.push(inner);
        }
    }
    for suffix in [" to taste", " optional", " (optional)", " for garnish", " for serving", " as needed"] {
        if food.to_lowercase().ends_with(suffix) && food.len() > suffix.len() {
            let cut = food.len() - suffix.len();
            notes.push(food[cut..].trim().to_string());
            food = food[..cut].trim().to_string();
        }
    }
    if let Some(n) = note.take() {
        if !n.is_empty() {
            notes.push(n);
        }
    }
    if !notes.is_empty() {
        note = Some(notes.join(", "));
    }

    Ingredient { section: None, quantity, quantity_max, unit, food: if food.is_empty() { None } else { Some(food) }, note, original }
}

/// Parses a block of text lines into ingredients, honouring section headings
/// (lines ending in ':' or starting with '#', or all-caps short lines).
pub fn parse_ingredient_block(text: &str) -> Vec<Ingredient> {
    let mut out = vec![];
    let mut section: Option<String> = None;
    for raw in text.lines() {
        let line = raw.trim();
        if line.is_empty() {
            continue;
        }
        if let Some(h) = section_heading(line) {
            section = Some(h);
            continue;
        }
        let mut ing = parse_ingredient(line);
        ing.section = section.clone();
        out.push(ing);
    }
    out
}

/// Returns the heading text if a line looks like a section heading.
pub fn section_heading(line: &str) -> Option<String> {
    let l = line.trim();
    if let Some(h) = l.strip_prefix('#') {
        let h = h.trim_start_matches('#').trim();
        return (!h.is_empty()).then(|| h.to_string());
    }
    if l.ends_with(':') && l.len() > 1 && l.len() < 60 && !l.chars().next().unwrap().is_ascii_digit() {
        return Some(l.trim_end_matches(':').trim().to_string());
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    type Parts = (Option<f64>, Option<f64>, Option<String>, Option<String>, Option<String>);

    fn p(s: &str) -> Parts {
        let i = parse_ingredient(s);
        (i.quantity, i.quantity_max, i.unit, i.food, i.note)
    }

    fn s(v: &str) -> Option<String> {
        Some(v.to_string())
    }

    #[test]
    fn basics() {
        assert_eq!(p("2 cups flour"), (Some(2.0), None, s("cup"), s("flour"), None));
        assert_eq!(p("1 1/2 cups sugar, divided"), (Some(1.5), None, s("cup"), s("sugar"), s("divided")));
        assert_eq!(p("1½ tbsp. olive oil"), (Some(1.5), None, s("tbsp"), s("olive oil"), None));
        assert_eq!(p("½ tsp salt"), (Some(0.5), None, s("tsp"), s("salt"), None));
        assert_eq!(p("3 large eggs, beaten"), (Some(3.0), None, None, s("large eggs"), s("beaten")));
        assert_eq!(p("2-3 cloves garlic, minced"), (Some(2.0), Some(3.0), s("clove"), s("garlic"), s("minced")));
        assert_eq!(p("2 to 3 lbs chicken thighs"), (Some(2.0), Some(3.0), s("lb"), s("chicken thighs"), None));
        assert_eq!(p("500g plain flour"), (Some(500.0), None, s("g"), s("plain flour"), None));
        assert_eq!(p("1,5 l Milch"), (Some(1.5), None, s("l"), s("Milch"), None));
        assert_eq!(p("Salt and pepper to taste"), (None, None, None, s("Salt and pepper"), s("to taste")));
        assert_eq!(p("1 (14 oz) can diced tomatoes"), (Some(1.0), None, s("can"), s("diced tomatoes"), s("14 oz")));
        assert_eq!(p("a pinch of nutmeg"), (Some(1.0), None, s("pinch"), s("nutmeg"), None));
        assert_eq!(p("2 T butter"), (Some(2.0), None, s("tbsp"), s("butter"), None));
        assert_eq!(p("1 t vanilla"), (Some(1.0), None, s("tsp"), s("vanilla"), None));
        assert_eq!(p("one onion, diced"), (Some(1.0), None, None, s("onion"), s("diced")));
        assert_eq!(p("1 large onion"), (Some(1.0), None, None, s("large onion"), None));
        assert_eq!(p("- 4 ounces cream cheese (softened)"), (Some(4.0), None, s("oz"), s("cream cheese"), s("softened")));
        assert_eq!(p("Fresh parsley"), (None, None, None, s("Fresh parsley"), None));
        assert_eq!(p("Cups of joy"), (None, None, None, s("Cups of joy"), None));
        assert_eq!(p("Head of lettuce"), (None, None, None, s("Head of lettuce"), None));
        assert_eq!(p("10 ounce can crushed pineapple"), (Some(1.0), None, s("can"), s("crushed pineapple"), s("10 oz")));
        assert_eq!(p("4-ounces of shredded cheddar cheese"), (Some(4.0), None, s("oz"), s("shredded cheddar cheese"), None));
        assert_eq!(p("unsalted butter (cold, cubed)").3, s("unsalted butter"));
        assert_eq!(p("1 - 8oz package cream cheese"), (Some(1.0), None, s("package"), s("cream cheese"), s("8 oz")));
        assert_eq!(p("1 3-ounce package cream cheese"), (Some(1.0), None, s("package"), s("cream cheese"), s("3 oz")));
        assert_eq!(p("2 8-ounce packages cream cheese, softened"), (Some(2.0), None, s("package"), s("cream cheese"), s("8 oz, softened")));
        // typographic fractions and footnote markers (seen in real Mealie data)
        assert_eq!(p("1 ¹/₂ cups Cheddar Cheese"), (Some(1.5), None, s("cup"), s("Cheddar Cheese"), None));
        assert_eq!(p("²¹⁄₃₂ cup sugar").0, Some(21.0 / 32.0));
        assert_eq!(p("1¹/₄ cups milk").0, Some(1.25));
        assert_eq!(p("4 Tablespoon chili powder²"), (Some(4.0), None, s("tbsp"), s("chili powder"), None));
        assert_eq!(p("1⁄2\tcup\tbutter\t"), (Some(0.5), None, s("cup"), s("butter"), None));
    }

    #[test]
    fn sections() {
        let v = parse_ingredient_block("For the dough:\n2 cups flour\n\n# Topping\n1 tbsp sugar");
        assert_eq!(v.len(), 2);
        assert_eq!(v[0].section.as_deref(), Some("For the dough"));
        assert_eq!(v[1].section.as_deref(), Some("Topping"));
    }

    #[test]
    fn numbers() {
        assert_eq!(parse_number("1 1/2"), Some(1.5));
        assert_eq!(parse_number("3/4"), Some(0.75));
        assert_eq!(parse_number("2,5"), Some(2.5));
        assert_eq!(parse_number("1/0"), None);
    }
}
