use ploke_core::ArcStr;
use serde::{Deserialize, Serialize};
use std::{fmt, str::FromStr, sync::Arc};
use url::Url;

use super::{model_types::{ModelKey, ModelVariant}, Quant};

// ----- minimal error type -----
#[derive(Debug, thiserror::Error)]
pub(crate) enum IdError {
    #[error("invalid identifier: {0}")]
    Invalid(&'static str),
    #[error("invalid URL")]
    Url(#[from] url::ParseError),
}

macro_rules! newtype_arcstr {
    ($name:ident) => {
        #[derive(Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
        #[serde(transparent)]
        pub(crate) struct $name(ploke_core::ArcStr);
        impl $name {
            pub(crate) fn new(s: impl AsRef<str>) -> Self {
                Self(ploke_core::ArcStr::from(s.as_ref()))
            }
            pub(crate) fn as_str(&self) -> &str {
                &self.0
            }
        }
        impl std::fmt::Debug for $name {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                f.debug_tuple(stringify!($name)).field(&self.0).finish()
            }
        }
    };
}
newtype_arcstr!(ProviderSlug);
newtype_arcstr!(ProviderName);
newtype_arcstr!(ApiKeyEnv);
newtype_arcstr!(EndpointPath);
// The name of the model as used in `Endpoint`'s `model_name` or model::Response's `name`
newtype_arcstr!(ModelName);

// ----- strongly-typed string wrappers (easy to swap inner later) -----
#[derive(Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub(crate) struct Author(String);
impl fmt::Debug for Author {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("Author").field(&self.0).finish()
    }
}
impl Author {
    pub(crate) fn new(s: impl Into<String>) -> Result<Self, IdError> {
        let s = s.into();
        if s.is_empty() || s.contains(' ') {
            return Err(IdError::Invalid("author"));
        }
        Ok(Self(s))
    }
    pub(crate) fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub(crate) struct ModelSlug(String);
impl fmt::Debug for ModelSlug {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("Slug").field(&self.0).finish()
    }
}
impl ModelSlug {
    pub(crate) fn new(s: impl Into<String>) -> Result<Self, IdError> {
        let s = s.into();
        if s.is_empty() || s.contains(' ') {
            return Err(IdError::Invalid("slug"));
        }
        Ok(Self(s))
    }
    pub(crate) fn as_str(&self) -> &str {
        &self.0
    }
}

/// The "tag" field on endpoint requests in the form {provider_slug}/{quantization} where
/// {provider_slug} is lowercase.
/// - Note: There may or may not be a quantization included
#[derive(Clone, Debug)]
pub(crate) struct EndpointTag {
    pub(crate) provider_name: ProviderSlug,
    pub(crate) quantization: Option<Quant>,
}

impl Serialize for EndpointTag {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let s = if let Some(q) = self.quantization {
            format!("{}/{}", self.provider_name.as_str(), q.as_str())
        } else {
            self.provider_name.as_str().to_string()
        };
        serializer.serialize_str(&s)
    }
}

