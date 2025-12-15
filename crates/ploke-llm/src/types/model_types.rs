use ploke_core::ArcStr;
use std::{
    fmt::{self, Display},
    str::FromStr,
};

use serde::{Deserialize, Serialize};

use crate::{
    Author, IdError, InputModality, InstructType, Modality, ModelSlug, OutputModality, Tokenizer,
    registry::user_prefs::DEFAULT_MODEL,
};

#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize, PartialOrd, Ord)]
#[serde(try_from = "&str")]
/// The ModelKey is in the form {author}/{model}, and does not contain the `:{variant}` convention
/// that may vary across routers/providers, e.g. for OpenRouter
/// - ModelKey might be: deepseek/deepseek-r1
/// - but there may be a ModelId, `deepseek/deepseek-r1:free`, which may correspond to an Endpoint
///   such as `deepinfra/deepseek-r1`
pub struct ModelKey {
    pub author: Author,  // e.g. "openai", "nousresearch"
    pub slug: ModelSlug, // e.g. "gpt-5", "deephermes-3-llama-3-8b-preview"
}

impl Display for ModelKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}/{}", self.author.as_str(), self.slug.as_str())
    }
}

impl Default for ModelKey {
    fn default() -> Self {
        // TODO: Update to include user config override
        DEFAULT_MODEL.clone()
    }
}
impl<'a> TryFrom<&'a str> for ModelKey {
    type Error = IdError;

    fn try_from(s: &'a str) -> Result<Self, Self::Error> {
        let (author, slug) = s
            .split_once('/')
            .ok_or(IdError::Invalid("missing '/' in ModelKey"))?;
        if slug.contains(':') {
            return Err(IdError::Invalid(
                "ModelKey has unexpected ':', perhaps you meant to use ModelId?",
            ));
        }
        Ok(ModelKey {
            author: Author::new(author)?,
            slug: ModelSlug::new(slug)?,
        })
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Default, PartialOrd, Ord)]
pub struct ModelId {
    pub key: ModelKey,
    pub variant: Option<ModelVariant>,
}

impl From<ModelKey> for ModelId {
    fn from(key: ModelKey) -> Self {
        Self { key, variant: None }
    }
}

impl ModelId {
    #[allow(dead_code)]
    pub(crate) fn with_variant(mut self, variant: Option<ModelVariant>) -> Self {
        self.variant = variant;
        self
    }
    pub(crate) fn from_parts(key: ModelKey, variant: Option<ModelVariant>) -> Self {
        Self { key, variant }
    }
    pub(crate) fn to_request_string(&self) -> String {
        let Self { key, variant } = self;
        let mut out = key.to_string();
        if let Some(v) = variant {
            out.push(':');
            out.push_str(v.as_str());
        }
        out
    }
}
/// Helper function for use with `#[serde(serialize_with = "...")]` to serialize
/// a `ModelId` using the same format as `ModelId::to_request_string`.
pub(crate) fn serialize_model_id_as_request_string<S>(
    model_id: &ModelId,
    serializer: S,
) -> Result<S::Ok, S::Error>
where
    S: serde::Serializer,
{
    serializer.serialize_str(&model_id.to_request_string())
}

#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize, PartialOrd, Ord)]
pub enum ModelVariant {
    Free,
    Beta,
    Extended,
    Thinking,
    Online,
    Nitro,
    Floor,
    Other(ArcStr),
}

impl ModelVariant {
    pub fn as_str(&self) -> &str {
        match self {
            Self::Free => "free",
            Self::Beta => "beta",
            Self::Extended => "extended",
            Self::Thinking => "thinking",
            Self::Online => "online",
            Self::Nitro => "nitro",
            Self::Floor => "floor",
            Self::Other(s) => s.as_ref(),
        }
    }
}

impl fmt::Display for ModelId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.variant {
            None => write!(f, "{}/{}", self.key.author.as_str(), self.key.slug.as_str()),
            Some(v) => write!(
                f,
                "{}/{}:{}",
                self.key.author.as_str(),
                self.key.slug.as_str(),
                v.as_str()
            ),
        }
    }
}

impl<'de> serde::Deserialize<'de> for ModelId {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        FromStr::from_str(&s).map_err(serde::de::Error::custom)
    }
}

