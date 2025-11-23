use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::borrow::Borrow;
use std::fmt;
use std::ops::Deref;
use std::sync::Arc;

#[repr(transparent)]
#[derive(Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Default)]
pub struct ArcStr(pub Arc<str>);

impl fmt::Debug for ArcStr {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // Debug prints with quotes like a normal &str
        fmt::Debug::fmt(self.as_ref(), f)
    }
}

impl Deref for ArcStr {
    type Target = str;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl AsRef<str> for ArcStr {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

impl Borrow<str> for ArcStr {
    fn borrow(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for ArcStr {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

// Conversions
impl From<Arc<str>> for ArcStr {
    fn from(a: Arc<str>) -> Self {
        ArcStr(a)
    }
}

impl From<ArcStr> for Arc<str> {
    fn from(a: ArcStr) -> Self {
        a.0
    }
}

impl From<String> for ArcStr {
    fn from(s: String) -> Self {
        ArcStr(Arc::<str>::from(s)) // no extra copy
    }
}

impl From<&str> for ArcStr {
    fn from(s: &str) -> Self {
        // Allocates once to own the data (required to create an Arc<str>)
        ArcStr(Arc::<str>::from(s))
    }
}

// Serde
impl Serialize for ArcStr {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&self.0)
    }
}

impl<'de> Deserialize<'de> for ArcStr {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        // Exactly one allocation for the owned string buffer.
        let s = String::deserialize(deserializer)?;
        Ok(ArcStr(Arc::<str>::from(s)))
    }
}
