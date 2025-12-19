use super::*;
use ploke_core::ArcStr;
use std::collections::HashSet;
use std::sync::{Mutex, OnceLock};
use tracing::warn;

// Deduplicated warning for newly encountered OpenRouter schema values.
fn log_unknown(category: &'static str, value: &str) {
    static SEEN: OnceLock<Mutex<HashSet<String>>> = OnceLock::new();
    let set = SEEN.get_or_init(|| Mutex::new(HashSet::new()));
    let mut guard = set.lock().expect("unknown-set mutex poisoned");
    let key = format!("{category}:{value}");
    if guard.insert(key) {
        warn!(
            category,
            value, "Unknown OpenRouter schema value encountered"
        );
    }
}

macro_rules! string_enum_with_unknown {
    ($(#[$meta:meta])* $name:ident, $category:literal, { $($variant:ident => $text:literal),+ $(,)? }) => {
        $(#[$meta])*
        #[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
        pub enum $name {
            $( $variant, )+
            /// Catch-all for new values emitted by OpenRouter; logs once per value.
            Unknown(ArcStr),
        }

        impl serde::Serialize for $name {
            fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
            where
                S: serde::Serializer,
            {
                let s = match self {
                    $(Self::$variant => $text,)+
                    Self::Unknown(v) => v.as_ref(),
                };
                serializer.serialize_str(s)
            }
        }

        impl<'de> serde::Deserialize<'de> for $name {
            fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
            where
                D: serde::Deserializer<'de>,
            {
                let raw = String::deserialize(deserializer)?;
                let v = match raw.as_str() {
                    $( $text => Self::$variant, )+
                    other => {
                        log_unknown($category, other);
                        Self::Unknown(ArcStr::from(other))
                    }
                };
                Ok(v)
            }
        }
    };
}

// Parameters supported by OpenRouter models.
string_enum_with_unknown!(
    SupportedParameters,
    "supported_parameters",
    {
        FrequencyPenalty => "frequency_penalty",
        IncludeReasoning => "include_reasoning",
        LogitBias => "logit_bias",
        Logprobs => "logprobs",
        MaxTokens => "max_tokens",
        MinP => "min_p",
        PresencePenalty => "presence_penalty",
        Reasoning => "reasoning",
        RepetitionPenalty => "repetition_penalty",
        ResponseFormat => "response_format",
        Seed => "seed",
        Stop => "stop",
        StructuredOutputs => "structured_outputs",
        Temperature => "temperature",
        ToolChoice => "tool_choice",
        Tools => "tools",
        TopA => "top_a",
        TopK => "top_k",
        TopLogprobs => "top_logprobs",
        TopP => "top_p",
        WebSearchOptions => "web_search_options",
        Verbosity => "verbosity"
    }
);

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialOrd, Ord, PartialEq, Eq, Hash)]
#[serde(rename_all = "snake_case")]
pub enum Verbosity {
    Low,
    Medium,
    High,
}

pub trait SupportsTools {
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

string_enum_with_unknown!(
    InputModality,
    "input_modality",
    {
        Text => "text",
        Image => "image",
        Audio => "audio",
        Video => "video",
        File => "file"
    }
);

/// Possible output modalities that a model can produce.
string_enum_with_unknown!(
    OutputModality,
    "output_modality",
    {
        Text => "text",
        Image => "image",
        Audio => "audio",
        Embeddings => "embeddings"
    }
);

string_enum_with_unknown!(
    Modality,
    "modality",
    {
        TextToText => "text->text",
        TextImageToText => "text+image->text",
        TextImageToTextImage => "text+image->text+image",
        TextToEmbeddings => "text->embeddings"
    }
);

string_enum_with_unknown!(
    Tokenizer,
    "tokenizer",
    {
        Claude => "Claude",
        Cohere => "Cohere",
        DeepSeek => "DeepSeek",
        GPT => "GPT",
        Gemini => "Gemini",
        Grok => "Grok",
        Llama2 => "Llama2",
        Llama3 => "Llama3",
        Llama4 => "Llama4",
        Mistral => "Mistral",
        Nova => "Nova",
        Other => "Other",
        Qwen => "Qwen",
        Qwen3 => "Qwen3",
        Router => "Router"
    }
);

string_enum_with_unknown!(
    InstructType,
    "instruct_type",
    {
        Qwq => "qwq",
        Phi3 => "phi3",
        Vicuna => "vicuna",
        Qwen3 => "qwen3",
        CodeLlama => "code-llama",
        DeepSeekV31 => "deepseek-v3.1",
        ChatML => "chatml",
        Mistral => "mistral",
        Airoboros => "airoboros",
        DeepSeekR1 => "deepseek-r1",
        Llama3 => "llama3",
        Gemma => "gemma",
        Alpaca => "alpaca",
        None => "none"
    }
);

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
pub enum Quant {
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
    pub fn as_str(&self) -> &'static str {
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
