//! Passive JSON value carrier for embedded upstream payloads.
//!
//! This avoids making `serde_json` part of the library surface while still
//! allowing records to carry opaque JSON subtrees from upstream schemas.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

/// Opaque JSON-compatible value embedded in passive record schemas.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum JsonRecordValue {
    Null,
    Bool(bool),
    I64(i64),
    U64(u64),
    F64(f64),
    String(String),
    Array(Vec<JsonRecordValue>),
    Object(BTreeMap<String, JsonRecordValue>),
}
