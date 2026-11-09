//! Grocery aisle classification for shopping lists.

pub const AISLES: &[&str] = &[
    "Produce",
    "Meat & Seafood",
    "Dairy & Eggs",
    "Bakery",
    "Pantry",
    "Baking",
    "Spices & Seasonings",
    "Canned & Jarred",
    "Frozen",
    "Beverages",
    "Household",
    "Other",
];

/// Keyword table; earlier rules win, and longer phrases are listed before the
/// words they contain (e.g. "coconut milk" before "milk").
const RULES: &[(&str, &[&str])] = &[
    ("Frozen", &["frozen", "ice cream", "puff pastry", "phyllo", "filo"]),
    (
        "Canned & Jarred",
        &[
            "barbecue sauce",
            "bbq sauce",
            "artichoke heart",
            "pico de gallo",
            "enchilada sauce",
            "marinara",
            "pasta sauce",
            "alfredo sauce",
            "canned",
            "can of",
            "tinned",
            "coconut milk",
            "tomato paste",
            "tomato sauce",
            "crushed tomatoes",
            "diced tomatoes",
            "passata",
            "broth",
            "stock",
            "bouillon",
            "beans",
            "chickpeas",
            "lentils",
            "salsa",
            "pesto",
            "olives",
            "capers",
            "pickles",
            "jam",
            "jelly",
            "peanut butter",
            "tahini",
            "mayonnaise",
            "mayo",
            "ketchup",
            "mustard",
            "soy sauce",
            "fish sauce",
            "worcestershire",
            "hot sauce",
            "sriracha",
            "honey",
            "maple syrup",
            "molasses",
            "vinegar",
        ],
    ),
    (
        "Spices & Seasonings",
        &[
            "rub",
            "ranch seasoning",
            "taco seasoning",
            "salt",
            "pepper flakes",
            "peppercorn",
            "black pepper",
            "white pepper",
            "cumin",
            "paprika",
            "turmeric",
            "cinnamon",
            "nutmeg",
            "ground clove",
            "whole clove",
            "cardamom",
            "coriander seed",
            "ground coriander",
            "chili powder",
            "chilli powder",
            "cayenne",
            "oregano",
            "dried basil",
            "dried thyme",
            "thyme dried",
            "bay leaf",
            "bay leaves",
            "garam masala",
            "curry powder",
            "allspice",
            "ginger powder",
            "ground ginger",
            "garlic powder",
            "onion powder",
            "italian seasoning",
            "seasoning",
            "spice",
            "fennel seed",
            "mustard seed",
            "saffron",
            "star anise",
            "sumac",
            "za'atar",
            "herbes de provence",
            "vanilla",
        ],
    ),
    (
        "Meat & Seafood",
        &[
            "hamburger",
            "ground beef",
            "pepperoni",
            "meatball",
            "chicken",
            "beef",
            "pork",
            "lamb",
            "turkey",
            "bacon",
            "sausage",
            "ham",
            "prosciutto",
            "salami",
            "chorizo",
            "steak",
            "mince",
            "ground meat",
            "veal",
            "duck",
            "fish",
            "salmon",
            "tuna",
            "cod",
            "shrimp",
            "prawn",
            "crab",
            "lobster",
            "scallop",
            "mussel",
            "clam",
            "anchovy",
            "anchovies",
            "tilapia",
            "halibut",
            "trout",
        ],
    ),
    (
        "Dairy & Eggs",
        &[
            "milk",
            "buttermilk",
            "butter",
            "cream",
            "cheese",
            "parmesan",
            "mozzarella",
            "cheddar",
            "feta",
            "ricotta",
            "yogurt",
            "yoghurt",
            "egg",
            "sour cream",
            "crème fraîche",
            "creme fraiche",
            "half-and-half",
            "ghee",
            "mascarpone",
        ],
    ),
    (
        "Baking",
        &[
            "cake mix",
            "pie crust",
            "corn syrup",
            "karo",
            "masa harina",
            "extract",
            "toffee",
            "caramel",
            "marshmallow",
            "graham cracker",
            "flour",
            "sugar",
            "baking powder",
            "baking soda",
            "bicarbonate",
            "yeast",
            "cocoa",
            "chocolate chip",
            "chocolate",
            "cornstarch",
            "corn starch",
            "cornflour",
            "gelatin",
            "sprinkles",
            "food coloring",
            "powdered sugar",
            "icing sugar",
            "brown sugar",
            "shortening",
            "almond extract",
            "cream of tartar",
        ],
    ),
    (
        "Bakery",
        &[
            "hamburger bun",
            "hot dog bun",
            "slider bun",
            "bread",
            "baguette",
            "bun",
            "roll",
            "tortilla",
            "pita",
            "naan",
            "croissant",
            "bagel",
            "brioche",
        ],
    ),
    (
        "Produce",
        &[
            "artichoke",
            "onion",
            "garlic",
            "shallot",
            "scallion",
            "green onion",
            "leek",
            "tomato",
            "potato",
            "carrot",
            "celery",
            "pepper",
            "chili",
            "chile",
            "jalapeño",
            "jalapeno",
            "lettuce",
            "spinach",
            "kale",
            "cabbage",
            "broccoli",
            "cauliflower",
            "zucchini",
            "courgette",
            "squash",
            "pumpkin",
            "cucumber",
            "eggplant",
            "aubergine",
            "mushroom",
            "avocado",
            "lemon",
            "lime",
            "orange",
            "apple",
            "banana",
            "berry",
            "berries",
            "blueberry",
            "strawberry",
            "raspberry",
            "blackberry",
            "cranberry",
            "grape",
            "pear",
            "peach",
            "mango",
            "pineapple",
            "cherry",
            "cherries",
            "melon",
            "basil",
            "parsley",
            "cilantro",
            "coriander",
            "mint",
            "dill",
            "rosemary",
            "thyme",
            "sage",
            "chive",
            "ginger",
            "corn",
            "pea",
            "bean sprout",
            "green bean",
            "asparagus",
            "beet",
            "radish",
            "fennel",
            "sweet potato",
            "yam",
            "arugula",
            "rocket",
            "herb",
            "fruit",
            "vegetable",
        ],
    ),
    (
        "Pantry",
        &[
            "millet",
            "oreo",
            "tortilla chip",
            "pretzel",
            "rice",
            "pasta",
            "spaghetti",
            "noodle",
            "macaroni",
            "penne",
            "oats",
            "oatmeal",
            "quinoa",
            "couscous",
            "bulgur",
            "cereal",
            "oil",
            "nuts",
            "almond",
            "walnut",
            "pecan",
            "cashew",
            "peanut",
            "pistachio",
            "seed",
            "raisin",
            "dried fruit",
            "breadcrumb",
            "panko",
            "cracker",
            "chips",
            "polenta",
            "cornmeal",
            "lentil",
            "barley",
        ],
    ),
    (
        "Beverages",
        &["water", "juice", "wine", "beer", "coffee", "tea", "soda", "sparkling", "rum", "vodka", "brandy", "whiskey", "liqueur"],
    ),
    ("Household", &["foil", "parchment", "paper towel", "plastic wrap", "toothpick", "skewer", "napkin", "soap"]),
];

