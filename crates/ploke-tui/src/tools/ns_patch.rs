use std::{borrow::Cow, ops::Deref as _, sync::Arc};

use serde::{Deserialize, Serialize};

use crate::{
    rag::tools::apply_ns_code_edit_tool,
    tools::ToolResult,
    tools::validators::{validate_file_path_basic, validate_unified_diff},
};
use serde_json::json;

/// Type for non-semantic file patching
pub struct NsPatch;

// Simple description of the diff format, since the mpath crate will handle the parsing for us.
//
// NOTE:
// If we run into issues with the kind of input we are getting from LLMs, this is one place we
// could try to improve/iterate.
//
// For example, we may want to include an example like
// --- a/src/main.rs
// +++ b/src/main.rs
// @@ -1 +1 @@
// -println!("Old");
// +println!("New");
const DIFF_DESCR: &str = r#"Raw Unified Diff"#;

// TODO: Add a macro to take care of forming the json version automatically
lazy_static::lazy_static! {
    static ref NS_PATCH_PARAMETERS: serde_json::Value = serde_json::json!({
        "type": "object",
        "properties": {
            "patches": {
                "type": "array",
                "minItems": 1,
                "items": {
                    "type": "object",
                    "properties": {
                        "file": { "type": "string", "description": "Absolute or crate-root-relative file path." },
                        "diff": { "type": "string", "description": DIFF_DESCR },
                        "reasoning": {
                            "type": "string",
                            "description": "One-sentence description of why these changes are being made"
                        },
                    },
                    "required": ["file", "diff", "reasoning"],
                    "additionalProperties": false
                }
            },
            "confidence": {
                "type": "number",
                "minimum": 0.0,
                "maximum": 1.0,
                "description": "Optional confidence indicator for the edit proposal."
            }
        },
        "required": ["patches"],
    });
}

#[derive(Debug, Clone, Deserialize)]
pub struct NsPatchParams<'a> {
    #[serde(borrow)]
    pub patches: Vec<NsPatchBorrowed<'a>>,
    #[serde(default)]
    pub confidence: Option<f32>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct NsPatchBorrowed<'a> {
    #[serde(borrow)]
    pub file: Cow<'a, str>,
    #[serde(borrow)]
    pub diff: Cow<'a, str>,
    #[serde(borrow)]
    pub reasoning: Cow<'a, str>,
}

// Basically the same as `CodeEditParamsOwned`, might want to use the same type or something
// - restructure into enum?
#[derive(Debug, Clone, Serialize)]
pub struct NsPatchParamsOwned {
    pub patches: Vec<NsPatchOwned>,
    pub confidence: Option<f32>,
}

#[derive(Debug, Clone, Serialize)]
pub struct NsPatchOwned {
    pub file: String,
    pub diff: String,
    pub reasoning: String,
}

// TODO: Consider adding a field to `rag_types::ApplyCodeEditResult`, which is used by GatCodeEdit,
// and have the same return type so both can be handled in the same diff viewer
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApplyNsPatchResult {
    pub ok: bool,
    /// Number of edits staged into an EditProposal
    pub staged: usize,
    /// Number of edits applied immediately (0 unless auto-confirm is enabled and synchronous)
    pub applied: usize,
    /// Display-friendly file paths included in this proposal
    pub files: Vec<String>,
    /// Preview mode used for the summary ("diff" or "codeblock")
    pub preview_mode: String,
    /// Whether auto-confirm is enabled in config (application may proceed asynchronously)
    pub auto_confirmed: bool,
}

use super::{ToolDescr, ToolError, ToolErrorCode, ToolInvocationError, ToolName};
impl super::Tool for NsPatch {
    type Output = ApplyNsPatchResult;
    type OwnedParams = NsPatchParamsOwned;
    type Params<'de> = NsPatchParams<'de>;

    fn name() -> super::ToolName {
        super::ToolName::NsPatch
    }

    fn description() -> super::ToolDescr {
        super::ToolDescr::NsPatch
    }

