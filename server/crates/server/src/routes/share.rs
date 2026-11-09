//! Public share links. A household member can publish a recipe at
//! `/s/<token>`, a read-only page anyone with the link can open without an
//! account. The token is random (128 bits) and is the only key: the page
//! shows that one recipe and its photo, nothing else from the household.

use axum::extract::{Path, State};
use axum::http::{header, HeaderValue, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::Json;
use larder_core::{Ingredient, Instruction};
use serde_json::{json, Value};

use crate::auth::{random_token, AuthUser};
use crate::db::{now, RowExt};
use crate::error::{AppError, AppResult};
use crate::recipes::{self, recipe_from_row, Recipe, RECIPE_COLS};
use crate::state::AppState;

/// Hex characters in a share token (16 random bytes).
const TOKEN_LEN: usize = 32;

fn share_url(state: &AppState, token: &str) -> String {
    format!("{}/s/{token}", state.config.public_url.trim_end_matches('/'))
}

fn share_json(state: &AppState, token: &str, created_at: &str) -> Value {
    json!({ "token": token, "url": share_url(state, token), "created_at": created_at })
}

async fn find_share(state: &AppState, recipe_id: &str) -> AppResult<Option<(String, String)>> {
    Ok(sqlx::query("SELECT token, created_at FROM recipe_shares WHERE recipe_id = $1")
        .bind(recipe_id)
        .fetch_optional(&state.db.pool)
        .await?
        .map(|r| (r.s("token"), r.s("created_at"))))
}

/// `GET /api/recipes/{id}/share`: the recipe's public link, or 404 if it has none.
pub async fn get_share(State(state): State<AppState>, user: AuthUser, Path(id): Path<String>) -> AppResult<Json<Value>> {
    let r = recipes::load(&state, &user.household_id, &user.id, &id).await?;
    let (token, created_at) = find_share(&state, &r.id).await?.ok_or_else(|| AppError::not_found("recipe is not shared"))?;
    Ok(Json(share_json(&state, &token, &created_at)))
}

/// `POST /api/recipes/{id}/share`: creates the public link, or returns the existing one.
pub async fn create_share(State(state): State<AppState>, user: AuthUser, Path(id): Path<String>) -> AppResult<Response> {
    if !state.config.public_sharing {
        return Err(AppError::forbidden("public sharing is turned off on this server"));
    }
    let r = recipes::load(&state, &user.household_id, &user.id, &id).await?;
    if let Some((token, created_at)) = find_share(&state, &r.id).await? {
        return Ok((StatusCode::OK, Json(share_json(&state, &token, &created_at))).into_response());
    }
    let token = random_token(TOKEN_LEN / 2);
    let created_at = now();
    // Two members sharing at once: the second insert does nothing and both get the first token.
    sqlx::query(
        "INSERT INTO recipe_shares (recipe_id, household_id, token, created_by, created_at) VALUES ($1, $2, $3, $4, $5) \
         ON CONFLICT (recipe_id) DO NOTHING",
    )
    .bind(&r.id)
    .bind(&user.household_id)
    .bind(&token)
    .bind(&user.id)
    .bind(&created_at)
    .execute(&state.db.pool)
    .await?;
    let (token, created_at) = find_share(&state, &r.id).await?.ok_or_else(|| AppError::internal("share link was not saved"))?;
    Ok((StatusCode::CREATED, Json(share_json(&state, &token, &created_at))).into_response())
}

/// `DELETE /api/recipes/{id}/share`: turns the public link off. Sharing again makes a new link.
pub async fn delete_share(State(state): State<AppState>, user: AuthUser, Path(id): Path<String>) -> AppResult<StatusCode> {
    let r = recipes::load(&state, &user.household_id, &user.id, &id).await?;
    sqlx::query("DELETE FROM recipe_shares WHERE recipe_id = $1").bind(&r.id).execute(&state.db.pool).await?;
    Ok(StatusCode::NO_CONTENT)
}

/// `GET /s/{token}`: the public recipe page.
pub async fn public_page(State(state): State<AppState>, Path(token): Path<String>) -> Response {
    match load_shared(&state, &token).await {
        Ok(Some(r)) => html_response(StatusCode::OK, render_recipe(&state, &r)),
        Ok(None) => html_response(StatusCode::NOT_FOUND, render_missing()),
        Err(e) => {
            tracing::error!("loading shared recipe: {}", e.message);
            html_response(
                StatusCode::INTERNAL_SERVER_ERROR,
                render_page("Larder", "", "<p>Something went wrong. Please try again later.</p>"),
            )
        }
    }
}

async fn load_shared(state: &AppState, token: &str) -> AppResult<Option<Recipe>> {
    if !state.config.public_sharing || token.len() != TOKEN_LEN || !token.bytes().all(|b| b.is_ascii_hexdigit()) {
        return Ok(None);
    }
    // $1 is the favourites user in RECIPE_COLS' companion expression; no user here, so is_fav is 0.
    let sql = format!("SELECT {RECIPE_COLS}, 0 AS is_fav FROM recipes r JOIN recipe_shares s ON s.recipe_id = r.id WHERE s.token = $1");
    let row = sqlx::query(&sql).bind(token.to_ascii_lowercase()).fetch_optional(&state.db.pool).await?;
    Ok(row.map(|r| recipe_from_row(&r)))
}

fn html_response(status: StatusCode, body: String) -> Response {
    let mut res = (status, body).into_response();
    let h = res.headers_mut();
    h.insert(header::CONTENT_TYPE, HeaderValue::from_static("text/html; charset=utf-8"));
    // A turned-off link must stop working at once, so nothing is cached.
    h.insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
    h.insert("x-robots-tag", HeaderValue::from_static("noindex, nofollow"));
    // The token is in the URL: never send it to other sites as a referrer.
    h.insert(header::REFERRER_POLICY, HeaderValue::from_static("no-referrer"));
    h.insert(header::X_CONTENT_TYPE_OPTIONS, HeaderValue::from_static("nosniff"));
    h.insert(
        header::CONTENT_SECURITY_POLICY,
        HeaderValue::from_static(
            "default-src 'none'; img-src 'self'; style-src 'unsafe-inline'; base-uri 'none'; form-action 'none'; frame-ancestors 'none'",
        ),
    );
    res
}

/// Escapes text for HTML element content and quoted attribute values.
fn esc(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&#39;"),
            _ => out.push(c),
        }
    }
    out
}

