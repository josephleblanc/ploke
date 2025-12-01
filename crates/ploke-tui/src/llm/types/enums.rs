use super::*;

/// Parameters supported by OpenRouter models.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialOrd, Ord, PartialEq, Eq, Hash)]
#[serde(rename_all = "snake_case")]
pub enum SupportedParameters {
    FrequencyPenalty,
    IncludeReasoning,
    LogitBias,
    Logprobs,
    MaxTokens,
    MinP,
    PresencePenalty,
    Reasoning,
    RepetitionPenalty,
    ResponseFormat,
    Seed,
    Stop,
    StructuredOutputs,
    Temperature,
    ToolChoice,
    Tools,
    TopA,
    TopK,
    TopLogprobs,
    TopP,
    WebSearchOptions,
    Verbosity
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialOrd, Ord, PartialEq, Eq, Hash)]
#[serde(rename_all = "snake_case")]
pub enum Verbosity {
    Low,
    Medium,
    High
}


pub(crate) trait SupportsTools {
    fn supports_tools(&self) -> bool;
}

impl SupportsTools for &[SupportedParameters] {
    fn supports_tools(&self) -> bool {
        self.contains(&SupportedParameters::Tools)
    }
}
impl SupportsTools for &Vec<SupportedParameters> {
    fn supports_tools(&self) -> bool {
        self.contains(&SupportedParameters::Tools)
    }
}

/// Possible input modalities that a model can accept.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, PartialOrd, Eq)]
#[serde(rename_all = "snake_case")]
pub(crate) enum InputModality {
    Text,
    Image,
    Audio,
    Video,
    File,
}

/// Possible output modalities that a model can produce.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, PartialOrd, Eq)]
#[serde(rename_all = "snake_case")]
pub(crate) enum OutputModality {
    Text,
    Image,
    Audio, // no endpoints actually have the audio field?
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash, PartialOrd)]
#[serde(rename_all = "kebab-case")]
#[allow(clippy::enum_variant_names)]
pub(crate) enum Modality {
    #[serde(rename = "text->text")]
    TextToText,
    #[serde(rename = "text+image->text")]
    TextImageToText,
    #[serde(rename = "text+image->text+image")]
    TextImageToTextImage,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash, PartialOrd)]
pub(crate) enum Tokenizer {
    Claude,
    Cohere,
    DeepSeek,
    #[allow(clippy::upper_case_acronyms)]
    GPT,
    Gemini,
    Grok,
    Llama2,
    Llama3,
    Llama4,
    Mistral,
    Nova,
    Other,
    Qwen,
    Qwen3,
    Router,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash, PartialOrd)]
pub(crate) enum InstructType {
    #[serde(rename = "qwq")]
    Qwq,
    #[serde(rename = "phi3")]
    Phi3,
    #[serde(rename = "vicuna")]
    Vicuna,
    #[serde(rename = "qwen3")]
    Qwen3,
    #[serde(rename = "code-llama")]
    CodeLlama,
    #[serde(rename = "deepseek-v3.1")]
    DeepSeekV31,
    #[serde(rename = "chatml")]
    ChatML,
    #[serde(rename = "mistral")]
    Mistral,
    #[serde(rename = "airoboros")]
    Airoboros,
    #[serde(rename = "deepseek-r1")]
    DeepSeekR1,
    #[serde(rename = "llama3")]
    Llama3,
    #[serde(rename = "gemma")]
    Gemma,
    #[serde(rename = "alpaca")]
    Alpaca,
    #[serde(rename = "none")]
    None,
}

// From the official docs at openrouter:
// - `int4`: Integer (4 bit)
// - `int8`: Integer (8 bit)
// - `fp4`: Floating point (4 bit)
// - `fp6`: Floating point (6 bit)
// - `fp8`: Floating point (8 bit)
// - `fp16`: Floating point (16 bit)
// - `bf16`: Brain floating point (16 bit)
// - `fp32`: Floating point (32 bit)
// - `unknown`: Unknown
/// The level of quantization of the endpoint, e.g.
///     "quantization": "fp4",
#[derive(Debug, Clone, Serialize, Deserialize, Copy, PartialEq, PartialOrd, Eq, Hash)]
#[allow(non_camel_case_types)]
pub(crate) enum Quant {
    int4,
    int8,
    fp4,
    fp6,
    fp8,
    fp16,
    bf16,
    fp32,
    unknown,
}

impl Quant {
    pub(crate) fn as_str(&self) -> &'static str {
        match self {
            Quant::int4 => "int4",
            Quant::int8 => "int8",
            Quant::fp4 => "fp4",
            Quant::fp6 => "fp6",
            Quant::fp8 => "fp8",
            Quant::fp16 => "fp16",
            Quant::bf16 => "bf16",
            Quant::fp32 => "fp32",
            Quant::unknown => "unknown",
        }
    }
}