    fn schema() -> &'static serde_json::Value {
        NS_PATCH_PARAMETERS.deref()
    }

    fn adapt_error(err: ToolInvocationError) -> ToolError {
        let hint = "Use an absolute path or crate-root-relative file path (e.g., \"Cargo.toml\").";
        match err {
            ToolInvocationError::Exec(ploke_error::Error::Domain(
                ploke_error::DomainError::Io { message },
            )) => ToolError::new(ToolName::NsPatch, ToolErrorCode::Io, message).retry_hint(hint),
            ToolInvocationError::Exec(ploke_error::Error::Domain(
                ploke_error::DomainError::Ui { message },
            )) => ToolError::new(ToolName::NsPatch, ToolErrorCode::InvalidFormat, message)
                .retry_hint(hint),
            other => other.into_tool_error(ToolName::NsPatch),
        }
    }

    fn build(ctx: &super::Ctx) -> Self
    where
        Self: Sized,
    {
        Self
    }

    fn deserialize_params<'a>(json: &'a str) -> Result<Self::Params<'a>, ToolInvocationError> {
        let params: NsPatchParams<'a> =
            serde_json::from_str(json).map_err(|e| ToolInvocationError::Deserialize {
                source: e,
                raw: Some(json.to_string()),
            })?;
        validate_params(&params)?;
        Ok(params)
    }

    // TODO:refactor
    // consider adding an enum to the `Edit` type in ploke/crates/ploke-tui/src/rag/utils.rs
    // instead of using NsPatchOwned, depending on how we want to handle the edit
    // proposal/application, adding a new variant to `Edit` might make it easier to make this new
    // tool work with our existing types.
    fn into_owned<'de>(params: &Self::Params<'de>) -> Self::OwnedParams {
        NsPatchParamsOwned {
            confidence: params.confidence,
            patches: params
                .patches
                .iter()
                .map(|p| NsPatchOwned {
                    file: p.file.clone().into_owned(),
                    diff: p.diff.clone().into_owned(),
                    reasoning: p.reasoning.clone().into_owned(),
                })
                .collect(),
        }
    }

    async fn execute<'de>(
        params: Self::Params<'de>,
        ctx: super::Ctx,
    ) -> Result<ToolResult, ploke_error::Error> {
        use crate::rag::utils::{ApplyCodeEditRequest, Edit, ToolCallParams};

        let typed_req = ApplyCodeEditRequest {
            confidence: params.confidence,
            edits: params
                .patches
                .iter()
                .map(|p| Edit::Patch {
                    file: p.file.clone().into_owned(),
                    diff: p.diff.clone().into_owned(),
                    reasoning: p.reasoning.clone().into_owned(),
                })
                .collect(),
        };

        let request_id = ctx.request_id;
        let call_id = ctx.call_id.clone();
        let params_env = ToolCallParams {
            state: Arc::clone(&ctx.state),
            event_bus: Arc::clone(&ctx.event_bus),
            request_id,
            parent_id: ctx.parent_id,
            name: Self::name(),
            typed_req,
            call_id,
        };
        apply_ns_code_edit_tool(params_env).await?;
        crate::tools::code_edit::print_code_edit_results(&ctx, request_id, ToolName::NsPatch).await
    }
}