impl FromStr for ModelId {
    type Err = IdError;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let (author, rest) = s
            .trim()
            .split_once('/')
            .ok_or(IdError::Invalid("missing '/'"))?;
        let (slug, variant) = match rest.split_once(':') {
            Some((slug, v)) if !slug.is_empty() => (slug, Some(v)),
            _ if !rest.is_empty() => (rest, None),
            _ => return Err(IdError::Invalid("missing slug")),
        };
        Ok(Self {
            key: ModelKey {
                author: Author::new(author)?,
                slug: ModelSlug::new(slug)?,
            },
            variant: variant.map(|v| match v {
                "free" => ModelVariant::Free,
                "beta" => ModelVariant::Beta,
                "extended" => ModelVariant::Extended,
                "thinking" => ModelVariant::Thinking,
                "online" => ModelVariant::Online,
                "nitro" => ModelVariant::Nitro,
                "floor" => ModelVariant::Floor,
                other => ModelVariant::Other(ArcStr::from(other)),
            }),
        })
    }
}

/// Architecture details of a model, including input/output modalities and tokenizer info.
#[derive(Debug, Clone, Serialize, Deserialize, PartialOrd, PartialEq, Eq)]
pub struct Architecture {
    /// Input modalities supported by this model (text, image, audio, video).
    pub input_modalities: Vec<InputModality>,
    pub modality: Modality,
    pub output_modalities: Vec<OutputModality>,
    pub tokenizer: Tokenizer,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub instruct_type: Option<InstructType>,
}

#[cfg(test)]
mod tests {
    use crate::router_only::cli::MODELS_JSON_ARCH;

    use super::*;
    use serde_json::json;
    use std::str::FromStr;

    // Macros to keep tests concise and consistent
    macro_rules! assert_model_key_ok {
        ($s:expr, $author:expr, $slug:expr) => {{
            let mk = ModelKey::try_from($s).expect("ModelKey parse ok");
            assert_eq!(mk.author.as_str(), $author);
            assert_eq!(mk.slug.as_str(), $slug);
        }};
    }
    macro_rules! assert_model_key_err {
        ($s:expr) => {{
            let res = ModelKey::try_from($s);
            assert!(res.is_err(), "expected error for input: {}", $s);
        }};
    }
    macro_rules! assert_model_id_case {
        // Assert ModelId parses and round-trips, with expected author/slug and variant pattern
        ($s:expr => $author:expr, $slug:expr, $variant_pat:pat) => {{
            let mid = ModelId::from_str($s).expect("ModelId parse ok");
            assert_eq!(mid.key.author.as_str(), $author);
            assert_eq!(mid.key.slug.as_str(), $slug);
            assert!(
                matches!(mid.variant, $variant_pat),
                "variant mismatch: {:?}",
                mid.variant
            );
            assert_eq!(mid.to_string(), $s.trim(), "round-trip display");
        }};
    }

    // -- `Architecture` tests --
    // Examples from API response field for `Architecture` to use in tests (see comments above)

    fn parse_arch_and_assert(
        raw: &str,
        expected_inputs: &[InputModality],
        expected_modality: Modality,
        expected_outputs: &[OutputModality],
        expected_tokenizer: Tokenizer,
        expected_instruct: Option<InstructType>,
    ) {
        let arch: Architecture = serde_json::from_str(raw).expect("parse Architecture");
        assert_eq!(arch.input_modalities, expected_inputs);
        assert_eq!(arch.modality, expected_modality);
        assert_eq!(arch.output_modalities, expected_outputs);
        assert_eq!(arch.tokenizer, expected_tokenizer);
        assert_eq!(arch.instruct_type, expected_instruct);

        // Also check round-trip preserves canonical strings
        let round = serde_json::to_string(&arch).expect("serialize Architecture");
        let reparsed: Architecture = serde_json::from_str(&round).expect("reparse Architecture");
        assert_eq!(reparsed.input_modalities, expected_inputs);
        assert_eq!(reparsed.modality, expected_modality);
        assert_eq!(reparsed.output_modalities, expected_outputs);
        assert_eq!(reparsed.tokenizer, expected_tokenizer);
        assert_eq!(reparsed.instruct_type, expected_instruct);
    }

    #[test]
    fn test_architecture_simple() {
        // 1) text -> text, tokenizer: Qwen3
        let raw = r#"{
            "input_modalities": ["text"],
            "modality": "text->text",
            "output_modalities": ["text"],
            "tokenizer": "Qwen3"
        }"#;
        parse_arch_and_assert(
            raw,
            &[InputModality::Text],
            Modality::TextToText,
            &[OutputModality::Text],
            Tokenizer::Qwen3,
            None,
        );

