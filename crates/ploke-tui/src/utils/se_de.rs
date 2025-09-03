use ploke_core::ArcStr;
use serde::{Deserialize as _, Deserializer, Serializer};
use serde_json::Value;

pub(crate) fn de_arc_str<'de, D>(deserializer: D) -> Result<ArcStr, D::Error>
where
    D: Deserializer<'de>,
{
    let s = String::deserialize(deserializer)?; // allocate a String
    Ok(ArcStr::from(s)) // convert into Arc<str> without another copy
}

/// Serialize an `Arc<str>` as a JSON string without copying the contents.
pub(crate) fn se_arc_str<S>(value: &ArcStr, serializer: S) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    serializer.serialize_str(value)
}

pub(crate) fn string_or_f64_opt<'de, D>(deserializer: D) -> Result<Option<f64>, D::Error>
where
    D: Deserializer<'de>,
{
    let value = Option::<Value>::deserialize(deserializer)?;
    match value {
        Some(Value::Number(n)) => n.as_f64().map(Some).ok_or_else(|| serde::de::Error::custom("Invalid number")),
        Some(Value::String(s)) => s.parse().map(Some).map_err(serde::de::Error::custom),
        Some(_) => Err(serde::de::Error::custom("Expected number or string")),
        None => Ok(None),
    }
}

pub(crate) fn string_or_f64<'de, D>(deserializer: D) -> Result<f64, D::Error>
where
    D: Deserializer<'de>,
{
    let value = Value::deserialize(deserializer)?;
    match value {
        Value::Number(n) => n.as_f64().ok_or_else(|| serde::de::Error::custom("Invalid number")),
        Value::String(s) => s.parse().map_err(serde::de::Error::custom),
        _ => Err(serde::de::Error::custom("Expected number or string")),
    }
}
