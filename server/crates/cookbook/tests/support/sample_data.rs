//! Sample recipes and procedurally generated "food photos" shared by the
//! integration tests and the `sample` example.
#![allow(dead_code)]

use std::collections::BTreeMap;

use larder_cookbook::BookRecipe;
use larder_core::{Ingredient, Instruction, RecipeData};

/// A plated-food-ish picture: wooden table, plate, and a colourful dish.
pub fn food_photo(w: u32, h: u32, seed: u32, format: image::ImageFormat) -> Vec<u8> {
    let palette = [
        ([196, 92, 38], [236, 180, 74]),
        ([120, 150, 60], [214, 70, 52]),
        ([232, 196, 120], [150, 75, 40]),
        ([170, 40, 50], [245, 225, 190]),
        ([90, 120, 160], [240, 200, 90]),
    ];
    let (food_a, food_b) = palette[(seed as usize) % palette.len()];
    let (cx, cy) = (w as f32 * (0.45 + 0.1 * ((seed % 3) as f32 / 2.0)), h as f32 * 0.52);
    let plate_r = w.min(h) as f32 * 0.42;
    let img = image::RgbImage::from_fn(w, h, |x, y| {
        let (xf, yf) = (x as f32, y as f32);
        // wood
        let grain = ((yf * 0.045 + (xf * 0.004).sin() * 3.0 + seed as f32).sin() * 0.5 + 0.5) * 22.0;
        let mut px = [(118.0 + grain), (82.0 + grain * 0.7), (52.0 + grain * 0.4)];
        let d = ((xf - cx).powi(2) + (yf - cy).powi(2)).sqrt();
        // shadow
        let sd = ((xf - cx - plate_r * 0.06).powi(2) + (yf - cy - plate_r * 0.08).powi(2)).sqrt();
        if sd < plate_r * 1.02 {
            for c in &mut px {
                *c *= 0.72;
            }
        }
        if d < plate_r {
            let rim = (d / plate_r).powi(6);
            let v = 246.0 - rim * 30.0;
            px = [v, v - 2.0, v - 8.0];
            if d < plate_r * 0.68 {
                let t = ((xf * 0.05 + seed as f32).sin() * (yf * 0.06).cos() * 0.5 + 0.5).clamp(0.0, 1.0);
                let n = (((xf * 0.31).sin() * (yf * 0.27).cos()) * 0.5 + 0.5) * 18.0;
                for i in 0..3 {
                    px[i] = food_a[i] as f32 * (1.0 - t) + food_b[i] as f32 * t - n;
                }
                // herbs
                if ((xf * 0.11 + yf * 0.07).sin() * (xf * 0.05 - yf * 0.13).cos()) > 0.93 {
                    px = [60.0, 120.0, 50.0];
                }
            }
        }
        image::Rgb([px[0].clamp(0.0, 255.0) as u8, px[1].clamp(0.0, 255.0) as u8, px[2].clamp(0.0, 255.0) as u8])
    });
    let mut out = vec![];
    image::DynamicImage::ImageRgb8(img).write_to(&mut std::io::Cursor::new(&mut out), format).unwrap();
    out
}

fn ings(lines: &[&str]) -> Vec<Ingredient> {
    let mut section: Option<String> = None;
    let mut out = vec![];
    for l in lines {
        if let Some(s) = l.strip_prefix("## ") {
            section = Some(s.to_string());
            continue;
        }
        let mut i = Ingredient::parse(l);
        i.section = section.clone();
        out.push(i);
    }
    out
}

fn steps(lines: &[&str]) -> Vec<Instruction> {
    let mut section: Option<String> = None;
    let mut out = vec![];
    for l in lines {
        if let Some(s) = l.strip_prefix("## ") {
            section = Some(s.to_string());
            continue;
        }
        out.push(Instruction { section: section.clone(), text: l.to_string() });
    }
    out
}

#[allow(clippy::too_many_arguments)]
fn recipe(
    name: &str,
    desc: &str,
    cats: &[&str],
    tags: &[&str],
    yield_q: Option<f64>,
    prep: Option<i64>,
    cook: Option<i64>,
    ingredients: &[&str],
    method: &[&str],
    notes: &str,
) -> RecipeData {
    let mut r = RecipeData {
        name: name.into(),
        description: desc.into(),
        categories: cats.iter().map(|s| s.to_string()).collect(),
        tags: tags.iter().map(|s| s.to_string()).collect(),
        yield_quantity: yield_q,
        prep_time_minutes: prep,
        cook_time_minutes: cook,
        ingredients: ings(ingredients),
        instructions: steps(method),
        notes: notes.into(),
        ..Default::default()
    };
    r.normalize();
    r
}

