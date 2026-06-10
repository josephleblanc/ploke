//! First model-override entry: the direct Google (Vertex) Gemini
//! `MALFORMED_FUNCTION_CALL` quirk.
//!
//! Observed behavior: direct-Google `gemini-2.5*`/`gemini-3.5*` return a
//! `MALFORMED_FUNCTION_CALL` finish reason when a structured tool call (e.g. a
//! multi-line unified-diff argument) is truncated by a too-small output-token
//! budget. The production broad-patch turn sends no `max_tokens` at all
//! (`LLMParameters::default().max_tokens == None`), so Vertex applies a small
//! default that truncates the call; thinking models (gemini-2.5-pro) make this
//! worse because reasoning tokens consume the budget before the call is
//! emitted.
//!
//! Empirically verified (live spike, 2026-06-10): bumping the budget to 8192
//! eliminated the malformation for the SAME `tool_choice=auto` + string-diff
//! schema, while `tool_choice=required` did NOT fix it. The mitigation is
//! therefore a `max_tokens` floor, not a forced tool choice.
//!
//! Floor raised to 16384 after the state7 live run (gemini-2.5-pro): at 8192 the
//! malformation was gone but the thinking model's reasoning tokens consumed the
//! budget before the patch finished, yielding `finish_reason=length`
//! (OUTPUT_TRUNCATED). 16384 gives think+patch enough room; it is a cap, not
//! forced spend (the model stops when done). See
//! `docs/active/bugs/2026-06-10-direct-google-malformed-function-call-finish-reason.md`.

use ploke_llm::router_only::RouterVariants;
use ploke_llm::types::model_types::ModelId;

use super::{ModelOverride, ParamOverrides};

/// Model-family prefixes affected by the direct-Google malformed-function-call
/// quirk.
const AFFECTED_PREFIXES: &[&str] = &["gemini-2.5", "gemini-3.5"];

/// Output-token floor for affected direct-Google Gemini requests. 8192 was
/// proven to eliminate the malformation; raised to 16384 so the thinking model
/// (gemini-2.5-pro) has room for reasoning + a full patch without truncating
/// (`finish_reason=length`). This is a cap, not forced spend.
const MAX_TOKENS_FLOOR: u32 = 16384;

pub(super) fn resolve(router: RouterVariants, model_id: &ModelId) -> Option<ModelOverride> {
    if !is_affected(router, model_id) {
        return None;
    }
    Some(ModelOverride {
        // `Required` was empirically shown NOT to eliminate the malformation and
        // would trap the session loop (every turn forced to emit a tool call),
        // so no tool-choice override is recommended.
        tool_choice: None,
        params: ParamOverrides {
            max_tokens_floor: Some(MAX_TOKENS_FLOOR),
        },
    })
}

/// Match the direct Google (Vertex) route AND an affected Gemini family prefix.
///
/// OpenRouter-routed Google models travel through `RouterVariants::OpenRouter`
/// and are intentionally NOT matched: the malformation is specific to the
/// Vertex OpenAI-compatible direct path.
fn is_affected(router: RouterVariants, model_id: &ModelId) -> bool {
    matches!(router, RouterVariants::Google(_))
        && AFFECTED_PREFIXES
            .iter()
            .any(|prefix| model_id.key.slug.as_str().starts_with(prefix))
}
