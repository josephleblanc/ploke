use ploke_core::ArcStr;
use serde::{Deserialize, Serialize};
use std::fmt;
use url::Url;

use super::{
    Quant,
    model_types::{ModelKey, ModelVariant},
};

// ----- minimal error type -----
#[derive(Debug, thiserror::Error)]
pub enum IdError {
    #[error("invalid identifier: {0}")]
    Invalid(&'static str),
    #[error("invalid URL")]
    Url(#[from] url::ParseError),
}

macro_rules! newtype_arcstr {
    ($name:ident) => {
        #[derive(Clone, PartialEq, Eq, Hash, Serialize, Deserialize, PartialOrd)]
        #[serde(transparent)]
        pub struct $name(ploke_core::ArcStr);
        impl $name {
            pub fn new(s: impl AsRef<str>) -> Self {
                Self(ploke_core::ArcStr::from(s.as_ref()))
            }
            pub fn as_str(&self) -> &str {
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
#[derive(Clone, PartialEq, Eq, Hash, Serialize, Deserialize, PartialOrd, Ord)]
#[serde(transparent)]
pub struct Author(String);
impl fmt::Debug for Author {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("Author").field(&self.0).finish()
    }
}
impl Author {
    pub fn new(s: impl Into<String>) -> Result<Self, IdError> {
        let s = s.into();
        if s.is_empty() || s.contains(' ') {
            return Err(IdError::Invalid("author"));
        }
        Ok(Self(s))
    }
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Clone, PartialEq, Eq, Hash, Serialize, Deserialize, PartialOrd, Ord)]
#[serde(transparent)]
pub struct ModelSlug(String);
impl fmt::Debug for ModelSlug {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("Slug").field(&self.0).finish()
    }
}
impl ModelSlug {
    pub fn new(s: impl Into<String>) -> Result<Self, IdError> {
        let s = s.into();
        if s.is_empty() || s.contains(' ') {
            return Err(IdError::Invalid("slug"));
        }
        Ok(Self(s))
    }
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Embedding model name echoed by OpenRouter embeddings responses.
/// Typically lacks provider prefix (e.g., `text-embedding-3-small`), but we allow any non-empty token.
#[derive(Clone, PartialEq, Eq, Hash, Serialize, Deserialize, PartialOrd, Ord)]
#[serde(transparent)]
pub struct EmbeddingModelName(ArcStr);
impl fmt::Debug for EmbeddingModelName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("EmbeddingModelName").field(&self.0).finish()
    }
}
impl EmbeddingModelName {
    pub fn new(s: impl Into<String>) -> Result<Self, IdError> {
        let s = s.into();
        if s.trim().is_empty() {
            return Err(IdError::Invalid("embedding model name"));
        }
        if s.contains(' ') {
            return Err(IdError::Invalid("embedding model name contains whitespace"));
        }
        Ok(Self(ArcStr::from(s)))
    }
    pub fn as_str(&self) -> &str {
        &self.0
    }
}
impl fmt::Display for EmbeddingModelName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Identifier echoed by OpenRouter embedding responses (`id` field). Observed as a short token
/// without spaces; keep it strongly typed to avoid stringly plumbing.
#[derive(Clone, PartialEq, Eq, Hash, Serialize, Deserialize, PartialOrd, Ord)]
#[serde(transparent)]
pub struct EmbeddingResponseId(ArcStr);
impl fmt::Debug for EmbeddingResponseId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("EmbeddingResponseId").field(&self.0).finish()
    }
}
impl EmbeddingResponseId {
    pub fn new(s: impl Into<String>) -> Result<Self, IdError> {
        let s = s.into();
        if s.trim().is_empty() {
            return Err(IdError::Invalid("embedding response id"));
        }
        if s.contains(' ') {
            return Err(IdError::Invalid(
                "embedding response id contains whitespace",
            ));
        }
        Ok(Self(ArcStr::from(s)))
    }
    pub fn as_str(&self) -> &str {
        &self.0
    }
}
impl fmt::Display for EmbeddingResponseId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// The "tag" field on endpoint requests in the form {provider_slug}/{quantization} where
/// {provider_slug} is lowercase.
/// - Note: There may or may not be a quantization included
#[derive(Clone, Debug)]
pub struct EndpointTag {
    pub provider_name: ProviderSlug,
    pub quantization: Option<Quant>,
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
                    "a string like \"provider\" or \"provider/quant\", where the final segment \
                     is interpreted as quant only if it matches a known quant token",
                )
            }

            fn visit_str<E>(self, v: &str) -> Result<Self::Value, E>
            where
                E: serde::de::Error,
            {
                fn parse_quant(s: &str) -> Option<Quant> {
                    Some(match s {
                        "int4" => Quant::int4,
                        "int8" => Quant::int8,
                        "fp4" => Quant::fp4,
                        "fp6" => Quant::fp6,
                        "fp8" => Quant::fp8,
                        "fp16" => Quant::fp16,
                        "bf16" => Quant::bf16,
                        "fp32" => Quant::fp32,
                        "unknown" => Quant::unknown,
                        _ => return None,
                    })
                }

                // Split on the *last* '/', and only treat the suffix as quant if recognized.
                if let Some((provider_part, last_seg)) = v.rsplit_once('/')
                    && let Some(q) = parse_quant(last_seg)
                {
                    if provider_part.is_empty() {
                        return Err(E::custom("invalid format: empty provider"));
                    }
                    return Ok(EndpointTag {
                        provider_name: ProviderSlug::new(provider_part),
                        quantization: Some(q),
                    });
                }

                // Otherwise, the entire string is the provider/tag (even if it contains '/').
                if v.is_empty() {
                    return Err(E::custom("invalid format: empty provider"));
                }

                Ok(EndpointTag {
                    provider_name: ProviderSlug::new(v),
                    quantization: None,
                })
            }
        }

        deserializer.deserialize_str(Visitor)
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct BaseUrl(#[serde(with = "serde_url")] Url);
mod serde_url {
    use super::*;
    use serde::{Deserialize, Deserializer, Serializer, de::Error};
    pub fn serialize<S: Serializer>(u: &Url, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(u.as_str())
    }
    pub fn deserialize<'de, D: Deserializer<'de>>(d: D) -> Result<Url, D::Error> {
        let s = String::deserialize(d)?;
        Url::parse(&s).map_err(D::Error::custom)
    }
}

impl BaseUrl {
    pub fn as_url(&self) -> &Url {
        &self.0
    }
}

// Now Transport has only owned data
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum Transport {
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
pub struct ProviderConfig {
    pub key: ProviderKey,
    pub name: ProviderName,
    pub transport: Transport,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ProviderKey {
    pub slug: ProviderSlug,
}
impl ProviderKey {
    pub fn new(slug: &str) -> Result<Self, IdError> {
        Ok(Self {
            slug: ProviderSlug::new(slug),
        })
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct EndpointKey {
    pub model: ModelKey,
    pub variant: Option<ModelVariant>,
    // This is not actually the key, it is a name. The returned value this is taken from initially
    // is in the `Endpoint`, which does not return a `provider_slug`, but rather a ProviderName
    // pub provider: ProviderKey,
    pub provider: ProviderKey,
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::str::FromStr;

    use crate::ModelId;
    use crate::router_only::cli::MODELS_TXT_IDS;
    use crate::types::model_types::ModelVariant;

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

        let m2 = ModelId::from_str(
            "	qwen/qwen-plus-2025-07-28:beta  
",
        )
        .expect("parse trimmed m2");
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
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            let parsed = ModelId::from_str(line)
                .unwrap_or_else(|e| panic!("failed to parse line {} ({}): {}", idx + 1, line, e));
            assert_eq!(
                parsed.to_string(),
                line,
                "roundtrip display should match input"
            );
        }
    }
}
