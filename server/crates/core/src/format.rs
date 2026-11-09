//! Formatting quantities as kitchen fractions and rendering ingredients.

use crate::model::Ingredient;
use crate::units::{self, UnitSystem};

const FRACTIONS: &[(f64, &str)] = &[
    (0.125, "1/8"),
    (0.25, "1/4"),
    (1.0 / 3.0, "1/3"),
    (0.375, "3/8"),
    (0.5, "1/2"),
    (0.625, "5/8"),
    (2.0 / 3.0, "2/3"),
    (0.75, "3/4"),
    (0.875, "7/8"),
];

/// Formats a number as a mixed fraction where that reads naturally
/// ("1 1/2", "3/4"), otherwise as a trimmed decimal.
pub fn format_quantity(q: f64) -> String {
    if !q.is_finite() {
        return String::new();
    }
    if q >= 20.0 {
        return trim_decimal((q * 10.0).round() / 10.0);
    }
    let whole = q.trunc();
    let frac = q - whole;
    if frac < 0.03 {
        return trim_decimal(whole);
    }
    if frac > 0.97 {
        return trim_decimal(whole + 1.0);
    }
    for (v, s) in FRACTIONS {
        if (frac - v).abs() < 0.02 {
            return if whole == 0.0 { (*s).to_string() } else { format!("{} {}", whole as i64, s) };
        }
    }
    trim_decimal((q * 100.0).round() / 100.0)
}

/// Like [`format_quantity`] but with Unicode vulgar fractions ("1½").
pub fn format_quantity_unicode(q: f64) -> String {
    let s = format_quantity(q);
    let map =
        [("1/8", "⅛"), ("1/4", "¼"), ("1/3", "⅓"), ("3/8", "⅜"), ("1/2", "½"), ("5/8", "⅝"), ("2/3", "⅔"), ("3/4", "¾"), ("7/8", "⅞")];
    for (a, b) in map {
        if let Some(prefix) = s.strip_suffix(a) {
            return format!("{}{}", prefix.trim_end(), b);
        }
    }
    s
}

fn trim_decimal(v: f64) -> String {
    let s = format!("{:.2}", v);
    let s = s.trim_end_matches('0').trim_end_matches('.');
    if s == "-0" {
        "0".into()
    } else {
        s.to_string()
    }
}

/// Unit label for display, pluralised where appropriate.
pub fn unit_label(unit: &str, qty: Option<f64>) -> String {
    match units::unit_def(unit) {
        Some(def) => {
            if qty.is_some_and(|q| q > 1.0 + 1e-9) {
                def.plural.to_string()
            } else {
                def.singular.to_string()
            }
        }
        None => unit.to_string(),
    }
}

/// Applies scale + conversion and returns (quantity, quantity_max, unit).
pub fn scaled_parts(ing: &Ingredient, scale: f64, system: UnitSystem) -> (Option<f64>, Option<f64>, Option<String>) {
    let q = ing.quantity.map(|q| q * scale);
    let qmax = ing.quantity_max.map(|q| q * scale);
    if let (Some(qv), Some(unit)) = (q, ing.unit.as_deref()) {
        if let Some((cq, cu)) = units::convert(qv, unit, system) {
            let ratio = if qv != 0.0 { cq / qv } else { 1.0 };
            let round = |x: f64| if system == UnitSystem::Metric { units::round_metric(x, cu) } else { x };
            return (Some(round(cq)), qmax.map(|m| round(m * ratio)), Some(cu.to_string()));
        }
    }
    (q, qmax, ing.unit.clone())
}

/// Renders an ingredient as text, e.g. "1 1/2 cups flour, sifted".
/// When no structured data is available the original line is returned.
pub fn ingredient_display(ing: &Ingredient, scale: f64, system: UnitSystem) -> String {
    if ing.food.is_none() && ing.quantity.is_none() {
        return ing.original.clone();
    }
    let (q, qmax, unit) = scaled_parts(ing, scale, system);
    let mut parts: Vec<String> = vec![];
    if let Some(q) = q {
        match qmax {
            Some(m) if m > q => parts.push(format!("{}–{}", format_quantity(q), format_quantity(m))),
            _ => parts.push(format_quantity(q)),
        }
    }
    if let Some(u) = unit {
        parts.push(unit_label(&u, qmax.or(q)));
    }
    if let Some(f) = &ing.food {
        parts.push(f.clone());
    }
    let mut s = parts.join(" ");
    if let Some(n) = ing.note.as_deref().filter(|n| !n.is_empty()) {
        if s.is_empty() {
            s = n.to_string();
        } else {
            s.push_str(", ");
            s.push_str(n);
        }
    }
    s
}

/// Formats minutes as "1 hr 15 min".
pub fn format_minutes(m: i64) -> String {
    let (h, mm) = (m / 60, m % 60);
    match (h, mm) {
        (0, m) => format!("{m} min"),
        (h, 0) => format!("{h} hr"),
        (h, m) => format!("{h} hr {m} min"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fractions() {
        assert_eq!(format_quantity(1.5), "1 1/2");
        assert_eq!(format_quantity(0.333), "1/3");
        assert_eq!(format_quantity(2.0), "2");
        assert_eq!(format_quantity(0.1), "0.1");
        assert_eq!(format_quantity(1.999), "2");
        assert_eq!(format_quantity(250.0), "250");
        assert_eq!(format_quantity_unicode(1.5), "1½");
        assert_eq!(format_quantity_unicode(0.25), "¼");
    }

    #[test]
    fn display() {
        let ing = Ingredient::parse("1 1/2 cups flour, sifted");
        assert_eq!(ing.display(1.0, UnitSystem::Original), "1 1/2 cups flour, sifted");
        assert_eq!(ing.display(2.0, UnitSystem::Original), "3 cups flour, sifted");
        assert_eq!(ing.display(1.0, UnitSystem::Metric), "355 ml flour, sifted");
        let ing = Ingredient::parse("2-3 cloves garlic");
        assert_eq!(ing.display(1.0, UnitSystem::Original), "2–3 cloves garlic");
        assert_eq!(format_minutes(75), "1 hr 15 min");
    }
}
