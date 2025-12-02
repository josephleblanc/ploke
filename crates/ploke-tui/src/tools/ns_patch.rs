use std::{borrow::Cow, ops::Deref as _, sync::Arc};

use serde::{Deserialize, Serialize};

use crate::{rag::tools::apply_ns_code_edit_tool, tools::ToolResult};

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
                "items": {
                    "type": "object",
                    "properties": {
                        "file": { "type": "string", "description": "Absolute or workspace-relative file path." },
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

// TODO: 
//  - [ ] add tests
//      - [ ]  verify that NS_PATCH_PARAMETERS serializes into Value correctly.

use super::{ ToolName, ToolDescr };
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

    fn build(ctx: &super::Ctx) -> Self
    where
        Self: Sized,
    {
        Self
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
                    reasoning: p.diff.clone().into_owned(),
                })
                .collect(),
        }
    }

    async fn execute<'de>(
        params: Self::Params<'de>,
        ctx: super::Ctx,
    ) -> Result<ToolResult, ploke_error::Error> {
        use crate::rag::tools::apply_code_edit_tool;
        use crate::rag::utils::{ApplyCodeEditRequest, Edit, ToolCallParams};


        let typed_req = ApplyCodeEditRequest {
            confidence: params.confidence,
            edits: params
                .patches
                .iter()
                .cloned()
                .map(|p| Edit::Patch {
                    file: p.file.clone().into_owned(),
                    diff: p.diff.clone().into_owned(),
                    reasoning: p.diff.clone().into_owned(),
                })
                .collect(),
        };

        let request_id = ctx.request_id;
        let call_id = ctx.call_id;
        let params_env = ToolCallParams {
            state: Arc::clone(&ctx.state),
            event_bus: Arc::clone(&ctx.event_bus),
            request_id,
            parent_id: ctx.parent_id,
            name: Self::name(),
            typed_req,
            call_id,
        };
        // build result from proposal registry
        let proposal_opt = { 
            ctx.state.proposals.read().await
                .get(&request_id).cloned() };
        todo!("This is where we would handle the result of trying to stage the proposed edit.");
        apply_ns_code_edit_tool(params_env);
        crate::tools::code_edit::print_code_edit_results(&ctx, request_id).await
    }
}
