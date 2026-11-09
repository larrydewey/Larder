//! Unit catalogue, alias matching and metric/imperial conversion.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Dimension {
    Volume,
    Mass,
    Count,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum UnitSystem {
    #[default]
    Original,
    Metric,
    Imperial,
}

impl UnitSystem {
    pub fn parse(s: &str) -> Self {
        match s.to_ascii_lowercase().as_str() {
            "metric" => Self::Metric,
            "imperial" | "us" => Self::Imperial,
            _ => Self::Original,
        }
    }
}

pub struct UnitDef {
    pub key: &'static str,
    pub singular: &'static str,
    pub plural: &'static str,
    pub dim: Dimension,
    /// Millilitres for volume, grams for mass, 1 for counts.
    pub factor: f64,
    pub metric: bool,
    /// Lower-case aliases (matched case-insensitively) …
    pub aliases: &'static [&'static str],
    /// … and case-sensitive aliases (e.g. "T" = tablespoon, "t" = teaspoon).
    pub cs_aliases: &'static [&'static str],
}

pub static UNITS: &[UnitDef] = &[
    UnitDef {
        key: "tsp",
        singular: "tsp",
        plural: "tsp",
        dim: Dimension::Volume,
        factor: 4.92892,
        metric: false,
        aliases: &["tsp", "tsps", "teaspoon", "teaspoons", "tea spoon", "tea spoons", "tspn", "c. à c."],
        cs_aliases: &["t"],
    },
    UnitDef {
        key: "tbsp",
        singular: "tbsp",
        plural: "tbsp",
        dim: Dimension::Volume,
        factor: 14.7868,
        metric: false,
        aliases: &[
            "tbsp",
            "tbsps",
            "tbs",
            "tbl",
            "tbls",
            "tablespoon",
            "tablespoons",
            "table spoon",
            "table spoons",
            "tblsp",
            "c. à s.",
            "el",
        ],
        cs_aliases: &["T", "Tb"],
    },
    UnitDef {
        key: "fl oz",
        singular: "fl oz",
        plural: "fl oz",
        dim: Dimension::Volume,
        factor: 29.5735,
        metric: false,
        aliases: &["fl oz", "fl. oz", "fl.oz", "floz", "fluid ounce", "fluid ounces", "fl ounce", "fl ounces"],
        cs_aliases: &[],
    },
    UnitDef {
        key: "cup",
        singular: "cup",
        plural: "cups",
        dim: Dimension::Volume,
        factor: 236.588,
        metric: false,
        aliases: &["cup", "cups", "c", "cp", "tasse", "tassen"],
        cs_aliases: &["C"],
    },
    UnitDef {
        key: "pint",
        singular: "pint",
        plural: "pints",
        dim: Dimension::Volume,
        factor: 473.176,
        metric: false,
        aliases: &["pint", "pints", "pt", "pts"],
        cs_aliases: &[],
    },
    UnitDef {
        key: "quart",
        singular: "quart",
        plural: "quarts",
        dim: Dimension::Volume,
        factor: 946.353,
        metric: false,
        aliases: &["quart", "quarts", "qt", "qts"],
        cs_aliases: &[],
    },
    UnitDef {
        key: "gallon",
        singular: "gallon",
        plural: "gallons",
        dim: Dimension::Volume,
        factor: 3785.41,
        metric: false,
        aliases: &["gallon", "gallons", "gal", "gals"],
        cs_aliases: &[],
    },
    UnitDef {
        key: "ml",
        singular: "ml",
        plural: "ml",
        dim: Dimension::Volume,
        factor: 1.0,
        metric: true,
        aliases: &["ml", "mls", "milliliter", "milliliters", "millilitre", "millilitres", "cc", "cc."],
        cs_aliases: &["mL"],
    },
    UnitDef {
        key: "cl",
        singular: "cl",
        plural: "cl",
        dim: Dimension::Volume,
        factor: 10.0,
        metric: true,
        aliases: &["cl", "centiliter", "centiliters", "centilitre", "centilitres"],
        cs_aliases: &[],
    },
    UnitDef {
        key: "dl",
        singular: "dl",
        plural: "dl",
        dim: Dimension::Volume,
        factor: 100.0,
        metric: true,
        aliases: &["dl", "deciliter", "deciliters", "decilitre", "decilitres"],
        cs_aliases: &[],
    },
    UnitDef {
        key: "l",
        singular: "l",
        plural: "l",
        dim: Dimension::Volume,
        factor: 1000.0,
        metric: true,
        aliases: &["l", "liter", "liters", "litre", "litres", "ltr", "ltrs", "lt"],
        cs_aliases: &["L"],
    },
    UnitDef {
        key: "oz",
        singular: "oz",
        plural: "oz",
        dim: Dimension::Mass,
        factor: 28.3495,
        metric: false,
        aliases: &["oz", "ozs", "ounce", "ounces", "oz."],
        cs_aliases: &[],
    },
    UnitDef {
        key: "lb",
        singular: "lb",
        plural: "lb",
        dim: Dimension::Mass,
        factor: 453.592,
        metric: false,
        aliases: &["lb", "lbs", "pound", "pounds", "lb.", "lbs."],
        cs_aliases: &[],
    },
    UnitDef {
        key: "mg",
        singular: "mg",
        plural: "mg",
        dim: Dimension::Mass,
        factor: 0.001,
        metric: true,
        aliases: &["mg", "milligram", "milligrams", "milligramme", "milligrammes"],
        cs_aliases: &[],
    },
    UnitDef {
        key: "g",
        singular: "g",
        plural: "g",
        dim: Dimension::Mass,
        factor: 1.0,
        metric: true,
        aliases: &["g", "gr", "grs", "gram", "grams", "gramme", "grammes", "gm", "gms"],
        cs_aliases: &[],
    },
    UnitDef {
        key: "kg",
        singular: "kg",
        plural: "kg",
        dim: Dimension::Mass,
        factor: 1000.0,
        metric: true,
        aliases: &["kg", "kgs", "kilogram", "kilograms", "kilo", "kilos", "kilogramme", "kilogrammes"],
        cs_aliases: &[],
    },
    UnitDef {
        key: "pinch",
        singular: "pinch",
        plural: "pinches",
        dim: Dimension::Count,
        factor: 1.0,
        metric: false,
        aliases: &["pinch", "pinches", "pn"],
        cs_aliases: &[],
    },
    UnitDef {
        key: "dash",
        singular: "dash",
        plural: "dashes",
        dim: Dimension::Count,
        factor: 1.0,
        metric: false,
        aliases: &["dash", "dashes"],
        cs_aliases: &[],
    },
    UnitDef {
        key: "clove",
        singular: "clove",
        plural: "cloves",
        dim: Dimension::Count,
        factor: 1.0,
        metric: false,
        aliases: &["clove", "cloves"],
        cs_aliases: &[],
    },
    UnitDef {
        key: "can",
        singular: "can",
        plural: "cans",
        dim: Dimension::Count,
        factor: 1.0,
        metric: false,
        aliases: &["can", "cans", "tin", "tins"],
        cs_aliases: &[],
    },
    UnitDef {
        key: "jar",
        singular: "jar",
        plural: "jars",
        dim: Dimension::Count,
        factor: 1.0,
        metric: false,
        aliases: &["jar", "jars"],
        cs_aliases: &[],
    },
    UnitDef {
        key: "package",
        singular: "package",
        plural: "packages",
        dim: Dimension::Count,
        factor: 1.0,
        metric: false,
        aliases: &[
            "package",
            "packages",
            "pkg",
            "pkgs",
            "packet",
            "packets",
            "pack",
            "packs",
            "box",
            "boxes",
            "bag",
            "bags",
            "envelope",
            "envelopes",
        ],
        cs_aliases: &[],
    },
    UnitDef {
        key: "slice",
        singular: "slice",
        plural: "slices",
        dim: Dimension::Count,
        factor: 1.0,
        metric: false,
        aliases: &["slice", "slices"],
        cs_aliases: &[],
    },
    UnitDef {
        key: "piece",
        singular: "piece",
        plural: "pieces",
        dim: Dimension::Count,
        factor: 1.0,
        metric: false,
        aliases: &["piece", "pieces", "pc", "pcs"],
        cs_aliases: &[],
    },
    UnitDef {
        key: "stick",
        singular: "stick",
        plural: "sticks",
        dim: Dimension::Count,
        factor: 1.0,
        metric: false,
        aliases: &["stick", "sticks"],
        cs_aliases: &[],
    },
    UnitDef {
        key: "bunch",
        singular: "bunch",
        plural: "bunches",
        dim: Dimension::Count,
        factor: 1.0,
        metric: false,
        aliases: &["bunch", "bunches", "bn"],
        cs_aliases: &[],
    },
    UnitDef {
        key: "sprig",
        singular: "sprig",
        plural: "sprigs",
        dim: Dimension::Count,
        factor: 1.0,
        metric: false,
        aliases: &["sprig", "sprigs"],
        cs_aliases: &[],
    },
    UnitDef {
        key: "handful",
        singular: "handful",
        plural: "handfuls",
        dim: Dimension::Count,
        factor: 1.0,
        metric: false,
        aliases: &["handful", "handfuls"],
        cs_aliases: &[],
    },
    UnitDef {
        key: "head",
        singular: "head",
        plural: "heads",
        dim: Dimension::Count,
        factor: 1.0,
        metric: false,
        aliases: &["head", "heads"],
        cs_aliases: &[],
    },
    UnitDef {
        key: "drop",
        singular: "drop",
        plural: "drops",
        dim: Dimension::Count,
        factor: 1.0,
        metric: false,
        aliases: &["drop", "drops"],
        cs_aliases: &[],
    },
];

