#![allow(
    dead_code,
    reason = "evolving api surface, may be useful, written 2025-12-15"
)]

use once_cell::sync::OnceCell;
use reqwest::header::{HeaderMap, HeaderName, HeaderValue};
use url::Url;

pub fn default_headers() -> HeaderMap {
    let mut headers = HeaderMap::new();
    let referer = HeaderName::from_static("http-referer");
    let x_title = HeaderName::from_static("x-title");
    headers.insert(
        referer,
        HeaderValue::from_static("https://github.com/ploke-ai/ploke"),
    );
    headers.insert(x_title, HeaderValue::from_static("Ploke TUI E2E Tests"));
    headers
}

pub struct OpenRouterEnv {
    pub key: String,
    pub base_url: reqwest::Url,
}
impl OpenRouterEnv {
    pub fn new(key: String, base_url: reqwest::Url) -> Self {
        Self { key, base_url }
    }
}

pub fn openrouter_env() -> Option<OpenRouterEnv> {
    // Try current process env first; if missing, load from .env as a fallback
    let key_opt = std::env::var("OPENROUTER_API_KEY").ok();
    let key = match key_opt {
        Some(k) if !k.trim().is_empty() => k,
        _ => {
            let _ = dotenvy::dotenv();
            let k = std::env::var("OPENROUTER_API_KEY").ok()?;
            if k.trim().is_empty() {
                return None;
            }
            k
        }
    };
    Some(OpenRouterEnv::new(key, openrouter_url()))
}

pub static OPENROUTER_URL: OnceCell<url::Url> = OnceCell::new();

use crate::Router;
pub fn openrouter_url() -> Url {
    OPENROUTER_URL
        .get_or_init(|| {
            let base_url_str = crate::router_only::openrouter::OpenRouter::BASE_URL;
            Url::parse(base_url_str).expect("Failed to parse OpenRouter BASE_URL into a valid URL")
        })
        .clone()
}
