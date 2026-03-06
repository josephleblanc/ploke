use ploke_core::ArcStr;
use reqwest::Url;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::collections::HashSet;
use std::convert::Infallible;
use std::fmt::{self, Display};
use std::str::FromStr;
use std::sync::{Mutex, OnceLock};

#[derive(Clone, PartialOrd, PartialEq, Debug, Serialize, Deserialize)]
pub struct ProvidersResponse {
    pub data: Vec<Provider>,
}

#[derive(Clone, PartialOrd, PartialEq, Debug, Serialize, Deserialize)]
pub struct Provider {
    pub name: ProviderName,
    #[serde(deserialize_with = "deserialize_optional_url")]
    pub privacy_policy_url: Option<Url>,
    pub slug: ProviderSlug,
    #[serde(deserialize_with = "deserialize_optional_url")]
    pub status_page_url: Option<Url>,
    #[serde(deserialize_with = "deserialize_optional_url")]
    pub terms_of_service_url: Option<Url>,
}

fn deserialize_optional_url<'de, D>(deserializer: D) -> Result<Option<Url>, D::Error>
where
    D: Deserializer<'de>,
{
    let raw = Option::<String>::deserialize(deserializer)?;
    let Some(value) = raw else {
        return Ok(None);
    };
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Ok(None);
    }
    match Url::parse(trimmed) {
        Ok(url) => Ok(Some(url)),
        Err(err) => {
            tracing::warn!(
                "openrouter provider entry had invalid URL '{}': {err}",
                trimmed
            );
            Ok(None)
        }
    }
}

// Deduplicated warning for newly encountered OpenRouter provider schema values.
fn log_unknown(category: &'static str, value: &str) {
    static SEEN: OnceLock<Mutex<HashSet<String>>> = OnceLock::new();
    let set = SEEN.get_or_init(|| Mutex::new(HashSet::new()));
    let mut guard = set.lock().expect("unknown-set mutex poisoned");
    let key = format!("{category}:{value}");
    if guard.insert(key) {
        tracing::warn!(
            category,
            value,
            "Unknown OpenRouter provider value encountered"
        );
    }
}

// Best-effort canonical mapping table. Unknown values are still accepted and preserved.
const KNOWN_PROVIDER_PAIRS: &[(&str, &str)] = &[
    ("ModelRun", "modelrun"),
    ("Z.AI", "z-ai"),
    ("WandB", "wandb"),
    ("Kluster", "klusterai"),
    ("Together", "together"),
    ("Cerebras", "cerebras"),
    ("Venice", "venice"),
    ("Morph", "morph"),
    ("Moonshot AI", "moonshotai"),
    ("OpenAI", "openai"),
    ("Stealth", "stealth"),
    ("SambaNova", "sambanova"),
    ("AtlasCloud", "atlas-cloud"),
    ("Amazon Bedrock", "amazon-bedrock"),
    ("Groq", "groq"),
    ("Featherless", "featherless"),
    ("NextBit", "nextbit"),
    ("Atoma", "atoma"),
    ("AI21", "ai21"),
    ("Minimax", "minimax"),
    ("BaseTen", "baseten"),
    ("Mistral", "mistral"),
    ("Anthropic", "anthropic"),
    ("Lambda", "lambda"),
    ("Hyperbolic", "hyperbolic"),
    ("NCompass", "ncompass"),
    ("Azure", "azure"),
    ("DeepSeek", "deepseek"),
    ("Crusoe", "crusoe"),
    ("Cohere", "cohere"),
    ("Google", "google-vertex"),
    ("Mancer 2", "mancer"),
    ("Novita", "novita"),
    ("Perplexity", "perplexity"),
    ("Avian", "avian"),
    ("SiliconFlow", "siliconflow"),
    ("Switchpoint", "switchpoint"),
    ("Inflection", "inflection"),
    ("Fireworks", "fireworks"),
    ("xAI", "xai"),
    ("Google AI Studio", "google-ai-studio"),
    ("Infermatic", "infermatic"),
    ("InferenceNet", "inference-net"),
    ("Inception", "inception"),
    ("Nebius", "nebius"),
    ("Alibaba", "alibaba"),
    ("Friendli", "friendli"),
    ("Chutes", "chutes"),
    ("Targon", "targon"),
    ("Ubicloud", "ubicloud"),
    ("Cloudflare", "cloudflare"),
    ("AionLabs", "aion-labs"),
    ("Liquid", "liquid"),
    ("DeepInfra", "deepinfra"),
    ("Nineteen", "nineteen"),
    ("Enfer", "enfer"),
    ("OpenInference", "open-inference"),
    ("CrofAI", "crofai"),
    ("Phala", "phala"),
    ("Meta", "meta"),
    ("Parasail", "parasail"),
    ("GMICloud", "gmicloud"),
    ("Nvidia", "nvidia"),
    ("Arcee AI", "arcee-ai"),
    ("BytePlus", "byteplus"),
    ("Black Forest Labs", "black-forest-labs"),
    ("StreamLake", "streamlake"),
    ("Amazon Nova", "amazon-nova"),
    ("GoPomelo", "gopomelo"),
    ("Relace", "relace"),
    ("FakeProvider", "fake-provider"),
    ("Cirrascale", "cirrascale"),
    ("Clarifai", "clarifai"),
    ("Modular", "modular"),
    ("Sourceful", "sourceful"),
    ("Mara", "mara"),
    ("Xiaomi", "xiaomi"),
];

