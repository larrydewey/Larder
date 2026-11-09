//! Rule-based recipe categorisation, used to organise collections that were
//! imported without categories.
//!
//! Each category has keywords. A keyword in the recipe *name* counts far more
//! than one in the description or ingredients; the highest score wins. Ties
//! go to the category listed first.

use crate::model::RecipeData;

/// Category names Larder suggests, in cookbook-friendly order.
pub const CATEGORIES: &[&str] = &[
    "Breakfast",
    "Appetizers & Snacks",
    "Soups & Stews",
    "Salads",
    "Main Dishes",
    "Side Dishes",
    "Breads",
    "Desserts",
    "Drinks",
    "Sauces & Condiments",
    "Seasonings & Spice Mixes",
    "Household & DIY",
];

const RULES: &[(&str, &[&str])] = &[
    (
        "Household & DIY",
        &[
            "goo b gone",
            "goo gone",
            "stain remover",
            "counter cleaner",
            "deodorant",
            "cleaner",
            "cleaning",
            "detergent",
            "laundry",
            "soap",
            "lotion",
            "lip balm",
            "bath salt",
            "bath bomb",
            "scrub",
            "dishwasher",
            "disinfectant",
            "air freshener",
            "candle",
            "play dough",
            "playdough",
            "washing soda",
            "body butter",
            "toothpaste",
        ],
    ),
    (
        "Seasonings & Spice Mixes",
        &[
            "soup mix",
            "dressing mix",
            "seasoning mix",
            "ranch seasoning",
            "seasoning",
            "spice mix",
            "spice blend",
            "rub",
            "taco seasoning",
            "everything bagel seasoning",
            "chip seasoning",
        ],
    ),
    (
        "Drinks",
        &[
            "drink",
            "smoothie",
            "lemonade",
            "limeade",
            "tea",
            "coffee",
            "latte",
            "cocoa",
            "hot chocolate",
            "punch",
            "wassail",
            "cider",
            "shake",
            "milkshake",
            "cocktail",
            "mocktail",
            "juice",
            "spiced milk",
            "eggnog",
            "horchata",
            "agua fresca",
            "kombucha",
            "sangria",
        ],
    ),
    (
        "Breakfast",
        &[
            "griddle cake",
            "yogurt",
            "breakfast log",
            "pancake",
            "waffle",
            "french toast",
            "oatmeal",
            "porridge",
            "granola",
            "omelet",
            "omelette",
            "frittata",
            "breakfast",
            "crepe",
            "hash brown",
            "breakfast hash",
            "quiche",
            "overnight oats",
            "eggs benedict",
            "scrambled eggs",
            "shakshuka",
            "breakfast burrito",
        ],
    ),
    (
        "Desserts",
        &[
            "treats",
            "rice krispie",
            "gingerbread",
            "peanut butter cups",
            "crust",
            "pie crust",
            "pie dough",
            "candy",
            "muddy buddies",
            "snowball",
            "cake",
            "cookie",
            "brownie",
            "pie",
            "cheesecake",
            "frosting",
            "icing",
            "fudge",
            "candy",
            "pudding",
            "crisp",
            "crumble",
            "cobbler",
            "dessert",
            "tart",
            "ice cream",
            "blondie",
            "cupcake",
            "truffle",
            "mousse",
            "brulee",
            "fondant",
            "toffee",
            "caramel",
            "custard",
            "macaron",
            "meringue",
            "sorbet",
            "trifle",
            "bar",
            "bars",
            "snickerdoodle",
            "shortbread",
            "baklava",
            "tiramisu",
            "donut",
            "doughnut",
            "cinnamon roll",
            "sweet roll",
            "buttercream",
            "ganache",
            "cannoli",
            "eclair",
            "turnover",
            "galette",
        ],
    ),
    (
        "Breads",
        &[
            "bread",
            "biscuit",
            "roll",
            "rolls",
            "bun",
            "buns",
            "sourdough",
            "focaccia",
            "tortilla",
            "naan",
            "pizza dough",
            "bagel",
            "loaf",
            "cornbread",
            "dough",
            "brioche",
            "challah",
            "pita",
            "flatbread",
            "scone",
            "muffin",
            "muffins",
            "pretzel",
            "croissant",
            "english muffin",
        ],
    ),
    (
        "Soups & Stews",
        &["soup", "stew", "chili", "chowder", "bisque", "gumbo", "pho", "ramen", "broth", "minestrone", "goulash", "posole", "pozole"],
    ),
    ("Salads", &["salad", "slaw", "coleslaw", "caprese"]),
    (
        "Sauces & Condiments",
        &[
            "applesauce",
            "reduction",
            "au jus",
            "queso",
            "condensed milk",
            "sauce",
            "dressing",
            "gravy",
            "mayo",
            "mayonnaise",
            "ketchup",
            "pesto",
            "vinaigrette",
            "marinade",
            "syrup",
            "jam",
            "jelly",
            "compote",
            "relish",
            "pickles",
            "pickled",
            "chutney",
            "aioli",
            "glaze",
            "salsa",
            "dip",
            "hummus",
            "guacamole",
            "curd",
            "preserves",
            "frosting",
        ],
    ),
    (
        "Appetizers & Snacks",
        &[
            "cheese ball",
            "kettle corn",
            "chex mix",
            "fruit cups",
            "pumpkin seeds",
            "appetizer",
            "bites",
            "poppers",
            "wings",
            "nachos",
            "snack",
            "trail mix",
            "deviled eggs",
            "crackers",
            "chips",
            "dip",
            "salsa",
            "guacamole",
            "hummus",
            "bruschetta",
            "crostini",
            "pinwheels",
            "sliders",
            "meatballs",
            "energy balls",
            "popcorn",
            "jerky",
        ],
    ),
    (
        "Side Dishes",
        &[
            "lentils",
            "lentil",
            "squash",
            "carrots",
            "broccoli",
            "brussels sprouts",
            "asparagus",
            "vegetables",
            "corn",
            "side",
            "mashed",
            "roasted vegetables",
            "rice",
            "pilaf",
            "risotto",
            "beans",
            "stuffing",
            "mac and cheese",
            "macaroni and cheese",
            "potatoes",
            "fries",
            "green beans",
            "corn on the cob",
            "coleslaw",
            "au gratin",
            "scalloped",
        ],
    ),
    (
        "Main Dishes",
        &[
            "tilapia",
            "cod",
            "halibut",
            "meatball",
            "sloppy joe",
            "crunchwrap",
            "bowl",
            "hash",
            "chicken",
            "beef",
            "pork",
            "steak",
            "fish",
            "salmon",
            "shrimp",
            "tuna",
            "turkey",
            "lamb",
            "sausage",
            "lasagna",
            "casserole",
            "tacos",
            "taco",
            "enchilada",
            "enchiladas",
            "curry",
            "pasta",
            "spaghetti",
            "fettuccine",
            "alfredo",
            "burger",
            "pizza",
            "meatloaf",
            "stir fry",
            "stir-fry",
            "roast",
            "fajitas",
            "burrito",
            "quesadilla",
            "pot pie",
            "ham",
            "ribs",
            "brisket",
            "pulled pork",
            "gnocchi",
            "ravioli",
            "noodles",
            "mostaccioli",
            "ziti",
            "chops",
            "tenders",
            "nuggets",
            "kabob",
            "kebab",
            "sandwich",
            "wrap",
            "meatza",
        ],
    ),
];

