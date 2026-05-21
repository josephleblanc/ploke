Date: 20-05-26

Verified surface: `cargo test -p ploke-llm google` passed 12 focused unit tests; `cargo test -p ploke-llm google_models_fixture_adapts_to_direct_registry_rows` passed fixture-backed Google `/openai/models` deserialization and direct-route adaptation; `cargo test -p ploke-llm --features live_api_tests --no-run live_google_chat_step_forced_tool_call_success_or_quota` compiled the ignored live Google tool-call test; `cargo test -p ploke-llm --features live_api_tests live_google_chat_step_forced_tool_call_success_or_quota -- --ignored --nocapture` passed a live Google forced `chat_step` tool call; `cargo test -p ploke-tui direct_google_` passed direct-Google model-browser selection/expand tests; `cargo test -p ploke-tui openrouter_row_still_requests_endpoints_before_selection` passed the OpenRouter compatibility picker test; `cargo test -p ploke-tui source_badges_and_details_distinguish_openrouter_from_google_rows` passed model-browser source badge rendering coverage; `cargo test -p ploke-tui model_browser_help_explains_source_badges` passed model-browser overlay help coverage; `cargo test -p ploke-tui help_commands_includes_registry_sections_and_footer` passed global `?` help coverage; `cargo test -p ploke-tui live_google_chat_session_executes_list_dir_tool_call_success_or_quota -- --ignored --nocapture` passed a live Google TUI session-loop canary that executes `list_dir` through the event-bus tool path; `cargo run -p ploke-tui` passed a native interactive PTY smoke after selecting `model router google`, switching to `google/gemini-2.5-flash`, submitting `Reply with exactly: google-native-smoke-ok`, and receiving `google-native-smoke-ok`; `cargo check -p ploke-llm`, `cargo check -p ploke-tui`, and `cargo check -p ploke-eval` passed; `cargo test -p ploke-eval merge_model_registries_prefers_direct_google_row_on_id_collision` passed; `cargo test -p ploke-eval benchmark_runtime_config_overrides_llm_timeout` passed; `cargo test -p ploke-tui test_model_router_parser_show_and_set` passed. This was not a headless eval adapter run.

Update 2026-05-20: `Google` now implements `HasModels` through a Google OpenAI-compatible model-list adapter in `ploke-llm`. Verified with `cargo test -p ploke-llm google` and `cargo check -p ploke-tui`; this was not a live Google request.

Update 2026-05-20: `Google` now implements `RouterCalibration` with a direct model-key calibration id, and `ploke-llm` has an ignored live `chat_step` forced-tool-call test for Google. The direct Google provider path should not fake OpenRouter endpoint/provider metadata; until TUI/eval routing is generalized, Google tool capability is carried by the Google model-list adapter. Verified by the surface above.

Update 2026-05-20: The provider-neutral route boundary now exists as `LlmRoute` in `ploke-llm`: OpenRouter routes carry provider endpoint metadata, while Google routes carry direct model capability without a provider endpoint. `RuntimeConfig` now carries an explicit `active_router`, `ploke-eval` sets it from the selected route, and `ploke-tui` live chat request construction dispatches `RouterVariants::Google` through `ChatCompRequest<Google>`.

Update 2026-05-20: Model registry rows now carry route provenance. Direct Google rows are tagged as `direct_google`, `ploke-eval model refresh` can merge OpenRouter and Google model catalogs, and direct Google rows win same-id collisions so `google/*` can resolve to the direct route without fake provider endpoints. TUI operator commands now show both OpenRouter and Google API-key diagnostics, include an active `model router [openrouter|google]` command, search the active router's model catalog, and show direct Google route information instead of provider endpoints.

Update 2026-05-21: Live Google now reaches the `ploke-tui` session/tool loop. The ignored canary `live_google_chat_session_executes_list_dir_tool_call_success_or_quota` constructs `ChatSession<Google>`, lets Google choose tools with `tool_choice=auto`, executes a `list_dir` tool call through the normal event-bus `ToolCallRequested`/`process_tool`/`ToolCallCompleted` path, and receives a final assistant response in the same session. This proves the TUI session loop can consume a live Google tool call, but it is still not a native interactive TUI run and does not yet prove the `ploke-eval` headless adapter route.

Update 2026-05-21: Native interactive TUI routing now works with direct Google. In a real `cargo run -p ploke-tui` PTY session, `/model router google` set `RuntimeConfig.active_router`, `/model use google/gemini-2.5-flash` selected a direct Google Gemini model, and a normal submitted user prompt received the exact live assistant response `google-native-smoke-ok`. The same run also showed `google/gemini-2.0-flash` is no longer available for this account: Google returned 404 with "This model models/gemini-2.0-flash is no longer available to new users." Future live smokes should prefer `google/gemini-2.5-flash` unless the refreshed model catalog says otherwise.

Update 2026-05-21: The model picker now treats direct Google rows as direct routes instead of OpenRouter rows with missing providers. Google search results carry `route_source = direct_google` into `ModelBrowserItem`, pressing `s` selects the model with no provider pin, and expanding a direct row does not request OpenRouter endpoints. Fixture-backed Google model-list coverage now parses `test_data/google/openai_models.json` and verifies direct route provenance plus tool-capability filtering.

Update 2026-05-21: The model picker now renders route-source badges on every row. OpenRouter-supplied rows show `[openrouter]`, direct Google rows show `[google]`, the badges use distinct colors, expanded rows show the source detail, and both overlay `? Help` plus global help explain how to read overlapping model ids.

