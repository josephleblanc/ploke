# Implementation log 022 — Close gaps between tests and real tool use; improve logging and provider handling (2025-08-21)

Summary
- We are receiving 400 errors from OpenRouter: "Unrecognized key(s) in object: 'allow', 'deny'." This indicates our request body included a `provider` object with keys not accepted by the /chat/completions endpoint for the selected model/provider.
- Our current tests pass because they assert payload shapes that include a `provider` field; however, in real use this field is rejected by the API and prevents tool flows from starting.
- We also lack concise, structured logs that summarize whether tools will be used, which tools were offered, and the provider/tool-support decision path. This makes diagnosing failures cumbersome.

Observed vs Tested Gaps
- Test payloads vs Real API:
  - Tests assert a `provider` field with `{ allow, deny, order }`, but real API returns 400 for these keys on chat completions. Our tests did not include an integration step against the live endpoint and therefore did not detect this mismatch.
- Tool support path:
  - Code computes `supports_tools_cache` and honors `require_tool_support`, but logs did not concisely show:
    - model, provider_type, base_url, provider_slug
    - supports_tools_cache
    - require_tool_support (enforcement)
    - planned/use_tools and the list of tool names being sent
  - The 404 "support tool" fallback path existed but emitted a generic warning without provider/model details.
- Provider selection UX:
  - We do have a model browser and a StateCommand::SelectModelProvider to pin a model to a specific provider (OpenRouter endpoint). However, we do not currently expose an explicit CLI command like `provider select <model_id> <provider_slug>`; users relying solely on command mode might not be able to reliably pick a tool-capable provider.

Changes in this PR
- Removed the `provider` field from outgoing chat requests to OpenRouter (workaround for 400 on unrecognized keys). Updated unit tests accordingly.
- Added structured, concise logging fields to help triage tool path decisions:
  - At plan time: model, base_url, provider_type, provider_slug, supports_tools_cache, require_tool_support.
  - At dispatch time: use_tools and tool names included.
  - On fallback: model and provider_slug plus the error message that triggered a retry without tools.
  - On API error: status + first body for quick diagnosis.
- Wrote this report summarizing the test vs real gaps and proposing follow-ups.

Why this fixes the immediate issue
- The "Unrecognized key(s): 'allow', 'deny'" error blocked requests before any tool calls could be negotiated. Removing the invalid field allows the model to proceed and either call tools or respond directly. The new logs reveal what the runtime believed about tool capability and what it actually sent.

Docs that are helpful for this investigation
- crates/ploke-tui/docs/openrouter/tool_calling.md
  - Confirms the standard chat/completions interface and shows canonical request bodies. Notably, their examples do not include a `provider` field.
- crates/ploke-tui/docs/openrouter/list_endpoints_for_model.md
  - Useful for reasoning about provider-specific endpoints and understanding that models can have multiple endpoints with different capabilities.
- crates/ploke-tui/docs/openrouter/available_providers.md
  - Useful to understand available upstream providers and their policies; helpful context for provider selection UX.

Responding to the suspicion about provider selection and tool support
- Agree. The primary failure mode here is a mismatch between the selected provider endpoint and the request body we construct. Even if a model supports tools, some endpoints may not, and additional router preferences may be rejected.
- We already support selection via the model browser (StateCommand::SelectModelProvider). For CLI parity and clarity, we should add a command, for example:
  - `provider select <model_id> <provider_slug>`
  - This would call the same state command we already handle in dispatcher and set `active_provider`, ensuring the chosen provider is used.
- We should also expose "tools-only" enforcement toggles (already present: `provider tools-only on|off`) and ensure `model refresh` repopulates capabilities and tool support signals.

Suggested follow-ups (might require editing or accessing these files)
- CLI command to select provider:
  - crates/ploke-tui/src/app/commands/parser.rs (add Command::ProviderSelect)
  - crates/ploke-tui/src/app/commands/exec.rs (handle ProviderSelect to send StateCommand::SelectModelProvider)
- Provider registry and capabilities:
  - crates/ploke-tui/src/llm/registry.rs (if we want to enrich defaults or handle provider-specific quirks)
  - crates/ploke-tui/src/user_config.rs (optional: validate provider_slug on switch)
- Improve capability checks:
  - crates/ploke-tui/src/user_config.rs (use refreshed capabilities to warn when selecting a provider that is not tool-capable while tools-only is enforced)
- Optional: add an integration test harness (offline mocks) to assert that our outgoing request omits invalid fields for OpenRouter chat/completions.

Focused logging fields added
- Plan:
  - model, base_url, provider_type, provider_slug, supports_tools_cache, require_tool_support
- Dispatch:
  - use_tools, tools (comma-separated function names)
- Fallback (404 tool unsupported):
  - model, provider_slug, error_text
- API error summary:
  - status, model, error_text

Notes
- If OpenRouter reintroduces a supported `provider` shape for chat/completions, we can gate it behind a feature flag or provider-type detection. For now, removing it unblocks tool calls and prevents 400s.
- We preserved the "tools-only" enforcement semantics which will fail early if the active model is not marked tool-capable in the cache; the new logs make that decision visible.
