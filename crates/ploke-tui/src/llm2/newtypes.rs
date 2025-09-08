use ploke_core::ArcStr;
use serde::{Deserialize, Serialize};
use std::{fmt, str::FromStr, sync::Arc};
use url::Url;

use super::enums::Quant;

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
        #[derive(Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
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

// ----- strongly-typed string wrappers (easy to swap inner later) -----
#[derive(Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
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

#[derive(Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
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

#[derive(Clone, PartialEq, Eq, Hash, Serialize, Deserialize, Debug)]
#[serde(transparent)]
/// The canonical id in the form `author/model`, e.g.
pub struct ModelId(ArcStr);

impl ModelId {
    pub fn new(s: impl AsRef<str>) -> Self {
        Self(ArcStr::from(s.as_ref()))
    }
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// The "tag" field on endpoint requests in the form {provider_slug}/{quantization} where
/// {provider_slug} is lowercase
pub struct EndpointTag {
    provider_name: ProviderSlug,
    quantization: Quant,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct BaseUrl(#[serde(with = "serde_url")] Url);
mod serde_url {
    use super::*;
    use serde::{Deserialize, Deserializer, Serialize, Serializer, de::Error};
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

// ----- canonical keys -----
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub(crate) struct ModelKey {
    pub(crate) author: Author,
    pub(crate) slug: ModelSlug,
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
    pub(crate) fn from_string(model_id: String) -> Result< Self, IdError > {
        let mut parts = model_id.trim()
            .split('/');
        // AI: Fill in the following todo items AI!
        let author = parts.next().unwrap_or_else( todo!("return an error") );
        let slug = parts.next().unwrap_or_else( todo!("return an error") );
        let id = ModelId::new(model_id);
        Ok(Self { author: Author::new(author)?, slug: ModelSlug::new( slug )?, id })
    }
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
    pub provider: ProviderKey,
}