Verdict: Google is integrated through `ploke-llm`, the `ploke-tui` model picker, the `ploke-tui` session-loop tool path, and native interactive TUI routing. It is not yet at OpenRouter parity for the `ploke-eval` headless adapter.

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
- `ploke-llm` has fixture-backed Google model-list parsing and adaptation tests for the direct model catalog.
- `ploke-tui` model picker direct Google rows now select without provider endpoints and preserve the OpenRouter endpoint-fetch path for OpenRouter rows.
- `ploke-tui` model picker rows show colored `[openrouter]` and `[google]` source badges, and help text explains the distinction for overlapping ids.
- `ploke-tui` has an ignored live Google session-loop canary that proves a Google tool call can enter the normal event-bus tool execution path and complete `list_dir`.
- Native `ploke-tui` can select `model router google`, switch to `google/gemini-2.5-flash`, submit a normal user prompt, and receive a live Google assistant response.

**Still Missing For Parity**
- `Google` still does not implement `HasEndpoint`, intentionally. OpenRouter endpoint metadata drives provider/tool support, but direct Google has no provider endpoint selection. Any remaining code that requires endpoint metadata must be generalized rather than fed fake Google endpoints.
- Provider pinning remains OpenRouter-specific. Direct Google routes intentionally do not persist provider endpoint preferences.
- Google tool calls are proven live at `ploke-llm::chat_step` and through the `ploke-tui` session loop. Native operator routing is proven for a normal chat response. The remaining proof is direct Google selection through the `ploke-eval` headless adapter.

**Work To Reach Parity**
1. Done: add Google model-list response types and an adapter from Google `/openai/models` into the project’s model registry shape.
2. Done: add `impl HasModels for Google`.
3. Done: decide what replaces OpenRouter endpoint/provider metadata for Google. Direct Google has no provider endpoint selection in this integration; model/tool capability is modeled as direct-provider model capability, not fake OpenRouter endpoints.
4. Done: add `impl RouterCalibration for Google`.
5. Done: `RuntimeConfig` now carries explicit router selection, and TUI request construction dispatches Google routes through `Google` instead of inferring every live request as OpenRouter.
6. Done: add Google-aware API-key diagnostics and model commands, including direct Google route display and TUI `model router`.
7. Done: add route provenance to registry rows and refresh `ploke-eval` model registry from both OpenRouter and direct Google catalogs.
8. Done for the basic catalog/picker path: fixture-backed Google model-list tests cover direct route provenance, and the TUI model picker no longer requests OpenRouter endpoints for direct Google rows. The picker also labels each row as `[openrouter]` or `[google]` so overlapping ids remain visually distinct.
9. Partially done: live ignored tests now cover Google tool calls through `chat_step` and one `ploke-tui` session-loop tool execution, and a native interactive TUI smoke covers operator route selection plus a live Google chat response. Still missing: `ploke-eval` headless adapter.

The next implementation slice is eval/runtime validation: carry direct Google through the `ploke-eval` headless adapter, then tighten any remaining persistence around active route selection.

**Next Implementation Steps**
10. Done for the session layer: prove direct Google through the `ploke-tui` session/tool loop, not only through `ploke-llm::chat_step`.
   - Source facts: `RuntimeConfig.active_router`, the selected model id, the existing tool definitions, and the normal `LlmEvent`/tool-call session flow.
   - Semantic object: the selected LLM route, where `RouterVariants::Google` means a direct Google model route with no OpenRouter provider endpoint.
   - Projection: the ignored live session canary shows Google model output entering the same event-bus tool-call loop used by OpenRouter.
   - Renderer/test surface: `cargo test -p ploke-tui live_google_chat_session_executes_list_dir_tool_call_success_or_quota -- --ignored --nocapture`.
   - Likely `ploke-tui` touch points:
     - [manager/mod.rs](/home/brasides/code/ploke/crates/ploke-tui/src/llm/manager/mod.rs:593): live request construction already branches on `active_router`; verify the Google branch carries the same tools, tool-choice policy, timeout, and message history as OpenRouter.
     - [session.rs](/home/brasides/code/ploke/crates/ploke-tui/src/llm/manager/session.rs:594): recorded and recorded-prefix providers are the right harness for replay-style validation through the real session loop.
     - [exec.rs](/home/brasides/code/ploke/crates/ploke-tui/src/app/commands/exec.rs:761): `model search` now uses the active router; this is the operator entry point to verify before a native run.
     - [core.rs](/home/brasides/code/ploke/crates/ploke-tui/src/app_state/core.rs:210): `RuntimeConfig.active_router` is the runtime authority for direct Google dispatch.

11. Done for the native operator layer: prove direct Google through a real interactive TUI session, not only a test harness.
    - Source facts: operator command input, `RuntimeConfig.active_router`, selected `active_model`, and the normal submitted user prompt path.
    - Projection: `/model router google` followed by `/model use google/gemini-2.5-flash` makes the production TUI dispatch a live chat request through `ChatCompRequest<Google>`.
    - Renderer/test surface: `cargo run -p ploke-tui` in a PTY; observed `Active model router: google`, model switch to `google/gemini-2.5-flash`, submitted `Reply with exactly: google-native-smoke-ok`, received `google-native-smoke-ok`.
    - Operational note: `google/gemini-2.0-flash` returned 404 as unavailable to new users during the same smoke.

12. Carry the same route into the `ploke-eval` headless adapter.
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

13. After runtime validation, tighten persistence and operator semantics.
    - Decide whether `active_router` is intentionally runtime-only in `ploke-tui`, or whether user config should persist it alongside the selected model.
    - Keep OpenRouter provider pinning scoped to OpenRouter. For direct Google, persisted selection should be model plus route provenance, not provider slug.
    - Add or adjust operator text only as a renderer over the route object: `openrouter` can show providers/endpoints; `google` should show direct route capability and key status.
