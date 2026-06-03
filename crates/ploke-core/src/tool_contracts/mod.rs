//! Serde-owned tool argument and result shapes for persisted/replay/UI readers.
//!
//! These types are wasm-safe: they depend only on `serde` and other `ploke-core`
//! modules. Execution crates such as `ploke-tui` re-export them for tool identity.

mod decode_enums;
mod dto;
mod error;
mod ui;

pub use decode_enums::{
    PersistedToolCallArguments, PersistedToolResultContent, ToolCallArguments, ToolResultContent,
};
pub use dto::*;
pub use error::{
    ToolErrorCode, ToolErrorWire, ToolLlmErrorPayload, ToolLlmErrorValue, ToolRetryContext,
    ToolRetryContextField, ToolRetryContextValue,
};
pub use ui::{ToolUiField, ToolUiPayload, ToolVerbosity, tool_error_code_label};
