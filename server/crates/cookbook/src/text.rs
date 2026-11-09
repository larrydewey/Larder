//! Small typographic helpers applied to user text before it is handed to Typst.
//! (Everything here only changes how text *looks*; escaping is not needed
//! because the template renders JSON strings literally.)

use std::sync::LazyLock;

use regex::Regex;

static FRACTION: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?:\b(\d+) )?\b(1/8|1/4|1/3|3/8|1/2|5/8|2/3|3/4|7/8)\b").expect("valid regex"));

/// Replaces ASCII kitchen fractions with Unicode vulgar fractions:
/// `"1 1/2 cups"` → `"1½ cups"`, `"1/4–1/2 tsp"` → `"¼–½ tsp"`.
pub fn unicode_fractions(s: &str) -> String {
    FRACTION
        .replace_all(s, |c: &regex::Captures| {
            let f = match &c[2] {
                "1/8" => "⅛",
                "1/4" => "¼",
                "1/3" => "⅓",
                "3/8" => "⅜",
                "1/2" => "½",
                "5/8" => "⅝",
                "2/3" => "⅔",
                "3/4" => "¾",
                _ => "⅞",
            };
            match c.get(1) {
                Some(whole) => format!("{}{}", whole.as_str(), f),
                None => f.to_string(),
            }
        })
        .into_owned()
}

/// Converts straight quotes to typographic ones and `--` to an en dash.
pub fn smart_quotes(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 8);
    let mut prev: Option<char> = None;
    let mut chars = s.chars().peekable();
    while let Some(c) = chars.next() {
        let opening = prev.is_none_or(|p| p.is_whitespace() || "([{—–-/".contains(p));
        match c {
            '"' => out.push(if opening { '“' } else { '”' }),
            '\'' => out.push(if opening && chars.peek().is_some_and(|n| n.is_alphanumeric()) {
                // Apostrophes that start a word ('tis, '90s) are rare compared to
                // opening single quotes, but both look fine with ’ for numbers.
                if chars.peek().is_some_and(|n| n.is_ascii_digit()) {
                    '’'
                } else {
                    '‘'
                }
            } else {
                '’'
            }),
            '-' if chars.peek() == Some(&'-') => {
                chars.next();
                out.push('–');
            }
            c if c.is_control() && c != '\n' && c != '\t' => continue,
            '\u{feff}' => continue,
            c => out.push(c),
        }
        prev = Some(c);
    }
    out
}

/// Tidies a free-text field: trims, typographic quotes.
pub fn tidy(s: &str) -> String {
    smart_quotes(s.trim())
}

/// Splits text into paragraphs on blank lines; single newlines become spaces.
pub fn paragraphs(s: &str) -> Vec<String> {
    let mut out = vec![];
    let mut cur: Vec<&str> = vec![];
    for line in s.lines() {
        let l = line.trim();
        if l.is_empty() {
            if !cur.is_empty() {
                out.push(tidy(&cur.join(" ")));
                cur.clear();
            }
        } else {
            cur.push(l);
        }
    }
    if !cur.is_empty() {
        out.push(tidy(&cur.join(" ")));
    }
    out
}

/// Lines of text (for notes that are written as lists), empty lines dropped.
pub fn note_items(s: &str) -> Vec<String> {
    paragraphs(s)
}

/// Upper-cases the first character.
pub fn capitalize_first(s: &str) -> String {
    let mut c = s.chars();
    match c.next() {
        Some(f) => f.to_uppercase().chain(c).collect(),
        None => String::new(),
    }
}

/// Turns schema.org style nutrition keys ("saturatedFatContent", "fiber_content")
/// into labels ("Saturated fat", "Fiber").
pub fn nutrition_label(key: &str) -> String {
    let key = key.trim();
    let key = key.strip_suffix("Content").unwrap_or(key);
    let key = key.strip_suffix("_content").unwrap_or(key);
    let mut words = String::new();
    for (i, ch) in key.chars().enumerate() {
        if ch == '_' || ch == '-' {
            words.push(' ');
        } else if ch.is_uppercase() && i > 0 {
            words.push(' ');
            words.extend(ch.to_lowercase());
        } else {
            words.push(ch);
        }
    }
    let words = larder_core::text::collapse_ws(&words);
    match words.to_lowercase().as_str() {
        "carbohydrate" => "Carbohydrates".into(),
        "serving size" => "Serving size".into(),
        _ => capitalize_first(&words),
    }
}

/// Sort position of a nutrition key (the usual label order).
pub fn nutrition_rank(key: &str) -> usize {
    let k = key.to_lowercase();
    const ORDER: &[&str] = &[
        "serving",
        "calorie",
        "fat",
        "saturated",
        "trans",
        "unsaturated",
        "cholesterol",
        "sodium",
        "carbohydrate",
        "fiber",
        "sugar",
        "protein",
    ];
    ORDER.iter().position(|o| k.starts_with(o)).unwrap_or(ORDER.len())
}

/// The host part of an http(s) URL without a leading `www.`.
pub fn url_host(url: &str) -> Option<String> {
    let rest = url.trim().strip_prefix("https://").or_else(|| url.trim().strip_prefix("http://"))?;
    let host = rest.split(['/', '?', '#']).next()?.split('@').next_back()?;
    let host = host.split(':').next()?;
    let host = host.strip_prefix("www.").unwrap_or(host);
    (!host.is_empty() && host.contains('.')).then(|| host.to_lowercase())
}

/// Accent- and case-insensitive key used for alphabetical sorting.
pub fn sort_key(s: &str) -> String {
    let folded = larder_core::text::fold(s);
    folded.trim_start_matches(|c: char| !c.is_alphanumeric()).to_string()
}

/// The index letter for a term ("#" for digits/symbols).
pub fn index_letter(s: &str) -> String {
    match sort_key(s).chars().next() {
        Some(c) if c.is_alphabetic() => c.to_uppercase().collect(),
        Some(_) => "#".into(),
        None => "#".into(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fractions() {
        assert_eq!(unicode_fractions("1 1/2 cups flour"), "1½ cups flour");
        assert_eq!(unicode_fractions("1/4–1/2 tsp salt"), "¼–½ tsp salt");
        assert_eq!(unicode_fractions("cut into 1/2-inch dice"), "cut into ½-inch dice");
        assert_eq!(unicode_fractions("11/2 and 1/23"), "11/2 and 1/23");
    }

    #[test]
    fn quotes() {
        assert_eq!(smart_quotes(r#"Mom's "best" pie -- really"#), "Mom’s “best” pie – really");
        assert_eq!(smart_quotes("'quoted'"), "‘quoted’");
    }

    #[test]
    fn labels() {
        assert_eq!(nutrition_label("saturatedFatContent"), "Saturated fat");
        assert_eq!(nutrition_label("calories"), "Calories");
        assert_eq!(nutrition_label("fiber_content"), "Fiber");
        assert_eq!(url_host("https://www.example.com/a/b?c"), Some("example.com".into()));
        assert_eq!(url_host("javascript:alert(1)"), None);
        assert_eq!(index_letter("Éclairs"), "E");
        assert_eq!(index_letter("7-Layer Dip"), "#");
    }
}
