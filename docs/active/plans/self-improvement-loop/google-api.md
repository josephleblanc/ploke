Date: 20-05-26

Commit anchors:
- `364683fb Clarify model picker route source labels`: latest committed UI clarification. OpenRouter-supplied rows now render `[via OpenRouter]`, direct Google rows render `[via Google API]`, source details and help text explain overlapping `google/*` ids, and the screenshot-shaped regression case is covered.
- `0760b42a Add direct Google model picker parity`: direct Google model-list fixture parsing, registry adaptation, direct route selection, and model-picker behavior without OpenRouter endpoint fetches.

Verified surface: `cargo test -p ploke-llm google` passed 12 focused unit tests; `cargo test -p ploke-llm google_models_fixture_adapts_to_direct_registry_rows` passed fixture-backed Google `/openai/models` deserialization and direct-route adaptation; `cargo test -p ploke-llm --features live_api_tests --no-run live_google_chat_step_forced_tool_call_success_or_quota` compiled the ignored live Google tool-call test; `cargo test -p ploke-llm --features live_api_tests live_google_chat_step_forced_tool_call_success_or_quota -- --ignored --nocapture` passed a live Google forced `chat_step` tool call; `cargo test -p ploke-tui direct_google_` passed direct-Google model-browser selection/expand tests; `cargo test -p ploke-tui openrouter_row_still_requests_endpoints_before_selection` passed the OpenRouter compatibility picker test; `cargo test -p ploke-tui source_badges_and_details_distinguish_openrouter_from_google_rows` passed model-browser source badge rendering coverage; `cargo test -p ploke-tui model_browser_help_explains_source_badges` passed model-browser overlay help coverage; `cargo test -p ploke-tui help_commands_includes_registry_sections_and_footer` passed global `?` help coverage; `cargo test -p ploke-tui live_google_chat_session_executes_list_dir_tool_call_success_or_quota -- --ignored --nocapture` passed a live Google TUI session-loop canary that executes `list_dir` through the event-bus tool path; `cargo run -p ploke-tui` passed a native interactive PTY smoke after selecting `model router google`, switching to `google/gemini-2.5-flash`, submitting `Reply with exactly: google-native-smoke-ok`, and receiving `google-native-smoke-ok`; `cargo check -p ploke-llm`, `cargo check -p ploke-tui`, and `cargo check -p ploke-eval` passed; `cargo test -p ploke-eval merge_model_registries_prefers_direct_google_row_on_id_collision` passed; `cargo test -p ploke-eval benchmark_runtime_config_overrides_llm_timeout` passed; `cargo test -p ploke-tui test_model_router_parser_show_and_set` passed. This was not a headless eval adapter run.

Update 2026-05-20: `Google` now implements `HasModels` through a Google OpenAI-compatible model-list adapter in `ploke-llm`. Verified with `cargo test -p ploke-llm google` and `cargo check -p ploke-tui`; this was not a live Google request.

Update 2026-05-20: `Google` now implements `RouterCalibration` with a direct model-key calibration id, and `ploke-llm` has an ignored live `chat_step` forced-tool-call test for Google. The direct Google provider path should not fake OpenRouter endpoint/provider metadata; until TUI/eval routing is generalized, Google tool capability is carried by the Google model-list adapter. Verified by the surface above.

Update 2026-05-20: The provider-neutral route boundary now exists as `LlmRoute` in `ploke-llm`: OpenRouter routes carry provider endpoint metadata, while Google routes carry direct model capability without a provider endpoint. `RuntimeConfig` now carries an explicit `active_router`, `ploke-eval` sets it from the selected route, and `ploke-tui` live chat request construction dispatches `RouterVariants::Google` through `ChatCompRequest<Google>`.

Update 2026-05-20: Model registry rows now carry route provenance. Direct Google rows are tagged as `direct_google`, `ploke-eval model refresh` can merge OpenRouter and Google model catalogs, and direct Google rows win same-id collisions so `google/*` can resolve to the direct route without fake provider endpoints. TUI operator commands now show both OpenRouter and Google API-key diagnostics, include an active `model router [openrouter|google]` command, search the active router's model catalog, and show direct Google route information instead of provider endpoints.

Update 2026-05-21: Live Google now reaches the `ploke-tui` session/tool loop. The ignored canary `live_google_chat_session_executes_list_dir_tool_call_success_or_quota` constructs `ChatSession<Google>`, lets Google choose tools with `tool_choice=auto`, executes a `list_dir` tool call through the normal event-bus `ToolCallRequested`/`process_tool`/`ToolCallCompleted` path, and receives a final assistant response in the same session. This proves the TUI session loop can consume a live Google tool call, but it is still not a native interactive TUI run and does not yet prove the `ploke-eval` headless adapter route.

