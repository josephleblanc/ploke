use reqwest::Url;
use serde::{Deserialize, Deserializer, Serialize};

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
            log::warn!(
                "openrouter provider entry had invalid URL '{}': {err}",
                trimmed
            );
            Ok(None)
        }
    }
}

#[derive(Clone, Copy, PartialOrd, PartialEq, Debug, Serialize, Deserialize)]
#[allow(non_camel_case_types)]
pub enum ProviderName {
    Sourceful,
    ModelRun,
    Modular,
    Clarifai,
    Cirrascale,
    FakeProvider,
    Relace,
    GoPomelo,
    #[serde(rename = "Amazon Nova")]
    AmazonNova,
    StreamLake,
    #[serde(rename = "Black Forest Labs")]
    BlackForestLabs,
    #[serde(rename = "Arcee AI")]
    ArceeAI,
    BytePlus,
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
    Nvidia,
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
    Mara,
    Xiaomi,
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
    pub fn as_str(&self) -> &'static str {
        match self {
            ProviderName::ModelRun => "ModelRun",
            ProviderName::ZdotAI => "Z.AI",
            ProviderName::WandB => "WandB",
            ProviderName::Kluster => "Kluster",
            ProviderName::Together => "Together",
            ProviderName::Cerebras => "Cerebras",
            ProviderName::Venice => "Venice",
            ProviderName::Morph => "Morph",
            ProviderName::MoonshotAI => "Moonshot AI",
            ProviderName::OpenAI => "OpenAI",
            ProviderName::Stealth => "Stealth",
            ProviderName::SambaNova => "SambaNova",
            ProviderName::AtlasCloud => "AtlasCloud",
            ProviderName::AmazonBedrock => "Amazon Bedrock",
            ProviderName::Groq => "Groq",
            ProviderName::Featherless => "Featherless",
            ProviderName::NextBit => "NextBit",
            ProviderName::Atoma => "Atoma",
            ProviderName::AI21 => "AI21",
            ProviderName::Minimax => "Minimax",
            ProviderName::BaseTen => "BaseTen",
            ProviderName::Mistral => "Mistral",
            ProviderName::Anthropic => "Anthropic",
            ProviderName::Lambda => "Lambda",
            ProviderName::Hyperbolic => "Hyperbolic",
            ProviderName::NCompass => "NCompass",
            ProviderName::Azure => "Azure",
            ProviderName::DeepSeek => "DeepSeek",
            ProviderName::Crusoe => "Crusoe",
            ProviderName::Cohere => "Cohere",
            ProviderName::Google => "Google",
            ProviderName::Mancer2 => "Mancer 2",
            ProviderName::Novita => "Novita",
            ProviderName::Perplexity => "Perplexity",
            ProviderName::Avian => "Avian",
            ProviderName::SiliconFlow => "SiliconFlow",
            ProviderName::Switchpoint => "Switchpoint",
            ProviderName::Inflection => "Inflection",
            ProviderName::Fireworks => "Fireworks",
            ProviderName::xAI => "xAI",
            ProviderName::GoogleAIStudio => "Google AI Studio",
            ProviderName::Infermatic => "Infermatic",
            ProviderName::InferenceNet => "InferenceNet",
            ProviderName::Inception => "Inception",
            ProviderName::Nebius => "Nebius",
            ProviderName::Nvidia => "Nvidia",
            ProviderName::Alibaba => "Alibaba",
            ProviderName::Friendli => "Friendli",
            ProviderName::Chutes => "Chutes",
            ProviderName::Targon => "Targon",
            ProviderName::Ubicloud => "Ubicloud",
            ProviderName::Cloudflare => "Cloudflare",
            ProviderName::AionLabs => "AionLabs",
            ProviderName::Liquid => "Liquid",
            ProviderName::DeepInfra => "DeepInfra",
            ProviderName::Nineteen => "Nineteen",
            ProviderName::Enfer => "Enfer",
            ProviderName::OpenInference => "OpenInference",
            ProviderName::CrofAI => "CrofAI",
            ProviderName::Phala => "Phala",
            ProviderName::Meta => "Meta",
            ProviderName::Parasail => "Parasail",
            ProviderName::GMICloud => "GMICloud",
            ProviderName::ArceeAI => "Arcee AI",
            ProviderName::BytePlus => "BytePlus",
            ProviderName::BlackForestLabs => "Black Forest Labs",
            ProviderName::StreamLake => "StreamLake",
            ProviderName::AmazonNova => "Amazon Nova",
            ProviderName::GoPomelo => "GoPomelo",
            ProviderName::Relace => "Relace",
            ProviderName::FakeProvider => "FakeProvider",
            ProviderName::Cirrascale => "Cirrascale",
            ProviderName::Clarifai => "Clarifai",
            ProviderName::Modular => "Modular",
            ProviderName::Sourceful => "Sourceful",
            ProviderName::Mara => "Mara",
            ProviderName::Xiaomi => "Xiaomi",
        }
    }
    pub fn has_slug(self, other: ProviderSlug) -> bool {
        self.to_slug() == other
    }

    pub fn to_slug(self) -> ProviderSlug {
        match self {
            ProviderName::Modular => ProviderSlug::modular,
            ProviderName::Clarifai => ProviderSlug::clarifai,
            ProviderName::StreamLake => ProviderSlug::streamlake,
            ProviderName::ZdotAI => ProviderSlug::z_ai,
            ProviderName::WandB => ProviderSlug::wandb,
            ProviderName::Kluster => ProviderSlug::klusterai,
            ProviderName::Together => ProviderSlug::together,
            ProviderName::Cerebras => ProviderSlug::cerebras,
            ProviderName::Venice => ProviderSlug::venice,
            ProviderName::Morph => ProviderSlug::morph,
            ProviderName::MoonshotAI => ProviderSlug::moonshotai,
            ProviderName::OpenAI => ProviderSlug::openai,
            ProviderName::Stealth => ProviderSlug::stealth,
            ProviderName::SambaNova => ProviderSlug::sambanova,
            ProviderName::AtlasCloud => ProviderSlug::atlas_cloud,
            ProviderName::AmazonBedrock => ProviderSlug::amazon_bedrock,
            ProviderName::Groq => ProviderSlug::groq,
            ProviderName::Featherless => ProviderSlug::featherless,
            ProviderName::NextBit => ProviderSlug::nextbit,
            ProviderName::Atoma => ProviderSlug::atoma,
            ProviderName::AI21 => ProviderSlug::ai21,
            ProviderName::Minimax => ProviderSlug::minimax,
            ProviderName::BaseTen => ProviderSlug::baseten,
            ProviderName::Mistral => ProviderSlug::mistral,
            ProviderName::Anthropic => ProviderSlug::anthropic,
            ProviderName::Lambda => ProviderSlug::lambda,
            ProviderName::Hyperbolic => ProviderSlug::hyperbolic,
            ProviderName::NCompass => ProviderSlug::ncompass,
            ProviderName::Azure => ProviderSlug::azure,
            ProviderName::DeepSeek => ProviderSlug::deepseek,
            ProviderName::Crusoe => ProviderSlug::crusoe,
            ProviderName::Cohere => ProviderSlug::cohere,
            ProviderName::Google => ProviderSlug::google_vertex,
            ProviderName::Mancer2 => ProviderSlug::mancer,
            ProviderName::Novita => ProviderSlug::novita,
            ProviderName::Perplexity => ProviderSlug::perplexity,
            ProviderName::Avian => ProviderSlug::avian,
            ProviderName::SiliconFlow => ProviderSlug::siliconflow,
            ProviderName::Switchpoint => ProviderSlug::switchpoint,
            ProviderName::Inflection => ProviderSlug::inflection,
            ProviderName::Fireworks => ProviderSlug::fireworks,
            ProviderName::xAI => ProviderSlug::xai,
            ProviderName::GoogleAIStudio => ProviderSlug::google_ai_studio,
            ProviderName::Infermatic => ProviderSlug::infermatic,
            ProviderName::InferenceNet => ProviderSlug::inference_net,
            ProviderName::Inception => ProviderSlug::inception,
            ProviderName::Nebius => ProviderSlug::nebius,
            ProviderName::Alibaba => ProviderSlug::alibaba,
            ProviderName::Friendli => ProviderSlug::friendli,
            ProviderName::Chutes => ProviderSlug::chutes,
            ProviderName::Targon => ProviderSlug::targon,
            ProviderName::Ubicloud => ProviderSlug::ubicloud,
            ProviderName::Cloudflare => ProviderSlug::cloudflare,
            ProviderName::AionLabs => ProviderSlug::aion_labs,
            ProviderName::Liquid => ProviderSlug::liquid,
            ProviderName::DeepInfra => ProviderSlug::deepinfra,
            ProviderName::Nineteen => ProviderSlug::nineteen,
            ProviderName::Enfer => ProviderSlug::enfer,
            ProviderName::OpenInference => ProviderSlug::open_inference,
            ProviderName::CrofAI => ProviderSlug::crofai,
            ProviderName::Phala => ProviderSlug::phala,
            ProviderName::Meta => ProviderSlug::meta,
            ProviderName::Parasail => ProviderSlug::parasail,
            ProviderName::GMICloud => ProviderSlug::gmicloud,
            ProviderName::Nvidia => ProviderSlug::nvidia,
            ProviderName::ArceeAI => ProviderSlug::arceeai,
            ProviderName::BytePlus => ProviderSlug::byteplus,
            ProviderName::BlackForestLabs => ProviderSlug::blackforestlabs,
            ProviderName::AmazonNova => ProviderSlug::amazon_nova,
            ProviderName::GoPomelo => ProviderSlug::gopomelo,
            ProviderName::Relace => ProviderSlug::relace,
            ProviderName::FakeProvider => ProviderSlug::fake_provider,
            ProviderName::Cirrascale => ProviderSlug::cirrascale,
            ProviderName::ModelRun => ProviderSlug::model_run,
            ProviderName::Sourceful => ProviderSlug::sourceful,
            ProviderName::Mara => ProviderSlug::mara,
            ProviderName::Xiaomi => ProviderSlug::xiaomi,
        }
    }
}

