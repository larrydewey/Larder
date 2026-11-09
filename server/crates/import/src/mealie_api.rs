//! Live import from a Mealie (v1+) server through its REST API.

use reqwest::header::{ACCEPT, USER_AGENT as UA};
use reqwest::StatusCode;
use serde_json::Value;

use crate::mealie::recipe_from_mealie_json;
use crate::net::{check_status, fetch_image_with, read_limited, TIMEOUT, USER_AGENT};
use crate::util::{as_f64, text_of};
use crate::{ImageSource, ImportError, ImportedRecipe};

const PER_PAGE: usize = 50;
const MAX_PAGES: usize = 2_000;
const MAX_JSON_BYTES: usize = 20 * 1024 * 1024;

/// Client for a Mealie server, authenticated with an API token.
#[derive(Clone)]
pub struct MealieClient {
    client: reqwest::Client,
    base_url: url::Url,
    token: String,
}

impl std::fmt::Debug for MealieClient {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MealieClient").field("base_url", &self.base_url.as_str()).finish_non_exhaustive()
    }
}

impl MealieClient {
    /// `base_url` is the server root (e.g. `https://mealie.example.com`, a
    /// sub-path is kept); `token` is a Mealie API token.
    pub fn new(client: reqwest::Client, base_url: &str, token: &str) -> Result<Self, ImportError> {
        let mut base = crate::net::check_url(base_url)?;
        // Accept URLs pasted from the browser such as ".../g/home" or ".../api".
        let mut path = base.path().trim_end_matches('/').to_string();
        for suffix in ["/api", "/g/home"] {
            if let Some(p) = path.strip_suffix(suffix) {
                path = p.to_string();
            }
        }
        base.set_path(&format!("{path}/"));
        base.set_query(None);
        base.set_fragment(None);
        let token = token.trim().trim_start_matches("Bearer ").trim().to_string();
        if token.is_empty() {
            return Err(ImportError::InvalidUrl("a Mealie API token is required".into()));
        }
        Ok(MealieClient { client, base_url: base, token })
    }

    fn url(&self, segments: &[&str]) -> Result<url::Url, ImportError> {
        let mut u = self.base_url.clone();
        {
            let mut s = u.path_segments_mut().map_err(|_| ImportError::InvalidUrl(self.base_url.to_string()))?;
            s.pop_if_empty();
            s.extend(segments);
        }
        Ok(u)
    }

    async fn get_json(&self, url: url::Url) -> Result<Value, ImportError> {
        let resp = self
            .client
            .get(url)
            .header(UA, USER_AGENT)
            .header(ACCEPT, "application/json")
            .bearer_auth(&self.token)
            .timeout(TIMEOUT)
            .send()
            .await?;
        if matches!(resp.status(), StatusCode::UNAUTHORIZED | StatusCode::FORBIDDEN) {
            return Err(ImportError::Unauthorized);
        }
        check_status(&resp)?;
        let body = read_limited(resp, MAX_JSON_BYTES).await?;
        serde_json::from_slice(&body).map_err(|e| ImportError::InvalidFormat(format!("unexpected response from Mealie: {e}")))
    }

    /// Lists all recipe slugs (paginated `GET /api/recipes`).
    pub async fn list_slugs(&self) -> Result<Vec<String>, ImportError> {
        let mut slugs = vec![];
        let mut seen = std::collections::HashSet::new();
        for page in 1..=MAX_PAGES {
            let mut u = self.url(&["api", "recipes"])?;
            u.query_pairs_mut()
                .append_pair("page", &page.to_string())
                .append_pair("perPage", &PER_PAGE.to_string())
                .append_pair("orderBy", "created_at")
                .append_pair("orderDirection", "asc");
            let v = self.get_json(u).await?;
            let items = match &v {
                Value::Array(a) => a.as_slice(), // very old servers returned a bare list
                _ => v.get("items").and_then(Value::as_array).map(Vec::as_slice).unwrap_or(&[]),
            };
            let before = slugs.len();
            for it in items {
                if let Some(s) = text_of(it, &["slug"]) {
                    if seen.insert(s.clone()) {
                        slugs.push(s);
                    }
                }
            }
            let total_pages = ["total_pages", "totalPages"].iter().find_map(|k| v.get(*k)).and_then(as_f64).unwrap_or(0.0) as usize;
            if v.is_array() || items.is_empty() || slugs.len() == before || page >= total_pages {
                break;
            }
        }
        Ok(slugs)
    }