Update 2026-05-21: Native interactive TUI routing now works with direct Google. In a real `cargo run -p ploke-tui` PTY session, `/model router google` set `RuntimeConfig.active_router`, `/model use google/gemini-2.5-flash` selected a direct Google Gemini model, and a normal submitted user prompt received the exact live assistant response `google-native-smoke-ok`. The same run also showed `google/gemini-2.0-flash` is no longer available for this account: Google returned 404 with "This model models/gemini-2.0-flash is no longer available to new users." Future live smokes should prefer `google/gemini-2.5-flash` unless the refreshed model catalog says otherwise.

Update 2026-05-21: The model picker now treats direct Google rows as direct routes instead of OpenRouter rows with missing providers. Google search results carry `route_source = direct_google` into `ModelBrowserItem`, pressing `s` selects the model with no provider pin, and expanding a direct row does not request OpenRouter endpoints. Fixture-backed Google model-list coverage now parses `test_data/google/openai_models.json` and verifies direct route provenance plus tool-capability filtering.

Update 2026-05-21: The model picker now renders route-source badges on every row. OpenRouter-supplied rows show `[via OpenRouter]`, direct Google rows show `[via Google API]`, the badges use distinct colors, expanded rows show the source detail, and both overlay `? Help` plus global help explain how to read overlapping model ids.

Update 2026-05-21: The first `ploke-eval` headless-adapter route slice is implemented. `tui_adapter::ModelSelection` now carries the dispatch router, direct Google selections clear OpenRouter provider pins, and `start_attempt_runtime` writes `RuntimeConfig.active_router` before submitting the prompt to vanilla `ploke-tui`. `crate::cli::headless_model_selection` maps direct-Google registry rows, or explicit `--provider google`, to the Google router. Verified with `cargo test -p ploke-eval model_selection_sets_direct_google_route_without_provider_pin --lib`, `cargo test -p ploke-eval broad_tui_attempt_google_provider_selects_google_router --lib`, `cargo check -p ploke-eval`, and `git diff --check`. This is still not a real headless adapter attempt.

Update 2026-05-21: `ploke-protocol::JsonAdjudicator` is now route-aware for JSON adjudication. `JsonLlmConfig` carries `route_source`, direct Google builds `ChatCompRequest<Google>`, OpenRouter provider preferences stay on the OpenRouter branch, and direct Google provenance records include `route_source = direct_google`. `ploke-eval` protocol config resolution maps direct-Google registry rows, or explicit `--provider google`, to the Google route. Verified with `cargo test -p ploke-protocol google_json_request --lib`, `cargo test -p ploke-eval protocol_llm_config_google_provider_selects_direct_google_route --lib`, `cargo test -p ploke-eval protocol_llm_config_openrouter_provider_keeps_provider_pin --lib`, `cargo check -p ploke-records --features protocol`, `cargo check -p ploke-eval`, and `git diff --check`. This is request-shape and config proof, not a live protocol review.

Update 2026-05-21: A live isolated `ploke-eval` headless-adapter canary now proves the adapter reaches the direct Google endpoint from an isolated published request. Probe root: `/home/brasides/.ploke-eval/probes/p1-google-headless-canary-20260521-1`. The request, prompt, result path, candidate workspace, and evidence roots were adapted under that root, the probe used a local clone at commit `36ae2ae3` with its own `.git`, and the command ran with `OPENROUTER_API_KEY` removed from the process. Both an explicit `--model-id google/gemini-2.5-flash --provider google` attempt and a persisted-config attempt reached `https://generativelanguage.googleapis.com/v1beta/openai/chat/completions`; both aborted before staging an edit with HTTP 429. This is a live route proof, not a successful headless edit completion.

Update 2026-05-21: Correction: do not persist or recommend `google/models/gemini-2.5-flash`. Google `/openai/models` may return ids with a `models/` prefix, but the project-level model id must remain `google/gemini-2.5-flash`; the Google adapter now strips the raw API prefix when building registry rows. The persisted model registry was refreshed after that fix to 393 rows, including 53 direct Google rows. Active and parent-patcher selections are both set to `google/gemini-2.5-flash`, and that row now has `route_source = direct_google`.

Verdict: Google is integrated through `ploke-llm`, the `ploke-tui` model picker, the `ploke-tui` session-loop tool path, native interactive TUI routing, the first `ploke-eval` headless route-selection plumbing, and `ploke-protocol` JSON adjudicator request routing. A live isolated headless adapter canary reached the direct Google endpoint, but it did not complete an edit because Google returned HTTP 429. Eval embeddings are allowed to remain OpenRouter-backed for now, so the remaining blocker before a full live run is Google chat quota/rate availability, not embedding strictness.

