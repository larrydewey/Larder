//! Food name normalisation.
//!
//! Imported recipes spell the same food many ways ("large Eggs", "- Eggs",
//! "Freshly ground black pepper", "Tomatoe"). [`normalize_food`] turns a
//! parsed food phrase into a tidy canonical name plus the descriptive words
//! that belong in the note, and [`food_key`] gives a singular, accent-folded
//! key so "egg" and "eggs" are treated as the same food.

/// Preparation, temperature and size words that describe how a food is used
/// rather than what it is. They move from the food name to the note.
const DESCRIPTORS: &[&str] = &[
    "chopped",
    "diced",
    "minced",
    "sliced",
    "grated",
    "shredded",
    "softened",
    "melted",
    "cubed",
    "crushed",
    "beaten",
    "divided",
    "peeled",
    "seeded",
    "halved",
    "quartered",
    "toasted",
    "julienned",
    "trimmed",
    "rinsed",
    "drained",
    "zested",
    "juiced",
    "mashed",
    "sifted",
    "packed",
    "cold",
    "warm",
    "chilled",
    "fresh",
    "freshly",
    "finely",
    "roughly",
    "coarsely",
    "thinly",
    "lightly",
    "loosely",
    "firmly",
    "large",
    "medium",
    "small",
    "extra-large",
    "jumbo",
    "heaping",
    "level",
    "optional",
    "cooked",
    "room-temperature",
];

/// Common misspellings seen in real recipe collections.
const SPELLING: &[(&str, &str)] = &[
    ("tomatoe", "tomato"),
    ("potatoe", "potato"),
    ("jalepeno", "jalapeño"),
    ("jalepenos", "jalapeños"),
    ("jalapeno", "jalapeño"),
    ("jalapenos", "jalapeños"),
    ("cayanne", "cayenne"),
    ("cayan", "cayenne"),
    ("lemmon", "lemon"),
    ("epsome", "epsom"),
    ("worchestershire", "worcestershire"),
    ("worcestshire", "worcestershire"),
    ("parmesean", "parmesan"),
    ("brocolli", "broccoli"),
    ("brocoli", "broccoli"),
    ("zuchini", "zucchini"),
    ("zuccini", "zucchini"),
    ("cinamon", "cinnamon"),
    ("mozzerella", "mozzarella"),
    ("mozarella", "mozzarella"),
    ("vanila", "vanilla"),
    ("tumeric", "turmeric"),
    ("oregeno", "oregano"),
    ("rasberry", "raspberry"),
    ("rasberries", "raspberries"),
    ("avacado", "avocado"),
    ("avacados", "avocados"),
    ("siracha", "sriracha"),
    ("carmel", "caramel"),
    ("expresso", "espresso"),
    ("cardamon", "cardamom"),
    ("habenero", "habanero"),
    ("proscuitto", "prosciutto"),
    ("pinapple", "pineapple"),
];

/// Different names for the same ingredient, mapped to one canonical name.
/// Matched against the whole (lower-case, descriptor-free) food phrase.
const SYNONYMS: &[(&[&str], &str)] = &[
    (
        &["all purpose flour", "ap flour", "plain flour", "all-purpose white flour", "all purpose white flour", "white flour"],
        "all-purpose flour",
    ),
    (&["confectioners sugar", "confectioners' sugar", "confectioner's sugar", "icing sugar", "powdered sugar"], "powdered sugar"),
    (&["white sugar", "granulated sugar", "granulated white sugar", "white granulated sugar", "cane sugar"], "sugar"),
    (&["light brown sugar"], "light brown sugar"),
    (&["heavy whipping cream", "whipping cream", "heavy cream", "double cream"], "heavy cream"),
    (&["bicarbonate of soda", "bicarb", "bicarb soda", "sodium bicarbonate"], "baking soda"),
    (&["scallion", "scallions", "spring onion", "spring onions", "green onion", "green onions"], "green onions"),
    (&["extra virgin olive oil", "extra-virgin olive oil", "evoo"], "extra-virgin olive oil"),
    (&["ground black pepper", "black pepper", "cracked black pepper", "ground pepper"], "black pepper"),
    (&["ground cinnamon"], "cinnamon"),
    (&["ground cumin"], "cumin"),
    (&["ground nutmeg"], "nutmeg"),
    (&["vanilla", "vanilla extract", "pure vanilla extract", "pure vanilla"], "vanilla extract"),
    (&["garlic clove", "garlic cloves", "clove garlic", "cloves garlic", "cloves of garlic", "clove of garlic"], "garlic"),
    (&["egg", "eggs", "whole egg", "whole eggs"], "eggs"),
    (&["corn starch", "cornflour"], "cornstarch"),
    (&["kosher sea salt"], "kosher salt"),
    (&["table salt"], "salt"),
];

