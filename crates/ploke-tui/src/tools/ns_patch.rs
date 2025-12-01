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

#[derive(Debug, Clone, Serialize)]
pub struct CodeEditParamsOwned {
    pub patches: Vec<PatchOwned>,
    pub confidence: Option<f32>,
}

#[derive(Debug, Clone, Serialize)]
pub struct PatchOwned {
    pub file: String,
    pub diff: String,
    pub reasoning: String,
}

// TODO: 
//  - [ ] add tests
//      - [ ]  verify that NS_PATCH_PARAMETERS serializes into Value correctly.

impl super::Tool for NsPatch {
    type Params<'de> = NsPatchParams<'de>;

    use super::{ ToolName, ToolDescr };
    type Output;

    type OwnedParams;

    type Params<'de>
    where
        Self: 'de;

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

    fn into_owned<'de>(params: &Self::Params<'de>) -> Self::OwnedParams {
        todo!()
    }

    fn execute<'de>(
        params: Self::Params<'de>,
        ctx: super::Ctx,
    ) -> impl std::future::Future<Output = Result<super::ToolResult, ploke_error::Error>> + Send
    {
        todo!()
    }
}