/// Escaped text with line breaks kept.
fn esc_lines(s: &str) -> String {
    esc(s.trim()).replace('\n', "<br>")
}

fn minutes(m: i64) -> String {
    match (m / 60, m % 60) {
        (0, m) => format!("{m} min"),
        (h, 0) => format!("{h} h"),
        (h, m) => format!("{h} h {m} min"),
    }
}

fn number(q: f64) -> String {
    if (q - q.round()).abs() < 1e-9 {
        format!("{}", q.round() as i64)
    } else {
        format!("{}", (q * 100.0).round() / 100.0)
    }
}

/// An http(s) source link for attribution; anything else is shown as text only.
fn safe_source_url(u: &str) -> Option<String> {
    let parsed = url::Url::parse(u.trim()).ok()?;
    matches!(parsed.scheme(), "http" | "https").then(|| parsed.to_string())
}

fn source_label(name: Option<&str>, url: Option<&str>) -> Option<String> {
    if let Some(n) = name.map(str::trim).filter(|n| !n.is_empty()) {
        return Some(n.to_string());
    }
    let host = url::Url::parse(url?.trim()).ok()?.host_str()?.to_string();
    Some(host.trim_start_matches("www.").to_string())
}

fn sections<'a, T>(items: &'a [T], section: impl Fn(&'a T) -> Option<&'a str>) -> Vec<(Option<&'a str>, Vec<&'a T>)> {
    let mut out: Vec<(Option<&str>, Vec<&T>)> = vec![];
    for it in items {
        let s = section(it).map(str::trim).filter(|s| !s.is_empty());
        match out.last_mut() {
            Some((cur, list)) if *cur == s => list.push(it),
            _ => out.push((s, vec![it])),
        }
    }
    out
}