/// Words kept capitalised (proper nouns and adjectives).
const PROPER: &[&str] = &[
    "parmesan",
    "parmigiano",
    "reggiano",
    "romano",
    "asiago",
    "gruyère",
    "gruyere",
    "dijon",
    "worcestershire",
    "italian",
    "french",
    "greek",
    "mexican",
    "cajun",
    "creole",
    "thai",
    "chinese",
    "japanese",
    "korean",
    "indian",
    "spanish",
    "swiss",
    "colby",
    "kalamata",
    "tabasco",
    "sriracha",
    "bisquick",
    "karo",
    "oreo",
    "oreos",
    "nutella",
    "crisco",
    "velveeta",
    "philadelphia",
    "frank's",
    "cheddar",
];

/// Multi-word proper names whose words are ordinary on their own.
const PROPER_PHRASES: &[&str] = &[
    "new mexico",
    "new york",
    "new zealand",
    "old bay",
    "rice krispies",
    "hidden valley",
    "granny smith",
    "yukon gold",
    "monterey jack",
    "texas toast",
];

fn is_descriptor(word: &str) -> bool {
    let w = word.trim_matches(|c: char| !c.is_alphanumeric() && c != '-');
    DESCRIPTORS.contains(&w)
        || w.ends_with("-chopped")
        || w.ends_with("-diced")
        || w.ends_with("-minced")
        || w.ends_with("-sliced")
        || w.ends_with("-grated")
}

/// Result of normalising a food phrase.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NormalizedFood {
    /// Canonical display name, e.g. "Parmesan cheese".
    pub name: String,
    /// Words moved out of the name, in their original order, e.g. "freshly grated".
    pub descriptors: Option<String>,
    /// Extra note text found in the phrase (parenthetical remarks).
    pub remark: Option<String>,
}