/// Looks up a unit definition by canonical key.
pub fn unit_def(key: &str) -> Option<&'static UnitDef> {
    UNITS.iter().find(|u| u.key == key)
}

/// Maps any alias (e.g. "Tablespoons", "T", "lbs.") to its canonical key.
pub fn canonical_unit(s: &str) -> Option<&'static str> {
    let trimmed = s.trim();
    if let Some(u) = UNITS.iter().find(|u| u.cs_aliases.contains(&trimmed)) {
        return Some(u.key);
    }
    let lower = trimmed.to_lowercase();
    let lower = lower.trim_end_matches('.');
    UNITS.iter().find(|u| u.key == lower || u.aliases.iter().any(|a| a.trim_end_matches('.') == lower)).map(|u| u.key)
}

/// Every alias with its canonical key, longest first (for prefix matching in the parser).
pub fn alias_table() -> Vec<(&'static str, &'static str, bool)> {
    let mut v: Vec<(&'static str, &'static str, bool)> = vec![];
    for u in UNITS {
        v.push((u.key, u.key, false));
        for a in u.aliases {
            v.push((a, u.key, false));
        }
        for a in u.cs_aliases {
            v.push((a, u.key, true));
        }
    }
    v.sort_by_key(|b| std::cmp::Reverse(b.0.len()));
    v
}

