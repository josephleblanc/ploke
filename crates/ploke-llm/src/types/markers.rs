use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialOrd, PartialEq, Eq)]
pub struct ListMarker;

impl ListMarker {
    pub const VALUE: &'static str = "list";
}

impl Serialize for ListMarker {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(Self::VALUE)
    }
}

impl<'de> Deserialize<'de> for ListMarker {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        struct V;
        impl serde::de::Visitor<'_> for V {
            type Value = ListMarker;
            fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
                write!(f, "\"{}\"", ListMarker::VALUE)
            }
            fn visit_str<E>(self, s: &str) -> Result<Self::Value, E>
            where
                E: serde::de::Error,
            {
                if s == ListMarker::VALUE {
                    Ok(ListMarker)
                } else {
                    Err(E::invalid_value(serde::de::Unexpected::Str(s), &self))
                }
            }
        }
        deserializer.deserialize_str(V)
    }
}

#[derive(Debug, Clone, Copy, PartialOrd, PartialEq, Eq)]
pub struct EmbeddingMarker;

impl EmbeddingMarker {
    pub const VALUE: &'static str = "embedding";
}

impl Serialize for EmbeddingMarker {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(Self::VALUE)
    }
}

impl<'de> Deserialize<'de> for EmbeddingMarker {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        struct V;
        impl serde::de::Visitor<'_> for V {
            type Value = EmbeddingMarker;
            fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
                write!(f, "\"{}\"", EmbeddingMarker::VALUE)
            }
            fn visit_str<E>(self, s: &str) -> Result<Self::Value, E>
            where
                E: serde::de::Error,
            {
                if s == EmbeddingMarker::VALUE {
                    Ok(EmbeddingMarker)
                } else {
                    Err(E::invalid_value(serde::de::Unexpected::Str(s), &self))
                }
            }
        }
        deserializer.deserialize_str(V)
    }
}