/// A varied, realistic set of recipes (sections, notes, nutrition, sources,
/// non-Latin text, a very long title, a recipe with no ingredients…).
pub fn sample_recipes(with_images: bool) -> Vec<BookRecipe> {
    let mut v: Vec<RecipeData> = vec![];
    let mut r = recipe(
        "Grandma's Buttermilk Pancakes",
        "Light, fluffy and golden — the Sunday-morning stack that started it all. Make the batter the night before for even more tender pancakes.",
        &["Breakfast"],
        &["Family favorite", "Quick"],
        Some(4.0),
        Some(10),
        Some(20),
        &[
            "2 cups all-purpose flour",
            "2 tbsp sugar",
            "2 tsp baking powder",
            "1/2 tsp baking soda",
            "1/2 tsp salt",
            "2 cups buttermilk",
            "2 large eggs",
            "3 tbsp unsalted butter, melted, plus more for the pan",
            "1 tsp vanilla extract",
            "## To serve",
            "maple syrup",
            "1 1/2 cups fresh blueberries",
        ],
        &[
            "Whisk the flour, sugar, baking powder, baking soda and salt together in a large bowl.",
            "In a second bowl whisk the buttermilk, eggs, melted butter and vanilla until smooth.",
            "Pour the wet ingredients into the dry and stir gently with a spatula until just combined — a few lumps are fine. Rest the batter for 10 minutes.",
            "Heat a griddle over medium heat and brush with butter. Pour 1/4 cup batter per pancake and cook until bubbles form on the surface and the edges look set, about 2 minutes.",
            "Flip and cook until golden, 1–2 minutes more. Keep warm in a low oven while you cook the rest.",
        ],
        "Don't overmix the batter or the pancakes will be tough.\n\nFor blueberry pancakes, scatter berries over each pancake right after pouring rather than stirring them into the batter.",
    );
    r.source_name = Some("Grandma Rose's recipe box".into());
    r.nutrition = BTreeMap::from([
        ("calories".to_string(), "420 kcal".to_string()),
        ("proteinContent".to_string(), "12 g".to_string()),
        ("fatContent".to_string(), "14 g".to_string()),
        ("carbohydrateContent".to_string(), "62 g".to_string()),
    ]);
    v.push(r);

    let mut r = recipe(
        "Crème Brûlée",
        "Silky vanilla custard under a crackling caramel crust.",
        &["Desserts"],
        &["French", "Make ahead"],
        Some(6.0),
        Some(20),
        Some(45),
        &["2 cups heavy cream", "1 vanilla bean, split", "5 egg yolks", "1/2 cup sugar", "6 tsp demerara sugar, for the tops"],
        &[
            "Heat the oven to 300°F (150°C). Warm the cream with the vanilla bean until steaming, then let it infuse for 15 minutes.",
            "Whisk the yolks and sugar until pale. Slowly whisk in the warm cream, then strain into a jug.",
            "Divide among six ramekins set in a roasting pan. Pour hot water into the pan to come halfway up the ramekins and bake until just set with a slight wobble, 35–45 minutes.",
            "Chill for at least 4 hours. Sprinkle each with 1 tsp demerara sugar and caramelise with a torch.",
        ],
        "The custards keep, covered, for up to 3 days in the fridge — brûlée them just before serving.",
    );
    r.source_name = Some("Le Cordon Bleu at Home".into());
    r.source_url = Some("https://www.example.com/recipes/creme-brulee?ref=book".into());
    v.push(r);

    v.push(recipe(
        "Classic Beef Bourguignon with Pearl Onions, Mushrooms, Smoked Bacon Lardons and a Rich Burgundy Wine Sauce",
        "A very long title to make sure headings wrap gracefully across several lines without breaking the layout.",
        &["Mains"],
        &["Slow cooking", "French", "Winter"],
        Some(6.0),
        Some(30),
        Some(180),
        &[
            "## For the stew",
            "3 lb beef chuck, cut into 2-inch pieces",
            "6 oz thick-cut bacon, cut into lardons",
            "2 tbsp olive oil",
            "1 large carrot, sliced",
            "1 onion, sliced",
            "3 cloves garlic, crushed",
            "2 tbsp all-purpose flour",
            "3 cups red Burgundy wine",
            "2 cups beef stock",
            "1 tbsp tomato paste",
            "1 bay leaf",
            "4 sprigs thyme",
            "## For the garnish",
            "18 pearl onions, peeled",
            "1 lb cremini mushrooms, quartered",
            "2 tbsp butter",
            "chopped parsley",
        ],
        &[
            "## Brown the meat",
            "Heat the oven to 325°F. Crisp the bacon in a Dutch oven over medium heat; remove with a slotted spoon.",
            "Pat the beef dry and brown it in batches in the bacon fat with a little oil. Don't crowd the pan.",
            "## Braise",
            "Soften the carrot and onion in the same pan, add the garlic, then return the beef and bacon. Sprinkle over the flour and toss.",
            "Add the wine, stock, tomato paste and herbs. Bring to a simmer, cover and braise in the oven for 2½–3 hours until the beef is very tender.",
            "## Finish",
            "Brown the pearl onions and mushrooms in butter and stir into the stew. Season, scatter with parsley and serve with buttered noodles or crusty bread.",
        ],
        "Even better the next day.",
    ));

    let mut r = recipe(
        "Ελληνική Σαλάτα (Greek Village Salad)",
        "Χωριάτικη — ripe tomatoes, cucumber, olives and a slab of feta. Борщ and 日本語 text test fallback fonts.",
        &["Salads"],
        &["Vegetarian", "Summer", "Quick"],
        Some(4.0),
        Some(15),
        None,
        &[
            "4 ripe tomatoes, cut into wedges",
            "1 cucumber, sliced",
            "1 small red onion, thinly sliced",
            "1 green bell pepper, sliced",
            "1/2 cup Kalamata olives",
            "7 oz feta cheese, in one piece",
            "1 tsp dried oregano",
            "1/4 cup extra-virgin olive oil",
        ],
        &["Combine the vegetables and olives in a shallow bowl.", "Top with the feta, sprinkle with oregano and drizzle with the oil."],
        "",
    );
    r.categories = vec!["salads".into()];
    v.push(r);

    v.push(recipe(
        "Simple Tomato Soup",
        "",
        &["Soups"],
        &["Vegetarian", "Winter"],
        Some(4.0),
        Some(10),
        Some(30),
        &[
            "2 tbsp butter",
            "1 onion, diced",
            "2 cloves garlic",
            "28 oz canned whole tomatoes",
            "2 cups vegetable stock",
            "1/2 cup heavy cream",
            "salt and pepper",
        ],
        &[
            "Soften the onion in butter, add garlic.",
            "Add tomatoes and stock; simmer 20 minutes.",
            "Blend until smooth, stir in the cream and season.",
        ],
        "",
    ));

    v.push(recipe(
        "Kitchen Wisdom: Seasoning Cast Iron",
        "Not a recipe so much as a ritual.",
        &[],
        &[],
        None,
        None,
        None,
        &[],
        &[],
        "Scrub, dry thoroughly over a flame, rub with a whisper of oil and bake upside down at 450°F for an hour. Repeat as often as you like.",
    ));

    v.push(recipe(
        "Herb Butter",
        "",
        &["Basics"],
        &[],
        None,
        None,
        None,
        &[
            "1/2 cup salted butter, softened",
            "2 tbsp chopped parsley",
            "1 tbsp chopped chives",
            "1 clove garlic, grated",
            "zest of 1/2 lemon",
        ],
        &[],
        "",
    ));

    v.push(recipe(
        "Lemon Drizzle Cake",
        "Tender, buttery sponge soaked in tangy lemon syrup.",
        &["Desserts", "Baking"],
        &["Afternoon tea"],
        Some(10.0),
        Some(20),
        Some(45),
        &[
            "## Cake",
            "3/4 cup unsalted butter, softened",
            "3/4 cup sugar",
            "3 eggs",
            "1 1/2 cups self-raising flour",
            "zest of 2 lemons",
            "3 tbsp milk",
            "## Drizzle",
            "juice of 2 lemons",
            "1/2 cup powdered sugar",
        ],
        &[
            "Heat the oven to 350°F and line a loaf pan.",
            "Cream the butter and sugar until pale and fluffy, then beat in the eggs one at a time.",
            "Fold in the flour, lemon zest and milk. Scrape into the pan and bake for 40–45 minutes.",
            "Mix the lemon juice and sugar. Prick the warm cake all over and pour the drizzle over.",
        ],
        "",
    ));

    let mut r = recipe(
        "Roast Chicken with Lemon & Thyme",
        "The Sunday roast, simplified.",
        &["Mains"],
        &["Family favorite"],
        Some(4.0),
        Some(15),
        Some(80),
        &[
            "1 whole chicken, about 4 lb",
            "1 lemon, halved",
            "1 head garlic, halved",
            "6 sprigs thyme",
            "2 tbsp butter, softened",
            "1 tsp kosher salt",
        ],
        &[
            "Heat the oven to 425°F. Pat the chicken dry and season inside and out.",
            "Stuff the cavity with lemon, garlic and thyme; rub the skin with butter.",
            "Roast for 70–80 minutes until the juices run clear. Rest 15 minutes before carving.",
        ],
        "",
    );
    r.source_url = Some("https://cooking.example.org/roast-chicken".into());
    v.push(r);

    v.push(recipe(
        "Overnight Oats",
        "",
        &["Breakfast"],
        &["Quick", "Make ahead"],
        Some(1.0),
        Some(5),
        None,
        &["1/2 cup rolled oats", "1/2 cup milk", "1/4 cup Greek yogurt", "1 tbsp chia seeds", "1 tsp honey"],
        &["Stir everything together in a jar, cover and refrigerate overnight."],
        "",
    ));

    let photo_for = |i: usize| -> Option<Vec<u8>> {
        if !with_images {
            return None;
        }
        match i {
            0 => Some(food_photo(1800, 1200, 1, image::ImageFormat::Jpeg)),
            1 => Some(food_photo(900, 1200, 3, image::ImageFormat::Png)), // portrait
            2 => Some(food_photo(1600, 900, 2, image::ImageFormat::Jpeg)),
            3 => Some(food_photo(1200, 800, 4, image::ImageFormat::WebP)),
            7 => Some(food_photo(1000, 1000, 0, image::ImageFormat::Png)),
            8 => Some(b"this is not an image".to_vec()),
            _ => None,
        }
    };
    v.into_iter().enumerate().map(|(i, data)| BookRecipe { id: format!("sample-{i}"), data, image: photo_for(i) }).collect()
}

