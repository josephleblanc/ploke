use serde::{Deserialize, Serialize};

#[derive(Clone, PartialOrd, PartialEq, Debug, Serialize, Deserialize)]
pub struct ProvidersResponse {
    pub data: Vec<Provider>,
}

#[derive(Clone, PartialOrd, PartialEq, Debug, Serialize, Deserialize)]
pub struct Provider {
    pub name: ProviderName,
    pub privacy_policy_url: Option<String>,
    pub slug: Slug,
    pub status_page_url: Option<String>,
    pub terns_of_service_url: Option<String>,
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
    pub fn to_slug(self) {
        // Implement this AI!
        todo!()
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
        let s = serde_json::to_string(self)
            .expect("Slug should always serialize to JSON string");
        // Remove the surrounding quotes that JSON adds.
        f.write_str(&s[1..s.len() - 1])
    }
}

impl Slug {
    pub fn to_provider_name(self) {
        // Implement this AI!
        todo!()
    }
}