**Already Done**
- `ploke-llm` has a `Google` router type with OpenAI-compatible constants and `GOOGLE_API_KEY` auth: [google/mod.rs](/home/brasides/code/ploke/crates/ploke-llm/src/router_only/google/mod.rs:159).
- Google request serialization is wired through generic `ChatCompRequest<Google>`, including model-id conversion from `google/gemini-*` to API-facing `gemini-*`: [google/mod.rs](/home/brasides/code/ploke/crates/ploke-llm/src/router_only/google/mod.rs:135), [router_only/mod.rs](/home/brasides/code/ploke/crates/ploke-llm/src/router_only/mod.rs:426).
- Google `extra_body.google.thinking_config` and `cached_content` structs exist and are unit-tested: [google/mod.rs](/home/brasides/code/ploke/crates/ploke-llm/src/router_only/google/mod.rs:14), [google_chat_completion.rs](/home/brasides/code/ploke/crates/ploke-llm/src/request/tests/google_chat_completion.rs:15).
- The shared parser can accept Google’s OpenAI-compatible success response shape: [shape_tests.rs](/home/brasides/code/ploke/crates/ploke-llm/src/response/shape_tests.rs:6).
- The generic HTTP sender `chat_step<R: Router>` can technically send Google requests because it uses `R::COMPLETION_URL` and `R::resolve_api_key`: [session.rs](/home/brasides/code/ploke/crates/ploke-llm/src/manager/session.rs:281).
- `Google` implements `HasModels` with its own OpenAI-compatible model-list response types and an adapter into the existing shared model registry item shape: [google/mod.rs](/home/brasides/code/ploke/crates/ploke-llm/src/router_only/google/mod.rs:21).
- `Google` implements `RouterCalibration` without provider preferences or endpoint routing, using the direct model key as the calibration key.
- `ploke-llm` has an ignored live Google `chat_step` forced-tool-call test. It requires `--features live_api_tests -- --ignored` plus `GOOGLE_API_KEY`.
- Provider routing now has a shared `LlmRoute` carrier. OpenRouter routes carry endpoint/provider metadata; direct Google routes carry model-level tool capability.
- `RuntimeConfig` carries an explicit active router, and `ploke-tui` live chat request construction dispatches `RouterVariants::Google` through `Google` instead of OpenRouter.
- `ploke-eval` route validation accepts direct Google routes and no longer requires Google to provide OpenRouter endpoint metadata.
- `ploke-llm` has fixture-backed Google model-list parsing and adaptation tests for the direct model catalog.
- `ploke-tui` model picker direct Google rows now select without provider endpoints and preserve the OpenRouter endpoint-fetch path for OpenRouter rows.
- `ploke-tui` model picker rows show colored `[via OpenRouter]` and `[via Google API]` source badges, and help text explains the distinction for overlapping ids.
- `ploke-tui` has an ignored live Google session-loop canary that proves a Google tool call can enter the normal event-bus tool execution path and complete `list_dir`.
- Native `ploke-tui` can select `model router google`, switch to `google/gemini-2.5-flash`, submit a normal user prompt, and receive a live Google assistant response.
- `ploke-eval` headless model selection can now carry direct Google router intent into `ploke-tui` by setting `RuntimeConfig.active_router` instead of relying on ambient defaults.
- `ploke-protocol` JSON adjudication can now dispatch direct Google requests through `ChatCompRequest<Google>` instead of always using `OpenRouter::default_chat_completion()`.
- `ploke-eval` isolated headless canary reached the direct Google chat completion endpoint from a probe-local published request and probe-local clone, with OpenRouter credentials removed from the process.
- Active and parent-patcher model selections now point at `google/gemini-2.5-flash`, whose refreshed registry row has `route_source = direct_google`.

**Still Missing For Parity**
- `Google` still does not implement `HasEndpoint`, intentionally. OpenRouter endpoint metadata drives provider/tool support, but direct Google has no provider endpoint selection. Any remaining code that requires endpoint metadata must be generalized rather than fed fake Google endpoints.
- Provider pinning remains OpenRouter-specific. Direct Google routes intentionally do not persist provider endpoint preferences.
- Google tool calls are proven live at `ploke-llm::chat_step` and through the `ploke-tui` session loop. Native operator routing is proven for a normal chat response. `ploke-eval` headless adapter routing is proven to reach Google, but not to complete an edit while the Google API is returning HTTP 429.
- Eval embeddings still use the OpenRouter-backed `mistralai/codestral-embed-2505` path. This is acceptable for the next full loop because the current operator decision allows embeddings through OpenRouter; revisit only if the requirement returns to strict zero-OpenRouter network.

