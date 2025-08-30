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
    pub slug: Author,
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
    pub fn has_slug(self, other: Author) -> bool {
        self.to_slug() == other
    }

    pub fn to_slug(self) -> Author {
        match self {
            ProviderName::ZdotAI => Author::z_ai,
            ProviderName::WandB => Author::wandb,
            ProviderName::Kluster => Author::klusterai,
            ProviderName::Together => Author::together,
            ProviderName::Cerebras => Author::cerebras,
            ProviderName::Venice => Author::venice,
            ProviderName::Morph => Author::morph,
            ProviderName::MoonshotAI => Author::moonshotai,
            ProviderName::OpenAI => Author::openai,
            ProviderName::Stealth => Author::stealth,
            ProviderName::SambaNova => Author::sambanova,
            ProviderName::AtlasCloud => Author::atlas_cloud,
            ProviderName::AmazonBedrock => Author::amazon_bedrock,
            ProviderName::Groq => Author::groq,
            ProviderName::Featherless => Author::featherless,
            ProviderName::NextBit => Author::nextbit,
            ProviderName::Atoma => Author::atoma,
            ProviderName::AI21 => Author::ai21,
            ProviderName::Minimax => Author::minimax,
            ProviderName::BaseTen => Author::baseten,
            ProviderName::Mistral => Author::mistral,
            ProviderName::Anthropic => Author::anthropic,
            ProviderName::Lambda => Author::lambda,
            ProviderName::Hyperbolic => Author::hyperbolic,
            ProviderName::NCompass => Author::ncompass,
            ProviderName::Azure => Author::azure,
            ProviderName::DeepSeek => Author::deepseek,
            ProviderName::Crusoe => Author::crusoe,
            ProviderName::Cohere => Author::cohere,
            ProviderName::Google => Author::google_vertex,
            ProviderName::Mancer2 => Author::mancer,
            ProviderName::Novita => Author::novita,
            ProviderName::Perplexity => Author::perplexity,
            ProviderName::Avian => Author::avian,
            ProviderName::SiliconFlow => Author::siliconflow,
            ProviderName::Switchpoint => Author::switchpoint,
            ProviderName::Inflection => Author::inflection,
            ProviderName::Fireworks => Author::fireworks,
            ProviderName::xAI => Author::xai,
            ProviderName::GoogleAIStudio => Author::google_ai_studio,
            ProviderName::Infermatic => Author::infermatic,
            ProviderName::InferenceNet => Author::inference_net,
            ProviderName::Inception => Author::inception,
            ProviderName::Nebius => Author::nebius,
            ProviderName::Alibaba => Author::alibaba,
            ProviderName::Friendli => Author::friendli,
            ProviderName::Chutes => Author::chutes,
            ProviderName::Targon => Author::targon,
            ProviderName::Ubicloud => Author::ubicloud,
            ProviderName::Cloudflare => Author::cloudflare,
            ProviderName::AionLabs => Author::aion_labs,
            ProviderName::Liquid => Author::liquid,
            ProviderName::DeepInfra => Author::deepinfra,
            ProviderName::Nineteen => Author::nineteen,
            ProviderName::Enfer => Author::enfer,
            ProviderName::OpenInference => Author::open_inference,
            ProviderName::CrofAI => Author::crofai,
            ProviderName::Phala => Author::phala,
            ProviderName::Meta => Author::meta,
            ProviderName::Parasail => Author::parasail,
            ProviderName::GMICloud => Author::gmicloud,
        }
    }
}

#[derive(Clone, Copy, PartialOrd, PartialEq, Debug, Serialize, Deserialize)]
#[allow(non_camel_case_types)]
// NOTE: These names are extremely confusing. What is referred to below as a "slug" is referred to
// in the request for the model endpoints as the "author", e.g.
// - https://openrouter.ai/api/v1/models/qwen/qwen3-30b-a3b-thinking-2507/endpoints
pub enum Author {
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

impl Display for Author {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // We know the enum serializes to a string, so this is cheap.
        let s = serde_json::to_string(self).expect("Author should always serialize to JSON string");
        // Remove the surrounding quotes that JSON adds.
        f.write_str(&s[1..s.len() - 1])
    }
}

impl Author {
    pub fn has_provider_name(self, other: ProviderName) -> bool {
        self.to_provider_name() == other
    }