#[derive(Clone, Copy, PartialOrd, PartialEq, Debug, Serialize, Deserialize)]
#[allow(non_camel_case_types)]
pub enum ProviderSlug {
    sourceful,
    #[serde(rename = "modelrun")]
    model_run,
    modular,
    clarifai,
    cirrascale,
    #[serde(rename = "fake-provider")]
    fake_provider,
    relace,
    gopomelo,
    #[serde(rename = "amazon-nova")]
    amazon_nova,
    streamlake,
    #[serde(rename = "black-forest-labs")]
    blackforestlabs,
    #[serde(rename = "arcee-ai")]
    arceeai,
    byteplus,
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
    nvidia,
    mara,
    xiaomi,
}

impl Display for ProviderSlug {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // We know the enum serializes to a string, so this is cheap.
        let s = serde_json::to_string(self)
            .expect("ProviderSlug should always serialize to JSON string");
        // Remove the surrounding quotes that JSON adds.
        f.write_str(&s[1..s.len() - 1])
    }
}

impl ProviderSlug {
    pub fn has_provider_name(self, other: ProviderName) -> bool {
        self.to_provider_name() == other
    }

    pub fn to_provider_name(self) -> ProviderName {
        match self {
            ProviderSlug::z_ai => ProviderName::ZdotAI,
            ProviderSlug::wandb => ProviderName::WandB,
            ProviderSlug::klusterai => ProviderName::Kluster,
            ProviderSlug::together => ProviderName::Together,
            ProviderSlug::cerebras => ProviderName::Cerebras,
            ProviderSlug::venice => ProviderName::Venice,
            ProviderSlug::morph => ProviderName::Morph,
            ProviderSlug::moonshotai => ProviderName::MoonshotAI,
            ProviderSlug::openai => ProviderName::OpenAI,
            ProviderSlug::stealth => ProviderName::Stealth,
            ProviderSlug::sambanova => ProviderName::SambaNova,
            ProviderSlug::atlas_cloud => ProviderName::AtlasCloud,
            ProviderSlug::amazon_bedrock => ProviderName::AmazonBedrock,
            ProviderSlug::groq => ProviderName::Groq,
            ProviderSlug::featherless => ProviderName::Featherless,
            ProviderSlug::nextbit => ProviderName::NextBit,
            ProviderSlug::atoma => ProviderName::Atoma,
            ProviderSlug::ai21 => ProviderName::AI21,
            ProviderSlug::minimax => ProviderName::Minimax,
            ProviderSlug::baseten => ProviderName::BaseTen,
            ProviderSlug::mistral => ProviderName::Mistral,
            ProviderSlug::anthropic => ProviderName::Anthropic,
            ProviderSlug::lambda => ProviderName::Lambda,
            ProviderSlug::hyperbolic => ProviderName::Hyperbolic,
            ProviderSlug::ncompass => ProviderName::NCompass,
            ProviderSlug::azure => ProviderName::Azure,
            ProviderSlug::deepseek => ProviderName::DeepSeek,
            ProviderSlug::crusoe => ProviderName::Crusoe,
            ProviderSlug::cohere => ProviderName::Cohere,
            ProviderSlug::google_vertex => ProviderName::Google,
            ProviderSlug::mancer => ProviderName::Mancer2,
            ProviderSlug::novita => ProviderName::Novita,
            ProviderSlug::perplexity => ProviderName::Perplexity,
            ProviderSlug::avian => ProviderName::Avian,
            ProviderSlug::siliconflow => ProviderName::SiliconFlow,
            ProviderSlug::switchpoint => ProviderName::Switchpoint,
            ProviderSlug::inflection => ProviderName::Inflection,
            ProviderSlug::fireworks => ProviderName::Fireworks,
            ProviderSlug::xai => ProviderName::xAI,
            ProviderSlug::google_ai_studio => ProviderName::GoogleAIStudio,
            ProviderSlug::infermatic => ProviderName::Infermatic,
            ProviderSlug::inference_net => ProviderName::InferenceNet,
            ProviderSlug::inception => ProviderName::Inception,
            ProviderSlug::nebius => ProviderName::Nebius,
            ProviderSlug::alibaba => ProviderName::Alibaba,
            ProviderSlug::friendli => ProviderName::Friendli,
            ProviderSlug::chutes => ProviderName::Chutes,
            ProviderSlug::targon => ProviderName::Targon,
            ProviderSlug::ubicloud => ProviderName::Ubicloud,
            ProviderSlug::cloudflare => ProviderName::Cloudflare,
            ProviderSlug::aion_labs => ProviderName::AionLabs,
            ProviderSlug::liquid => ProviderName::Liquid,
            ProviderSlug::deepinfra => ProviderName::DeepInfra,
            ProviderSlug::nineteen => ProviderName::Nineteen,
            ProviderSlug::enfer => ProviderName::Enfer,
            ProviderSlug::open_inference => ProviderName::OpenInference,
            ProviderSlug::crofai => ProviderName::CrofAI,
            ProviderSlug::phala => ProviderName::Phala,
            ProviderSlug::meta => ProviderName::Meta,
            ProviderSlug::parasail => ProviderName::Parasail,
            ProviderSlug::gmicloud => ProviderName::GMICloud,
            ProviderSlug::nvidia => ProviderName::Nvidia,
            ProviderSlug::arceeai => ProviderName::ArceeAI,
            ProviderSlug::byteplus => ProviderName::BytePlus,
            ProviderSlug::blackforestlabs => ProviderName::BlackForestLabs,
            ProviderSlug::streamlake => ProviderName::StreamLake,
            ProviderSlug::amazon_nova => ProviderName::AmazonNova,
            ProviderSlug::gopomelo => ProviderName::GoPomelo,
            ProviderSlug::relace => ProviderName::Relace,
            ProviderSlug::fake_provider => ProviderName::FakeProvider,
            ProviderSlug::cirrascale => ProviderName::Cirrascale,
            ProviderSlug::clarifai => ProviderName::Clarifai,
            ProviderSlug::modular => ProviderName::Modular,
            ProviderSlug::model_run => ProviderName::ModelRun,
            ProviderSlug::sourceful => ProviderName::Sourceful,
            ProviderSlug::mara => ProviderName::Mara,
            ProviderSlug::xiaomi => ProviderName::Xiaomi,
        }
    }
}

