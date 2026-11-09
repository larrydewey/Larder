//! Recipe extraction from HTML pages: schema.org JSON-LD first, then
//! microdata (`itemscope itemtype=".../Recipe"`).

use scraper::{ElementRef, Html, Selector};
use serde_json::{json, Map, Value};

use crate::schema_org::{find_recipes, has_type, recipe_from_json};
use crate::util::{clean_block, clean_line, host_of, resolve_url, text_of};
use crate::{ImageSource, ImportError, ImportedRecipe};

fn sel(s: &str) -> Selector {
    Selector::parse(s).expect("valid selector")
}

/// Parses a JSON-LD script body, tolerating common breakage (HTML comments,
/// CDATA wrappers, raw newlines inside strings, trailing semicolons).
fn parse_ld_json(raw: &str) -> Option<Value> {
    let t = raw.trim();
    if let Ok(v) = serde_json::from_str(t) {
        return Some(v);
    }
    let mut s = t
        .trim_start_matches("<!--")
        .trim_end_matches("-->")
        .replace("/*<![CDATA[*/", "")
        .replace("/*]]>*/", "")
        .replace("<![CDATA[", "")
        .replace("]]>", "");
    s = s.trim().trim_end_matches(';').to_string();
    if let Ok(v) = serde_json::from_str(&s) {
        return Some(v);
    }
    // Raw control characters inside strings are invalid JSON but common.
    let s: String = s.chars().map(|c| if c.is_control() { ' ' } else { c }).collect();
    if let Ok(v) = serde_json::from_str(&s) {
        return Some(v);
    }
    // Trailing commas before } or ].
    let re = regex::Regex::new(r",\s*([}\]])").unwrap();
    serde_json::from_str(&re.replace_all(&s, "$1")).ok()
}

fn meta_content(doc: &Html, keys: &[&str]) -> Option<String> {
    let s = sel("meta");
    for k in keys {
        for m in doc.select(&s) {
            let v = m.value();
            let name = v.attr("property").or_else(|| v.attr("name")).or_else(|| v.attr("itemprop"));
            if name.is_some_and(|n| n.eq_ignore_ascii_case(k)) {
                if let Some(c) = v.attr("content").map(clean_line).filter(|c| !c.is_empty()) {
                    return Some(c);
                }
            }
        }
    }
    None
}

/// Score used to pick the best of several Recipe objects on a page.
fn completeness(v: &Value) -> usize {
    let mut s = 0;
    for (k, w) in [("name", 1), ("recipeIngredient", 4), ("ingredients", 3), ("recipeInstructions", 4), ("image", 1)] {
        if v.get(k).is_some_and(|x| !x.is_null()) {
            s += w;
        }
    }
    s
}

/// Extracts a recipe from an HTML page. `page_url` is used to resolve relative
/// URLs and as the recipe's `source_url`.
pub fn parse_html(html: &str, page_url: &str) -> Result<ImportedRecipe, ImportError> {
    let doc = Html::parse_document(html);
    let base = resolve_url(page_url, None);
    let base_ref = base.as_deref();

    // 1. JSON-LD
    let mut docs: Vec<Value> = vec![];
    for script in doc.select(&sel("script")) {
        let ty = script.value().attr("type").unwrap_or("").to_ascii_lowercase();
        if !ty.contains("ld+json") {
            continue;
        }
        let raw: String = script.text().collect();
        if let Some(v) = parse_ld_json(&raw) {
            docs.push(v);
        }
    }
    let mut best: Option<&Value> = None;
    let mut publisher: Option<String> = None;
    for d in &docs {
        for r in find_recipes(d) {
            if best.is_none_or(|b| completeness(r) > completeness(b)) {
                best = Some(r);
            }
        }
        publisher = publisher.or_else(|| site_name_from_ld(d));
    }

    let mut recipe = match best {
        Some(v) => {
            let mut v = v.clone();
            resolve_graph_refs(&mut v, &docs);
            recipe_from_json(&v, base_ref)
        }
        None => None,
    };

    // 2. Microdata
    if recipe.as_ref().is_none_or(|r| r.data.ingredients.is_empty() && r.data.instructions.is_empty()) {
        if let Some(v) = microdata_recipe(&doc, base_ref) {
            if let Some(r) = recipe_from_json(&v, base_ref) {
                if !r.data.ingredients.is_empty() || !r.data.instructions.is_empty() || recipe.is_none() {
                    recipe = Some(r);
                }
            }
        }
    }

    let mut recipe = recipe.ok_or(ImportError::NoRecipeFound)?;
    let d = &mut recipe.data;
    if d.name.trim().is_empty() {
        d.name = meta_content(&doc, &["og:title", "twitter:title"])
            .or_else(|| doc.select(&sel("title")).next().map(|t| clean_line(&t.text().collect::<String>())))
            .unwrap_or_default();
    }
    if d.description.trim().is_empty() {
        if let Some(desc) = meta_content(&doc, &["og:description", "description"]) {
            d.description = desc;
        }
    }
    if let Some(b) = &base {
        d.source_url = Some(b.clone());
    }
    if d.source_name.is_none() {
        d.source_name = publisher
            .or_else(|| meta_content(&doc, &["og:site_name", "application-name"]))
            .or_else(|| d.source_url.as_deref().and_then(host_of));
    }
    if recipe.image.is_none() {
        recipe.image = meta_content(&doc, &["og:image", "og:image:url", "twitter:image"])
            .and_then(|u| resolve_url(&u, base_ref))
            .map(ImageSource::Url);
    }
    recipe.created_at = None;
    recipe.data.normalize();
    if recipe.data.name.is_empty() {
        return Err(ImportError::NoRecipeFound);
    }
    Ok(recipe)
}

