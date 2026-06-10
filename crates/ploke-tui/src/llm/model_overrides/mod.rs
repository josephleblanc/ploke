//! Model-specific request quirk overrides.
//!
//! A small, non-ad-hoc registry mapping `(router, model)` to request
//! adaptations that work around documented per-model API quirks. Each entry
//! lives in its own submodule, owns its match predicate, and returns a typed
//! [`ModelOverride`]. [`resolve`] dispatches to the entries in order.
//!
//! The first (and currently only) entry mitigates the direct Google Gemini
//! `MALFORMED_FUNCTION_CALL` quirk; see
//! `docs/active/bugs/2026-06-10-direct-google-malformed-function-call-finish-reason.md`.

use ploke_llm::request::ToolChoice;
use ploke_llm::router_only::RouterVariants;
use ploke_llm::types::model_types::ModelId;

mod google_gemini;

/// Per-model LLM parameter overrides (token budget, temperature, ...).
///
/// Extension point: add fields here as model-specific parameter quirks are
/// discovered, then apply them at the request-build chokepoint alongside
/// [`ModelOverride::tool_choice`].
#[derive(Debug, Clone, Default)]
pub struct ParamOverrides {
    /// Minimum `max_tokens` to request for this model. The chokepoint raises the
    /// effective `max_tokens` to this floor when the request leaves it unset or
    /// requests a smaller budget; it never lowers a larger explicit budget.
    ///
    /// Motivation: direct-Google Gemini truncates a structured tool call when
    /// the output-token budget is too small, surfacing as
    /// `MALFORMED_FUNCTION_CALL`. A generous floor lets the model finish
    /// emitting the call. See the malformed-function-call bug doc.
    pub max_tokens_floor: Option<u32>,
}

impl ParamOverrides {
    /// Apply the parameter overrides to an effective `max_tokens`, returning the
    /// adjusted value. Raises to [`Self::max_tokens_floor`] only when the
    /// current value is unset or below the floor; an explicit larger budget is
    /// preserved.
    pub fn effective_max_tokens(&self, current: Option<u32>) -> Option<u32> {
        match (self.max_tokens_floor, current) {
            (Some(floor), Some(value)) => Some(value.max(floor)),
            (Some(floor), None) => Some(floor),
            (None, value) => value,
        }
    }
}

/// Typed carrier of per-model request adaptations resolved at the
/// request-build chokepoint. Start minimal; extend as new quirks are
/// registered.
#[derive(Debug, Clone)]
pub struct ModelOverride {
    /// Recommended tool-selection override for this model, if any.
    pub tool_choice: Option<ToolChoice>,
    /// Per-model parameter overrides (currently empty; see [`ParamOverrides`]).
    pub params: ParamOverrides,
}

/// Resolve the request overrides for `(router, model_id)`, if any entry
/// matches. Returns `None` for models with no registered quirk.
pub fn resolve(router: RouterVariants, model_id: &ModelId) -> Option<ModelOverride> {
    google_gemini::resolve(router, model_id)
}

#[cfg(test)]
mod tests {
    use super::*;
    use ploke_llm::router_only::google::Google;
    use ploke_llm::router_only::openrouter::OpenRouter;

    fn model(id: &str) -> ModelId {
        id.parse().expect("valid model id")
    }

    #[test]
    fn resolve_sets_max_tokens_floor_without_tool_choice_for_direct_google_gemini_families() {
        let router = RouterVariants::Google(Google);

        for slug in ["google/gemini-2.5-flash", "google/gemini-3.5-flash"] {
            let resolved = resolve(router, &model(slug))
                .unwrap_or_else(|| panic!("expected override for direct Google {slug}"));
            // Forcing a tool choice was empirically useless and loop-trapping;
            // the mitigation is a token-budget floor, not a tool choice.
            assert!(
                resolved.tool_choice.is_none(),
                "no tool_choice override expected for {slug}, got {:?}",
                resolved.tool_choice
            );
            let floor = resolved
                .params
                .max_tokens_floor
                .unwrap_or_else(|| panic!("expected a max_tokens floor for {slug}"));
            assert!(
                floor >= 8000,
                "max_tokens floor for {slug} must be generous (>=8000), got {floor}"
            );
        }
    }

    #[test]
    fn effective_max_tokens_raises_only_when_unset_or_below_floor() {
        let params = ParamOverrides {
            max_tokens_floor: Some(8192),
        };
        assert_eq!(params.effective_max_tokens(None), Some(8192));
        assert_eq!(params.effective_max_tokens(Some(1024)), Some(8192));
        assert_eq!(params.effective_max_tokens(Some(16384)), Some(16384));

        let empty = ParamOverrides::default();
        assert_eq!(empty.effective_max_tokens(Some(1024)), Some(1024));
        assert_eq!(empty.effective_max_tokens(None), None);
    }

    #[test]
    fn resolve_none_for_openrouter_google_route() {
        let router = RouterVariants::OpenRouter(OpenRouter);
        assert!(
            resolve(router, &model("google/gemini-2.5-flash")).is_none(),
            "OpenRouter-routed Google models must not match the direct-Google quirk"
        );
    }

    #[test]
    fn resolve_none_for_non_gemini_direct_google_model() {
        let router = RouterVariants::Google(Google);
        assert!(
            resolve(router, &model("google/gemini-1.5-flash")).is_none(),
            "only the affected gemini-2.5/3.5 families should match"
        );
    }
}
