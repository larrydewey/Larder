//! Recipe Keeper: export zip with `recipes.html` (one `div.recipe-details`
//! per recipe, annotated with `itemprop`s) and an `images/` folder.

use larder_core::{Instruction, RecipeData};
use scraper::{ElementRef, Html, Selector};

use crate::archive::{dirname, resolve_relative, SafeZip};
use crate::schema_org::{ingredients_from_lines, parse_iso8601_duration};
use crate::util::{as_bool, clean_block, clean_line, nutrition_key, parse_yield_text, resolve_url, split_list, strip_step_number};
use crate::{Collector, ImageSource, ImportError, ImportedRecipe};

pub(crate) fn parse_zip(zip: &mut SafeZip, c: &mut Collector) -> Result<(), ImportError> {
    let html_name = zip
        .names()
        .find(|n| n.eq_ignore_ascii_case("recipes.html") || n.to_ascii_lowercase().ends_with("/recipes.html"))
        .map(str::to_string)
        .ok_or_else(|| ImportError::UnsupportedFormat("recipes.html not found in the archive".into()))?;
    let html = zip.read_string(&html_name)?;
    let base = dirname(&html_name).to_string();
    let mut lookup = |src: &str| -> Option<Vec<u8>> {
        let path = resolve_relative(&base, src)?;
        let name = zip.find_ci(&path)?;
        zip.read(&name).ok()
    };
    if parse_document(&html, &mut lookup, c) == 0 {
        return Err(ImportError::UnsupportedFormat("recipes.html contains no Recipe Keeper recipes (div.recipe-details)".into()));
    }
    Ok(())
}

fn sel(s: &str) -> Selector {
    Selector::parse(s).expect("valid selector")
}

fn text_of(el: ElementRef) -> String {
    clean_line(&el.text().collect::<Vec<_>>().join(" "))
}

/// Text lines of a container: one per `<p>`/`<li>`/`<div>` child, or its text lines.
fn lines_of(el: ElementRef) -> Vec<String> {
    let p = sel("p, li");
    let mut lines: Vec<String> = el.select(&p).map(text_of).collect();
    if lines.is_empty() {
        lines = clean_block(&el.inner_html()).lines().map(clean_line).collect();
    }
    lines
}