fn render_ingredients(items: &[Ingredient]) -> String {
    let mut html = String::new();
    for (title, list) in sections(items, |i| i.section.as_deref()) {
        if let Some(t) = title {
            html.push_str(&format!("<h3>{}</h3>", esc(t)));
        }
        html.push_str("<ul class=\"ingredients\">");
        for i in list.iter().filter(|i| !i.original.trim().is_empty()) {
            html.push_str(&format!("<li>{}</li>", esc(i.original.trim())));
        }
        html.push_str("</ul>");
    }
    html
}

fn render_instructions(items: &[Instruction]) -> String {
    let mut html = String::new();
    for (title, list) in sections(items, |i| i.section.as_deref()) {
        if let Some(t) = title {
            html.push_str(&format!("<h3>{}</h3>", esc(t)));
        }
        html.push_str("<ol class=\"steps\">");
        for s in list.iter().filter(|s| !s.text.trim().is_empty()) {
            html.push_str(&format!("<li>{}</li>", esc_lines(&s.text)));
        }
        html.push_str("</ol>");
    }
    html
}

fn render_recipe(state: &AppState, r: &Recipe) -> String {
    let d = &r.data;
    let base = state.config.public_url.trim_end_matches('/');
    let image_path = r.image_id.as_ref().map(|i| format!("/api/images/{i}?size=medium"));

    let mut head = format!("<meta property=\"og:type\" content=\"article\"><meta property=\"og:title\" content=\"{}\">", esc(&d.name));
    let summary: String = d.description.trim().chars().take(200).collect();
    if !summary.is_empty() {
        head.push_str(&format!(
            "<meta name=\"description\" content=\"{0}\"><meta property=\"og:description\" content=\"{0}\">",
            esc(&summary)
        ));
    }
    if let Some(p) = &image_path {
        head.push_str(&format!("<meta property=\"og:image\" content=\"{}\">", esc(&format!("{base}{p}"))));
    }

    let mut body = String::new();
    if let Some(p) = &image_path {
        body.push_str(&format!("<img class=\"photo\" src=\"{}\" alt=\"\">", esc(p)));
    }
    if let Some(c) = d.categories.first() {
        body.push_str(&format!("<p class=\"category\">{}</p>", esc(c)));
    }
    body.push_str(&format!("<h1>{}</h1>", esc(&d.name)));
    if !d.description.trim().is_empty() {
        body.push_str(&format!("<p class=\"description\">{}</p>", esc_lines(&d.description)));
    }

    let mut facts = vec![];
    for (label, value) in [("Prep", d.prep_time_minutes), ("Cook", d.cook_time_minutes), ("Total", d.total_time_minutes)] {
        if let Some(m) = value.filter(|m| *m > 0) {
            facts.push((label, minutes(m)));
        }
    }
    if let Some(q) = d.yield_quantity.filter(|q| *q > 0.0) {
        facts.push(("Makes", format!("{} {}", number(q), d.yield_unit).trim().to_string()));
    }
    if !facts.is_empty() {
        body.push_str("<dl class=\"facts\">");
        for (label, value) in facts {
            body.push_str(&format!("<div><dt>{label}</dt><dd>{}</dd></div>", esc(&value)));
        }
        body.push_str("</dl>");
    }

    if !d.ingredients.is_empty() {
        body.push_str("<section><h2>Ingredients</h2>");
        body.push_str(&render_ingredients(&d.ingredients));
        body.push_str("</section>");
    }
    if !d.instructions.is_empty() {
        body.push_str("<section><h2>Instructions</h2>");
        body.push_str(&render_instructions(&d.instructions));
        body.push_str("</section>");
    }
    if !d.notes.trim().is_empty() {
        body.push_str(&format!("<section><h2>Notes</h2><p>{}</p></section>", esc_lines(&d.notes)));
    }
    if let Some(label) = source_label(d.source_name.as_deref(), d.source_url.as_deref()) {
        let source = match d.source_url.as_deref().and_then(safe_source_url) {
            Some(u) => format!("<a href=\"{}\" rel=\"nofollow noopener noreferrer\">{}</a>", esc(&u), esc(&label)),
            None => esc(&label),
        };
        body.push_str(&format!("<p class=\"source\">Adapted from {source}</p>"));
    }
    render_page(&d.name, &head, &body)
}

