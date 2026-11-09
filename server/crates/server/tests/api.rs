//! End-to-end API tests. Run against SQLite always, and against PostgreSQL when
//! `LARDER_TEST_POSTGRES_URL` is set (the database is wiped first).

use axum::body::Body;
use axum::http::{Request, StatusCode};
use axum::Router;
use http_body_util::BodyExt;
use larder_server::config::Config;
use serde_json::{json, Value};
use tower::ServiceExt;

struct TestApp {
    app: Router,
    _dir: tempfile::TempDir,
}

impl TestApp {
    async fn new(db_url: Option<String>) -> Self {
        Self::with_config(db_url, |_| {}).await
    }

    async fn with_config(db_url: Option<String>, adjust: impl FnOnce(&mut Config)) -> Self {
        let dir = tempfile::tempdir().unwrap();
        let url = db_url.unwrap_or_else(|| format!("sqlite://{}", dir.path().join("test.db").display()));
        let mut config = Config::for_tests(dir.path().to_path_buf(), url);
        adjust(&mut config);
        let state = larder_server::build_state(config).await.expect("state");
        Self { app: larder_server::app(state), _dir: dir }
    }

    /// GET without authentication, returning status, headers and the body as text.
    async fn page(&self, path: &str) -> (StatusCode, axum::http::HeaderMap, String) {
        let resp = self.app.clone().oneshot(Request::builder().uri(path).body(Body::empty()).unwrap()).await.unwrap();
        let (status, headers) = (resp.status(), resp.headers().clone());
        let bytes = resp.into_body().collect().await.unwrap().to_bytes();
        (status, headers, String::from_utf8_lossy(&bytes).into_owned())
    }

    async fn req(&self, method: &str, path: &str, token: Option<&str>, body: Option<Value>) -> (StatusCode, Value) {
        let mut b = Request::builder().method(method).uri(path);
        if let Some(t) = token {
            b = b.header("authorization", format!("Bearer {t}"));
        }
        let req = match body {
            Some(v) => b.header("content-type", "application/json").body(Body::from(v.to_string())).unwrap(),
            None => b.body(Body::empty()).unwrap(),
        };
        let resp = self.app.clone().oneshot(req).await.unwrap();
        let status = resp.status();
        let bytes = resp.into_body().collect().await.unwrap().to_bytes();
        let v = serde_json::from_slice(&bytes).unwrap_or(Value::Null);
        (status, v)
    }

    async fn raw(&self, req: Request<Body>) -> (StatusCode, Vec<u8>) {
        let resp = self.app.clone().oneshot(req).await.unwrap();
        let status = resp.status();
        (status, resp.into_body().collect().await.unwrap().to_bytes().to_vec())
    }
}

async fn reset_postgres(url: &str) {
    use sqlx::Connection;
    let mut conn = sqlx::PgConnection::connect(url).await.expect("connect postgres");
    sqlx::query("DROP SCHEMA public CASCADE").execute(&mut conn).await.unwrap();
    sqlx::query("CREATE SCHEMA public").execute(&mut conn).await.unwrap();
}

fn png_bytes() -> Vec<u8> {
    let img = image::RgbImage::from_fn(64, 48, |x, y| image::Rgb([x as u8 * 3, y as u8 * 4, 90]));
    let mut out = Vec::new();
    image::DynamicImage::ImageRgb8(img).write_to(&mut std::io::Cursor::new(&mut out), image::ImageFormat::Png).unwrap();
    out
}

fn multipart(field: &str, filename: &str, bytes: &[u8], extra: &[(&str, &str)]) -> (String, Vec<u8>) {
    let boundary = "XBOUNDARYX";
    let mut body = Vec::new();
    for (k, v) in extra {
        body.extend_from_slice(format!("--{boundary}\r\nContent-Disposition: form-data; name=\"{k}\"\r\n\r\n{v}\r\n").as_bytes());
    }
    body.extend_from_slice(
        format!("--{boundary}\r\nContent-Disposition: form-data; name=\"{field}\"; filename=\"{filename}\"\r\nContent-Type: application/octet-stream\r\n\r\n").as_bytes(),
    );
    body.extend_from_slice(bytes);
    body.extend_from_slice(format!("\r\n--{boundary}--\r\n").as_bytes());
    (format!("multipart/form-data; boundary={boundary}"), body)
}