/// Normalises a food phrase. Returns `None` if nothing food-like remains.
pub fn normalize_food(raw: &str) -> Option<NormalizedFood> {
    let mut text = crate::text::collapse_ws(raw);
    // stray bullets/dashes and trailing punctuation or footnote asterisks
    text = text.trim_start_matches(['-', '–', '—', '•', '*', '·', ' ']).trim_end_matches([':', ';', ',', '*', '.', ' ']).to_string();
    // parenthetical remark (also an unbalanced "(" left by a truncated line)
    let mut remark = None;
    if let Some(open) = text.find('(') {
        let close = text[open..].find(')').map(|c| open + c);
        let inner = match close {
            Some(c) => text[open + 1..c].to_string(),
            None => text[open + 1..].to_string(),
        };
        let after = close.map(|c| text[c + 1..].to_string()).unwrap_or_default();
        let inner = inner.trim().trim_end_matches(',').to_string();
        if !inner.is_empty() {
            remark = Some(inner);
        }
        text = format!("{} {}", text[..open].trim(), after.trim()).trim().to_string();
    }
    let mut kept: Vec<String> = vec![];
    let mut moved: Vec<String> = vec![];
    for w in text.split_whitespace() {
        let lw = w.to_lowercase();
        if is_descriptor(&lw) {
            moved.push(lw.trim_matches(|c: char| !c.is_alphanumeric() && c != '-').to_string());
        } else if lw == "and" && kept.is_empty() {
            // "and minced garlic" -> skip dangling conjunction
        } else {
            kept.push(w.to_string());
        }
    }
    // "salt and pepper" stays; drop trailing connectors left by moved words
    while kept.last().is_some_and(|w| matches!(w.to_lowercase().as_str(), "and" | "or" | "of" | "for" | "to")) {
        kept.pop();
    }
    if kept.is_empty() {
        return None;
    }
    // fix spelling word by word, lower-case everything but proper nouns and acronyms
    let words: Vec<String> = kept
        .iter()
        .map(|w| {
            let lw = w.to_lowercase();
            let fixed = SPELLING.iter().find(|(bad, _)| *bad == lw).map(|(_, good)| good.to_string()).unwrap_or(lw);
            let is_acronym = w.len() >= 2 && w.chars().all(|c| c.is_ascii_uppercase());
            if is_acronym {
                w.to_string()
            } else if PROPER.contains(&fixed.as_str()) {
                capitalise(&fixed)
            } else {
                fixed
            }
        })
        .collect();
    let mut name = words.join(" ");
    for phrase in PROPER_PHRASES {
        if let Some(pos) = name.to_lowercase().find(phrase) {
            let at_word = pos == 0 || name.as_bytes()[pos - 1] == b' ';
            if at_word && name.is_char_boundary(pos) && name.is_char_boundary(pos + phrase.len()) {
                let cap: Vec<String> = phrase.split(' ').map(capitalise).collect();
                name.replace_range(pos..pos + phrase.len(), &cap.join(" "));
            }
        }
    }
    let lower = name.to_lowercase();
    if let Some((_, canon)) = SYNONYMS.iter().find(|(alts, _)| alts.contains(&lower.as_str())) {
        name = canon.to_string();
    }
    moved.dedup();
    // Size and "fresh" read best first and on their own ("large, beaten";
    // "fresh, finely chopped"); the rest keeps the author's order.
    let mut leading: Vec<String> = vec![];
    moved.retain(|w| {
        if matches!(w.as_str(), "fresh" | "large" | "medium" | "small" | "extra-large" | "jumbo") {
            leading.push(w.clone());
            false
        } else {
            true
        }
    });
    let mut groups = leading;
    if !moved.is_empty() {
        groups.push(moved.join(" "));
    }
    let descriptors = if groups.is_empty() { None } else { Some(groups.join(", ")) };
    Some(NormalizedFood { name, descriptors, remark })
}

fn capitalise(w: &str) -> String {
    let mut c = w.chars();
    match c.next() {
        Some(f) => f.to_uppercase().collect::<String>() + c.as_str(),
        None => String::new(),
    }
}

/// Singular, accent-folded key for matching the same food across recipes.
pub fn food_key(name: &str) -> String {
    let folded = crate::text::fold(name);
    let words: Vec<String> = folded.split_whitespace().map(singular).collect();
    words.join(" ")
}

