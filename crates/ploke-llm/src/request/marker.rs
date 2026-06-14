use serde::{Deserialize, Serialize};

// Marker for response_format -> { "type": "json_object" }
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct JsonObjMarker;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ResponseFormat {
    #[serde(rename = "json_object")]
    JsonObject,
    #[serde(rename = "json_schema")]
    JsonSchema {
        json_schema: JsonSchemaResponseFormat,
    },
}

impl ResponseFormat {
    pub fn json_schema(name: impl Into<String>, strict: bool, schema: serde_json::Value) -> Self {
        Self::JsonSchema {
            json_schema: JsonSchemaResponseFormat {
                name: name.into(),
                strict,
                schema,
            },
        }
    }

    pub fn json_schema_name(&self) -> Option<&str> {
        match self {
            Self::JsonSchema { json_schema } => Some(json_schema.name.as_str()),
            Self::JsonObject => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct JsonSchemaResponseFormat {
    pub name: String,
    #[serde(default)]
    pub strict: bool,
    pub schema: serde_json::Value,
}

impl From<JsonObjMarker> for ResponseFormat {
    fn from(_: JsonObjMarker) -> Self {
        Self::JsonObject
    }
}

impl Serialize for JsonObjMarker {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        ResponseFormat::JsonObject.serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for JsonObjMarker {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        match ResponseFormat::deserialize(deserializer)? {
            ResponseFormat::JsonObject => Ok(JsonObjMarker),
            ResponseFormat::JsonSchema { .. } => Err(serde::de::Error::custom(
                "expected response_format type 'json_object'",
            )),
        }
    }
}