fn canonical_slug_for_name(name: &str) -> Option<&'static str> {
    KNOWN_PROVIDER_PAIRS
        .iter()
        .find_map(|(n, s)| (*n == name).then_some(*s))
        .or_else(|| {
            KNOWN_PROVIDER_PAIRS
                .iter()
                .find_map(|(n, s)| n.eq_ignore_ascii_case(name).then_some(*s))
        })
}

fn canonical_name_for_slug(slug: &str) -> Option<&'static str> {
    KNOWN_PROVIDER_PAIRS
        .iter()
        .find_map(|(n, s)| (*s == slug).then_some(*n))
        .or_else(|| {
            KNOWN_PROVIDER_PAIRS
                .iter()
                .find_map(|(n, s)| s.eq_ignore_ascii_case(slug).then_some(*n))
        })
}

fn is_known_provider_name(name: &str) -> bool {
    canonical_slug_for_name(name).is_some()
}

fn is_known_provider_slug(slug: &str) -> bool {
    canonical_name_for_slug(slug).is_some()
}

#[derive(Clone, PartialOrd, PartialEq, Eq, Ord, Hash, Debug)]
pub struct ProviderName(ArcStr);

impl ProviderName {
    pub fn new(value: impl AsRef<str>) -> Self {
        Self(ArcStr::from(value.as_ref()))
    }

    pub fn as_str(&self) -> &str {
        self.0.as_ref()
    }

    pub fn has_slug(&self, other: &ProviderSlug) -> bool {
        self.to_slug().as_ref().is_some_and(|s| s == other)
    }

    // Best-effort normalization to canonical slug.
    pub fn to_slug(&self) -> Option<ProviderSlug> {
        canonical_slug_for_name(self.as_str()).map(ProviderSlug::new)
    }
}

impl Display for ProviderName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl Serialize for ProviderName {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(self.as_str())
    }
}

impl<'de> Deserialize<'de> for ProviderName {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let raw = String::deserialize(deserializer)?;
        if !is_known_provider_name(&raw) {
            log_unknown("provider_name", &raw);
        }
        Ok(Self::new(raw))
    }
}

impl FromStr for ProviderName {
    type Err = Infallible;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(Self::new(s))
    }
}

#[derive(Clone, PartialOrd, PartialEq, Eq, Ord, Hash, Debug)]
pub struct ProviderSlug(ArcStr);

impl ProviderSlug {
    pub fn new(value: impl AsRef<str>) -> Self {
        Self(ArcStr::from(value.as_ref()))
    }

    pub fn as_str(&self) -> &str {
        self.0.as_ref()
    }

    pub fn has_provider_name(&self, other: &ProviderName) -> bool {
        self.to_provider_name().as_ref().is_some_and(|n| n == other)
    }