/// Quantity in base units (ml or g) if the unit is a measurable one.
pub fn to_base(qty: f64, unit: &str) -> Option<(f64, Dimension)> {
    let def = unit_def(unit)?;
    match def.dim {
        Dimension::Count => None,
        d => Some((qty * def.factor, d)),
    }
}

/// Converts a quantity to the requested unit system, picking a sensible unit.
/// Returns `None` when no conversion applies (counts, free-text units, or
/// already in the target system).
pub fn convert(qty: f64, unit: &str, system: UnitSystem) -> Option<(f64, &'static str)> {
    let def = unit_def(unit)?;
    if def.dim == Dimension::Count {
        return None;
    }
    match system {
        UnitSystem::Original => None,
        UnitSystem::Metric if def.metric => None,
        UnitSystem::Imperial if !def.metric => None,
        UnitSystem::Metric => {
            let base = qty * def.factor;
            Some(match def.dim {
                Dimension::Volume if base >= 1000.0 => (base / 1000.0, "l"),
                Dimension::Volume => (base, "ml"),
                _ if base >= 1000.0 => (base / 1000.0, "kg"),
                _ => (base, "g"),
            })
        }
        UnitSystem::Imperial => {
            let base = qty * def.factor;
            Some(match def.dim {
                Dimension::Volume => {
                    let cups = base / 236.588;
                    if cups >= 16.0 {
                        (base / 3785.41, "gallon")
                    } else if cups >= 4.0 {
                        (base / 946.353, "quart")
                    } else if cups >= 0.25 {
                        (cups, "cup")
                    } else if base >= 14.7868 - 0.5 {
                        (base / 14.7868, "tbsp")
                    } else {
                        (base / 4.92892, "tsp")
                    }
                }
                _ => {
                    let oz = base / 28.3495;
                    if oz >= 16.0 {
                        (base / 453.592, "lb")
                    } else {
                        (oz, "oz")
                    }
                }
            })
        }
    }
}

/// Rounds converted metric values to kitchen-friendly precision.
pub fn round_metric(qty: f64, unit: &str) -> f64 {
    match unit {
        "ml" | "g" if qty >= 100.0 => (qty / 5.0).round() * 5.0,
        "ml" | "g" if qty >= 10.0 => qty.round(),
        "ml" | "g" => (qty * 10.0).round() / 10.0,
        "l" | "kg" => (qty * 100.0).round() / 100.0,
        _ => qty,
    }
}

/// Converts `qty` of `from` unit into `to` unit if both share a dimension.
pub fn convert_between(qty: f64, from: &str, to: &str) -> Option<f64> {
    if from == to {
        return Some(qty);
    }
    let (a, b) = (unit_def(from)?, unit_def(to)?);
    if a.dim != b.dim || a.dim == Dimension::Count {
        return None;
    }
    Some(qty * a.factor / b.factor)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn aliases() {
        assert_eq!(canonical_unit("Tablespoons"), Some("tbsp"));
        assert_eq!(canonical_unit("T"), Some("tbsp"));
        assert_eq!(canonical_unit("t"), Some("tsp"));
        assert_eq!(canonical_unit("lbs."), Some("lb"));
        assert_eq!(canonical_unit("L"), Some("l"));
        assert_eq!(canonical_unit("cc"), Some("ml"));
        assert_eq!(canonical_unit("whatever"), None);
    }

    #[test]
    fn conversions() {
        let (q, u) = convert(2.0, "cup", UnitSystem::Metric).unwrap();
        assert_eq!(u, "ml");
        assert!((q - 473.176).abs() < 0.01);
        let (q, u) = convert(500.0, "g", UnitSystem::Imperial).unwrap();
        assert_eq!(u, "lb");
        assert!((q - 1.102).abs() < 0.01);
        let (_, u) = convert(10.0, "ml", UnitSystem::Imperial).unwrap();
        assert_eq!(u, "tsp");
        assert!(convert(1.0, "cup", UnitSystem::Imperial).is_none());
        assert!(convert(1.0, "clove", UnitSystem::Metric).is_none());
        assert!((convert_between(1.0, "lb", "oz").unwrap() - 16.0).abs() < 0.01);
        assert!(convert_between(1.0, "lb", "cup").is_none());
    }
}
