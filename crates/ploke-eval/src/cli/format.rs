use std::sync::OnceLock;

use regex::Regex;

pub(crate) fn display_context_length(item: &ploke_llm::request::models::ResponseItem) -> String {
    item.context_length
        .or(item.top_provider.context_length)
        .map(|value| value.to_string())
        .unwrap_or_default()
}

pub(crate) fn display_price_per_million(value: f64) -> String {
    format!("${:.2}/M", value * 1_000_000.0)
}

pub(crate) fn model_size_string(item: &ploke_llm::request::models::ResponseItem) -> String {
    extract_model_size(item.description.as_ref())
        .or_else(|| extract_model_size(item.name.as_str()))
        .unwrap_or_default()
}

pub(crate) fn extract_model_size(text: &str) -> Option<String> {
    static MIXTURE_RE: OnceLock<Regex> = OnceLock::new();
    static BILLION_PARAMS_RE: OnceLock<Regex> = OnceLock::new();
    static MILLION_PARAMS_RE: OnceLock<Regex> = OnceLock::new();
    static SUFFIX_RE: OnceLock<Regex> = OnceLock::new();

    let mix_match = MIXTURE_RE
        .get_or_init(|| Regex::new(r"(?i)\b\d+x\d+(?:\.\d+)?[BM]\b").expect("valid regex"))
        .find(text)
        .map(|m| m.as_str().to_string());
    if mix_match.is_some() {
        return mix_match;
    }

    if let Some(caps) = BILLION_PARAMS_RE
        .get_or_init(|| {
            Regex::new(r"(?i)\b(\d+(?:\.\d+)?)\s*billion\s+parameters?\b").expect("valid regex")
        })
        .captures(text)
    {
        return Some(format!("{}B", &caps[1]));
    }

    if let Some(caps) = MILLION_PARAMS_RE
        .get_or_init(|| {
            Regex::new(r"(?i)\b(\d+(?:\.\d+)?)\s*million\s+parameters?\b").expect("valid regex")
        })
        .captures(text)
    {
        return Some(format!("{}M", &caps[1]));
    }

    SUFFIX_RE
        .get_or_init(|| Regex::new(r"(?i)\b\d+(?:\.\d+)?[BM]\b").expect("valid regex"))
        .find(text)
        .map(|m| m.as_str().to_string())
}
