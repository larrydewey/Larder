//! Network helpers: page scraping and image downloads.

use std::time::Duration;

use reqwest::header::{ACCEPT, ACCEPT_LANGUAGE, CONTENT_TYPE, USER_AGENT as UA};

use crate::{image_extension, parse_html, ImportError, ImportedRecipe};

/// User-Agent sent with every outgoing request.
pub const USER_AGENT: &str = "Mozilla/5.0 (compatible; Larder/0.1; +https://github.com/larrydewey/larder)";
pub(crate) const TIMEOUT: Duration = Duration::from_secs(20);
/// Largest HTML page we read.
const MAX_PAGE_BYTES: usize = 10 * 1024 * 1024;
/// Largest image we download.
pub const MAX_IMAGE_BYTES: usize = 20 * 1024 * 1024;

pub(crate) fn check_url(url: &str) -> Result<url::Url, ImportError> {
    let u = url::Url::parse(url.trim()).map_err(|e| ImportError::InvalidUrl(format!("{url}: {e}")))?;
    if !matches!(u.scheme(), "http" | "https") || u.host_str().is_none() {
        return Err(ImportError::InvalidUrl(format!("{url}: only http(s) URLs are supported")));
    }
    Ok(u)
}

/// Reads a response body, failing once it exceeds `max` bytes.
pub(crate) async fn read_limited(mut resp: reqwest::Response, max: usize) -> Result<Vec<u8>, ImportError> {
    if resp.content_length().is_some_and(|l| l as usize > max) {
        return Err(ImportError::TooLarge(format!("response is larger than {} MB", max / 1024 / 1024)));
    }
    let mut buf = Vec::new();
    while let Some(chunk) = resp.chunk().await? {
        if buf.len() + chunk.len() > max {
            return Err(ImportError::TooLarge(format!("response is larger than {} MB", max / 1024 / 1024)));
        }
        buf.extend_from_slice(&chunk);
    }
    Ok(buf)
}

pub(crate) fn check_status(resp: &reqwest::Response) -> Result<(), ImportError> {
    let status = resp.status();
    if status.is_success() {
        return Ok(());
    }
    let mut u = resp.url().clone();
    u.set_query(None);
    Err(ImportError::Http { status: status.as_u16(), url: u.to_string() })
}

/// Fetches a page (browser-like headers, 20 s timeout, redirects as configured
/// on `client`) and extracts its recipe with [`parse_html`].
pub async fn scrape_url(client: &reqwest::Client, url: &str) -> Result<ImportedRecipe, ImportError> {
    let u = check_url(url)?;
    let resp = client
        .get(u)
        .header(UA, USER_AGENT)
        .header(ACCEPT, "text/html,application/xhtml+xml,application/xml;q=0.9,*/*;q=0.8")
        .header(ACCEPT_LANGUAGE, "en-US,en;q=0.9")
        .timeout(TIMEOUT)
        .send()
        .await?;
    check_status(&resp)?;
    let final_url = resp.url().to_string();
    let ct = resp.headers().get(CONTENT_TYPE).and_then(|v| v.to_str().ok()).unwrap_or("").to_ascii_lowercase();
    let charset = ct.split(';').find_map(|p| p.trim().strip_prefix("charset=")).map(|c| c.trim_matches('"').to_string());
    let body = read_limited(resp, MAX_PAGE_BYTES).await?;
    let html = decode_body(&body, charset.as_deref());
    // The page URL recorded is the final one after redirects.
    parse_html(&html, &final_url)
}

/// Decodes a body as UTF-8; Latin-1/Windows-1252 bodies are mapped byte-wise.
fn decode_body(body: &[u8], charset: Option<&str>) -> String {
    let latin = charset.is_some_and(|c| {
        let c = c.to_ascii_lowercase();
        c == "iso-8859-1" || c == "latin1" || c == "windows-1252" || c == "cp1252"
    });
    if latin {
        return body.iter().map(|&b| b as char).collect();
    }
    String::from_utf8_lossy(body).into_owned()
}

/// Downloads an image (≤ 20 MB) and verifies it is one, by content type or magic bytes.
pub async fn fetch_image(client: &reqwest::Client, url: &str) -> Result<Vec<u8>, ImportError> {
    fetch_image_with(client, url, None).await
}

pub(crate) async fn fetch_image_with(client: &reqwest::Client, url: &str, bearer: Option<&str>) -> Result<Vec<u8>, ImportError> {
    let u = check_url(url)?;
    let mut req = client
        .get(u)
        .header(UA, USER_AGENT)
        .header(ACCEPT, "image/avif,image/webp,image/png,image/jpeg,image/*;q=0.8,*/*;q=0.5")
        .timeout(TIMEOUT);
    if let Some(t) = bearer {
        req = req.bearer_auth(t);
    }
    let resp = req.send().await?;
    check_status(&resp)?;
    let ct = resp.headers().get(CONTENT_TYPE).and_then(|v| v.to_str().ok()).unwrap_or("").to_ascii_lowercase();
    let bytes = read_limited(resp, MAX_IMAGE_BYTES).await?;
    if bytes.is_empty() {
        return Err(ImportError::NotAnImage("empty response".into()));
    }
    let is_svg = ct.starts_with("image/svg");
    if image_extension(&bytes).is_some() || (ct.starts_with("image/") && !is_svg) {
        Ok(bytes)
    } else {
        Err(ImportError::NotAnImage(if ct.is_empty() { "unknown content type".into() } else { ct }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn url_checks() {
        assert!(check_url("https://example.com/a").is_ok());
        assert!(matches!(check_url("file:///etc/passwd"), Err(ImportError::InvalidUrl(_))));
        assert!(matches!(check_url("not a url"), Err(ImportError::InvalidUrl(_))));
    }

    #[test]
    fn latin1() {
        assert_eq!(decode_body(&[0x43, 0x72, 0xE8, 0x6D, 0x65], Some("ISO-8859-1")), "Crème");
    }
}
