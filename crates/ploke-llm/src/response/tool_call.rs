use crate::utils::se_de::{de_arc_str, se_arc_str};
use ploke_core::{
    ArcStr,
    tool_types::{FunctionMarker, ToolName},
};
use serde::{Deserialize, Serialize};

#[derive(Deserialize, Serialize, Debug, Clone, PartialOrd, PartialEq)]
pub struct ToolCall {
    #[serde(
        deserialize_with = "de_arc_str",
        serialize_with = "se_arc_str",
        rename = "id"
    )]
    pub call_id: ArcStr,

    #[serde(rename = "type")]
    pub call_type: FunctionMarker,
    pub function: FunctionCall,
}

#[derive(Deserialize, Serialize, Debug, Clone, PartialOrd, PartialEq)]
pub struct FunctionCall {
    pub name: ToolName,
    // Store raw JSON arguments - needs to be owned String for deserialization from OpenRouter
    pub arguments: String,
}