async fn scenario(t: &TestApp) {
    // --- setup & auth ---
    let (s, cfg) = t.req("GET", "/api/config", None, None).await;
    assert_eq!(s, StatusCode::OK);
    assert_eq!(cfg["setup_required"], true);

    let (s, admin) = t
        .req(
            "POST",
            "/api/auth/register",
            None,
            Some(json!({"username": "Alice", "password": "password123", "email": "alice@example.com"})),
        )
        .await;
    assert_eq!(s, StatusCode::CREATED, "{admin}");
    assert_eq!(admin["user"]["is_admin"], true);
    assert_eq!(admin["user"]["household_role"], "owner");
    let tok = admin["access_token"].as_str().unwrap().to_string();

    // registration closed for strangers
    let (s, _) = t.req("POST", "/api/auth/register", None, Some(json!({"username": "eve", "password": "password123"}))).await;
    assert_eq!(s, StatusCode::FORBIDDEN);

    // login (case-insensitive, also by email), bad password
    let (s, _) = t.req("POST", "/api/auth/login", None, Some(json!({"username": "alice", "password": "nope-nope"}))).await;
    assert_eq!(s, StatusCode::UNAUTHORIZED);
    let (s, login) =
        t.req("POST", "/api/auth/login", None, Some(json!({"username": "ALICE@example.com", "password": "password123"}))).await;
    assert_eq!(s, StatusCode::OK);
    let refresh = login["refresh_token"].as_str().unwrap().to_string();
    let (s, r2) = t.req("POST", "/api/auth/refresh", None, Some(json!({"refresh_token": refresh}))).await;
    assert_eq!(s, StatusCode::OK);
    // old refresh token is rotated out
    let (s, _) = t.req("POST", "/api/auth/refresh", None, Some(json!({"refresh_token": refresh}))).await;
    assert_eq!(s, StatusCode::UNAUTHORIZED);
    assert!(r2["refresh_token"].is_string());

    let (s, _) = t.req("GET", "/api/users/me", None, None).await;
    assert_eq!(s, StatusCode::UNAUTHORIZED);
    let (s, me) =
        t.req("PATCH", "/api/users/me", Some(&tok), Some(json!({"display_name": "Alice A.", "preferences": {"units": "metric"}}))).await;
    assert_eq!(s, StatusCode::OK, "{me}");
    assert_eq!(me["display_name"], "Alice A.");
    assert_eq!(me["preferences"]["units"], "metric");

    // --- household & invites ---
    let (s, inv) = t.req("POST", "/api/household/invites", Some(&tok), None).await;
    assert_eq!(s, StatusCode::CREATED, "{inv}");
    let code = inv["code"].as_str().unwrap().to_string();
    let (s, bob) = t
        .req(
            "POST",
            "/api/auth/register",
            None,
            Some(json!({"username": "bob", "password": "password123", "invite_code": code.to_lowercase()})),
        )
        .await;
    assert_eq!(s, StatusCode::CREATED, "{bob}");
    assert_eq!(bob["user"]["household_id"], admin["user"]["household_id"]);
    assert_eq!(bob["user"]["household_role"], "member");
    let bob_tok = bob["access_token"].as_str().unwrap().to_string();
    let (s, _) = t.req("POST", "/api/household/invites", Some(&bob_tok), None).await;
    assert_eq!(s, StatusCode::FORBIDDEN);
    let (s, hh) = t.req("GET", "/api/household", Some(&bob_tok), None).await;
    assert_eq!(s, StatusCode::OK);
    assert_eq!(hh["members"].as_array().unwrap().len(), 2);

    // --- recipes ---
    let rid = "6f1c1f1e-1111-4a4a-9b9b-000000000001";
    let body = json!({
        "id": rid,
        "name": "Classic Pancakes",
        "description": "Fluffy weekend pancakes",
        "yield_quantity": 4,
        "prep_time_minutes": 10,
        "cook_time_minutes": 15,
        "ingredients": [
            {"original": "1 1/2 cups all-purpose flour"},
            {"original": "2 tbsp sugar"},
            {"original": "2 eggs"},
            {"original": "1 1/4 cups milk", "section": "Wet"}
        ],
        "instructions": [{"text": "Mix dry."}, {"text": "Add wet and cook."}],
        "tags": ["Breakfast", "quick"],
        "categories": ["Breakfast"]
    });
    let (s, rec) = t.req("POST", "/api/recipes", Some(&tok), Some(body.clone())).await;
    assert_eq!(s, StatusCode::CREATED, "{rec}");
    assert_eq!(rec["slug"], "classic-pancakes");
    assert_eq!(rec["total_time_minutes"], 25);
    assert_eq!(rec["ingredients"][0]["quantity"], 1.5);
    assert_eq!(rec["ingredients"][0]["unit"], "cup");
    assert_eq!(rec["ingredients"][3]["section"], "Wet");
    assert_eq!(rec["version"], 1);
    // idempotent create with same id
    let (s, again) = t.req("POST", "/api/recipes", Some(&tok), Some(body)).await;
    assert_eq!(s, StatusCode::OK);
    assert_eq!(again["id"], rid);

    let (s, r2) = t
        .req(
            "POST",
            "/api/recipes",
            Some(&bob_tok),
            Some(
                json!({"name": "Classic Pancakes", "ingredients": [{"original": "salt"}], "categories": ["Breakfast"], "tags": ["savory"]}),
            ),
        )
        .await;
    assert_eq!(s, StatusCode::CREATED);
    assert_eq!(r2["slug"], "classic-pancakes-2");
    let (s, _) = t.req("POST", "/api/recipes", Some(&tok), Some(json!({"name": "  "}))).await;
    assert_eq!(s, StatusCode::BAD_REQUEST);
    let (s, lasagna) = t
        .req(
            "POST",
            "/api/recipes",
            Some(&tok),
            Some(json!({"name": "Crème Lasagna", "yield_quantity": 6, "rating": 5,
            "ingredients": [{"original": "500 g ground beef"}, {"original": "1 cup milk"}, {"original": "2 eggs"}],
            "categories": ["Mains"], "tags": ["Italian"]})),
        )
        .await;
    assert_eq!(s, StatusCode::CREATED);
    let lasagna_id = lasagna["id"].as_str().unwrap().to_string();

    // get by slug, list/search/filter/sort
    let (s, g) = t.req("GET", "/api/recipes/classic-pancakes", Some(&bob_tok), None).await;
    assert_eq!(s, StatusCode::OK);
    assert_eq!(g["id"], rid);
    let (_, l) = t.req("GET", "/api/recipes?q=creme", Some(&tok), None).await;
    assert_eq!(l["total"], 1, "{l}");
    assert_eq!(l["items"][0]["name"], "Crème Lasagna");
    let (_, l) = t.req("GET", "/api/recipes?q=flour%20sugar", Some(&tok), None).await;
    assert_eq!(l["total"], 1);
    let (_, l) = t.req("GET", "/api/recipes?category=breakfast", Some(&tok), None).await;
    assert_eq!(l["total"], 2);
    let (_, l) = t.req("GET", "/api/recipes?category=breakfast&tag=QUICK", Some(&tok), None).await;
    assert_eq!(l["total"], 1);
    let (_, l) = t.req("GET", "/api/recipes?sort=-rating&per_page=1&page=1", Some(&tok), None).await;
    assert_eq!(l["total"], 3);
    assert_eq!(l["items"].as_array().unwrap().len(), 1);
    assert_eq!(l["items"][0]["name"], "Crème Lasagna");

    // favorites are per user
    let (s, _) = t.req("PUT", &format!("/api/recipes/{rid}/favorite"), Some(&tok), None).await;
    assert_eq!(s, StatusCode::NO_CONTENT);
    let (_, l) = t.req("GET", "/api/recipes?favorites=true", Some(&tok), None).await;
    assert_eq!(l["total"], 1);
    let (_, l) = t.req("GET", "/api/recipes?favorites=true", Some(&bob_tok), None).await;
    assert_eq!(l["total"], 0);

    // update with optimistic concurrency
    let mut upd = g.clone();
    upd["name"] = json!("Best Pancakes");
    upd["version"] = json!(1);
    let (s, u) = t.req("PUT", &format!("/api/recipes/{rid}"), Some(&tok), Some(upd.clone())).await;
    assert_eq!(s, StatusCode::OK, "{u}");
    assert_eq!(u["version"], 2);
    assert_eq!(u["slug"], "best-pancakes");
    assert_eq!(u["is_favorite"], true);
    let (s, c) = t.req("PUT", &format!("/api/recipes/{rid}"), Some(&bob_tok), Some(upd)).await;
    assert_eq!(s, StatusCode::CONFLICT);
    assert_eq!(c["code"], "conflict");
    assert_eq!(c["current"]["version"], 2);

    // tags
    let (_, tags) = t.req("GET", "/api/tags", Some(&tok), None).await;
    let names: Vec<&str> = tags.as_array().unwrap().iter().map(|t| t["name"].as_str().unwrap()).collect();
    assert!(names.contains(&"quick") && names.contains(&"Italian"), "{tags}");
    let (s, _) = t.req("POST", "/api/tags/rename", Some(&tok), Some(json!({"kind": "tag", "from": "quick", "to": "Fast"}))).await;
    assert_eq!(s, StatusCode::NO_CONTENT);
    let (_, l) = t.req("GET", "/api/recipes?tag=fast", Some(&tok), None).await;
    assert_eq!(l["total"], 1);
    let (_, cats) = t.req("GET", "/api/categories", Some(&tok), None).await;
    assert_eq!(cats.as_array().unwrap().iter().find(|c| c["name"] == "Breakfast").unwrap()["count"], 2);

    // parsing
    let (s, parsed) = t.req("POST", "/api/parse/ingredients", Some(&tok), Some(json!({"lines": ["2-3 cloves garlic, minced"]}))).await;
    assert_eq!(s, StatusCode::OK);
    assert_eq!(parsed[0]["quantity_max"], 3.0);
    let (s, e) = t.req("POST", "/api/parse/ingredients", Some(&tok), Some(json!({"lines": ["x"], "use_llm": true}))).await;
    assert_eq!(s, StatusCode::BAD_REQUEST);
    assert_eq!(e["code"], "llm_disabled");

    // images
    let (ct, mp) = multipart("file", "p.png", &png_bytes(), &[]);
    let req = Request::builder()
        .method("PUT")
        .uri(format!("/api/recipes/{rid}/image"))
        .header("authorization", format!("Bearer {tok}"))
        .header("content-type", ct)
        .body(Body::from(mp))
        .unwrap();
    let (s, body) = t.raw(req).await;
    assert_eq!(s, StatusCode::OK, "{}", String::from_utf8_lossy(&body));
    let rec: Value = serde_json::from_slice(&body).unwrap();
    let img_url = rec["image_url"].as_str().unwrap().to_string();
    let (s, img) = t.raw(Request::builder().uri(format!("{img_url}?size=thumb")).body(Body::empty()).unwrap()).await;
    assert_eq!(s, StatusCode::OK);
    assert_eq!(&img[..2], &[0xFF, 0xD8]);
    // duplicate copies the image
    let (s, dup) = t.req("POST", &format!("/api/recipes/{rid}/duplicate"), Some(&tok), None).await;
    assert_eq!(s, StatusCode::CREATED);
    assert!(dup["image_url"].is_string());
    assert_ne!(dup["image_url"], rec["image_url"]);

    // --- meal plan ---
    let (s, mp1) = t
        .req(
            "POST",
            "/api/mealplan",
            Some(&tok),
            Some(json!({"date": "2026-11-10", "meal": "dinner", "recipe_id": lasagna_id, "servings": 12})),
        )
        .await;
    assert_eq!(s, StatusCode::CREATED, "{mp1}");
    assert_eq!(mp1["recipe_name"], "Crème Lasagna");
    let (s, _) =
        t.req("POST", "/api/mealplan", Some(&tok), Some(json!({"date": "2026-11-10", "meal": "breakfast", "recipe_id": rid}))).await;
    assert_eq!(s, StatusCode::CREATED);
    let (s, _) =
        t.req("POST", "/api/mealplan", Some(&tok), Some(json!({"date": "2026-11-11", "meal": "lunch", "title": "Leftovers"}))).await;
    assert_eq!(s, StatusCode::CREATED);
    let (s, _) = t.req("POST", "/api/mealplan", Some(&tok), Some(json!({"date": "2026-13-01", "meal": "lunch", "title": "x"}))).await;
    assert_eq!(s, StatusCode::BAD_REQUEST);
    let (_, week) = t.req("GET", "/api/mealplan?start=2026-11-09&end=2026-11-15", Some(&bob_tok), None).await;
    let week = week.as_array().unwrap();
    assert_eq!(week.len(), 3);
    assert_eq!(week[0]["meal"], "breakfast");
    assert_eq!(week[1]["meal"], "dinner");

    // --- shopping ---
    let (s, list) = t.req("POST", "/api/shopping-lists", Some(&tok), Some(json!({"name": "Weekly"}))).await;
    assert_eq!(s, StatusCode::CREATED);
    let lid = list["id"].as_str().unwrap().to_string();
    let (s, item) = t.req("POST", &format!("/api/shopping-lists/{lid}/items"), Some(&tok), Some(json!({"text": "2 lbs apples"}))).await;
    assert_eq!(s, StatusCode::CREATED, "{item}");
    assert_eq!(item["aisle"], "Produce");
    assert_eq!(item["unit"], "lb");
    let (s, full) = t
        .req(
            "POST",
            &format!("/api/shopping-lists/{lid}/add-mealplan"),
            Some(&tok),
            Some(json!({"start": "2026-11-09", "end": "2026-11-15"})),
        )
        .await;
    assert_eq!(s, StatusCode::OK, "{full}");
    let items = full["items"].as_array().unwrap();
    let milk = items.iter().find(|i| i["food"] == "milk").expect("milk merged");
    // lasagna 1 cup x2 (12/6) + pancakes 1 1/4 cup = 3.25 cups
    assert!((milk["quantity"].as_f64().unwrap() - 3.25).abs() < 0.01, "{milk}");
    assert_eq!(milk["recipe_ids"].as_array().unwrap().len(), 2);
    let eggs = items.iter().find(|i| i["food"] == "eggs").expect("eggs");
    assert!((eggs["quantity"].as_f64().unwrap() - 6.0).abs() < 0.01);
    assert_eq!(eggs["aisle"], "Dairy & Eggs");
    // aisle order: Produce before Meat before Dairy
    let aisles: Vec<&str> = items.iter().map(|i| i["aisle"].as_str().unwrap()).collect();
    assert_eq!(aisles[0], "Produce");
    let item_id = item["id"].as_str().unwrap();
    let (s, ch) =
        t.req("PATCH", &format!("/api/shopping-lists/{lid}/items/{item_id}"), Some(&bob_tok), Some(json!({"checked": true}))).await;
    assert_eq!(s, StatusCode::OK);
    assert_eq!(ch["checked"], true);
    let (_, lists) = t.req("GET", "/api/shopping-lists", Some(&tok), None).await;
    assert_eq!(lists[0]["checked_count"], 1);
    let (s, cleared) = t.req("POST", &format!("/api/shopping-lists/{lid}/clear-checked"), Some(&tok), None).await;
    assert_eq!(s, StatusCode::OK);
    assert!(cleared["items"].as_array().unwrap().iter().all(|i| i["checked"] == false));

    // --- sync ---
    let (s, sync) = t.req("GET", "/api/sync", Some(&tok), None).await;
    assert_eq!(s, StatusCode::OK);
    assert_eq!(sync["full"], true);
    assert_eq!(sync["recipes"].as_array().unwrap().len(), 4);
    assert_eq!(sync["shopping_lists"][0]["items"].as_array().unwrap().len(), cleared["items"].as_array().unwrap().len());
    let cursor = sync["cursor"].as_str().unwrap().to_string();
    let (s, _) = t.req("DELETE", &format!("/api/recipes/{}", dup["id"].as_str().unwrap()), Some(&tok), None).await;
    assert_eq!(s, StatusCode::NO_CONTENT);
    let (_, inc) = t.req("GET", &format!("/api/sync?since={}", urlencode(&cursor)), Some(&tok), None).await;
    assert_eq!(inc["full"], false);
    assert!(inc["deleted_recipe_ids"].as_array().unwrap().iter().any(|d| d == &dup["id"]));

    // --- admin ---
    let (s, users) = t.req("GET", "/api/admin/users", Some(&tok), None).await;
    assert_eq!(s, StatusCode::OK);
    assert_eq!(users.as_array().unwrap().len(), 2);
    let (s, _) = t.req("GET", "/api/admin/users", Some(&bob_tok), None).await;
    assert_eq!(s, StatusCode::FORBIDDEN);
    let bob_id = bob["user"]["id"].as_str().unwrap();
    let (s, _) = t.req("PATCH", &format!("/api/admin/users/{bob_id}"), Some(&tok), Some(json!({"disabled": true}))).await;
    assert_eq!(s, StatusCode::OK);
    let (s, _) = t.req("GET", "/api/users/me", Some(&bob_tok), None).await;
    assert_eq!(s, StatusCode::UNAUTHORIZED);

    // remove member -> gets own household; recipes stay with Alice's household
    let (s, _) = t.req("DELETE", &format!("/api/household/members/{bob_id}"), Some(&tok), None).await;
    assert_eq!(s, StatusCode::NO_CONTENT);
    let (_, hh) = t.req("GET", "/api/household", Some(&tok), None).await;
    assert_eq!(hh["members"].as_array().unwrap().len(), 1);
    let (s, _) = t.req("DELETE", &format!("/api/admin/users/{bob_id}"), Some(&tok), None).await;
    assert_eq!(s, StatusCode::NO_CONTENT);

    // unknown API route
    let (s, e) = t.req("GET", "/api/nope", Some(&tok), None).await;
    assert_eq!(s, StatusCode::NOT_FOUND);
    assert_eq!(e["code"], "not_found");

    extra_scenarios(t, &tok).await;
}