/// English singular for the last-word style plurals used in ingredient names.
pub fn singular(word: &str) -> String {
    const KEEP: &[&str] = &[
        "molasses",
        "hummus",
        "asparagus",
        "couscous",
        "swiss",
        "oats",
        "grits",
        "greens",
        "sprinkles",
        "noodles",
        "brussels",
        "chips",
        "flakes",
        "bits",
        "lentils",
        "beans",
        "peas",
        "chives",
        "glass",
        "bass",
        "citrus",
        "series",
        "species",
        "cheese",
        "mayonnaise",
        "chess",
    ];
    if KEEP.contains(&word) || word.len() <= 3 || word.ends_with("ss") || word.ends_with("us") || word.ends_with("is") {
        return word.to_string();
    }
    if let Some(stem) = word.strip_suffix("ies") {
        return format!("{stem}y");
    }
    if let Some(stem) = word.strip_suffix("ves") {
        if matches!(stem, "lea" | "loa" | "hal" | "cal" | "shel") {
            return format!("{stem}f");
        }
    }
    for suf in ["oes", "ches", "shes", "xes"] {
        if let Some(stem) = word.strip_suffix(suf) {
            return format!("{stem}{}", &suf[..suf.len() - 2]);
        }
    }
    if let Some(stem) = word.strip_suffix('s') {
        return stem.to_string();
    }
    word.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn n(s: &str) -> (String, Option<String>, Option<String>) {
        let f = normalize_food(s).unwrap();
        (f.name, f.descriptors, f.remark)
    }

    fn o(s: &str) -> Option<String> {
        Some(s.to_string())
    }

    #[test]
    fn normalises_real_world_spellings() {
        assert_eq!(n("large Eggs"), ("eggs".into(), o("large"), None));
        assert_eq!(n("- Eggs"), ("eggs".into(), None, None));
        assert_eq!(n("Large egg"), ("eggs".into(), o("large"), None));
        assert_eq!(n("freshly grated Parmesan cheese"), ("Parmesan cheese".into(), o("freshly grated"), None));
        assert_eq!(n("Grated Parmesan Cheese"), ("Parmesan cheese".into(), o("grated"), None));
        assert_eq!(n("unsalted butter (cold"), ("unsalted butter".into(), None, o("cold")));
        assert_eq!(n("Fresh Parsley Chopped"), ("parsley".into(), o("fresh, chopped"), None));
        assert_eq!(n("finely chopped fresh cilantro"), ("cilantro".into(), o("fresh, finely chopped"), None));
        assert_eq!(n("finely-diced yellow onion"), ("yellow onion".into(), o("finely-diced"), None));
        assert_eq!(n("Freshly ground black pepper"), ("black pepper".into(), o("freshly"), None));
        assert_eq!(n("Tomatoe"), ("tomato".into(), None, None));
        assert_eq!(n("Candied Jalepenos"), ("candied jalapeños".into(), None, None));
        assert_eq!(n("ground cinnamon"), ("cinnamon".into(), None, None));
        assert_eq!(n("Confectioners' Sugar"), ("powdered sugar".into(), None, None));
        assert_eq!(n("Heavy Whipping Cream"), ("heavy cream".into(), None, None));
        assert_eq!(n("pure vanilla extract"), ("vanilla extract".into(), None, None));
        assert_eq!(n("BBQ sauce"), ("BBQ sauce".into(), None, None));
        assert_eq!(n("ground cayenne pepper*"), ("ground cayenne pepper".into(), None, None));
        assert_eq!(n("Dijon mustard"), ("Dijon mustard".into(), None, None));
        assert_eq!(n("New Mexico chile"), ("New Mexico chile".into(), None, None));
        assert_eq!(n("new potatoes"), ("new potatoes".into(), None, None));
        assert_eq!(n("old bay seasoning"), ("Old Bay seasoning".into(), None, None));
        assert_eq!(n("bay leaves"), ("bay leaves".into(), None, None));
        assert_eq!(n("old fashioned oats"), ("old fashioned oats".into(), None, None));
        assert_eq!(n("Monterey Jack cheese"), ("Monterey Jack cheese".into(), None, None));
        assert_eq!(n("granny smith apples"), ("Granny Smith apples".into(), None, None));
        assert!(normalize_food("chopped").is_none());
    }

    #[test]
    fn keys_merge_plurals() {
        assert_eq!(food_key("eggs"), food_key("egg"));
        assert_eq!(food_key("tomatoes"), food_key("Tomato"));
        assert_eq!(food_key("berries"), "berry");
        assert_eq!(food_key("bay leaves"), "bay leaf");
        assert_eq!(food_key("rolled oats"), "rolled oats");
        assert_eq!(food_key("molasses"), "molasses");
        assert_eq!(food_key("Crème fraîche"), "creme fraiche");
        assert_eq!(food_key("peaches"), "peach");
    }
}
