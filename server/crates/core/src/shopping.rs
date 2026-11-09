//! Shopping list items and the rules for merging recipe ingredients into a list.
//! Shared by the server and the on-device engine so both behave identically.

use serde::{Deserialize, Serialize};

use crate::model::{Ingredient, RecipeData};
use crate::units::{self, UnitSystem};
use crate::{aisle, format};

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct ShoppingItem {
    pub id: String,
    pub list_id: String,
    pub food: Option<String>,
    pub quantity: Option<f64>,
    pub unit: Option<String>,
    pub note: Option<String>,
    pub display: String,
    pub aisle: String,
    pub checked: bool,
    pub position: i64,
    pub recipe_ids: Vec<String>,
    pub updated_at: String,
}

impl ShoppingItem {
    /// Recomputes the display string from the structured fields.
    pub fn refresh_display(&mut self) {
        self.display = item_display(&self.food, self.quantity, &self.unit, &self.note);
    }

    /// Builds an item from free text such as "2 lbs apples".
    pub fn from_text(text: &str) -> Self {
        let mut ing = Ingredient::parse(text);
        ing.canonicalize();
        let food = ing.food.or_else(|| Some(text.trim().to_string()).filter(|t| !t.is_empty()));
        let mut it = ShoppingItem {
            aisle: aisle_for(food.as_deref().unwrap_or(""), ing.unit.as_deref()).to_string(),
            food,
            quantity: ing.quantity,
            unit: ing.unit,
            note: ing.note,
            ..Default::default()
        };
        it.refresh_display();
        it
    }
}

/// Aisle for a food bought in `unit`: anything sold by the can or jar is
/// shelved with canned goods ("1 can tomatoes, diced"), otherwise by food.
pub fn aisle_for(food: &str, unit: Option<&str>) -> &'static str {
    match unit {
        Some("can") | Some("jar") => "Canned & Jarred",
        _ => aisle::classify(food),
    }
}

pub fn item_display(food: &Option<String>, quantity: Option<f64>, unit: &Option<String>, note: &Option<String>) -> String {
    let ing = Ingredient { quantity, unit: unit.clone(), food: food.clone(), note: note.clone(), ..Default::default() };
    format::ingredient_display(&ing, 1.0, UnitSystem::Original)
}

/// Sorts items into aisle order, then position, then name.
pub fn sort_items(items: &mut [ShoppingItem]) {
    items.sort_by(|a, b| {
        aisle::aisle_order(&a.aisle)
            .cmp(&aisle::aisle_order(&b.aisle))
            .then(a.position.cmp(&b.position))
            .then(a.display.to_lowercase().cmp(&b.display.to_lowercase()))
    });
}

/// A recipe to add to a list, optionally scaled to a number of servings.
pub struct RecipeAddition<'a> {
    pub recipe_id: &'a str,
    pub data: &'a RecipeData,
    pub servings: Option<f64>,
}

/// Result of a merge: which existing items changed and which are new.
#[derive(Debug, Default)]
pub struct MergeResult {
    pub updated: Vec<String>,
    pub created: Vec<String>,
}

fn units_compatible(a: &Option<String>, b: &Option<String>, qa: Option<f64>, qb: Option<f64>) -> bool {
    match (a, b) {
        (None, None) => true,
        (Some(a), Some(b)) => a == b || units::convert_between(1.0, b, a).is_some(),
        // "2 onions" + "onion" merge; "1 cup milk" + "milk" merge when one side has no quantity
        _ => qa.is_none() || qb.is_none(),
    }
}

fn round3(v: f64) -> f64 {
    (v * 1000.0).round() / 1000.0
}

/// Rounds up to the next multiple of `step` (with a little float tolerance).
fn ceil_to(v: f64, step: f64) -> f64 {
    ((v / step) - 1e-6).ceil() * step
}