    pub fn to_provider_name(self) -> ProviderName {
        match self {
            Author::z_ai => ProviderName::ZdotAI,
            Author::wandb => ProviderName::WandB,
            Author::klusterai => ProviderName::Kluster,
            Author::together => ProviderName::Together,
            Author::cerebras => ProviderName::Cerebras,
            Author::venice => ProviderName::Venice,
            Author::morph => ProviderName::Morph,
            Author::moonshotai => ProviderName::MoonshotAI,
            Author::openai => ProviderName::OpenAI,
            Author::stealth => ProviderName::Stealth,
            Author::sambanova => ProviderName::SambaNova,
            Author::atlas_cloud => ProviderName::AtlasCloud,
            Author::amazon_bedrock => ProviderName::AmazonBedrock,
            Author::groq => ProviderName::Groq,
            Author::featherless => ProviderName::Featherless,
            Author::nextbit => ProviderName::NextBit,
            Author::atoma => ProviderName::Atoma,
            Author::ai21 => ProviderName::AI21,
            Author::minimax => ProviderName::Minimax,
            Author::baseten => ProviderName::BaseTen,
            Author::mistral => ProviderName::Mistral,
            Author::anthropic => ProviderName::Anthropic,
            Author::lambda => ProviderName::Lambda,
            Author::hyperbolic => ProviderName::Hyperbolic,
            Author::ncompass => ProviderName::NCompass,
            Author::azure => ProviderName::Azure,
            Author::deepseek => ProviderName::DeepSeek,
            Author::crusoe => ProviderName::Crusoe,
            Author::cohere => ProviderName::Cohere,
            Author::google_vertex => ProviderName::Google,
            Author::mancer => ProviderName::Mancer2,
            Author::novita => ProviderName::Novita,
            Author::perplexity => ProviderName::Perplexity,
            Author::avian => ProviderName::Avian,
            Author::siliconflow => ProviderName::SiliconFlow,
            Author::switchpoint => ProviderName::Switchpoint,
            Author::inflection => ProviderName::Inflection,
            Author::fireworks => ProviderName::Fireworks,
            Author::xai => ProviderName::xAI,
            Author::google_ai_studio => ProviderName::GoogleAIStudio,
            Author::infermatic => ProviderName::Infermatic,
            Author::inference_net => ProviderName::InferenceNet,
            Author::inception => ProviderName::Inception,
            Author::nebius => ProviderName::Nebius,
            Author::alibaba => ProviderName::Alibaba,
            Author::friendli => ProviderName::Friendli,
            Author::chutes => ProviderName::Chutes,
            Author::targon => ProviderName::Targon,
            Author::ubicloud => ProviderName::Ubicloud,
            Author::cloudflare => ProviderName::Cloudflare,
            Author::aion_labs => ProviderName::AionLabs,
            Author::liquid => ProviderName::Liquid,
            Author::deepinfra => ProviderName::DeepInfra,
            Author::nineteen => ProviderName::Nineteen,
            Author::enfer => ProviderName::Enfer,
            Author::open_inference => ProviderName::OpenInference,
            Author::crofai => ProviderName::CrofAI,
            Author::phala => ProviderName::Phala,
            Author::meta => ProviderName::Meta,
            Author::parasail => ProviderName::Parasail,
            Author::gmicloud => ProviderName::GMICloud,
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

impl std::str::FromStr for Author {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "z-ai" => Ok(Author::z_ai),
            "wandb" => Ok(Author::wandb),
            "klusterai" => Ok(Author::klusterai),
            "together" => Ok(Author::together),
            "cerebras" => Ok(Author::cerebras),
            "venice" => Ok(Author::venice),
            "morph" => Ok(Author::morph),
            "moonshotai" => Ok(Author::moonshotai),
            "openai" => Ok(Author::openai),
            "stealth" => Ok(Author::stealth),
            "sambanova" => Ok(Author::sambanova),
            "atlas-cloud" => Ok(Author::atlas_cloud),
            "amazon-bedrock" => Ok(Author::amazon_bedrock),
            "groq" => Ok(Author::groq),
            "featherless" => Ok(Author::featherless),
            "nextbit" => Ok(Author::nextbit),
            "atoma" => Ok(Author::atoma),
            "ai21" => Ok(Author::ai21),
            "minimax" => Ok(Author::minimax),
            "baseten" => Ok(Author::baseten),
            "mistral" => Ok(Author::mistral),
            "anthropic" => Ok(Author::anthropic),
            "lambda" => Ok(Author::lambda),
            "hyperbolic" => Ok(Author::hyperbolic),
            "ncompass" => Ok(Author::ncompass),
            "azure" => Ok(Author::azure),
            "deepseek" => Ok(Author::deepseek),
            "crusoe" => Ok(Author::crusoe),
            "cohere" => Ok(Author::cohere),
            "google-vertex" => Ok(Author::google_vertex),
            "mancer" => Ok(Author::mancer),
            "novita" => Ok(Author::novita),
            "perplexity" => Ok(Author::perplexity),
            "avian" => Ok(Author::avian),
            "siliconflow" => Ok(Author::siliconflow),
            "switchpoint" => Ok(Author::switchpoint),
            "inflection" => Ok(Author::inflection),
            "fireworks" => Ok(Author::fireworks),
            "xai" => Ok(Author::xai),
            "google-ai-studio" => Ok(Author::google_ai_studio),
            "infermatic" => Ok(Author::infermatic),
            "inference-net" => Ok(Author::inference_net),
            "inception" => Ok(Author::inception),
            "nebius" => Ok(Author::nebius),
            "alibaba" => Ok(Author::alibaba),
            "friendli" => Ok(Author::friendli),
            "chutes" => Ok(Author::chutes),
            "targon" => Ok(Author::targon),
            "ubicloud" => Ok(Author::ubicloud),
            "cloudflare" => Ok(Author::cloudflare),
            "aion-labs" => Ok(Author::aion_labs),
            "liquid" => Ok(Author::liquid),
            "deepinfra" => Ok(Author::deepinfra),
            "nineteen" => Ok(Author::nineteen),
            "enfer" => Ok(Author::enfer),
            "open-inference" => Ok(Author::open_inference),
            "crofai" => Ok(Author::crofai),
            "phala" => Ok(Author::phala),
            "meta" => Ok(Author::meta),
            "parasail" => Ok(Author::parasail),
            "gmicloud" => Ok(Author::gmicloud),
            _ => Err(()),
        }
    }
}