fn site_name_from_ld(v: &Value) -> Option<String> {
    let mut stack = vec![v];
    let mut seen = 0;
    while let Some(x) = stack.pop() {
        seen += 1;
        if seen > 500 {
            break;
        }
        match x {
            Value::Array(a) => stack.extend(a.iter()),
            Value::Object(o) => {
                if has_type(x, "WebSite") || has_type(x, "Organization") {
                    if let Some(n) = text_of(x, &["name"]).map(|s| clean_line(&s)).filter(|s| !s.is_empty()) {
                        return Some(n);
                    }
                }
                if has_type(x, "Recipe") {
                    if let Some(n) = o.get("publisher").and_then(|p| text_of(p, &["name"])) {
                        return Some(clean_line(&n));
                    }
                }
                stack.extend(o.values().filter(|v| v.is_object() || v.is_array()));
            }
            _ => {}
        }
    }
    None
}

/// Replaces `{"@id": ...}` references in a few fields (image, publisher, steps)
/// with the referenced node from the page's `@graph`s.
fn resolve_graph_refs(recipe: &mut Value, docs: &[Value]) {
    let mut nodes: Map<String, Value> = Map::new();
    for d in docs {
        let graph = d.get("@graph").and_then(Value::as_array).map(|a| a.as_slice()).unwrap_or(std::slice::from_ref(d));
        for n in graph {
            if let Some(id) = n.get("@id").and_then(Value::as_str) {
                nodes.insert(id.to_string(), n.clone());
            }
        }
    }
    if nodes.is_empty() {
        return;
    }
    let Some(obj) = recipe.as_object_mut() else { return };
    for key in ["image", "publisher", "author", "recipeInstructions", "nutrition", "aggregateRating"] {
        if let Some(v) = obj.get_mut(key) {
            resolve_ref(v, &nodes, 0);
        }
    }
}

fn resolve_ref(v: &mut Value, nodes: &Map<String, Value>, depth: usize) {
    if depth > 4 {
        return;
    }
    match v {
        Value::Object(o) if o.len() == 1 && o.contains_key("@id") => {
            if let Some(n) = o.get("@id").and_then(Value::as_str).and_then(|id| nodes.get(id)) {
                *v = n.clone();
            }
        }
        Value::Array(a) => a.iter_mut().for_each(|x| resolve_ref(x, nodes, depth + 1)),
        _ => {}
    }
}

// ---------------------------------------------------------------------------
// Microdata
// ---------------------------------------------------------------------------

fn is_recipe_scope(el: &ElementRef) -> bool {
    el.value().attr("itemscope").is_some()
        && el
            .value()
            .attr("itemtype")
            .is_some_and(|t| t.split_whitespace().any(|t| t.trim_end_matches('/').to_ascii_lowercase().ends_with("schema.org/recipe")))
}

/// Value of an itemprop element per the microdata spec (simplified).
fn prop_value(el: &ElementRef, base: Option<&str>) -> String {
    let v = el.value();
    let attr = match v.name() {
        "meta" => v.attr("content"),
        "img" | "audio" | "video" | "source" | "embed" | "iframe" => {
            v.attr("src").or_else(|| v.attr("data-src")).or_else(|| v.attr("data-lazy-src"))
        }
        "a" | "link" | "area" => v.attr("href"),
        "time" => v.attr("datetime"),
        "data" | "meter" => v.attr("value"),
        _ => v.attr("content"),
    };
    if let Some(a) = attr {
        if matches!(v.name(), "img" | "a" | "link" | "source") {
            return resolve_url(a, base).unwrap_or_else(|| a.to_string());
        }
        return a.trim().to_string();
    }
    clean_block(&el.inner_html())
}

