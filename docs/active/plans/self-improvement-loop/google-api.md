Date: 20-05-26

Verified surface: `cargo test -p ploke-llm google` passed 12 focused unit tests; `cargo test -p ploke-llm --features live_api_tests --no-run live_google_chat_step_forced_tool_call_success_or_quota` compiled the ignored live Google tool-call test; `cargo check -p ploke-llm`, `cargo check -p ploke-tui`, and `cargo check -p ploke-eval` passed; `cargo test -p ploke-eval merge_model_registries_prefers_direct_google_row_on_id_collision` passed; `cargo test -p ploke-eval benchmark_runtime_config_overrides_llm_timeout` passed; `cargo test -p ploke-tui test_model_router_parser_show_and_set` passed. This was not a live Google request or native TUI run.

Update 2026-05-20: `Google` now implements `HasModels` through a Google OpenAI-compatible model-list adapter in `ploke-llm`. Verified with `cargo test -p ploke-llm google` and `cargo check -p ploke-tui`; this was not a live Google request.

Update 2026-05-20: `Google` now implements `RouterCalibration` with a direct model-key calibration id, and `ploke-llm` has an ignored live `chat_step` forced-tool-call test for Google. The direct Google provider path should not fake OpenRouter endpoint/provider metadata; until TUI/eval routing is generalized, Google tool capability is carried by the Google model-list adapter. Verified by the surface above.

Update 2026-05-20: The provider-neutral route boundary now exists as `LlmRoute` in `ploke-llm`: OpenRouter routes carry provider endpoint metadata, while Google routes carry direct model capability without a provider endpoint. `RuntimeConfig` now carries an explicit `active_router`, `ploke-eval` sets it from the selected route, and `ploke-tui` live chat request construction dispatches `RouterVariants::Google` through `ChatCompRequest<Google>`.

Update 2026-05-20: Model registry rows now carry route provenance. Direct Google rows are tagged as `direct_google`, `ploke-eval model refresh` can merge OpenRouter and Google model catalogs, and direct Google rows win same-id collisions so `google/*` can resolve to the direct route without fake provider endpoints. TUI operator commands now show both OpenRouter and Google API-key diagnostics, include an active `model router [openrouter|google]` command, search the active router's model catalog, and show direct Google route information instead of provider endpoints.

Verdict: Google is partially integrated in `ploke-llm` as an OpenAI-compatible request/response shape, but it is not yet at OpenRouter parity for the TUI tool loop or eval runner.

**Already Done**
- `ploke-llm` has a `Google` router type with OpenAI-compatible constants and `GEMINI_API_KEY` auth: [google/mod.rs](/home/brasides/code/ploke/crates/ploke-llm/src/router_only/google/mod.rs:159).
- Google request serialization is wired through generic `ChatCompRequest<Google>`, including model-id conversion from `google/gemini-*` to API-facing `gemini-*`: [google/mod.rs](/home/brasides/code/ploke/crates/ploke-llm/src/router_only/google/mod.rs:135), [router_only/mod.rs](/home/brasides/code/ploke/crates/ploke-llm/src/router_only/mod.rs:426).
- Google `extra_body.google.thinking_config` and `cached_content` structs exist and are unit-tested: [google/mod.rs](/home/brasides/code/ploke/crates/ploke-llm/src/router_only/google/mod.rs:14), [google_chat_completion.rs](/home/brasides/code/ploke/crates/ploke-llm/src/request/tests/google_chat_completion.rs:15).
- The shared parser can accept Google’s OpenAI-compatible success response shape: [shape_tests.rs](/home/brasides/code/ploke/crates/ploke-llm/src/response/shape_tests.rs:6).
- The generic HTTP sender `chat_step<R: Router>` can technically send Google requests because it uses `R::COMPLETION_URL` and `R::resolve_api_key`: [session.rs](/home/brasides/code/ploke/crates/ploke-llm/src/manager/session.rs:281).
- `Google` implements `HasModels` with its own OpenAI-compatible model-list response types and an adapter into the existing shared model registry item shape: [google/mod.rs](/home/brasides/code/ploke/crates/ploke-llm/src/router_only/google/mod.rs:21).
- `Google` implements `RouterCalibration` without provider preferences or endpoint routing, using the direct model key as the calibration key.
- `ploke-llm` has an ignored live Google `chat_step` forced-tool-call test. It requires `--features live_api_tests -- --ignored` plus `GEMINI_API_KEY`.
- Provider routing now has a shared `LlmRoute` carrier. OpenRouter routes carry endpoint/provider metadata; direct Google routes carry model-level tool capability.
- `RuntimeConfig` carries an explicit active router, and `ploke-tui` live chat request construction dispatches `RouterVariants::Google` through `Google` instead of OpenRouter.
- `ploke-eval` route validation accepts direct Google routes and no longer requires Google to provide OpenRouter endpoint metadata.

**Still Missing For Parity**
- `Google` still does not implement `HasEndpoint`, intentionally. OpenRouter endpoint metadata drives provider/tool support, but direct Google has no provider endpoint selection. TUI/eval code that requires endpoint metadata must be generalized rather than fed fake Google endpoints.
- Provider pinning remains OpenRouter-specific. Direct Google routes intentionally do not persist provider endpoint preferences.
- TUI direct Google selection is now exposed through `model router google`, but it has not been exercised in a native TUI run.
- Tool-call request shape is likely compatible, but not proven live. Official Gemini OpenAI compatibility docs show `tools` plus `tool_choice="auto"` are supported, and the same docs show the `/v1beta/openai/chat/completions` and `/v1beta/openai/models` endpoints with bearer auth. See Google docs: https://ai.google.dev/gemini-api/docs/openai.