/// Parses the `recipes.html` document. `image` loads a referenced image file.
/// Returns the number of recipe blocks found.
pub(crate) fn parse_document(html: &str, image: &mut dyn FnMut(&str) -> Option<Vec<u8>>, c: &mut Collector) -> usize {
    let doc = Html::parse_document(html);
    let recipe_sel = sel("div.recipe-details");
    let prop_sel = sel("[itemprop]");
    let mut found = 0;
    for (i, div) in doc.select(&recipe_sel).enumerate() {
        found += 1;
        let mut d = RecipeData::default();
        let mut favorite = false;
        let mut photo: Option<String> = None;
        let mut notes: Vec<String> = vec![];
        let mut source: Option<String> = None;
        for el in div.select(&prop_sel) {
            let prop = el.value().attr("itemprop").unwrap_or("").trim();
            let content = el.value().attr("content").map(clean_line);
            let value = || content.clone().unwrap_or_else(|| text_of(el));
            match prop {
                "name" => d.name = value(),
                "recipeYield" => {
                    let (q, u) = parse_yield_text(&value());
                    d.yield_quantity = q;
                    d.yield_unit = u;
                }
                "prepTime" => d.prep_time_minutes = parse_iso8601_duration(&value()),
                "cookTime" => d.cook_time_minutes = parse_iso8601_duration(&value()),
                "totalTime" => d.total_time_minutes = parse_iso8601_duration(&value()),
                "recipeCategory" => d.categories.extend(split_list(&value())),
                "recipeCourse" | "recipeCollection" => d.tags.extend(split_list(&value())),
                "recipeSource" => {
                    let link = el.select(&sel("a[href]")).next().and_then(|a| a.value().attr("href")).map(str::to_string);
                    source = link.or_else(|| Some(value()));
                }
                "recipeRating" => d.rating = value().trim().parse::<f64>().ok().map(|r| r.round() as i64),
                "recipeIsFavourite" | "recipeIsFavorite" => {
                    favorite = as_bool(&serde_json::Value::String(value()));
                }
                "recipeIngredients" | "recipeIngredient" => {
                    d.ingredients = ingredients_from_lines(&lines_of(el));
                }
                "recipeDirections" | "recipeInstructions" => {
                    let mut section = None;
                    for l in lines_of(el) {
                        let t = strip_step_number(&l);
                        if t.is_empty() {
                            continue;
                        }
                        if t.ends_with(':') && t.chars().count() < 50 && !t.contains(". ") {
                            section = Some(t.trim_end_matches(':').trim().to_string());
                            continue;
                        }
                        d.instructions.push(Instruction { section: section.clone(), text: t });
                    }
                }
                "recipeNotes" => {
                    let n = lines_of(el).join("\n");
                    if !n.trim().is_empty() {
                        notes.push(n.trim().to_string());
                    }
                }
                "description" | "recipeDescription" => d.description = clean_block(&el.inner_html()),
                p if p.starts_with("photo") => {
                    if photo.is_none() || p == "photo0" {
                        photo = el.value().attr("src").map(str::to_string);
                    }
                }
                p if p.starts_with("recipeNut") => {
                    let v = value();
                    if let Some(key) = nutrition_key(p) {
                        if !v.is_empty() {
                            let unit = match key.as_str() {
                                "calories" | "servingSize" => "",
                                "sodiumContent" | "cholesterolContent" => " mg",
                                _ if v.parse::<f64>().is_ok() => " g",
                                _ => "",
                            };
                            d.nutrition.insert(key, format!("{v}{unit}"));
                        }
                    }
                }
                _ => {}
            }
        }
        if photo.is_none() {
            photo = div.select(&sel("img.recipe-photo")).next().and_then(|i| i.value().attr("src")).map(str::to_string);
        }
        if let Some(src) = source.filter(|s| !s.is_empty()) {
            match resolve_url(&src, None) {
                Some(u) => d.source_url = Some(u),
                None => d.source_name = Some(src),
            }
        }
        d.notes = notes.join("\n\n");
        let image = photo.as_deref().and_then(|src| {
            if src.starts_with("http://") || src.starts_with("https://") {
                Some(ImageSource::Url(src.to_string()))
            } else {
                image(src).map(ImageSource::Bytes)
            }
        });
        let ctx = if d.name.is_empty() { format!("#{}", i + 1) } else { d.name.clone() };
        c.push(ImportedRecipe { data: d, image, created_at: None, favorite }, &ctx);
    }
    found
}

#[cfg(test)]
mod tests {
    use crate::archive::testutil::make_zip;
    use crate::{parse_file, ImageSource, Source};

    const JPEG: &[u8] = &[0xFF, 0xD8, 0xFF, 0xE0, 1, 2, 3];