/// The itemprops whose nearest itemscope ancestor is `scope`.
fn own_props<'a>(scope: ElementRef<'a>) -> Vec<(String, ElementRef<'a>)> {
    let mut out = vec![];
    let id = scope.id();
    for el in scope.descendants().filter_map(ElementRef::wrap) {
        if el.id() == id {
            continue;
        }
        let Some(props) = el.value().attr("itemprop") else { continue };
        let owner = el.ancestors().filter_map(ElementRef::wrap).find(|a| a.value().attr("itemscope").is_some());
        if owner.is_some_and(|o| o.id() == id) {
            for p in props.split_whitespace() {
                out.push((p.to_string(), el));
            }
        }
    }
    out
}

fn microdata_recipe(doc: &Html, base: Option<&str>) -> Option<Value> {
    let scope = doc.select(&sel("[itemscope][itemtype]")).find(is_recipe_scope)?;
    let mut obj = Map::new();
    obj.insert("@type".into(), json!("Recipe"));
    let mut ingredients: Vec<Value> = vec![];
    let mut steps: Vec<Value> = vec![];
    let mut multi: Map<String, Value> = Map::new();
    for (prop, el) in own_props(scope) {
        match prop.as_str() {
            "recipeIngredient" | "ingredients" => ingredients.push(json!(clean_line(&prop_value(&el, base)))),
            "recipeInstructions" | "instructions" => {
                if el.value().attr("itemscope").is_some() {
                    // HowToStep / HowToSection item
                    let mut step = Map::new();
                    for (p, e) in own_props(el) {
                        step.entry(p).or_insert_with(|| json!(prop_value(&e, base)));
                    }
                    if step.is_empty() {
                        step.insert("text".into(), json!(prop_value(&el, base)));
                    }
                    steps.push(Value::Object(step));
                } else {
                    let items: Vec<Value> =
                        el.select(&sel("li")).map(|li| json!(clean_line(&li.text().collect::<Vec<_>>().join(" ")))).collect();
                    if items.len() > 1 {
                        steps.extend(items);
                    } else {
                        steps.push(json!(prop_value(&el, base)));
                    }
                }
            }
            "aggregateRating" => {
                let mut r = Map::new();
                for (p, e) in own_props(el) {
                    r.insert(p, json!(prop_value(&e, base)));
                }
                obj.insert("aggregateRating".into(), Value::Object(r));
            }
            "nutrition" => {
                let mut n = Map::new();
                for (p, e) in own_props(el) {
                    n.insert(p, json!(clean_line(&prop_value(&e, base))));
                }
                obj.insert("nutrition".into(), Value::Object(n));
            }
            "image" | "photo" | "thumbnailUrl" => {
                let url = if el.value().attr("itemscope").is_some() {
                    own_props(el).into_iter().find(|(p, _)| p == "url" || p == "contentUrl").map(|(_, e)| prop_value(&e, base))
                } else {
                    Some(prop_value(&el, base))
                };
                if let Some(u) = url {
                    obj.entry("image").or_insert(json!(u));
                }
            }
            "publisher" | "author" => {
                if el.value().attr("itemscope").is_some() {
                    if let Some((_, e)) = own_props(el).into_iter().find(|(p, _)| p == "name") {
                        obj.entry(prop).or_insert(json!({"name": clean_line(&prop_value(&e, base))}));
                    }
                }
            }
            "recipeCategory" | "recipeCuisine" | "keywords" => {
                let arr = multi.entry(prop.clone()).or_insert_with(|| json!([]));
                if let Some(a) = arr.as_array_mut() {
                    a.push(json!(clean_line(&prop_value(&el, base))));
                }
            }
            "name" | "description" | "recipeYield" | "prepTime" | "cookTime" | "totalTime" | "url" | "dateCreated"
                if el.value().attr("itemscope").is_none() =>
            {
                obj.entry(prop).or_insert(json!(prop_value(&el, base)));
            }
            _ => {}
        }
    }
    if !ingredients.is_empty() {
        obj.insert("recipeIngredient".into(), Value::Array(ingredients));
    }
    if !steps.is_empty() {
        obj.insert("recipeInstructions".into(), Value::Array(steps));
    }
    for (k, v) in multi {
        obj.insert(k, v);
    }
    Some(Value::Object(obj))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn json_ld_graph_with_refs() {
        let html = r#"<html><head>
        <meta property="og:site_name" content="OG Site">
        <meta property="og:image" content="https://cdn.example/og.jpg">
        <script type="application/ld+json">
        {"@context":"https://schema.org","@graph":[
          {"@type":"Organization","@id":"https://bake.example/#org","name":"Bake Co"},
          {"@type":"ImageObject","@id":"https://bake.example/#img","url":"/uploads/bread.jpg"},
          {"@type":"Article","headline":"Bread"},
          {"@type":"Recipe","name":"Easy Bread","publisher":{"@id":"https://bake.example/#org"},
           "image":{"@id":"https://bake.example/#img"},
           "recipeYield":["1","1 loaf"],"totalTime":"PT3H",
           "recipeIngredient":["500 g flour","10 g salt","7 g yeast","350 ml water"],
           "recipeInstructions":[{"@type":"HowToStep","text":"Mix
           everything."},{"@type":"HowToStep","text":"Bake at 230&deg;C."}]}
        ]}
        </script></head><body></body></html>"#;
        let r = parse_html(html, "https://bake.example/bread?utm=1").unwrap();
        assert_eq!(r.data.name, "Easy Bread");
        assert_eq!(r.data.source_name.as_deref(), Some("Bake Co"));
        assert_eq!(r.data.source_url.as_deref(), Some("https://bake.example/bread?utm=1"));
        assert_eq!((r.data.yield_quantity, r.data.yield_unit.as_str()), (Some(1.0), "loaf"));
        assert_eq!(r.data.ingredients.len(), 4);
        assert_eq!(r.data.instructions[0].text, "Mix everything.");
        assert_eq!(r.data.instructions[1].text, "Bake at 230°C.");
        assert!(matches!(r.image, Some(ImageSource::Url(ref u)) if u == "https://bake.example/uploads/bread.jpg"));
    }

    #[test]
    fn microdata_fallback() {
        let html = r#"<html><head><title>Grandma's Pancakes | Food Blog</title>
        <meta property="og:site_name" content="Food Blog"></head><body>
        <div itemscope itemtype="http://schema.org/Recipe">
          <h1 itemprop="name">Grandma's Pancakes</h1>
          <img itemprop="image" src="/img/pancakes.jpg">
          <span itemprop="author" itemscope itemtype="http://schema.org/Person"><span itemprop="name">Grandma</span></span>
          <p itemprop="description">Fluffy &amp; light.</p>
          <meta itemprop="prepTime" content="PT10M"><time itemprop="cookTime" datetime="PT15M">15 min</time>
          <span itemprop="recipeYield">Serves 4</span>
          <span itemprop="recipeCategory">Breakfast</span>
          <ul><li itemprop="recipeIngredient">1 1/2 cups flour</li><li itemprop="ingredients">2 eggs</li></ul>
          <ol itemprop="recipeInstructions"><li>Whisk.</li><li>Fry.</li></ol>
          <div itemprop="aggregateRating" itemscope itemtype="http://schema.org/AggregateRating">
            <span itemprop="ratingValue">4.2</span></div>
        </div></body></html>"#;
        let r = parse_html(html, "https://blog.example/pancakes").unwrap();
        let d = &r.data;
        assert_eq!(d.name, "Grandma's Pancakes");
        assert_eq!(d.description, "Fluffy & light.");
        assert_eq!((d.prep_time_minutes, d.cook_time_minutes), (Some(10), Some(15)));
        assert_eq!(d.yield_quantity, Some(4.0));
        assert_eq!(d.categories, vec!["Breakfast"]);
        assert_eq!(d.ingredients.len(), 2);
        assert_eq!(d.ingredients[0].quantity, Some(1.5));
        assert_eq!(d.instructions.iter().map(|i| i.text.as_str()).collect::<Vec<_>>(), vec!["Whisk.", "Fry."]);
        assert_eq!(d.rating, Some(4));
        assert_eq!(d.source_name.as_deref(), Some("Food Blog"));
        assert!(matches!(r.image, Some(ImageSource::Url(ref u)) if u == "https://blog.example/img/pancakes.jpg"));
    }

    #[test]
    fn host_fallback_and_no_recipe() {
        let html = r#"<script type="application/ld+json">[{"@type":["Recipe"],"name":"Tea","recipeIngredient":"1 tea bag",
            "recipeInstructions":"Steep.",}]</script>"#;
        let r = parse_html(html, "https://www.tea.example/x").unwrap();
        assert_eq!(r.data.source_name.as_deref(), Some("tea.example"));
        assert!(matches!(parse_html("<html><body>Hello</body></html>", "https://x.example"), Err(ImportError::NoRecipeFound)));
    }
}
