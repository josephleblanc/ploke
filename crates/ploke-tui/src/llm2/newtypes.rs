use ploke_core::ArcStr;
use serde::{Deserialize, Serialize};
use std::{fmt, str::FromStr, sync::Arc};
use url::Url;

use super::enums::Quant;

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

#[derive(Clone, PartialEq, Eq, Hash, Serialize, Deserialize, Debug)]
#[serde(transparent)]
/// The canonical id in the form `author/model`, e.g.
pub(crate) struct ModelId(ArcStr);

impl ModelId {
    pub(crate) fn new(s: impl AsRef<str>) -> Self {
        Self(ArcStr::from(s.as_ref()))
    }
    pub(crate) fn as_str(&self) -> &str {
        &self.0
    }
}

/// The "tag" field on endpoint requests in the form {provider_slug}/{quantization} where
/// {provider_slug} is lowercase
pub(crate) struct EndpointTag {
    provider_name: ProviderSlug,
    quantization: Quant,
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

// ----- canonical keys -----
/// Must be of the form `{author}/{slug}`
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub(crate) struct ModelKey {
    /// {author}, e.g. `openai`
    pub(crate) author: Author,
    /// {slug}, e.g. `gpt-5`
    pub(crate) slug: ModelSlug,
    /// `{author}/{slug}`, e.g. `openai/gpt-5`
    pub(crate) id: ModelId,
}

impl ModelKey {
    pub(crate) fn from_author_slug(author: &str, slug: &str) -> Result<Self, IdError> {
        Ok(Self {
            author: Author::new(author)?,
            slug: ModelSlug::new(slug)?,
            id: ModelId::new(format!("{}/{}", author, slug)),
        })
    }
    pub(crate) fn id(&self) -> String {
        format!("{}/{}", self.author.as_str(), self.slug.as_str())
    }

    /// Input must be of the form `{author}/{model}`
    pub(crate) fn from_string(model_id: String) -> Result<Self, IdError> {
        let mut parts = model_id.trim().split('/');
        let author = parts
            .next()
            .ok_or(IdError::Invalid("missing author in model ID"))?;
        let slug = parts
            .next()
            .ok_or(IdError::Invalid("missing model slug in model ID"))?;

        // Ensure there are no extra parts
        if parts.next().is_some() {
            return Err(IdError::Invalid(
                "model ID should be in format 'author/slug'",
            ));
        }

        Ok(Self {
            author: Author::new(author)?,
            slug: ModelSlug::new(slug)?,
            id: ModelId::new(model_id),
        })
    }
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
    pub(crate) provider: ProviderKey,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_model_key_from_author_slug_valid() {
        let key = ModelKey::from_author_slug("openai", "gpt-4").unwrap();
        assert_eq!(key.author.as_str(), "openai");
        assert_eq!(key.slug.as_str(), "gpt-4");
        assert_eq!(key.id(), "openai/gpt-4");
    }

    #[test]
    fn test_model_key_from_string_valid() {
        let key = ModelKey::from_string("anthropic/claude-3".to_string()).unwrap();
        assert_eq!(key.author.as_str(), "anthropic");
        assert_eq!(key.slug.as_str(), "claude-3");
        assert_eq!(key.id(), "anthropic/claude-3");
    }

    #[test]
    fn test_model_key_from_string_invalid_format() {
        // Missing slash
        assert!(ModelKey::from_string("invalid-format".to_string()).is_err());
        
        // Too many slashes
        assert!(ModelKey::from_string("author/model/extra".to_string()).is_err());
        
        // Empty parts
        assert!(ModelKey::from_string("/".to_string()).is_err());
        assert!(ModelKey::from_string("author/".to_string()).is_err());
        assert!(ModelKey::from_string("/model".to_string()).is_err());
        
        // Empty string
        assert!(ModelKey::from_string("".to_string()).is_err());
    }

    #[test]
    fn test_model_key_from_string_whitespace_handling() {
        // Leading/trailing whitespace should be trimmed
        let key = ModelKey::from_string("  openai/gpt-4  ".to_string()).unwrap();
        assert_eq!(key.author.as_str(), "openai");
        assert_eq!(key.slug.as_str(), "gpt-4");
    }

    #[test]
    fn test_model_key_author_slug_validation() {
        // Test invalid author
        assert!(ModelKey::from_author_slug("", "model").is_err());
        assert!(ModelKey::from_author_slug("invalid author", "model").is_err());
        
        // Test invalid slug
        assert!(ModelKey::from_author_slug("author", "").is_err());
        assert!(ModelKey::from_author_slug("author", "invalid slug").is_err());
    }
}