/// Expresses a summed shopping amount in a sensible unit of the same system
/// and rounds it *up* to a kitchen-friendly step, so merged totals read like
/// "4 3/4 cups" rather than "73 tbsp" or "15.28 cups".
pub fn tidy_amount(qty: f64, unit: &str) -> (f64, String) {
    let Some(def) = units::unit_def(unit) else { return (round3(qty), unit.to_string()) };
    let base = qty * def.factor;
    let (q, u, step) = match (def.dim, def.metric, unit) {
        (units::Dimension::Volume, false, "fl oz") => (qty, "fl oz", 0.5),
        (units::Dimension::Volume, false, _) => {
            if base < 14.0 {
                (base / 4.92892, "tsp", 0.25)
            } else if base < 118.0 {
                (base / 14.7868, "tbsp", 0.5)
            } else if base < 3785.0 {
                (base / 236.588, "cup", 0.25)
            } else {
                (base / 946.353, "quart", 0.25)
            }
        }
        (units::Dimension::Volume, true, _) if base >= 1000.0 => (base / 1000.0, "l", 0.1),
        (units::Dimension::Volume, true, _) => (base, "ml", if base >= 100.0 { 10.0 } else { 5.0 }),
        (units::Dimension::Mass, false, _) if base >= 453.592 => (base / 453.592, "lb", 0.25),
        (units::Dimension::Mass, false, _) => (base / 28.3495, "oz", 0.5),
        (units::Dimension::Mass, true, _) if base >= 1000.0 => (base / 1000.0, "kg", 0.1),
        (units::Dimension::Mass, true, _) => (base, "g", if base >= 100.0 { 10.0 } else { 5.0 }),
        (units::Dimension::Count, _, _) => return (round3(qty), unit.to_string()),
    };
    (round3(ceil_to(q, step)), u.to_string())
}