fn urlencode(s: &str) -> String {
    url::form_urlencoded::byte_serialize(s.as_bytes()).collect()
}

/// Hook for import/export tests (kept separate so the core scenario stays readable).
async fn extra_scenarios(t: &TestApp, tok: &str) {
    transfer::run(t, tok).await;
    sharing::run(t, tok).await;
}

mod sharing {
    use super::*;

    pub async fn run(t: &TestApp, tok: &str) {
        let (_, cfg) = t.req("GET", "/api/config", None, None).await;
        assert_eq!(cfg["public_sharing"], true);

        let (s, r) = t
            .req(
                "POST",
                "/api/recipes",
                Some(tok),
                Some(json!({
                    "name": "Tom's <Soup> & \"Bread\"",
                    "description": "Rich and <i>warm</i>.",
                    "source_url": "javascript:alert(1)",
                    "source_name": "Aunt <May>",
                    "prep_time_minutes": 15,
                    "total_time_minutes": 90,
                    "yield_quantity": 4,
                    "yield_unit": "servings",
                    "notes": "Keeps a week.",
                    "ingredients": [
                        {"original": "2 lb tomatoes", "section": "Soup"},
                        {"original": "1 loaf <bread>", "section": "Toast"}
                    ],
                    "instructions": [{"text": "Simmer the tomatoes."}, {"text": "Toast the bread."}]
                })),
            )
            .await;
        assert_eq!(s, StatusCode::CREATED, "{r}");
        let rid = r["id"].as_str().unwrap().to_string();
        let share_path = format!("/api/recipes/{rid}/share");

        // not shared yet; sharing needs a signed-in member
        let (s, _) = t.req("GET", &share_path, Some(tok), None).await;
        assert_eq!(s, StatusCode::NOT_FOUND);
        let (s, _) = t.req("POST", &share_path, None, None).await;
        assert_eq!(s, StatusCode::UNAUTHORIZED);

        // create, then sharing again returns the same link
        let (s, sh) = t.req("POST", &share_path, Some(tok), None).await;
        assert_eq!(s, StatusCode::CREATED, "{sh}");
        let token = sh["token"].as_str().unwrap().to_string();
        assert_eq!(token.len(), 32);
        assert!(token.bytes().all(|b| b.is_ascii_hexdigit()));
        assert_eq!(sh["url"], format!("http://localhost:8080/s/{token}"));
        let (s, again) = t.req("POST", &share_path, Some(tok), None).await;
        assert_eq!(s, StatusCode::OK);
        assert_eq!(again["token"], token.as_str());
        let (s, got) = t.req("GET", &share_path, Some(tok), None).await;
        assert_eq!(s, StatusCode::OK);
        assert_eq!(got["url"], sh["url"]);

        // the public page needs no account, escapes everything and is not indexed or cached
        let (s, h, html) = t.page(&format!("/s/{token}")).await;
        assert_eq!(s, StatusCode::OK);
        assert_eq!(h["content-type"], "text/html; charset=utf-8");
        assert_eq!(h["x-robots-tag"], "noindex, nofollow");
        assert_eq!(h["cache-control"], "no-store");
        assert_eq!(h["referrer-policy"], "no-referrer");
        assert!(h["content-security-policy"].to_str().unwrap().contains("default-src 'none'"));
        assert!(html.contains("<h1>Tom&#39;s &lt;Soup&gt; &amp; &quot;Bread&quot;</h1>"), "{html}");
        assert!(html.contains("Rich and &lt;i&gt;warm&lt;/i&gt;."));
        assert!(html.contains("<h3>Soup</h3>") && html.contains("<h3>Toast</h3>"));
        assert!(html.contains("<li>1 loaf &lt;bread&gt;</li>"));
        assert!(html.contains("<li>Simmer the tomatoes.</li>"));
        assert!(html.contains("1 h 30 min") && html.contains("4 servings"));
        assert!(html.contains("Adapted from Aunt &lt;May&gt;"));
        assert!(!html.contains("javascript:"), "unsafe source links are not linked");
        assert!(!html.contains("<i>") && !html.contains("<Soup>"));

        // edits show up on the page
        let mut edited = r.clone();
        edited["name"] = json!("Tomato Soup");
        edited["source_url"] = json!("https://example.com/soup");
        let (s, _) = t.req("PUT", &format!("/api/recipes/{rid}"), Some(tok), Some(edited)).await;
        assert_eq!(s, StatusCode::OK);
        let (_, _, html) = t.page(&format!("/s/{token}")).await;
        assert!(html.contains("<h1>Tomato Soup</h1>"));
        assert!(html.contains(r#"<a href="https://example.com/soup" rel="nofollow noopener noreferrer">Aunt &lt;May&gt;</a>"#), "{html}");

        // unknown and malformed tokens
        for bad in ["/s/0123456789abcdef0123456789abcdef", "/s/nope", "/s/..%2F..%2Fetc%2Fpasswd"] {
            let (s, _, html) = t.page(bad).await;
            assert_eq!(s, StatusCode::NOT_FOUND, "{bad}");
            assert!(html.contains("Recipe not available"));
        }

        // turning the link off stops it at once; sharing again makes a new link
        let (s, _) = t.req("DELETE", &share_path, Some(tok), None).await;
        assert_eq!(s, StatusCode::NO_CONTENT);
        let (s, _, _) = t.page(&format!("/s/{token}")).await;
        assert_eq!(s, StatusCode::NOT_FOUND);
        let (s, sh2) = t.req("POST", &share_path, Some(tok), None).await;
        assert_eq!(s, StatusCode::CREATED);
        let token2 = sh2["token"].as_str().unwrap().to_string();
        assert_ne!(token2, token);

        // deleting the recipe removes its link
        let (s, _) = t.req("DELETE", &format!("/api/recipes/{rid}"), Some(tok), None).await;
        assert_eq!(s, StatusCode::NO_CONTENT);
        let (s, _, _) = t.page(&format!("/s/{token2}")).await;
        assert_eq!(s, StatusCode::NOT_FOUND);
    }
}

mod transfer {
    use super::*;

    async fn upload(t: &TestApp, tok: &str, path: &str, filename: &str, bytes: &[u8], extra: &[(&str, &str)]) -> (StatusCode, Value) {
        let (ct, body) = multipart("file", filename, bytes, extra);
        let req = Request::builder()
            .method("POST")
            .uri(path)
            .header("authorization", format!("Bearer {tok}"))
            .header("content-type", ct)
            .body(Body::from(body))
            .unwrap();
        let (s, b) = t.raw(req).await;
        (s, serde_json::from_slice(&b).unwrap_or(Value::Null))
    }

    async fn wait_job(t: &TestApp, tok: &str, id: &str) -> Value {
        for _ in 0..200 {
            let (_, job) = t.req("GET", &format!("/api/import/jobs/{id}"), Some(tok), None).await;
            if job["status"] == "done" || job["status"] == "failed" {
                return job;
            }
            tokio::time::sleep(std::time::Duration::from_millis(50)).await;
        }
        panic!("import job did not finish");
    }

    pub async fn run(t: &TestApp, tok: &str) {
        // plain text, preview then save
        let text = "Garlic Bread\n\nServes 4\n\nIngredients\n1 baguette\n4 tbsp butter\n3 cloves garlic\n\nInstructions\n1. Mix butter and garlic.\n2. Spread and bake 10 minutes.";
        let (s, prev) = t.req("POST", "/api/import/text", Some(tok), Some(json!({"text": text}))).await;
        assert_eq!(s, StatusCode::OK, "{prev}");
        assert_eq!(prev["name"], "Garlic Bread");
        assert_eq!(prev["ingredients"].as_array().unwrap().len(), 3);
        assert_eq!(prev["instructions"].as_array().unwrap().len(), 2);
        let (s, saved) = t.req("POST", "/api/import/text", Some(tok), Some(json!({"text": text, "save": true}))).await;
        assert_eq!(s, StatusCode::CREATED);
        assert!(saved["id"].is_string());

        // JSON-LD file import as a background job
        let jsonld = json!([
            {"@context": "https://schema.org", "@type": "Recipe", "name": "Tomato Soup", "recipeYield": "4 servings",
             "recipeIngredient": ["2 lbs tomatoes", "1 onion"], "recipeInstructions": [{"@type": "HowToStep", "text": "Simmer."}],
             "prepTime": "PT10M", "cookTime": "PT30M", "recipeCategory": "Soups"},
            {"@type": "Recipe", "name": "Garlic Bread", "recipeIngredient": ["1 baguette"]}
        ]);
        let (s, job) = upload(t, tok, "/api/import/file", "recipes.json", jsonld.to_string().as_bytes(), &[("source", "auto")]).await;
        assert_eq!(s, StatusCode::ACCEPTED, "{job}");
        let job = wait_job(t, tok, job["id"].as_str().unwrap()).await;
        assert_eq!(job["status"], "done", "{job}");
        assert_eq!(job["imported"], 1, "{job}");
        assert_eq!(job["skipped"], 1, "duplicate by name should be skipped: {job}");
        let (_, soup) = t.req("GET", &format!("/api/recipes/{}", job["recipe_ids"][0].as_str().unwrap()), Some(tok), None).await;
        assert_eq!(soup["name"], "Tomato Soup");
        assert_eq!(soup["total_time_minutes"], 40);
        assert_eq!(soup["categories"][0], "Soups");
        let (_, jobs) = t.req("GET", "/api/import/jobs", Some(tok), None).await;
        assert_eq!(jobs.as_array().unwrap().len(), 1);

        // bad file
        let (s, job) = upload(t, tok, "/api/import/file", "x.zip", b"not a zip", &[("source", "mealie")]).await;
        assert_eq!(s, StatusCode::ACCEPTED);
        let job = wait_job(t, tok, job["id"].as_str().unwrap()).await;
        assert_eq!(job["status"], "failed");

        // URL import rejects loopback addresses
        let (s, _) = t.req("POST", "/api/import/url", Some(tok), Some(json!({"url": "http://127.0.0.1:9/recipe"}))).await;
        assert_eq!(s, StatusCode::BAD_REQUEST);

        // backup export round-trips through the importer
        let req = Request::builder().uri("/api/export/json").header("authorization", format!("Bearer {tok}")).body(Body::empty()).unwrap();
        let (s, zip) = t.raw(req).await;
        assert_eq!(s, StatusCode::OK);
        assert_eq!(&zip[..2], b"PK");
        let (_, before) = t.req("GET", "/api/recipes", Some(tok), None).await;
        let (s, job) = upload(t, tok, "/api/import/file", "backup.zip", &zip, &[("source", "larder")]).await;
        assert_eq!(s, StatusCode::ACCEPTED);
        let job = wait_job(t, tok, job["id"].as_str().unwrap()).await;
        assert_eq!(job["status"], "done", "{job}");
        assert_eq!(job["imported"], 0, "{job}");
        assert_eq!(job["skipped"], before["total"], "{job}");
        let (s, job) = upload(t, tok, "/api/import/file", "backup.zip", &zip, &[("source", "auto"), ("skip_duplicates", "false")]).await;
        assert_eq!(s, StatusCode::ACCEPTED);
        let job = wait_job(t, tok, job["id"].as_str().unwrap()).await;
        assert_eq!(job["imported"], before["total"], "{job}");
        // images survive the round trip
        let (_, after) = t.req("GET", "/api/recipes?q=best%20pancakes", Some(tok), None).await;
        assert_eq!(after["total"], 2);
        assert!(after["items"].as_array().unwrap().iter().all(|r| r["image_url"].is_string()), "{after}");

        cookbook(t, tok).await;
    }

    async fn cookbook(t: &TestApp, tok: &str) {
        let (s, themes) = t.req("GET", "/api/export/cookbook/themes", Some(tok), None).await;
        assert_eq!(s, StatusCode::OK);
        assert!(themes.as_array().unwrap().len() >= 3, "{themes}");
        let req = Request::builder()
            .method("POST")
            .uri("/api/export/cookbook")
            .header("authorization", format!("Bearer {tok}"))
            .header("content-type", "application/json")
            .body(Body::from(json!({"title": "Our Book", "theme": "classic", "paper": "a5", "units": "metric"}).to_string()))
            .unwrap();
        let (s, pdf) = t.raw(req).await;
        assert_eq!(s, StatusCode::OK, "{}", String::from_utf8_lossy(&pdf[..pdf.len().min(500)]));
        assert_eq!(&pdf[..4], b"%PDF");
        assert!(pdf.len() > 10_000);
    }
}

#[tokio::test]
async fn sqlite_scenario() {
    let t = TestApp::new(None).await;
    scenario(&t).await;
}

#[tokio::test]
async fn postgres_scenario() {
    let Ok(url) = std::env::var("LARDER_TEST_POSTGRES_URL") else {
        eprintln!("skipping: LARDER_TEST_POSTGRES_URL not set");
        return;
    };
    reset_postgres(&url).await;
    let t = TestApp::new(Some(url)).await;
    scenario(&t).await;
}

#[tokio::test]
async fn profile_password_change_keeps_this_session_and_email_clears() {
    let t = TestApp::new(None).await;
    let (_, a) = t
        .req("POST", "/api/auth/register", None, Some(json!({"username": "amy", "password": "password123", "email": "amy@example.com"})))
        .await;
    let tok = a["access_token"].as_str().unwrap().to_string();
    let keep = a["refresh_token"].as_str().unwrap().to_string();
    let (_, other) = t.req("POST", "/api/auth/login", None, Some(json!({"username": "amy", "password": "password123"}))).await;
    let other_refresh = other["refresh_token"].as_str().unwrap().to_string();

    let (s, _) = t
        .req(
            "PATCH",
            "/api/users/me",
            Some(&tok),
            Some(json!({"current_password": "wrong-pass", "new_password": "new-password-1", "refresh_token": keep})),
        )
        .await;
    assert_eq!(s, StatusCode::FORBIDDEN);
    let (s, _) = t
        .req(
            "PATCH",
            "/api/users/me",
            Some(&tok),
            Some(json!({"current_password": "password123", "new_password": "new-password-1", "refresh_token": keep})),
        )
        .await;
    assert_eq!(s, StatusCode::OK);
    // this device's refresh token survives, the other session is signed out
    let (s, _) = t.req("POST", "/api/auth/refresh", None, Some(json!({"refresh_token": keep}))).await;
    assert_eq!(s, StatusCode::OK);
    let (s, _) = t.req("POST", "/api/auth/refresh", None, Some(json!({"refresh_token": other_refresh}))).await;
    assert_eq!(s, StatusCode::UNAUTHORIZED);

    // absent email: unchanged; explicit null: cleared
    let (_, me) = t.req("PATCH", "/api/users/me", Some(&tok), Some(json!({"display_name": "Amy"}))).await;
    assert_eq!(me["email"], "amy@example.com");
    let (_, me) = t.req("PATCH", "/api/users/me", Some(&tok), Some(json!({"email": null}))).await;
    assert!(me["email"].is_null(), "{me}");
}

#[tokio::test]
async fn organize_normalizes_and_categorizes() {
    let t = TestApp::new(None).await;
    let (_, a) = t.req("POST", "/api/auth/register", None, Some(json!({"username": "org", "password": "password123"}))).await;
    let tok = a["access_token"].as_str().unwrap().to_string();
    for (name, cats) in [("Chicken Noodle Soup", json!([])), ("Butter Cookies", json!(["Treats"]))] {
        let (s, _) = t
            .req(
                "POST",
                "/api/recipes",
                Some(&tok),
                Some(json!({"name": name, "categories": cats,
                "ingredients": [{"original": "2 Tablespoons unsalted butter, melted"}, {"original": "3 large Eggs"}]})),
            )
            .await;
        assert_eq!(s, StatusCode::CREATED);
    }
    let (_, soup) = t.req("GET", "/api/recipes/chicken-noodle-soup", Some(&tok), None).await;
    assert_eq!(soup["ingredients"][0]["original"], "2 tbsp unsalted butter, melted");
    assert_eq!(soup["ingredients"][1]["food"], "eggs");
    assert_eq!(soup["ingredients"][1]["note"], "large");

    let (s, dry) = t.req("POST", "/api/recipes/organize", Some(&tok), Some(json!({"dry_run": true}))).await;
    assert_eq!(s, StatusCode::OK, "{dry}");
    assert_eq!(dry["categorized"], 1, "{dry}");
    assert_eq!(dry["changes"][0]["category_after"], "Soups & Stews");
    let (_, still) = t.req("GET", "/api/recipes/chicken-noodle-soup", Some(&tok), None).await;
    assert_eq!(still["categories"], json!([]));

    let (_, done) = t.req("POST", "/api/recipes/organize", Some(&tok), Some(json!({}))).await;
    assert_eq!(done["recipes_changed"], 1);
    let (_, soup) = t.req("GET", "/api/recipes/chicken-noodle-soup", Some(&tok), None).await;
    assert_eq!(soup["categories"], json!(["Soups & Stews"]));
    // existing categories are kept unless asked to recategorise everything
    let (_, again) = t.req("POST", "/api/recipes/organize", Some(&tok), Some(json!({}))).await;
    assert_eq!(again["recipes_changed"], 0);
    let (_, all) = t.req("POST", "/api/recipes/organize", Some(&tok), Some(json!({"categorize": "all", "dry_run": true}))).await;
    assert_eq!(all["changes"][0]["category_after"], "Desserts");
}

#[tokio::test]
async fn share_links_stay_within_the_household() {
    let t = TestApp::with_config(None, |c| c.allow_registration = true).await;
    let (_, a) = t.req("POST", "/api/auth/register", None, Some(json!({"username": "ann", "password": "password123"}))).await;
    let (_, b) = t.req("POST", "/api/auth/register", None, Some(json!({"username": "ben", "password": "password123"}))).await;
    let (ann, ben) = (a["access_token"].as_str().unwrap(), b["access_token"].as_str().unwrap());
    let (_, r) = t.req("POST", "/api/recipes", Some(ann), Some(json!({"name": "Plum Cake"}))).await;
    let path = format!("/api/recipes/{}/share", r["id"].as_str().unwrap());
    let (s, sh) = t.req("POST", &path, Some(ann), None).await;
    assert_eq!(s, StatusCode::CREATED);
    // another household can neither see, create nor remove the link
    for method in ["GET", "POST", "DELETE"] {
        let (s, _) = t.req(method, &path, Some(ben), None).await;
        assert_eq!(s, StatusCode::NOT_FOUND, "{method}");
    }
    let (s, _, _) = t.page(&format!("/s/{}", sh["token"].as_str().unwrap())).await;
    assert_eq!(s, StatusCode::OK);
}

#[tokio::test]
async fn public_sharing_can_be_turned_off() {
    let t = TestApp::with_config(None, |c| c.public_sharing = false).await;
    let (_, cfg) = t.req("GET", "/api/config", None, None).await;
    assert_eq!(cfg["public_sharing"], false);
    let (_, a) = t.req("POST", "/api/auth/register", None, Some(json!({"username": "ann", "password": "password123"}))).await;
    let tok = a["access_token"].as_str().unwrap();
    let (_, r) = t.req("POST", "/api/recipes", Some(tok), Some(json!({"name": "Plum Cake"}))).await;
    let (s, e) = t.req("POST", &format!("/api/recipes/{}/share", r["id"].as_str().unwrap()), Some(tok), None).await;
    assert_eq!(s, StatusCode::FORBIDDEN, "{e}");
}