    // Best-effort normalization to canonical display name.
    pub fn to_provider_name(&self) -> Option<ProviderName> {
        canonical_name_for_slug(self.as_str()).map(ProviderName::new)
    }
}

impl Display for ProviderSlug {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl Serialize for ProviderSlug {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(self.as_str())
    }
}

impl<'de> Deserialize<'de> for ProviderSlug {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let raw = String::deserialize(deserializer)?;
        if !is_known_provider_slug(&raw) {
            log_unknown("provider_slug", &raw);
        }
        Ok(Self::new(raw))
    }
}

impl FromStr for ProviderSlug {
    type Err = Infallible;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(Self::new(s))
    }
}

#[cfg(test)]
mod unit_tests {
    use super::*;

    #[test]
    fn provider_types_deserialize_unknown_values() {
        let js = serde_json::json!({
            "data": [{
                "name": "Inceptron",
                "privacy_policy_url": null,
                "slug": "inceptron",
                "status_page_url": "",
                "terms_of_service_url": null
            }]
        });

        let parsed: ProvidersResponse = serde_json::from_value(js).expect("deserialize providers");
        let p = &parsed.data[0];
        assert_eq!(p.name.as_str(), "Inceptron");
        assert_eq!(p.slug.as_str(), "inceptron");
    }

    #[test]
    fn provider_normalization_is_best_effort() {
        let known_name = ProviderName::new("OpenAI");
        let known_slug = known_name
            .to_slug()
            .expect("expected known name to normalize to slug");
        assert_eq!(known_slug.as_str(), "openai");

        let unknown_name = ProviderName::new("Inceptron");
        assert!(unknown_name.to_slug().is_none());

        let unknown_slug = ProviderSlug::new("inceptron");
        assert!(unknown_slug.to_provider_name().is_none());
    }
}

// TODO:ploke-llm
#[cfg(all(feature = "live_api_tests", feature = "flakey_test"))]
#[cfg(test)]
mod tests {
    use reqwest::Client;
    use std::time::Duration;

    use crate::router_only::openrouter::OpenRouter;
    use crate::router_only::{Router, openrouter::providers::ProvidersResponse};
    use crate::utils::test_helpers::default_headers;
    // use crate::{
    //     test_harness::{default_headers, openrouter_env},
    //     user_config::openrouter_url,
    // };

    #[tokio::test]
    /// Flakey test to help notice when OpenRouter changes their provider list.
    async fn flakey_openrouter_providers() -> color_eyre::Result<()> {
        let client = Client::builder()
            .timeout(Duration::from_secs(5))
            .default_headers(default_headers())
            .build()
            .expect("client");
        let url = OpenRouter::PROVIDERS_URL;
        eprintln!("url: {}", url);
        let api_key = OpenRouter::resolve_api_key()?;

        let resp = client
            .get(url)
            .bearer_auth(&api_key)
            .send()
            .await
            .and_then(|r| r.error_for_status())?;
        let body = resp.json::<serde_json::Value>().await?;
        {
            use std::fs;
            use std::time::{SystemTime, UNIX_EPOCH};
            let mut dir = ploke_test_utils::workspace_root();
            dir.push("crates/ploke-tui/data/providers");
            fs::create_dir_all(&dir).ok();
            let ts = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs();
            let mut path = dir;
            path.push(format!("providers-{}.json", ts));
            let f = fs::File::create(&path)?;
            serde_json::to_writer_pretty(f, &body)?;
            eprintln!("wrote providers JSON to {}", path.display());
        }
        let providers_response: ProvidersResponse = serde_json::from_value(body)?;

        let count_providers = providers_response.data.len();
        assert_eq!(68, count_providers);

        let count_tos = providers_response
            .data
            .iter()
            .filter(|p| p.privacy_policy_url.is_some())
            .count();
        assert_eq!(62, count_tos);

        let count_status_page = providers_response
            .data
            .iter()
            .filter(|p| p.status_page_url.is_some())
            .count();
        assert_eq!(24, count_status_page);

        let count_pp = providers_response
            .data
            .iter()
            .filter(|p| p.privacy_policy_url.is_some())
            .count();
        assert_eq!(62, count_pp);
        Ok(())
    }
}
