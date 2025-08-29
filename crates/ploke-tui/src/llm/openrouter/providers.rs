use reqwest::Url;
use serde::{Deserialize, Serialize};

#[derive(Clone, PartialOrd, PartialEq, Debug, Serialize, Deserialize)]
pub struct ProvidersResponse {
    pub data: Vec<Provider>,
}

#[derive(Clone, PartialOrd, PartialEq, Debug, Serialize, Deserialize)]
pub struct Provider {
    pub name: ProviderName,
    pub privacy_policy_url: Option<Url>,
    pub slug: Slug,
    pub status_page_url: Option<Url>,
    pub terms_of_service_url: Option<Url>,
}

#[derive(Clone, Copy, PartialOrd, PartialEq, Debug, Serialize, Deserialize)]
#[allow(non_camel_case_types)]
pub enum ProviderName {
    #[serde(rename = "Z.AI")]
    ZdotAI,
    WandB,
    Kluster,
    Together,
    Cerebras,
    Venice,
    Morph,
    #[serde(rename = "Moonshot AI")]
    MoonshotAI,
    OpenAI,
    Stealth,
    SambaNova,
    AtlasCloud,
    #[serde(rename = "Amazon Bedrock")]
    AmazonBedrock,
    Groq,
    Featherless,
    NextBit,
    Atoma,
    AI21,
    Minimax,
    BaseTen,
    Mistral,
    Anthropic,
    Lambda,
    Hyperbolic,
    NCompass,
    Azure,
    DeepSeek,
    Crusoe,
    Cohere,
    Google,
    #[serde(rename = "Mancer 2")]
    Mancer2,
    Novita,
    Perplexity,
    Avian,
    SiliconFlow,
    Switchpoint,
    Inflection,
    Fireworks,
    xAI,
    #[serde(rename = "Google AI Studio")]
    GoogleAIStudio,
    Infermatic,
    InferenceNet,
    Inception,
    Nebius,
    Alibaba,
    Friendli,
    Chutes,
    Targon,
    Ubicloud,
    Cloudflare,
    AionLabs,
    Liquid,
    DeepInfra,
    Nineteen,
    Enfer,
    OpenInference,
    CrofAI,
    Phala,
    Meta,
    Parasail,
    GMICloud,
}
use std::fmt::{self, Display};

impl Display for ProviderName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // We know the enum serializes to a string, so this is cheap.
        let s = serde_json::to_string(self)
            .expect("ProviderName should always serialize to JSON string");
        // Remove the surrounding quotes that JSON adds.
        f.write_str(&s[1..s.len() - 1])
    }
}

impl ProviderName {
    pub fn has_slug(self, other: Slug) -> bool {
        self.to_slug() == other
    }