    /// Fetches one recipe and downloads its image (Mealie media may require the token).
    pub async fn fetch(&self, slug: &str) -> Result<ImportedRecipe, ImportError> {
        let v = self.get_json(self.url(&["api", "recipes", slug])?).await?;
        let mut r = recipe_from_mealie_json(&v);
        if r.data.source_name.is_none() {
            r.data.source_name = r.data.source_url.as_deref().and_then(crate::util::host_of).or_else(|| Some("Mealie".into()));
        }
        if let Some(id) = text_of(&v, &["id"]) {
            if v.get("image").is_some_and(|i| !i.is_null() && i != "") || v.get("image").is_none() {
                let u = self.url(&["api", "media", "recipes", &id, "images", "original.webp"])?;
                match fetch_image_with(&self.client, u.as_str(), Some(&self.token)).await {
                    Ok(b) => r.image = Some(ImageSource::Bytes(b)),
                    Err(e) => tracing::debug!(slug, "no Mealie image: {e}"),
                }
            }
        }
        Ok(r)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, Mutex};
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    #[test]
    fn base_url_normalisation() {
        let c = MealieClient::new(reqwest::Client::new(), "https://m.example/sub/g/home/", "tok").unwrap();
        assert_eq!(c.url(&["api", "recipes", "a b"]).unwrap().as_str(), "https://m.example/sub/api/recipes/a%20b");
        let c = MealieClient::new(reqwest::Client::new(), "http://m.example/api", "Bearer tok").unwrap();
        assert_eq!(c.url(&["api", "recipes"]).unwrap().as_str(), "http://m.example/api/recipes");
        assert_eq!(c.token, "tok");
        assert!(MealieClient::new(reqwest::Client::new(), "ftp://m.example", "t").is_err());
        assert!(MealieClient::new(reqwest::Client::new(), "https://m.example", " ").is_err());
    }

    /// Minimal HTTP/1.1 server on localhost serving canned responses (no external network).
    async fn serve(routes: Vec<(&'static str, &'static str, Vec<u8>)>) -> (String, Arc<Mutex<Vec<String>>>) {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let log = Arc::new(Mutex::new(vec![]));
        let log2 = log.clone();
        tokio::spawn(async move {
            loop {
                let Ok((mut sock, _)) = listener.accept().await else { break };
                let routes = routes.clone();
                let log = log2.clone();
                tokio::spawn(async move {
                    let mut buf = vec![0u8; 8192];
                    let n = sock.read(&mut buf).await.unwrap_or(0);
                    let req = String::from_utf8_lossy(&buf[..n]).to_string();
                    let path = req.split_whitespace().nth(1).unwrap_or("").to_string();
                    let authed = req.to_ascii_lowercase().contains("authorization: bearer secret");
                    log.lock().unwrap().push(path.clone());
                    let (status, ctype, body) = if !authed {
                        ("401 Unauthorized", "application/json", b"{}".to_vec())
                    } else if let Some((_, ct, b)) = routes.iter().find(|(p, _, _)| path.starts_with(p)) {
                        ("200 OK", *ct, b.clone())
                    } else {
                        ("404 Not Found", "text/plain", b"nope".to_vec())
                    };
                    let head = format!(
                        "HTTP/1.1 {status}\r\nContent-Type: {ctype}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                        body.len()
                    );
                    let _ = sock.write_all(head.as_bytes()).await;
                    let _ = sock.write_all(&body).await;
                });
            }
        });
        (format!("http://{addr}"), log)
    }

    #[tokio::test]
    async fn list_and_fetch_against_local_server() {
        let page1 = br#"{"page":1,"per_page":50,"total":3,"total_pages":2,"items":[{"slug":"a"},{"slug":"b"}]}"#.to_vec();
        let page2 = br#"{"page":2,"perPage":50,"total":3,"totalPages":2,"items":[{"slug":"c"}]}"#.to_vec();
        let recipe = br#"{"id":"abc-123","slug":"a","name":"Alpha","image":"x","recipeServings":2,
            "recipeIngredient":[{"note":"1 cup rice","display":"1 cup rice"}],
            "recipeInstructions":[{"text":"Cook rice."}],"tags":[{"name":"Easy"}]}"#
            .to_vec();
        let webp = b"RIFF\x10\x00\x00\x00WEBPVP8 data".to_vec();
        let (base, log) = serve(vec![
            ("/api/recipes?page=1", "application/json", page1),
            ("/api/recipes?page=2", "application/json", page2),
            ("/api/recipes/a", "application/json", recipe),
            ("/api/media/recipes/abc-123/images/original.webp", "image/webp", webp.clone()),
        ])
        .await;
        let http = reqwest::Client::builder().no_proxy().build().unwrap();
        let c = MealieClient::new(http.clone(), &base, "secret").unwrap();
        assert_eq!(c.list_slugs().await.unwrap(), vec!["a", "b", "c"]);
        let r = c.fetch("a").await.unwrap();
        assert_eq!(r.data.name, "Alpha");
        assert_eq!(r.data.yield_quantity, Some(2.0));
        assert_eq!(r.data.tags, vec!["Easy"]);
        assert_eq!(r.data.source_name.as_deref(), Some("Mealie"));
        assert!(matches!(r.image, Some(ImageSource::Bytes(ref b)) if *b == webp));
        assert!(log.lock().unwrap().iter().any(|p| p.contains("perPage=50")));

        let bad = MealieClient::new(http, &base, "wrong").unwrap();
        assert!(matches!(bad.list_slugs().await, Err(ImportError::Unauthorized)));
    }
}