fn words(s: &str) -> Vec<String> {
    crate::text::fold(s).split(|c: char| !c.is_alphanumeric()).filter(|w| !w.is_empty()).map(str::to_string).collect()
}

/// End position (1-based word index) of the last occurrence of `phrase` in `hay`.
fn phrase_end(hay: &[String], phrase: &str) -> Option<usize> {
    let p = words(phrase);
    if p.is_empty() || p.len() > hay.len() {
        return None;
    }
    hay.windows(p.len())
        .enumerate()
        .filter(|(_, w)| {
            w.iter().zip(&p).all(|(a, b)| a == b || *a == format!("{b}s") || *a == format!("{b}es") || crate::food::singular(a) == *b)
        })
        .map(|(i, _)| i + p.len())
        .next_back()
}

/// The part of a recipe name that names the dish: parenthetical remarks and
/// anything after "with" ("Pizza with Bacon and Corn") are dropped.
fn head_of_name(name: &str) -> Vec<String> {
    let mut s = String::new();
    let mut depth = 0;
    for c in name.chars() {
        match c {
            '(' | '{' | '[' => depth += 1,
            ')' | '}' | ']' => depth -= 1,
            _ if depth <= 0 => s.push(c),
            _ => {}
        }
    }
    let ws = words(&s);
    match ws.iter().position(|w| w == "with" || w == "without") {
        Some(i) if i > 0 => ws[..i].to_vec(),
        _ => ws,
    }
}