impl std::str::FromStr for ProviderSlug {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "modelrun" => Ok(ProviderSlug::model_run),
            "modular" => Ok(ProviderSlug::modular),
            "clarifai" => Ok(ProviderSlug::clarifai),
            "cirrascale" => Ok(ProviderSlug::cirrascale),
            "fake-provider" => Ok(ProviderSlug::fake_provider),
            "relace" => Ok(ProviderSlug::relace),
            "gopomelo" => Ok(ProviderSlug::gopomelo),
            "streamlake" => Ok(ProviderSlug::streamlake),
            "black-forest-labs" => Ok(ProviderSlug::blackforestlabs),
            "z-ai" => Ok(ProviderSlug::z_ai),
            "wandb" => Ok(ProviderSlug::wandb),
            "klusterai" => Ok(ProviderSlug::klusterai),
            "together" => Ok(ProviderSlug::together),
            "cerebras" => Ok(ProviderSlug::cerebras),
            "venice" => Ok(ProviderSlug::venice),
            "morph" => Ok(ProviderSlug::morph),
            "moonshotai" => Ok(ProviderSlug::moonshotai),
            "openai" => Ok(ProviderSlug::openai),
            "stealth" => Ok(ProviderSlug::stealth),
            "sambanova" => Ok(ProviderSlug::sambanova),
            "atlas-cloud" => Ok(ProviderSlug::atlas_cloud),
            "amazon-nova" => Ok(ProviderSlug::amazon_nova),
            "amazon-bedrock" => Ok(ProviderSlug::amazon_bedrock),
            "groq" => Ok(ProviderSlug::groq),
            "featherless" => Ok(ProviderSlug::featherless),
            "nextbit" => Ok(ProviderSlug::nextbit),
            "atoma" => Ok(ProviderSlug::atoma),
            "ai21" => Ok(ProviderSlug::ai21),
            "minimax" => Ok(ProviderSlug::minimax),
            "baseten" => Ok(ProviderSlug::baseten),
            "mistral" => Ok(ProviderSlug::mistral),
            "anthropic" => Ok(ProviderSlug::anthropic),
            "lambda" => Ok(ProviderSlug::lambda),
            "hyperbolic" => Ok(ProviderSlug::hyperbolic),
            "ncompass" => Ok(ProviderSlug::ncompass),
            "azure" => Ok(ProviderSlug::azure),
            "deepseek" => Ok(ProviderSlug::deepseek),
            "crusoe" => Ok(ProviderSlug::crusoe),
            "cohere" => Ok(ProviderSlug::cohere),
            "google-vertex" => Ok(ProviderSlug::google_vertex),
            "mancer" => Ok(ProviderSlug::mancer),
            "novita" => Ok(ProviderSlug::novita),
            "perplexity" => Ok(ProviderSlug::perplexity),
            "avian" => Ok(ProviderSlug::avian),
            "siliconflow" => Ok(ProviderSlug::siliconflow),
            "switchpoint" => Ok(ProviderSlug::switchpoint),
            "inflection" => Ok(ProviderSlug::inflection),
            "fireworks" => Ok(ProviderSlug::fireworks),
            "xai" => Ok(ProviderSlug::xai),
            "google-ai-studio" => Ok(ProviderSlug::google_ai_studio),
            "infermatic" => Ok(ProviderSlug::infermatic),
            "inference-net" => Ok(ProviderSlug::inference_net),
            "inception" => Ok(ProviderSlug::inception),
            "nebius" => Ok(ProviderSlug::nebius),
            "alibaba" => Ok(ProviderSlug::alibaba),
            "friendli" => Ok(ProviderSlug::friendli),
            "chutes" => Ok(ProviderSlug::chutes),
            "targon" => Ok(ProviderSlug::targon),
            "ubicloud" => Ok(ProviderSlug::ubicloud),
            "cloudflare" => Ok(ProviderSlug::cloudflare),
            "aion-labs" => Ok(ProviderSlug::aion_labs),
            "liquid" => Ok(ProviderSlug::liquid),
            "deepinfra" => Ok(ProviderSlug::deepinfra),
            "nineteen" => Ok(ProviderSlug::nineteen),
            "enfer" => Ok(ProviderSlug::enfer),
            "open-inference" => Ok(ProviderSlug::open_inference),
            "crofai" => Ok(ProviderSlug::crofai),
            "phala" => Ok(ProviderSlug::phala),
            "meta" => Ok(ProviderSlug::meta),
            "parasail" => Ok(ProviderSlug::parasail),
            "gmicloud" => Ok(ProviderSlug::gmicloud),
            "nvidia" => Ok(ProviderSlug::nvidia),
            "arcee-ai" => Ok(ProviderSlug::arceeai),
            "byteplus" => Ok(ProviderSlug::byteplus),
            "sourceful" => Ok(ProviderSlug::sourceful),
            "mara" => Ok(ProviderSlug::mara),
            "xiaomi" => Ok(ProviderSlug::xiaomi),
            _ => Err(()),
        }
    }
}

mod tests {
    use reqwest::Client;
    use std::time::Duration;

    use crate::llm::router_only::openrouter::OpenRouter;
    use crate::llm::router_only::{Router, openrouter::providers::ProvidersResponse};
    use crate::{
        test_harness::{default_headers, openrouter_env},
        user_config::openrouter_url,
    };

    #[tokio::test]
    #[cfg(feature = "live_api_tests")]
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
        assert_eq!(66, count_providers);

        let count_tos = providers_response
            .data
            .iter()
            .filter(|p| p.privacy_policy_url.is_some())
            .count();
        assert_eq!(60, count_tos);

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
        assert_eq!(60, count_pp);
        Ok(())
    }
}
