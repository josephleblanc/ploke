use ploke_core::ArcStr;
use std::{fmt, str::FromStr};

use serde::{Deserialize, Serialize};

use crate::llm2::{Author, IdError, ModelSlug, registry::user_prefs::DEFAULT_MODEL};

#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(try_from = "&str")]
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
impl<'a> TryFrom<&'a str> for ModelKey {
    type Error = IdError;

    fn try_from(s: &'a str) -> Result<Self, Self::Error> {
        let (author, slug) = s
            .split_once('/')
            .ok_or(IdError::Invalid("missing '/' in ModelKey"))?;
        if slug.contains(':') {
            return Err(IdError::Invalid("ModelKey has unexpected ':', perhaps you meant to use ModelId?"))
        }
        Ok(ModelKey {
            author: Author::new(author)?,
            slug: ModelSlug::new(slug)?,
        })
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
// AI: is there some way to leverage `FromStr` in serde if I want this to deserialize things like
// "deepseek/deepseek-v3" and "deepseek/deepseek-v3:free" AI?
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_model_key_from_author_slug_valid() {
        todo!()
    }

    #[test]
    fn test_model_key_from_string_valid() {
        todo!()
    }

    #[test]
    fn test_model_key_from_string_invalid_format() {
        todo!()
    }

    #[test]
    fn test_model_key_from_string_whitespace_handling() {
        todo!()
    }

    #[test]
    fn test_model_key_author_slug_validation() {
        todo!()
    }

    #[test]
    fn test_model_key_variant_simple() {
        todo!()
    }

    #[test]
    fn test_model_key_variant_empty() {
        todo!()
    }

    #[test]
    fn test_model_key_variant_unknown() {
        todo!()
    }
}