    pub fn to_slug(self) -> Slug {
        match self {
            ProviderName::ZdotAI => Slug::z_ai,
            ProviderName::WandB => Slug::wandb,
            ProviderName::Kluster => Slug::klusterai,
            ProviderName::Together => Slug::together,
            ProviderName::Cerebras => Slug::cerebras,
            ProviderName::Venice => Slug::venice,
            ProviderName::Morph => Slug::morph,
            ProviderName::MoonshotAI => Slug::moonshotai,
            ProviderName::OpenAI => Slug::openai,
            ProviderName::Stealth => Slug::stealth,
            ProviderName::SambaNova => Slug::sambanova,
            ProviderName::AtlasCloud => Slug::atlas_cloud,
            ProviderName::AmazonBedrock => Slug::amazon_bedrock,
            ProviderName::Groq => Slug::groq,
            ProviderName::Featherless => Slug::featherless,
            ProviderName::NextBit => Slug::nextbit,
            ProviderName::Atoma => Slug::atoma,
            ProviderName::AI21 => Slug::ai21,
            ProviderName::Minimax => Slug::minimax,
            ProviderName::BaseTen => Slug::baseten,
            ProviderName::Mistral => Slug::mistral,
            ProviderName::Anthropic => Slug::anthropic,
            ProviderName::Lambda => Slug::lambda,
            ProviderName::Hyperbolic => Slug::hyperbolic,
            ProviderName::NCompass => Slug::ncompass,
            ProviderName::Azure => Slug::azure,
            ProviderName::DeepSeek => Slug::deepseek,
            ProviderName::Crusoe => Slug::crusoe,
            ProviderName::Cohere => Slug::cohere,
            ProviderName::Google => Slug::google_vertex,
            ProviderName::Mancer2 => Slug::mancer,
            ProviderName::Novita => Slug::novita,
            ProviderName::Perplexity => Slug::perplexity,
            ProviderName::Avian => Slug::avian,
            ProviderName::SiliconFlow => Slug::siliconflow,
            ProviderName::Switchpoint => Slug::switchpoint,
            ProviderName::Inflection => Slug::inflection,
            ProviderName::Fireworks => Slug::fireworks,
            ProviderName::xAI => Slug::xai,
            ProviderName::GoogleAIStudio => Slug::google_ai_studio,
            ProviderName::Infermatic => Slug::infermatic,
            ProviderName::InferenceNet => Slug::inference_net,
            ProviderName::Inception => Slug::inception,
            ProviderName::Nebius => Slug::nebius,
            ProviderName::Alibaba => Slug::alibaba,
            ProviderName::Friendli => Slug::friendli,
            ProviderName::Chutes => Slug::chutes,
            ProviderName::Targon => Slug::targon,
            ProviderName::Ubicloud => Slug::ubicloud,
            ProviderName::Cloudflare => Slug::cloudflare,
            ProviderName::AionLabs => Slug::aion_labs,
            ProviderName::Liquid => Slug::liquid,
            ProviderName::DeepInfra => Slug::deepinfra,
            ProviderName::Nineteen => Slug::nineteen,
            ProviderName::Enfer => Slug::enfer,
            ProviderName::OpenInference => Slug::open_inference,
            ProviderName::CrofAI => Slug::crofai,
            ProviderName::Phala => Slug::phala,
            ProviderName::Meta => Slug::meta,
            ProviderName::Parasail => Slug::parasail,
            ProviderName::GMICloud => Slug::gmicloud,
        }
    }
}

#[derive(Clone, Copy, PartialOrd, PartialEq, Debug, Serialize, Deserialize)]
#[allow(non_camel_case_types)]
pub enum Slug {
    #[serde(rename = "z-ai")]
    z_ai,
    wandb,
    klusterai,
    together,
    cerebras,
    venice,
    morph,
    moonshotai,
    openai,
    stealth,
    sambanova,
    #[serde(rename = "atlas-cloud")]
    atlas_cloud,
    #[serde(rename = "amazon-bedrock")]
    amazon_bedrock,
    groq,
    featherless,
    nextbit,
    atoma,
    ai21,
    minimax,
    baseten,
    mistral,
    anthropic,
    lambda,
    hyperbolic,
    ncompass,
    azure,
    deepseek,
    crusoe,
    cohere,
    #[serde(rename = "google-vertex")]
    google_vertex,
    mancer,
    novita,
    perplexity,
    avian,
    siliconflow,
    switchpoint,
    inflection,
    fireworks,
    xai,
    #[serde(rename = "google-ai-studio")]
    google_ai_studio,
    infermatic,
    #[serde(rename = "inference-net")]
    inference_net,
    inception,
    nebius,
    alibaba,
    friendli,
    chutes,
    targon,
    ubicloud,
    cloudflare,
    #[serde(rename = "aion-labs")]
    aion_labs,
    liquid,
    deepinfra,
    nineteen,
    enfer,
    #[serde(rename = "open-inference")]
    open_inference,
    crofai,
    phala,
    meta,
    parasail,
    gmicloud,
}

impl Display for Slug {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // We know the enum serializes to a string, so this is cheap.
        let s = serde_json::to_string(self).expect("Slug should always serialize to JSON string");
        // Remove the surrounding quotes that JSON adds.
        f.write_str(&s[1..s.len() - 1])
    }
}

impl Slug {
    pub fn has_provider_name(self, other: ProviderName) -> bool {
        self.to_provider_name() == other
    }