**Work To Reach Parity**
1. Done: add Google model-list response types and an adapter from Google `/openai/models` into the project’s model registry shape.
2. Done: add `impl HasModels for Google`.
3. Done: decide what replaces OpenRouter endpoint/provider metadata for Google. Direct Google has no provider endpoint selection in this integration; model/tool capability is modeled as direct-provider model capability, not fake OpenRouter endpoints.
4. Done: add `impl RouterCalibration for Google`.
5. Done: `RuntimeConfig` now carries explicit router selection, and TUI request construction dispatches Google routes through `Google` instead of inferring every live request as OpenRouter.
6. Done: add Google-aware API-key diagnostics and model commands, including direct Google route display and TUI `model router`.
7. Done: add route provenance to registry rows and refresh `ploke-eval` model registry from both OpenRouter and direct Google catalogs.
8. Done for the basic catalog/picker path: fixture-backed Google model-list tests cover direct route provenance, and the TUI model picker no longer requests OpenRouter endpoints for direct Google rows. The picker also labels each row as `[via OpenRouter]` or `[via Google API]` so overlapping ids remain visually distinct.
9. Partially done: live ignored tests now cover Google tool calls through `chat_step` and one `ploke-tui` session-loop tool execution, and a native interactive TUI smoke covers operator route selection plus a live Google chat response. Still missing: `ploke-eval` headless adapter.

The next Google implementation slice is eval/runtime validation: run a recorded-prefix headless adapter attempt with direct Google route selection, then an ignored live canary when `GOOGLE_API_KEY` is present, then decide the strict embedding path before calling a full loop Google-only.

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
    - Status: first route-selection slice done; recorded-prefix and live headless attempt proof still pending.
    - Source facts: the merged model registry row, `route_source`, selected model id, optional OpenRouter provider preference, and the headless TUI `ModelSelection`.
    - Semantic object: eval-selected route, where OpenRouter routes may have provider preferences and direct Google routes must not.
    - Projection: a headless TUI attempt configured with `RouterVariants::Google` when the selected registry row is `direct_google`, or when the operator explicitly requests `--provider google`.
    - Renderer/test surface: current focused tests prove the route-selection carrier, and `cargo check -p ploke-eval` covers the adapter assignment into `RuntimeConfig.active_router`; the ignored live canary now accepts `PLOKE_EVAL_HEADLESS_TUI_MODEL_ID=google/gemini-2.5-flash` plus `PLOKE_EVAL_HEADLESS_TUI_PROVIDER=google` so it can test the direct route without changing persisted model config. Next proof should use an isolated published request, then an ignored live canary when `GOOGLE_API_KEY` is present.
    - Likely `ploke-eval` touch points:
      - [runner.rs](/home/brasides/code/ploke/crates/ploke-eval/src/runner.rs:1999): route resolution already uses `route_source`; keep direct Google on `LlmRoute::direct_google_model` and do not synthesize an endpoint.
      - [model_registry.rs](/home/brasides/code/ploke/crates/ploke-eval/src/model_registry.rs:64): registry refresh merges OpenRouter and Google catalogs; use this as the source for route provenance.
      - [cli.rs](/home/brasides/code/ploke/crates/ploke-eval/src/cli.rs:2845): `ModelSelection` construction is where OpenRouter provider preference can accidentally leak into direct Google selection.
      - [tui_adapter.rs](/home/brasides/code/ploke/crates/ploke-eval/src/cli/prototype1_state/edit_surface/tui_adapter.rs:1): the headless adapter must pass router/model intent into vanilla `ploke-tui` rather than interpreting provider endpoints itself.
      - [cli_tests.rs](/home/brasides/code/ploke/crates/ploke-eval/src/cli/prototype1_state/tests/cli_tests.rs:1218): existing ignored live headless attempt test is a candidate surface for a Google-specific canary once the adapter can select the route.

13. Done for protocol review routing: make `ploke-protocol::JsonAdjudicator` route-aware before any `stop_after = "complete"` loop.
    - Source facts: `JsonLlmConfig.model_id`, `JsonLlmConfig.route_source`, optional OpenRouter provider pin, and `JsonLlmProvenance`.
    - Semantic object: protocol-selected route, where OpenRouter JSON review may carry a provider pin and direct Google review must not.
    - Projection: `adjudicate_json` builds `ChatCompRequest<Google>` for direct Google and keeps OpenRouter provider preferences only on `ChatCompRequest<OpenRouter>`.
    - Renderer/test surface: focused request-shape and config tests listed in the 2026-05-21 update above.

14. After runtime validation, tighten persistence and operator semantics.
    - Decide whether `active_router` is intentionally runtime-only in `ploke-tui`, or whether user config should persist it alongside the selected model.
    - Keep OpenRouter provider pinning scoped to OpenRouter. For direct Google, persisted selection should be model plus route provenance, not provider slug.
    - Add or adjust operator text only as a renderer over the route object: `openrouter` can show providers/endpoints; `google` should show direct route capability and key status.