/// Picks an aisle for a food name.
pub fn classify(food: &str) -> &'static str {
    let f = crate::text::fold(food);
    if f.is_empty() {
        return "Other";
    }
    let words: Vec<&str> = f.split(|c: char| !c.is_alphanumeric() && c != '\'').filter(|w| !w.is_empty()).collect();
    // Best match: the keyword with the most words wins ("cream of tartar" over
    // "cream", "garlic powder" over "garlic"); ties go to the earlier rule.
    let mut best: Option<(usize, usize, &'static str)> = None;
    for (rank, (aisle, keywords)) in RULES.iter().enumerate() {
        for kw in keywords.iter() {
            let kw_words: Vec<&str> = kw.split(|c: char| !c.is_alphanumeric() && c != '\'').filter(|w| !w.is_empty()).collect();
            if !kw_words.is_empty() && contains_phrase(&words, &kw_words) {
                let better = match best {
                    None => true,
                    Some((n, r, _)) => kw_words.len() > n || (kw_words.len() == n && rank < r),
                };
                if better {
                    best = Some((kw_words.len(), rank, aisle));
                }
            }
        }
    }
    best.map(|b| b.2).unwrap_or("Other")
}

/// True if `phrase` occurs as consecutive whole words in `words`; the last
/// word may carry a plural ending ("egg" matches "eggs" but not "eggplant").
fn contains_phrase(words: &[&str], phrase: &[&str]) -> bool {
    if phrase.len() > words.len() {
        return false;
    }
    words.windows(phrase.len()).any(|w| {
        w.iter().zip(phrase).enumerate().all(|(i, (word, kw))| if i + 1 < phrase.len() { word == kw } else { word_matches(word, kw) })
    })
}