        // 2) text+image -> text, tokenizer: Other
        let raw = r#"{
            "input_modalities": ["text", "image"],
            "modality": "text+image->text",
            "output_modalities": ["text"],
            "tokenizer": "Other"
        }"#;
        parse_arch_and_assert(
            raw,
            &[InputModality::Text, InputModality::Image],
            Modality::TextImageToText,
            &[OutputModality::Text],
            Tokenizer::Other,
            None,
        );

        // 3) text+image -> text+image, tokenizer: Gemini
        let raw = r#"{
            "input_modalities": ["image", "text"],
            "modality": "text+image->text+image",
            "output_modalities": ["image", "text"],
            "tokenizer": "Gemini"
        }"#;
        parse_arch_and_assert(
            raw,
            &[InputModality::Image, InputModality::Text],
            Modality::TextImageToTextImage,
            &[OutputModality::Image, OutputModality::Text],
            Tokenizer::Gemini,
            None,
        );

        // 4) text -> text, tokenizer: Llama3, instruct_type: llama3
        let raw = r#"{
            "input_modalities": ["text"],
            "modality": "text->text",
            "output_modalities": ["text"],
            "tokenizer": "Llama3",
            "instruct_type": "llama3"
        }"#;
        parse_arch_and_assert(
            raw,
            &[InputModality::Text],
            Modality::TextToText,
            &[OutputModality::Text],
            Tokenizer::Llama3,
            Some(InstructType::Llama3),
        );

        // 5) text -> text, tokenizer: Qwen, instruct_type: deepseek-r1
        let raw = r#"{
            "input_modalities": ["text"],
            "modality": "text->text",
            "output_modalities": ["text"],
            "tokenizer": "Qwen",
            "instruct_type": "deepseek-r1"
        }"#;
        parse_arch_and_assert(
            raw,
            &[InputModality::Text],
            Modality::TextToText,
            &[OutputModality::Text],
            Tokenizer::Qwen,
            Some(InstructType::DeepSeekR1),
        );
    }

    #[test]
    fn test_architecture_invalid() {
        // Invalid modality string
        let bad_modality = r#"{
            "input_modalities": ["text"],
            "modality": "text=>text",
            "output_modalities": ["text"],
            "tokenizer": "Qwen3"
        }"#;
        assert!(serde_json::from_str::<Architecture>(bad_modality).is_err());

        // Invalid input modality value
        let bad_input = r#"{
            "input_modalities": ["txt"],
            "modality": "text->text",
            "output_modalities": ["text"],
            "tokenizer": "Qwen3"
        }"#;
        assert!(serde_json::from_str::<Architecture>(bad_input).is_err());

        // Invalid tokenizer value
        let bad_tok = r#"{
            "input_modalities": ["text"],
            "modality": "text->text",
            "output_modalities": ["text"],
            "tokenizer": "UnknownTokenizer"
        }"#;
        assert!(serde_json::from_str::<Architecture>(bad_tok).is_err());
    }

    #[test]
    fn test_architecture_from_arch_file() {
        use ploke_test_utils::workspace_root;

        let mut p = workspace_root();
        p.push(MODELS_JSON_ARCH);
        let data = std::fs::read_to_string(&p)
            .unwrap_or_else(|e| panic!("failed to read {}: {}", p.display(), e));
        let items: Vec<Architecture> = serde_json::from_str(&data)
            .unwrap_or_else(|e| panic!("failed to parse {}: {}", p.display(), e));

        let contains = |pred: &dyn Fn(&Architecture) -> bool| items.iter().any(pred);

        assert!(
            contains(&|a| {
                a.input_modalities == vec![InputModality::Text]
                    && a.modality == Modality::TextToText
                    && a.output_modalities == vec![OutputModality::Text]
                    && a.tokenizer == Tokenizer::Qwen3
                    && a.instruct_type.is_none()
            }),
            "expected Qwen3 text->text archetype present"
        );

        assert!(
            contains(&|a| {
                a.input_modalities == vec![InputModality::Text, InputModality::Image]
                    && a.modality == Modality::TextImageToText
                    && a.output_modalities == vec![OutputModality::Text]
                    && a.tokenizer == Tokenizer::Other
            }),
            "expected text+image->text Other present"
        );

        assert!(
            contains(&|a| {
                a.input_modalities == vec![InputModality::Image, InputModality::Text]
                    && a.modality == Modality::TextImageToTextImage
                    && a.output_modalities == vec![OutputModality::Image, OutputModality::Text]
                    && a.tokenizer == Tokenizer::Gemini
            }),
            "expected text+image->text+image Gemini present"
        );

        assert!(
            contains(&|a| {
                a.input_modalities == vec![InputModality::Text]
                    && a.modality == Modality::TextToText
                    && a.output_modalities == vec![OutputModality::Text]
                    && a.tokenizer == Tokenizer::Llama3
                    && a.instruct_type == Some(InstructType::Llama3)
            }),
            "expected Llama3 text->text with instruct llama3 present"
        );

        assert!(
            contains(&|a| {
                a.input_modalities == vec![InputModality::Text]
                    && a.modality == Modality::TextToText
                    && a.output_modalities == vec![OutputModality::Text]
                    && a.tokenizer == Tokenizer::Qwen
                    && a.instruct_type == Some(InstructType::DeepSeekR1)
            }),
            "expected Qwen text->text with instruct deepseek-r1 present"
        );
    }

    // -- `ModelKey` tests --
    #[test]
    fn test_model_key_from_author_slug_valid() {
        let mk = ModelKey {
            author: Author::new("openai").unwrap(),
            slug: ModelSlug::new("gpt-5").unwrap(),
        };
        assert_eq!(mk.author.as_str(), "openai");
        assert_eq!(mk.slug.as_str(), "gpt-5");
    }

    #[test]
    fn test_model_key_from_string_valid() {
        assert_model_key_ok!("openai/gpt-5", "openai", "gpt-5");
        assert_model_key_ok!(
            "nousresearch/deephermes-3-llama-3-8b-preview",
            "nousresearch",
            "deephermes-3-llama-3-8b-preview"
        );
    }

    #[test]
    fn test_model_key_from_string_invalid_format() {
        // Missing '/'
        assert_model_key_err!("openai-gpt-4");
        // Variant separator ':' not allowed in ModelKey
        let err = ModelKey::try_from("openai/gpt-4o:free").unwrap_err();
        match err {
            IdError::Invalid(msg) => assert!(msg.contains("ModelKey has unexpected ':'")),
            other => panic!("unexpected error: {other:?}"),
        }
    }

    #[test]
    fn test_model_key_from_string_whitespace_handling() {
        // No trim in ModelKey parsing, whitespace should fail via Author/ModelSlug validators
        assert_model_key_err!(" openai/gpt-4");
        assert_model_key_err!("openai /gpt-4");
        assert_model_key_err!("openai/gpt-4 ");
    }

    #[test]
    fn test_model_key_author_slug_validation() {
        // Spaces not allowed
        assert_model_key_err!("open ai/gpt-4");
        assert_model_key_err!("openai/gpt 4");
        // Empty segments not allowed
        assert_model_key_err!("/gpt-4");
        assert_model_key_err!("openai/");
    }

    // -- `ModelId` tests --
    #[test]
    fn test_model_id_variant_simple() {
        assert_model_id_case!(
            "google/gemma-2-9b-it:free" => "google", "gemma-2-9b-it", Some(ModelVariant::Free)
        );
        assert_model_id_case!(
            "qwen/qwen-plus-2025-07-28:thinking" => "qwen", "qwen-plus-2025-07-28", Some(ModelVariant::Thinking)
        );
    }

    #[test]
    fn test_model_id_variant_empty() {
        // Current behavior: empty variant is accepted as Other("") and preserved in Display
        let mid = ModelId::from_str("openai/gpt-4o:").expect("parse");
        assert!(matches!(mid.variant, Some(ModelVariant::Other(ref s)) if s.is_empty()));
        assert_eq!(mid.to_string(), "openai/gpt-4o:");
    }

    #[test]
    fn test_model_id_variant_unknown() {
        assert_model_id_case!(
            "openai/gpt-4o:fast" => "openai", "gpt-4o", Some(ModelVariant::Other(_))
        );
    }

    #[test]
    fn test_model_id_deserialize_from_json() {
        // Serde JSON string deserialization uses FromStr under the hood
        let raw = json!("openai/gpt-4o:beta");
        let mid: ModelId = serde_json::from_value(raw).expect("deserialize ModelId");
        assert!(matches!(mid.variant, Some(ModelVariant::Beta)));
        assert_eq!(mid.to_string(), "openai/gpt-4o:beta");
    }
}