/// `n` synthetic recipes spread over several categories (for timing tests).
pub fn synthetic_recipes(n: usize, image_every: usize) -> Vec<BookRecipe> {
    let cats = ["Breakfast", "Soups", "Salads", "Mains", "Sides", "Desserts", "Baking", "Drinks", "Preserves"];
    let foods =
        ["chicken", "carrots", "onions", "garlic", "potatoes", "lentils", "spinach", "tomatoes", "rice", "apples", "eggs", "mushrooms"];
    let photo = food_photo(1200, 800, 2, image::ImageFormat::Jpeg);
    (0..n)
        .map(|i| {
            let mut r = RecipeData {
                name: format!("{} {} Number {}", ["Hearty", "Simple", "Spiced", "Roasted", "Golden"][i % 5], foods[i % foods.len()], i),
                description: "A dependable weeknight recipe with plenty of flavour and very little fuss.".into(),
                categories: if i % 17 == 0 { vec![] } else { vec![cats[i % cats.len()].into()] },
                tags: vec![["Quick", "Vegetarian", "Family favorite"][i % 3].into()],
                yield_quantity: Some(4.0),
                prep_time_minutes: Some(15),
                cook_time_minutes: Some(30 + (i % 4) as i64 * 10),
                notes: if i % 3 == 0 { "Keeps well in the fridge for three days.".into() } else { String::new() },
                ingredients: (0..8)
                    .map(|k| Ingredient::parse(&format!("{} cups {}", 1 + k % 3, foods[(i + k) % foods.len()])))
                    .collect(),
                instructions: (0..6)
                    .map(|k| Instruction {
                        section: None,
                        text: format!("Step {k}: cook the ingredients gently, stirring now and then, until everything is tender and fragrant. Season to taste."),
                    })
                    .collect(),
                ..Default::default()
            };
            r.normalize();
            BookRecipe {
                id: format!("syn-{i}"),
                data: r,
                image: (image_every > 0 && i % image_every == 0).then(|| photo.clone()),
            }
        })
        .collect()
}