/// Merges scaled recipe ingredients into `items` (in place). Unchecked items
/// with the same food and a compatible unit are summed; everything else is
/// appended as a new item with an id from `new_id`.
pub fn merge_recipes(
    items: &mut Vec<ShoppingItem>,
    list_id: &str,
    additions: &[RecipeAddition<'_>],
    mut new_id: impl FnMut() -> String,
    now: &str,
) -> MergeResult {
    let mut result = MergeResult::default();
    let mut pos = items.iter().map(|i| i.position).max().map(|p| p + 1).unwrap_or(0);
    for add in additions {
        let scale = match (add.servings, add.data.yield_quantity) {
            (Some(s), Some(y)) if s > 0.0 && y > 0.0 => s / y,
            _ => 1.0,
        };
        for ing in &add.data.ingredients {
            let food = ing.food.clone().or_else(|| Some(ing.original.clone())).filter(|f| !f.trim().is_empty());
            let Some(food) = food else { continue };
            let key = crate::food::food_key(&food);
            let qty = ing.quantity_max.or(ing.quantity).map(|q| q * scale);
            let unit = ing.unit.clone();
            let found = items.iter().position(|it| {
                !it.checked
                    && it.food.as_deref().map(crate::food::food_key).as_deref() == Some(key.as_str())
                    && units_compatible(&it.unit, &unit, it.quantity, qty)
            });
            match found {
                Some(i) => {
                    let it = &mut items[i];
                    if let Some(q) = qty {
                        let add_q = match (&it.unit, &unit) {
                            (Some(a), Some(b)) if a != b => units::convert_between(q, b, a).unwrap_or(q),
                            _ => q,
                        };
                        let total = it.quantity.unwrap_or(0.0) + add_q;
                        if it.unit.is_none() {
                            it.unit = unit.clone();
                        }
                        match it.unit.clone() {
                            Some(u) => {
                                let (q, u) = tidy_amount(total, &u);
                                it.quantity = Some(q);
                                it.unit = Some(u);
                            }
                            None => it.quantity = Some(round3(total)),
                        }
                    }
                    if !it.recipe_ids.iter().any(|r| r == add.recipe_id) {
                        it.recipe_ids.push(add.recipe_id.to_string());
                    }
                    it.updated_at = now.to_string();
                    it.refresh_display();
                    if !result.created.contains(&it.id) && !result.updated.contains(&it.id) {
                        result.updated.push(it.id.clone());
                    }
                }
                None => {
                    let mut it = ShoppingItem {
                        id: new_id(),
                        list_id: list_id.to_string(),
                        aisle: aisle_for(&food, unit.as_deref()).to_string(),
                        food: Some(food),
                        quantity: qty.map(round3),
                        unit,
                        note: None,
                        display: String::new(),
                        checked: false,
                        position: pos,
                        recipe_ids: vec![add.recipe_id.to_string()],
                        updated_at: now.to_string(),
                    };
                    it.refresh_display();
                    pos += 1;
                    result.created.push(it.id.clone());
                    items.push(it);
                }
            }
        }
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    fn recipe(lines: &[&str], yield_q: Option<f64>) -> RecipeData {
        let mut d = RecipeData {
            name: "x".into(),
            yield_quantity: yield_q,
            ingredients: lines.iter().map(|l| Ingredient { original: l.to_string(), ..Default::default() }).collect(),
            ..Default::default()
        };
        d.normalize();
        d
    }

    #[test]
    fn merges_and_scales() {
        let a = recipe(&["1 cup milk", "2 eggs", "1 tbsp butter"], Some(6.0));
        let b = recipe(&["1 1/4 cups milk", "2 eggs", "salt"], Some(4.0));
        let mut items = vec![ShoppingItem::from_text("2 lbs apples")];
        items[0].id = "apples".into();
        let mut n = 0;
        let r = merge_recipes(
            &mut items,
            "list",
            &[
                RecipeAddition { recipe_id: "a", data: &a, servings: Some(12.0) },
                RecipeAddition { recipe_id: "b", data: &b, servings: None },
            ],
            || {
                n += 1;
                format!("id{n}")
            },
            "now",
        );
        let milk = items.iter().find(|i| i.food.as_deref() == Some("milk")).unwrap();
        assert_eq!((milk.quantity, milk.unit.as_deref()), (Some(3.25), Some("cup")));
        assert_eq!(milk.recipe_ids, vec!["a", "b"]);
        assert_eq!(milk.display, "3 1/4 cups milk");
        let eggs = items.iter().find(|i| i.food.as_deref() == Some("eggs")).unwrap();
        assert_eq!(eggs.quantity, Some(6.0));
        assert_eq!(eggs.aisle, "Dairy & Eggs");
        assert_eq!(r.created.len(), 4); // milk, eggs, butter, salt
        assert!(r.updated.is_empty());
        sort_items(&mut items);
        assert_eq!(items[0].id, "apples");
    }

    #[test]
    fn canned_goods_by_unit() {
        assert_eq!(ShoppingItem::from_text("1 can diced tomatoes").aisle, "Canned & Jarred");
        assert_eq!(ShoppingItem::from_text("10 ounce can crushed pineapple").aisle, "Canned & Jarred");
        assert_eq!(ShoppingItem::from_text("2 tomatoes").aisle, "Produce");
    }

    #[test]
    fn tidies_merged_amounts() {
        // Real-world totals that used to read badly.
        assert_eq!(tidy_amount(73.0, "tbsp"), (4.75, "cup".to_string()));
        assert_eq!(tidy_amount(15.28, "cup"), (15.5, "cup".to_string()));
        assert_eq!(tidy_amount(12.625, "tsp"), (4.5, "tbsp".to_string()));
        assert_eq!(tidy_amount(2.0, "tsp"), (2.0, "tsp".to_string()));
        assert_eq!(tidy_amount(1350.0, "g"), (1.4, "kg".to_string()));
        assert_eq!(tidy_amount(20.0, "oz"), (1.25, "lb".to_string()));
        assert_eq!(tidy_amount(3.0, "clove"), (3.0, "clove".to_string()));
        assert_eq!(tidy_amount(3.0, "handfuls of joy"), (3.0, "handfuls of joy".to_string()));
    }

    #[test]
    fn converts_compatible_units() {
        let a = recipe(&["1 lb flour"], None);
        let b = recipe(&["8 oz flour"], None);
        let mut items = vec![];
        let mut n = 0;
        let mut next = || {
            n += 1;
            n.to_string()
        };
        merge_recipes(&mut items, "l", &[RecipeAddition { recipe_id: "a", data: &a, servings: None }], &mut next, "t");
        let r = merge_recipes(&mut items, "l", &[RecipeAddition { recipe_id: "b", data: &b, servings: None }], &mut next, "t");
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].quantity, Some(1.5));
        assert_eq!(r.updated, vec!["1"]);
    }
}