fn render_missing() -> String {
    render_page(
        "Recipe not available",
        "",
        "<h1>Recipe not available</h1><p class=\"description\">This link has been turned off, or the recipe was removed.</p>",
    )
}

fn render_page(title: &str, head: &str, body: &str) -> String {
    format!(
        r#"<!doctype html>
<html lang="en">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<meta name="robots" content="noindex, nofollow">
<title>{title}</title>
{head}
<style>{STYLE}</style>
</head>
<body>
<main>
{body}
</main>
<footer>Shared with Larder</footer>
</body>
</html>
"#,
        title = esc(title),
    )
}

const STYLE: &str = r#"
:root { --bg: #FBF6EE; --fg: #2B211C; --muted: #6E6158; --accent: #B5532F; --line: #E7DCCD; }
@media (prefers-color-scheme: dark) { :root { --bg: #1E1916; --fg: #F2E9DE; --muted: #B8AA9C; --accent: #E0855F; --line: #3A302A; } }
* { box-sizing: border-box; }
body { margin: 0; background: var(--bg); color: var(--fg); font: 17px/1.6 system-ui, -apple-system, "Segoe UI", Roboto, sans-serif; }
main { max-width: 720px; margin: 0 auto; padding: 24px 16px 8px; }
.photo { display: block; width: 100%; max-height: 460px; object-fit: cover; border-radius: 14px; margin-bottom: 20px; }
.category { margin: 0; color: var(--accent); font-size: 13px; font-weight: 600; letter-spacing: .08em; text-transform: uppercase; }
h1 { font: 600 2.1em/1.15 Georgia, "Times New Roman", serif; margin: 4px 0 12px; }
h2 { font: 600 1.35em/1.2 Georgia, "Times New Roman", serif; margin: 32px 0 10px; padding-bottom: 6px; border-bottom: 1px solid var(--line); }
h3 { font-size: 1em; margin: 18px 0 6px; color: var(--muted); }
.description { color: var(--muted); margin: 0 0 16px; }
.facts { display: flex; flex-wrap: wrap; gap: 10px; margin: 16px 0 0; }
.facts div { border: 1px solid var(--line); border-radius: 10px; padding: 6px 12px; }
.facts dt { font-size: 12px; color: var(--muted); }
.facts dd { margin: 0; font-weight: 600; }
.ingredients { padding-left: 20px; }
.ingredients li { margin: 4px 0; }
.steps { padding-left: 24px; }
.steps li { margin: 10px 0; padding-left: 4px; }
.steps li::marker { color: var(--accent); font-weight: 700; }
.source { margin-top: 32px; color: var(--muted); font-size: 15px; }
a { color: var(--accent); }
footer { max-width: 720px; margin: 0 auto; padding: 16px 16px 32px; color: var(--muted); font-size: 13px; }
@media print { .photo { max-height: 300px; } footer { display: none; } body { background: #fff; color: #000; } }
"#;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn escapes_html() {
        assert_eq!(esc(r#"<b>"Tom's" & co</b>"#), "&lt;b&gt;&quot;Tom&#39;s&quot; &amp; co&lt;/b&gt;");
        assert_eq!(esc_lines(" a<\nb "), "a&lt;<br>b");
    }

    #[test]
    fn only_web_links_are_clickable() {
        assert!(safe_source_url("https://example.com/r").is_some());
        assert!(safe_source_url("javascript:alert(1)").is_none());
        assert!(safe_source_url("data:text/html,x").is_none());
        assert_eq!(source_label(None, Some("https://www.example.com/x")).as_deref(), Some("example.com"));
        assert_eq!(source_label(Some(" Grandma "), Some("https://example.com")).as_deref(), Some("Grandma"));
    }

    #[test]
    fn formats_times_and_amounts() {
        assert_eq!(minutes(45), "45 min");
        assert_eq!(minutes(60), "1 h");
        assert_eq!(minutes(90), "1 h 30 min");
        assert_eq!(number(4.0), "4");
        assert_eq!(number(1.5), "1.5");
    }
}