    pub fn to_provider_name(self) -> ProviderName {
        match self {
            Slug::z_ai => ProviderName::ZdotAI,
            Slug::wandb => ProviderName::WandB,
            Slug::klusterai => ProviderName::Kluster,
            Slug::together => ProviderName::Together,
            Slug::cerebras => ProviderName::Cerebras,
            Slug::venice => ProviderName::Venice,
            Slug::morph => ProviderName::Morph,
            Slug::moonshotai => ProviderName::MoonshotAI,
            Slug::openai => ProviderName::OpenAI,
            Slug::stealth => ProviderName::Stealth,
            Slug::sambanova => ProviderName::SambaNova,
            Slug::atlas_cloud => ProviderName::AtlasCloud,
            Slug::amazon_bedrock => ProviderName::AmazonBedrock,
            Slug::groq => ProviderName::Groq,
            Slug::featherless => ProviderName::Featherless,
            Slug::nextbit => ProviderName::NextBit,
            Slug::atoma => ProviderName::Atoma,
            Slug::ai21 => ProviderName::AI21,
            Slug::minimax => ProviderName::Minimax,
            Slug::baseten => ProviderName::BaseTen,
            Slug::mistral => ProviderName::Mistral,
            Slug::anthropic => ProviderName::Anthropic,
            Slug::lambda => ProviderName::Lambda,
            Slug::hyperbolic => ProviderName::Hyperbolic,
            Slug::ncompass => ProviderName::NCompass,
            Slug::azure => ProviderName::Azure,
            Slug::deepseek => ProviderName::DeepSeek,
            Slug::crusoe => ProviderName::Crusoe,
            Slug::cohere => ProviderName::Cohere,
            Slug::google_vertex => ProviderName::Google,
            Slug::mancer => ProviderName::Mancer2,
            Slug::novita => ProviderName::Novita,
            Slug::perplexity => ProviderName::Perplexity,
            Slug::avian => ProviderName::Avian,
            Slug::siliconflow => ProviderName::SiliconFlow,
            Slug::switchpoint => ProviderName::Switchpoint,
            Slug::inflection => ProviderName::Inflection,
            Slug::fireworks => ProviderName::Fireworks,
            Slug::xai => ProviderName::xAI,
            Slug::google_ai_studio => ProviderName::GoogleAIStudio,
            Slug::infermatic => ProviderName::Infermatic,
            Slug::inference_net => ProviderName::InferenceNet,
            Slug::inception => ProviderName::Inception,
            Slug::nebius => ProviderName::Nebius,
            Slug::alibaba => ProviderName::Alibaba,
            Slug::friendli => ProviderName::Friendli,
            Slug::chutes => ProviderName::Chutes,
            Slug::targon => ProviderName::Targon,
            Slug::ubicloud => ProviderName::Ubicloud,
            Slug::cloudflare => ProviderName::Cloudflare,
            Slug::aion_labs => ProviderName::AionLabs,
            Slug::liquid => ProviderName::Liquid,
            Slug::deepinfra => ProviderName::DeepInfra,
            Slug::nineteen => ProviderName::Nineteen,
            Slug::enfer => ProviderName::Enfer,
            Slug::open_inference => ProviderName::OpenInference,
            Slug::crofai => ProviderName::CrofAI,
            Slug::phala => ProviderName::Phala,
            Slug::meta => ProviderName::Meta,
            Slug::parasail => ProviderName::Parasail,
            Slug::gmicloud => ProviderName::GMICloud,
        }
    }
}

mod tests {
    use reqwest::Client;
    use std::time::Duration;

    use crate::{
        llm::providers::ProvidersResponse,
        test_harness::{default_headers, openrouter_env},
        user_config::openrouter_url,
    };

    #[tokio::test]
    #[ignore = "Live, flakey test"]
    /// Flakey test to help notice when OpenRouter changes their provider list.
    async fn flakey_openrouter_providers() -> color_eyre::Result<()> {
        let client = Client::builder()
            .timeout(Duration::from_secs(5))
            .default_headers(default_headers())
            .build()
            .expect("client");
        let url = openrouter_url().join("providers").expect("Malformed Url");
        eprintln!("url: {}", url);
        let api_key = openrouter_env().expect("No key").key;

        let resp = client
            .get(url)
            .bearer_auth(&api_key)
            .send()
            .await
            .and_then(|r| r.error_for_status())?;
        let providers_response: ProvidersResponse = resp.json().await?;

        let count_providers = providers_response.data.iter().count();
        assert_eq!(61, count_providers);

        let count_tos = providers_response
            .data
            .iter()
            .filter(|p| p.privacy_policy_url.is_some())
            .count();
        assert_eq!(54, count_tos);

        let count_status_page = providers_response
            .data
            .iter()
            .filter(|p| p.status_page_url.is_some())
            .count();
        assert_eq!(27, count_status_page);

        let count_pp = providers_response
            .data
            .iter()
            .filter(|p| p.privacy_policy_url.is_some())
            .count();
        assert_eq!(54, count_pp);
        Ok(())
    }
}