fn validate_params(params: &NsPatchParams<'_>) -> Result<(), ToolInvocationError> {
    if params.patches.is_empty() {
        return Err(ToolInvocationError::Validation(
            ToolError::new(
                ToolName::NsPatch,
                ToolErrorCode::MissingField,
                "patches must contain at least one entry",
            )
            .field("patches")
            .expected("non-empty array")
            .received("empty array")
            .retry_hint("Provide at least one patch with file, diff, and reasoning."),
        ));
    }

    if params.patches.len() > 1 {
        let count = params.patches.len();
        return Err(ToolInvocationError::Validation(
            ToolError::new(
                ToolName::NsPatch,
                ToolErrorCode::InvalidFormat,
                "ns_patch currently supports a single patch per call",
            )
            .field("patches")
            .expected("array length of 1")
            .received(count.to_string())
            .retry_hint("Send one patch per tool call.")
            .retry_context(json!({ "count": count })),
        ));
    }

    for (idx, patch) in params.patches.iter().enumerate() {
        validate_file_path_basic(ToolName::NsPatch, "file", patch.file.as_ref(), false).map_err(
            |err| {
                ToolInvocationError::Validation(err.retry_context(json!({
                    "patch_index": idx,
                    "field": "file",
                })))
            },
        )?;

        validate_unified_diff(ToolName::NsPatch, "diff", patch.diff.as_ref()).map_err(|err| {
            ToolInvocationError::Validation(
                err.retry_hint("Provide a unified diff with ---/+++ headers and @@ hunks.")
                    .retry_context(json!({
                        "patch_index": idx,
                        "field": "diff",
                    })),
            )
        })?;

        if patch.reasoning.trim().is_empty() {
            return Err(ToolInvocationError::Validation(
                ToolError::new(
                    ToolName::NsPatch,
                    ToolErrorCode::InvalidFormat,
                    "reasoning must be a non-empty sentence",
                )
                .field("reasoning")
                .expected("one-sentence description")
                .received("empty string")
                .retry_hint("Provide a short sentence describing why the change is needed.")
                .retry_context(json!({
                    "patch_index": idx,
                    "field": "reasoning",
                })),
            ));
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tools::Tool;
    use serde_json::json;
    use std::borrow::Cow;

    #[test]
    fn schema_matches_expected() {
        let schema = NsPatch::schema();
        let value = schema.clone();
        let expected = json!({
            "type": "object",
            "properties": {
                "patches": {
                    "type": "array",
                    "minItems": 1,
                    "items": {
                        "type": "object",
                        "properties": {
                            "file": { "type": "string", "description": "Absolute or crate-root-relative file path." },
                            "diff": { "type": "string", "description": DIFF_DESCR },
                            "reasoning": {
                                "type": "string",
                                "description": "One-sentence description of why these changes are being made"
                            }
                        },
                        "required": ["file", "diff", "reasoning"],
                        "additionalProperties": false
                    }
                },
                "confidence": {
                    "type": "number",
                    "minimum": 0.0,
                    "maximum": 1.0,
                    "description": "Optional confidence indicator for the edit proposal."
                }
            },
            "required": ["patches"],
        });
        assert_eq!(expected, value);
    }

    #[test]
    fn into_owned_preserves_reasoning() {
        let params = NsPatchParams {
            patches: vec![NsPatchBorrowed {
                file: Cow::Borrowed("src/lib.rs"),
                diff: Cow::Borrowed("--- a\\n+++ b\\n"),
                reasoning: Cow::Borrowed("update formatting"),
            }],
            confidence: Some(0.9),
        };
        let owned = NsPatch::into_owned(&params);
        assert_eq!(owned.patches.len(), 1);
        assert_eq!(owned.patches[0].reasoning, "update formatting");
    }

    #[test]
    fn deserialize_rejects_empty_patches() {
        let err = NsPatch::deserialize_params(r#"{"patches":[]}"#).expect_err("expected error");
        match err {
            ToolInvocationError::Validation(te) => {
                assert_eq!(te.code, ToolErrorCode::MissingField);
                assert_eq!(te.field, Some("patches"));
            }
            other => panic!("unexpected error: {other:?}"),
        }
    }

    #[test]
    fn deserialize_rejects_invalid_diff() {
        let input = r#"{"patches":[{"file":"src/lib.rs","diff":"not a diff","reasoning":"fix"}]}"#;
        let err = NsPatch::deserialize_params(input).expect_err("expected error");
        match err {
            ToolInvocationError::Validation(te) => {
                assert_eq!(te.code, ToolErrorCode::MalformedDiff);
                assert_eq!(te.field, Some("diff"));
            }
            other => panic!("unexpected error: {other:?}"),
        }
    }
}