/// Suggests the single best category for a recipe, if any rule matches.
///
/// English dish names put the dish last ("Chicken Noodle *Soup*", "Pork
/// *Salad*"), so the category whose keyword ends latest in the name wins,
/// longer phrases breaking ties. Only when the name says nothing does the
/// description count, and only then do ingredients hint at a main dish.
pub fn suggest_category(r: &RecipeData) -> Option<&'static str> {
    let name = head_of_name(&r.name);
    let rank = |cat: &str| CATEGORIES.iter().position(|c| *c == cat).unwrap_or(CATEGORIES.len());
    let best_in = |hay: &[String]| -> Option<&'static str> {
        let mut best: Option<(usize, usize, usize, &'static str)> = None;
        for (cat, keywords) in RULES.iter() {
            for kw in keywords.iter() {
                if let Some(end) = phrase_end(hay, kw) {
                    let cand = (end, kw.split_whitespace().count(), usize::MAX - rank(cat), *cat);
                    if best.is_none_or(|b| (cand.0, cand.1, cand.2) > (b.0, b.1, b.2)) {
                        best = Some(cand);
                    }
                }
            }
        }
        best.map(|b| b.3)
    };
    if let Some(c) = best_in(&name) {
        // "Lemon Chicken & Green Beans", "Pork Chops and Roasted Potatoes": a main
        // served with a side is a main dish.
        if c == "Side Dishes" {
            let mains = RULES.iter().find(|(c, _)| *c == "Main Dishes").map(|(_, k)| *k).unwrap_or(&[]);
            if mains.iter().any(|kw| phrase_end(&name, kw).is_some()) {
                return Some("Main Dishes");
            }
        }
        return Some(c);
    }
    if let Some(c) = best_in(&words(&r.description)) {
        return Some(c);
    }
    let ings: Vec<String> = r.ingredients.iter().filter_map(|i| i.food.as_deref()).flat_map(words).collect();
    let mains = RULES.iter().find(|(c, _)| *c == "Main Dishes").map(|(_, k)| *k).unwrap_or(&[]);
    mains.iter().any(|kw| phrase_end(&ings, kw).is_some()).then_some("Main Dishes")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::Ingredient;

    fn r(name: &str, ings: &[&str]) -> RecipeData {
        let mut d = RecipeData {
            name: name.into(),
            ingredients: ings.iter().map(|l| Ingredient { original: l.to_string(), ..Default::default() }).collect(),
            ..Default::default()
        };
        d.normalize();
        d
    }

    #[test]
    fn suggests_sensible_categories() {
        let cases = [
            ("Buttermilk Pancakes", "Breakfast"),
            ("Copycat Restaurant Garlic Cheddar Biscuits", "Breads"),
            ("Chocolate Chip Cookies", "Desserts"),
            ("Slow Cooker Chicken Curry", "Main Dishes"),
            ("Classic Chili", "Soups & Stews"),
            ("Homemade Mayonnaise", "Sauces & Condiments"),
            ("Smoky Popcorn Seasoning", "Seasonings & Spice Mixes"),
            ("Homemade Laundry Soap", "Household & DIY"),
            ("Hot Apple Cider", "Drinks"),
            ("Cardamom Spiced Milk", "Drinks"),
            ("Tomato Basil Salad Recipe", "Salads"),
            ("Spinach Artichoke Dip", "Appetizers & Snacks"),
            ("Turkey Pot Pie Recipe", "Main Dishes"),
            ("Sourdough Cinnamon Twist", "Breads"),
            ("Classic Herb Stuffing", "Side Dishes"),
            ("Raspberry Compote", "Sauces & Condiments"),
            ("Garlic Parmesan Pull-apart Bread", "Breads"),
            ("Oatmeal Raisin Cookies", "Desserts"),
            ("Chicken Noodle Soup", "Soups & Stews"),
            ("Grilled Pork Salad", "Salads"),
            ("Country Sausage Gravy", "Sauces & Condiments"),
            ("Creamy Ranch Dressing", "Sauces & Condiments"),
            ("Chocolate Peanut Butter Cups", "Desserts"),
            ("Taco Seasoning (Spice Mix)", "Seasonings & Spice Mixes"),
            ("Copycat French Onion Soup Mix Recipe", "Seasonings & Spice Mixes"),
            ("Pepperoni Pizza with Peppers and Olives", "Main Dishes"),
            ("Johnnycake Griddle Cakes", "Breakfast"),
            ("Brown Butter Crispy Rice Treats", "Desserts"),
            ("Cherry Reduction", "Sauces & Condiments"),
            ("Crab Dip", "Appetizers & Snacks"),
            ("Apple Pie without Refined Sugar", "Desserts"),
            ("Sheet Pan Lemon Chicken & Green Beans", "Main Dishes"),
            ("Pork Chops and Roasted Potatoes {Paleo}", "Main Dishes"),
            ("Creamy Mashed Potatoes Recipe", "Side Dishes"),
            ("Braised Lentils With Carrots and Onion", "Side Dishes"),
        ];
        for (name, want) in cases {
            assert_eq!(suggest_category(&r(name, &[])), Some(want), "{name}");
        }
        assert_eq!(suggest_category(&r("Sunday Special", &["2 lbs chicken thighs"])), Some("Main Dishes"));
        assert_eq!(suggest_category(&r("Mystery", &["1 cup water"])), None);
    }
}