    const HTML: &str = r#"<!DOCTYPE html><html><head><style>.x{}</style></head><body>
    <div class="recipe-details">
        <meta content="2c76fe7d" itemprop="recipeId">
        <meta content="True" itemprop="recipeIsFavourite">
        <meta content="5" itemprop="recipeRating">
        <table><tr><td><img src="images/2c76fe7d_0.jpg" class="recipe-photo"/></td>
        <td>
            <h2 itemprop="name">Baked Salmon Fillets Dijon</h2>
            <div>Courses: <span itemprop="recipeCourse">Main Dish</span></div>
            <div>Categories: <span>Fish</span><meta content="Fish" itemprop="recipeCategory"></div>
            <div>Collections: <span>Tried and True</span><meta content="Tried and True" itemprop="recipeCollection"></div>
            <div>Source: <span itemprop="recipeSource"><a href="https://www.allrecipes.example/recipe/22538/">https://www.allrecipes.example/recipe/22538/</a></span></div>
            <div>Serving size: <span itemprop="recipeYield">4 servings</span></div>
            <div>Preparation time: <span>10 mins</span><meta content="PT10M" itemprop="prepTime"></div>
            <div>Cooking time: <span>15 mins</span><meta content="PT15M" itemprop="cookTime"></div>
        </td></tr>
        <tr><td><h3>Ingredients</h3>
            <div class="recipe-ingredients" itemprop="recipeIngredients">
                <p>4 (4 ounce) fillets salmon</p>
                <p>3 tablespoons prepared Dijon-style mustard</p>
                <p>&#188; cup butter, melted</p>
            </div></td>
        <td><h3>Directions</h3>
            <div itemprop="recipeDirections">
                <p>1. Preheat oven to 400 degrees F.</p><p></p><p></p>
                <p>2. Place salmon skin-side down on foil.</p>
                <p>3. Bake for 15 minutes.</p>
            </div></td></tr></table>
        <div class="recipe-notes" itemprop="recipeNotes"><p>Great with rice.</p></div>
        <div>Calories: 330.6<meta content="330.6" itemprop="recipeNutCalories"></div>
        <div>Total Fat: 21.5g<meta content="21.5" itemprop="recipeNutTotalFat"></div>
        <div class="recipe-photos-div"><img src="images/2c76fe7d_0.jpg" class="recipe-photos" itemprop=photo0 /></div>
    </div>
    <div class="recipe-details">
        <meta content="False" itemprop="recipeIsFavourite">
        <h2 itemprop="name">Banana Muffins</h2>
        <span itemprop="recipeSource">Recipe box</span>
        <span itemprop="recipeYield">12</span>
        <div itemprop="recipeIngredients"><p>3 bananas</p></div>
        <div itemprop="recipeDirections"><p>Mash and bake.</p></div>
        <div class="recipe-notes" itemprop="recipeNotes"></div>
    </div>
    <div class="recipe-details"><h2 itemprop="name"></h2></div>
    </body></html>"#;

    #[test]
    fn recipe_keeper_zip() {
        let z = make_zip(&[("recipes.html", HTML.as_bytes()), ("images/2c76fe7d_0.jpg", JPEG)]);
        let b = parse_file(Source::Auto, "RecipeKeeper.zip", &z).unwrap();
        assert_eq!(b.source, Source::RecipeKeeper);
        assert_eq!(b.recipes.len(), 2);
        assert_eq!(b.errors.len(), 1, "{:?}", b.errors);
        let r = &b.recipes[0];
        let d = &r.data;
        assert_eq!(d.name, "Baked Salmon Fillets Dijon");
        assert!(r.favorite);
        assert_eq!(d.rating, Some(5));
        assert_eq!(d.tags, vec!["Main Dish", "Tried and True"]);
        assert_eq!(d.categories, vec!["Fish"]);
        assert_eq!(d.source_url.as_deref(), Some("https://www.allrecipes.example/recipe/22538/"));
        assert_eq!(d.source_name.as_deref(), Some("allrecipes.example"));
        assert_eq!((d.yield_quantity, d.prep_time_minutes, d.cook_time_minutes), (Some(4.0), Some(10), Some(15)));
        assert_eq!(d.ingredients.len(), 3);
        assert_eq!(d.ingredients[2].quantity, Some(0.25));
        assert_eq!(d.instructions.len(), 3);
        assert_eq!(d.instructions[0].text, "Preheat oven to 400 degrees F.");
        assert_eq!(d.notes, "Great with rice.");
        assert_eq!(d.nutrition.get("calories").map(String::as_str), Some("330.6"));
        assert_eq!(d.nutrition.get("fatContent").map(String::as_str), Some("21.5 g"));
        assert!(matches!(r.image, Some(ImageSource::Bytes(ref x)) if x == JPEG));
        let m = &b.recipes[1];
        assert!(!m.favorite);
        assert_eq!(m.data.source_name.as_deref(), Some("Recipe box"));
        assert_eq!(m.data.yield_quantity, Some(12.0));
        assert!(m.image.is_none());
    }
}