fn word_matches(word: &str, kw: &str) -> bool {
    if word == kw {
        return true;
    }
    match word.strip_prefix(kw) {
        Some(rest) => matches!(rest, "s" | "es"),
        // "berries" for "berry", "leaves" for "leaf"
        None => {
            (kw.ends_with('y') && word == format!("{}ies", &kw[..kw.len() - 1]))
                || (kw.ends_with('f') && word == format!("{}ves", &kw[..kw.len() - 1]))
        }
    }
}

pub fn aisle_order(aisle: &str) -> usize {
    AISLES.iter().position(|a| a.eq_ignore_ascii_case(aisle)).unwrap_or(AISLES.len() - 1)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classify_foods() {
        assert_eq!(classify("all-purpose flour"), "Baking");
        assert_eq!(classify("Red onions"), "Produce");
        assert_eq!(classify("coconut milk"), "Canned & Jarred");
        assert_eq!(classify("whole milk"), "Dairy & Eggs");
        assert_eq!(classify("chicken thighs"), "Meat & Seafood");
        assert_eq!(classify("kosher salt"), "Spices & Seasonings");
        assert_eq!(classify("frozen peas"), "Frozen");
        assert_eq!(classify("spaghetti"), "Pantry");
        assert_eq!(classify("unobtainium"), "Other");
        assert_eq!(classify("eggs"), "Dairy & Eggs");
        // Misfiles found in review: prefix matches and rule order.
        assert_eq!(classify("eggplant"), "Produce");
        assert_eq!(classify("salted butter"), "Dairy & Eggs");
        assert_eq!(classify("rolled oats"), "Pantry");
        assert_eq!(classify("peanuts"), "Pantry");
        assert_eq!(classify("cornmeal"), "Pantry");
        assert_eq!(classify("cream of tartar"), "Baking");
        assert_eq!(classify("hamburger buns"), "Bakery");
        assert_eq!(classify("garlic powder"), "Spices & Seasonings");
        assert_eq!(classify("3 cloves garlic"), "Produce");
        assert_eq!(classify("fresh blueberries"), "Produce");
        assert_eq!(classify("chicken broth"), "Canned & Jarred");
        assert_eq!(classify("bay leaves"), "Spices & Seasonings");
        assert_eq!(classify("ground cloves"), "Spices & Seasonings");
        assert_eq!(classify("yellow cake mix"), "Baking");
        assert_eq!(classify("hamburger buns"), "Bakery");
        assert_eq!(classify("ground hamburger"), "Meat & Seafood");
        assert_eq!(classify("quartered artichoke hearts"), "Canned & Jarred");
        assert_eq!(classify("maple extract"), "Baking");
    }
}
