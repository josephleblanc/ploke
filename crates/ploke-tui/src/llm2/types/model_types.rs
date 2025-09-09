use ploke_core::ArcStr;
use std::{fmt, str::FromStr};

use serde::{Deserialize, Serialize};

use crate::llm2::{registry::user_prefs::DEFAULT_MODEL, Author, IdError, ModelSlug};

#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub(crate) struct ModelKey {
    pub(crate) author: Author,  // e.g. "openai", "nousresearch"
    pub(crate) slug: ModelSlug, // e.g. "gpt-5", "deephermes-3-llama-3-8b-preview"
}

impl Default for ModelKey {
    fn default() -> Self {
        // TODO: Update to include user config override
        DEFAULT_MODEL.clone()
    }
}


#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
pub(crate) struct ModelId {
    pub(crate) key: ModelKey,
    pub(crate) variant: Option<ModelVariant>,
}

impl From<ModelKey> for ModelId {
    fn from(key: ModelKey) -> Self {
        Self { key, variant: None }
    }
}

impl ModelId {
    pub(crate) fn with_variant(mut self, variant: Option<ModelVariant>) -> Self {
        self.variant = variant;
        self
    }
    pub(crate) fn from_parts(key: ModelKey, variant: Option<ModelVariant>) -> Self {
        Self { key, variant }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub(crate) enum ModelVariant {
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