impl<'de> Deserialize<'de> for EndpointTag {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        struct Visitor;
        impl<'de> serde::de::Visitor<'de> for Visitor {
            type Value = EndpointTag;
            fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
                f.write_str(
                    "a string in format \"provider_slug\" or \"provider_slug/quantization\"",
                )
            }
            fn visit_str<E>(self, v: &str) -> Result<Self::Value, E>
            where
                E: serde::de::Error,
            {
                let parts: Vec<&str> = v.split('/').collect();
                match parts.as_slice() {
                    [provider] => Ok(EndpointTag {
                        provider_name: ProviderSlug::new(*provider),
                        quantization: None,
                    }),
                    [provider, quant] => {
                        let quantization = match *quant {
                            "int4" => Quant::int4,
                            "int8" => Quant::int8,
                            "fp4" => Quant::fp4,
                            "fp6" => Quant::fp6,
                            "fp8" => Quant::fp8,
                            "fp16" => Quant::fp16,
                            "bf16" => Quant::bf16,
                            "fp32" => Quant::fp32,
                            "unknown" => Quant::unknown,
                            _ => return Err(E::custom("invalid quantization value")),
                        };
                        Ok(EndpointTag {
                            provider_name: ProviderSlug::new(*provider),
                            quantization: Some(quantization),
                        })
                    }
                    _ => Err(E::custom("invalid format")),
                }
            }
        }
        deserializer.deserialize_str(Visitor)
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub(crate) struct BaseUrl(#[serde(with = "serde_url")] Url);
mod serde_url {
    use super::*;
    use serde::{Deserialize, Deserializer, Serialize, Serializer, de::Error};
    pub(crate) fn serialize<S: Serializer>(u: &Url, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(u.as_str())
    }
    pub(crate) fn deserialize<'de, D: Deserializer<'de>>(d: D) -> Result<Url, D::Error> {
        let s = String::deserialize(d)?;
        Url::parse(&s).map_err(D::Error::custom)
    }
}

impl BaseUrl {
    pub(crate) fn as_url(&self) -> &Url {
        &self.0
    }
}

// Now Transport has only owned data
#[derive(Clone, Debug, Serialize, Deserialize)]
pub(crate) enum Transport {
    OpenRouter {
        base: BaseUrl,
        allow: Vec<ProviderSlug>,
        api_key_env: ApiKeyEnv, // e.g. "OPENROUTER_API_KEY"
    },
    DirectOAI {
        base: BaseUrl,
        chat_path: EndpointPath, // e.g. "chat/completions"
        api_key_env: ApiKeyEnv,  // e.g. "GROQ_API_KEY"
    },
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub(crate) struct ProviderConfig {
    pub(crate) key: ProviderKey,
    pub(crate) name: ProviderName,
    pub(crate) transport: Transport,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub(crate) struct ProviderKey {
    pub(crate) slug: ProviderSlug,
}
impl ProviderKey {
    pub(crate) fn new(slug: &str) -> Result<Self, IdError> {
        Ok(Self {
            slug: ProviderSlug::new(slug),
        })
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub(crate) struct EndpointKey {
    pub(crate) model: ModelKey,
    pub(crate) variant: Option< ModelVariant >,
    pub(crate) provider: ProviderKey,
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::str::FromStr;

    use crate::llm2::ModelId;
    use crate::llm2::types::model_types::ModelVariant;

    // TODO:
    // add a helper macro for testing
    #[test]
    fn test_model_id_basic() {
        // Add tests for `ModelId` from the following items:
        //
        // "google/gemma-2-9b-it"
        // "qwen/qwen-plus-2025-07-28"
        let m1 = ModelId::from_str("google/gemma-2-9b-it").expect("parse m1");
        assert_eq!(m1.to_string(), "google/gemma-2-9b-it");
        assert!(m1.variant.is_none());
        assert_eq!(m1.key.author.as_str(), "google");
        assert_eq!(m1.key.slug.as_str(), "gemma-2-9b-it");

        let m2 = ModelId::from_str("qwen/qwen-plus-2025-07-28").expect("parse m2");
        assert_eq!(m2.to_string(), "qwen/qwen-plus-2025-07-28");
        assert!(m2.variant.is_none());
        assert_eq!(m2.key.author.as_str(), "qwen");
        assert_eq!(m2.key.slug.as_str(), "qwen-plus-2025-07-28");
    }
    #[test]
    fn test_model_id_variants() {
        // Add tests for `ModelId` from the following items:
        //
        // "google/gemma-2-9b-it:free"
        // "qwen/qwen-plus-2025-07-28:thinking"
        let m1 = ModelId::from_str("google/gemma-2-9b-it:free").expect("parse m1 variant");
        assert_eq!(m1.to_string(), "google/gemma-2-9b-it:free");
        assert_eq!(m1.key.author.as_str(), "google");
        assert_eq!(m1.key.slug.as_str(), "gemma-2-9b-it");
        assert!(matches!(m1.variant, Some(ModelVariant::Free)));

        let m2 = ModelId::from_str("qwen/qwen-plus-2025-07-28:thinking").expect("parse m2 variant");
        assert_eq!(m2.to_string(), "qwen/qwen-plus-2025-07-28:thinking");
        assert_eq!(m2.key.author.as_str(), "qwen");
        assert_eq!(m2.key.slug.as_str(), "qwen-plus-2025-07-28");
        assert!(matches!(m2.variant, Some(ModelVariant::Thinking)));
    }

    #[test]
    fn test_model_id_trim() {
        // Add tests for `ModelId` from the following items:
        let m1 = ModelId::from_str("  google/gemma-2-9b-it  ").expect("parse trimmed m1");
        assert_eq!(m1.to_string(), "google/gemma-2-9b-it");
        assert!(m1.variant.is_none());

        let m2 = ModelId::from_str("	qwen/qwen-plus-2025-07-28:beta  
").expect("parse trimmed m2");
        assert_eq!(m2.to_string(), "qwen/qwen-plus-2025-07-28:beta");
        assert!(matches!(m2.variant, Some(ModelVariant::Beta)));
    }

    #[test]
    fn test_model_id_empty() {
        // Add tests for `ModelId` from the following items:
        assert!(ModelId::from_str("").is_err());
        assert!(ModelId::from_str("   ").is_err());
        assert!(ModelId::from_str("openai/").is_err()); // missing slug
        assert!(ModelId::from_str("/gpt-4").is_err()); // missing author
    }

    #[test]
    fn test_model_id_invalid() {
        // Add tests for `ModelId` from the following items:
        assert!(ModelId::from_str("open ai/gpt-4").is_err()); // space in author
        assert!(ModelId::from_str("openai/gpt 4").is_err()); // space in slug
        assert!(ModelId::from_str("not-a-model-id").is_err()); // missing '/'
    }

    #[test]
    fn test_model_ids_from_file() {
        use ploke_test_utils::workspace_root;
        use crate::llm2::router_only::MODELS_TXT_IDS;

        // text file with strings of serialized ModelId split by newlines
        let text_file = MODELS_TXT_IDS;
        // The file
        // Add tests for `ModelId` for all items in the file
        let mut path = workspace_root();
        path.push(text_file);
        let content = fs::read_to_string(&path)
            .unwrap_or_else(|e| panic!("failed to read {}: {}", path.display(), e));

        for (idx, raw) in content.lines().enumerate() {
            let line = raw.trim();
            if line.is_empty() || line.starts_with('#') { continue; }
            let parsed = ModelId::from_str(line)
                .unwrap_or_else(|e| panic!("failed to parse line {} ({}): {}", idx + 1, line, e));
            assert_eq!(parsed.to_string(), line, "roundtrip display should match input");
        }
    }
}