**Work To Reach Parity**
1. Done: add Google model-list response types and an adapter from Google `/openai/models` into the project’s model registry shape.
2. Done: add `impl HasModels for Google`.
3. Done: decide what replaces OpenRouter endpoint/provider metadata for Google. Direct Google has no provider endpoint selection in this integration; model/tool capability is modeled as direct-provider model capability, not fake OpenRouter endpoints.
4. Done: add `impl RouterCalibration for Google`.
5. Done: `RuntimeConfig` now carries explicit router selection, and TUI request construction dispatches Google routes through `Google` instead of inferring every live request as OpenRouter.
6. Done: add Google-aware API-key diagnostics and model commands, including direct Google route display and TUI `model router`.
7. Done: add route provenance to registry rows and refresh `ploke-eval` model registry from both OpenRouter and direct Google catalogs.
8. Partially done: add live ignored tests for Google tool calls through `chat_step`, then through `ploke-tui` recorded/live tool loop, then through `ploke-eval` headless adapter.

The next implementation slice is live/runtime validation: prove direct Google through the native TUI tool loop or headless eval adapter, then tighten any remaining persistence around active route selection.

**Next Implementation Steps**
9. Prove direct Google through the `ploke-tui` session/tool loop, not only through `ploke-llm::chat_step`.
   - Source facts: `RuntimeConfig.active_router`, the selected model id, the existing tool definitions, and the normal `LlmEvent`/tool-call session flow.
   - Semantic object: the selected LLM route, where `RouterVariants::Google` means a direct Google model route with no OpenRouter provider endpoint.
   - Projection: a recorded or live TUI run that shows Google model output entering the same tool-call loop used by OpenRouter.
   - Renderer/test surface: an ignored live test or recorded-prefix test that drives the real session loop and asserts on tool request/completion behavior.
   - Likely `ploke-tui` touch points:
     - [manager/mod.rs](/home/brasides/code/ploke/crates/ploke-tui/src/llm/manager/mod.rs:593): live request construction already branches on `active_router`; verify the Google branch carries the same tools, tool-choice policy, timeout, and message history as OpenRouter.
     - [session.rs](/home/brasides/code/ploke/crates/ploke-tui/src/llm/manager/session.rs:594): recorded and recorded-prefix providers are the right harness for replay-style validation through the real session loop.
     - [exec.rs](/home/brasides/code/ploke/crates/ploke-tui/src/app/commands/exec.rs:761): `model search` now uses the active router; this is the operator entry point to verify before a native run.
     - [core.rs](/home/brasides/code/ploke/crates/ploke-tui/src/app_state/core.rs:210): `RuntimeConfig.active_router` is the runtime authority for direct Google dispatch.

10. Carry the same route into the `ploke-eval` headless adapter.
    - Source facts: the merged model registry row, `route_source`, selected model id, optional OpenRouter provider preference, and the headless TUI `ModelSelection`.
    - Semantic object: eval-selected route, where OpenRouter routes may have provider preferences and direct Google routes must not.
    - Projection: a headless TUI attempt configured with `RouterVariants::Google` when the selected registry row is `direct_google`.
    - Renderer/test surface: a focused headless adapter test, preferably recorded-prefix first, then an ignored live canary when `GEMINI_API_KEY` is present.
    - Likely `ploke-eval` touch points:
      - [runner.rs](/home/brasides/code/ploke/crates/ploke-eval/src/runner.rs:1999): route resolution already uses `route_source`; keep direct Google on `LlmRoute::direct_google_model` and do not synthesize an endpoint.
      - [model_registry.rs](/home/brasides/code/ploke/crates/ploke-eval/src/model_registry.rs:64): registry refresh merges OpenRouter and Google catalogs; use this as the source for route provenance.
      - [cli.rs](/home/brasides/code/ploke/crates/ploke-eval/src/cli.rs:2845): `ModelSelection` construction is where OpenRouter provider preference can accidentally leak into direct Google selection.
      - [tui_adapter.rs](/home/brasides/code/ploke/crates/ploke-eval/src/cli/prototype1_state/edit_surface/tui_adapter.rs:1): the headless adapter must pass router/model intent into vanilla `ploke-tui` rather than interpreting provider endpoints itself.
      - [cli_tests.rs](/home/brasides/code/ploke/crates/ploke-eval/src/cli/prototype1_state/tests/cli_tests.rs:1218): existing ignored live headless attempt test is a candidate surface for a Google-specific canary once the adapter can select the route.

11. After runtime validation, tighten persistence and operator semantics.
    - Decide whether `active_router` is intentionally runtime-only in `ploke-tui`, or whether user config should persist it alongside the selected model.
    - Keep OpenRouter provider pinning scoped to OpenRouter. For direct Google, persisted selection should be model plus route provenance, not provider slug.
    - Add or adjust operator text only as a renderer over the route object: `openrouter` can show providers/endpoints; `google` should show direct route capability and key status.
