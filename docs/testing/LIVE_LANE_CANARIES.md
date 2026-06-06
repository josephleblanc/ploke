# Live Lane Canaries

Last updated: 2026-06-04

Source inventory: `target/test-output/workspace-measurement/ignored_rust_tests_inventory_2026-06-04.md`.

This catalog normalizes the ignored live/API/operator tests from the inventory:
11 Google live tests, 10 OpenRouter/provider tests, and 6 operator-workspace
canaries. It cross-checks them against `docs/testing/TEST_GUIDELINES.md` and
`docs/testing/LIVE_GOOGLE.md`.

Policy summary

- Google live tests remain ignored, `live_api_tests`-gated where compiled by a
  feature, and ADC-based. Before claiming Google validation, export
  `GOOGLE_PROJECT_ID`, `GOOGLE_REGION`, `PLOKE_RUN_LIVE_TESTS=1`, and a model
  env such as `PLOKE_LIVE_GOOGLE_CHAT_MODEL=google/gemini-2.5-flash`, then run
  `cargo xtask auth google --strict-live` from the same shell. A skip or early
  return is not a green live result when `PLOKE_RUN_LIVE_TESTS=1` is set.
- OpenRouter/provider tests in this file are the ignored, expensive,
  model-sensitive, diagnostic, or fixture-refresh lane. Do not move fast/default
  OpenRouter regression coverage out of the default workspace lane merely to
  make `cargo test --workspace` offline-only.
- Operator-workspace canaries are manual. They are useful only when the operator
  supplies the named workspace/request env; otherwise their output documents that
  the live path was not exercised.
- Evidence should live under `target/test-output/live-lanes/` or another
  explicitly named artifact directory. Do not commit generated artifacts unless a
  separate task asks for that.

Common setup

Google direct route:

```sh
export GOOGLE_PROJECT_ID=${GOOGLE_PROJECT_ID:?set Google project}
export GOOGLE_REGION=${GOOGLE_REGION:?set Google region, for example global}
export PLOKE_RUN_LIVE_TESTS=1
export PLOKE_LIVE_GOOGLE_CHAT_MODEL=${PLOKE_LIVE_GOOGLE_CHAT_MODEL:-google/gemini-2.5-flash}
cargo xtask auth google --strict-live
mkdir -p target/test-output/live-lanes
```

OpenRouter/provider live route:

```sh
export OPENROUTER_API_KEY=${OPENROUTER_API_KEY:?set OpenRouter API key}
mkdir -p target/test-output/live-lanes
```

Operator workspace lane:

```sh
mkdir -p target/test-output/live-lanes
```

For command lines below, the `tee` target is the evidence log for that run. When
an individual test writes structured evidence, the row lists that path too.

## Google live tests (11)

All rows require the Google direct-route common setup above. Use ADC, not
`GOOGLE_API_KEY`, for the supported direct-Google route. Cadence defaults to
monthly and before/after Google router/model-registry/protocol changes unless a
row says otherwise.

| Test | Command | Additional env gate | Evidence artifact path | Suggested cadence |
| --- | --- | --- | --- | --- |
| `live_google_protocol_json_adjudication_uses_direct_route_success_or_quota` | `cargo test -p ploke-eval --features live_api_tests live_google_protocol_json_adjudication_uses_direct_route_success_or_quota -- --ignored --nocapture 2>&1 \| tee target/test-output/live-lanes/google_protocol_json_adjudication.log` | Optional `PLOKE_EVAL_LIVE_GOOGLE_MODEL_ID`; otherwise shared `PLOKE_LIVE_GOOGLE_CHAT_MODEL`. | `target/test-output/live-lanes/google_protocol_json_adjudication.log` | Monthly; also after protocol JSON routing or direct-Google config changes. |
| `live_google_child_runner_success` | `cargo test -p ploke-eval --features live_api_tests live_google_child_runner_success -- --ignored --nocapture 2>&1 \| tee target/test-output/live-lanes/google_child_runner_success.log` | Optional `PLOKE_EVAL_LIVE_GOOGLE_MODEL_ID`. The source is intentionally quarantined with an immediate panic; do not count as a scheduled green canary until rebuilt. | `target/test-output/live-lanes/google_child_runner_success.log`; if rebuilt past quarantine, temp `PLOKE_EVAL_HOME` artifacts from the test. | Quarantine-only; run only while rebuilding the child-runner acceptance contract. |
| `live_tui_router_staged_proposal_lowers_to_checked_artifact_delta` | `cargo test -p ploke-eval --features live_api_tests live_tui_router_staged_proposal_lowers_to_checked_artifact_delta -- --ignored --nocapture 2>&1 \| tee target/test-output/live-lanes/google_or_openrouter_tui_router_staged_proposal.log` | Google path requires the Google common setup; OpenRouter fallback requires `OPENROUTER_API_KEY`. Prefer Google here when validating this table. | `target/test-output/live-lanes/google_or_openrouter_tui_router_staged_proposal.log` | Monthly; also after edit-surface lowering or provider-router changes. |
| `live_google_step_child_plan` | `cargo test -p ploke-eval --features live_api_tests live_google_step_child_plan -- --ignored --nocapture 2>&1 \| tee target/test-output/live-lanes/google_step_child_plan.log` | Leave `PLOKE_EVAL_BROAD_TUI_SUMMARY_FIXTURE` unset; test-scoped env sets `PLOKE_EVAL_HEADLESS_TUI_LIVE=1`. The source is intentionally quarantined with an immediate panic. | `target/test-output/live-lanes/google_step_child_plan.log`; if rebuilt past quarantine, `prototype1/evaluations/live-step-canary.md` and `prototype1/messages/edit-harness-result/*` under the test temp campaign dir. | Quarantine-only; run only while rebuilding the prototype1-step fixture/contract. |
| `live_google_parallel_slots` | `cargo test -p ploke-eval --features live_api_tests live_google_parallel_slots -- --ignored --nocapture 2>&1 \| tee target/test-output/live-lanes/google_parallel_slots.log` | Leave `PLOKE_EVAL_BROAD_TUI_SUMMARY_FIXTURE` unset; test-scoped env sets slot limit/probe variables and `PLOKE_EVAL_HEADLESS_TUI_LIVE=1`. | `target/test-output/live-lanes/google_parallel_slots.log`; plus `prototype1/evaluations/live-step-canary.md`, slot-probe files, and `prototype1/messages/edit-harness-result/*` under the test temp campaign dir. | Monthly only when prototype1 broad-headless parallel slot behavior is under active development. |
| `xfail_google_vertex_broad_headless_tui_attempt_applies_edit_from_published_request` | `PLOKE_EVAL_LIVE_TUI_CANARY_DIR=target/test-output/live-lanes/google_vertex_broad_headless cargo test -p ploke-eval --features live_api_tests xfail_google_vertex_broad_headless_tui_attempt_applies_edit_from_published_request -- --ignored --nocapture 2>&1 \| tee target/test-output/live-lanes/google_vertex_broad_headless_xfail.log` | Optional `PLOKE_EVAL_HEADLESS_TUI_GOOGLE_MODEL_ID`; otherwise shared `PLOKE_LIVE_GOOGLE_CHAT_MODEL`. This is expected-failing/unsupported Vertex broad-headless coverage. | `target/test-output/live-lanes/google_vertex_broad_headless_xfail.log`; `target/test-output/live-lanes/google_vertex_broad_headless/run-*/{campaign.json,repo/,prototype1/**}`. | Quarantine/xfail only; run before deciding to support the Vertex broad-headless endpoint path. |
| `live_google_resolved_route_returns_content_success_or_quota` | `cargo test -p ploke-eval --features live_api_tests live_google_resolved_route_returns_content_success_or_quota -- --ignored --nocapture 2>&1 \| tee target/test-output/live-lanes/google_resolved_route_content.log` | Optional `PLOKE_EVAL_LIVE_GOOGLE_MODEL_ID`; otherwise shared `PLOKE_LIVE_GOOGLE_CHAT_MODEL`. | `target/test-output/live-lanes/google_resolved_route_content.log` | Monthly; also after eval route-resolution or Google registry changes. |
| `live_google_resolved_route_forces_list_dir_tool_call_success_or_quota` | `cargo test -p ploke-eval --features live_api_tests live_google_resolved_route_forces_list_dir_tool_call_success_or_quota -- --ignored --nocapture 2>&1 \| tee target/test-output/live-lanes/google_resolved_route_forced_tool.log` | Optional `PLOKE_EVAL_LIVE_GOOGLE_MODEL_ID`; otherwise shared `PLOKE_LIVE_GOOGLE_CHAT_MODEL`. | `target/test-output/live-lanes/google_resolved_route_forced_tool.log` | Monthly; also after forced-tool-call or eval route-resolution changes. |
| `live_google_chat_step_forced_tool_call_success_or_quota` | `cargo test -p ploke-llm --features live_api_tests live_google_chat_step_forced_tool_call_success_or_quota -- --ignored --nocapture 2>&1 \| tee target/test-output/live-lanes/google_llm_forced_tool.log` | Shared `PLOKE_LIVE_GOOGLE_CHAT_MODEL`; requires model with tool support. | `target/test-output/live-lanes/google_llm_forced_tool.log` | Monthly; first focused canary after Google auth/model changes. |
| `live_google_harness_router_command_runs_list_dir_through_llm_manager` | `cargo test -p ploke-tui --features live_api_tests live_google_harness_router_command_runs_list_dir_through_llm_manager -- --ignored --nocapture 2>&1 \| tee target/test-output/live-lanes/google_tui_router_command.log` | Shared `PLOKE_LIVE_GOOGLE_CHAT_MODEL`; requires ADC/project/region. | `target/test-output/live-lanes/google_tui_router_command.log` | Monthly; also after `/model router google`, LLM manager, or event-bus tool-dispatch changes. |
| `live_google_chat_session_executes_list_dir_tool_call_success_or_quota` | `cargo test -p ploke-tui --features live_api_tests live_google_chat_session_executes_list_dir_tool_call_success_or_quota -- --ignored --nocapture 2>&1 \| tee target/test-output/live-lanes/google_tui_chat_session_tool.log` | Shared `PLOKE_LIVE_GOOGLE_CHAT_MODEL`; requires ADC/project/region. | `target/test-output/live-lanes/google_tui_chat_session_tool.log` | Monthly; also after `ChatSession<Google>` or tool-call session-loop changes. |

## OpenRouter/provider ignored tests (10)

Rows that hit OpenRouter require `OPENROUTER_API_KEY`. Rows that refresh local
provider fixtures should not be promoted to the default live lane; use them only
when updating provider data. Default fast OpenRouter regression tests remain in
the default lane when their default features are enabled.

| Test | Command | Env gate | Evidence artifact path | Suggested cadence |
| --- | --- | --- | --- | --- |
| `live_openrouter_fixture_nodes_index_e2e` | `cargo test -p ploke-embed --features live_api_tests --test openrouter_live_fixture_nodes_e2e live_openrouter_fixture_nodes_index_e2e -- --ignored --nocapture --test-threads=1 2>&1 \| tee target/test-output/live-lanes/openrouter_fixture_nodes_e2e.log` | `OPENROUTER_API_KEY`; unset `OPENROUTER_EMBEDDINGS_URL`; optional `PLOKE_OPENROUTER_EMBED_MODEL`, `PLOKE_OPENROUTER_EMBED_DIMS`. | `target/test-output/live-lanes/openrouter_fixture_nodes_e2e.log`; structured JSON at `target/test-output/embedding/live/openrouter_fixture_nodes_e2e_<timestamp>.json`. | Monthly; also after embedding indexer, HNSW, or OpenRouter embedding config changes. |
| `live_openrouter_snippet_repro` | `cargo test -p ploke-embed --features live_api_tests --test openrouter_live_snippet_repro live_openrouter_snippet_repro -- --ignored --nocapture --test-threads=1 2>&1 \| tee target/test-output/live-lanes/openrouter_snippet_repro.log` | `OPENROUTER_API_KEY`; unset `OPENROUTER_EMBEDDINGS_URL`; optional `PLOKE_OPENROUTER_EMBED_MODEL`, `PLOKE_OPENROUTER_EMBED_DIMS`; requires `fixtures/snippets/graph_access.rs`. | `target/test-output/live-lanes/openrouter_snippet_repro.log`. | As-needed when the snippet regression, embedding dims, or OpenRouter embedding model changes. |
| `live_tui_adapter_canary_shows_inputs_outputs_and_applied_edit` | `PLOKE_EVAL_LIVE_TUI_CANARY_DIR=target/test-output/live-lanes/tui-adapter OPENROUTER_API_KEY=$OPENROUTER_API_KEY cargo test -p ploke-eval live_tui_adapter_canary_shows_inputs_outputs_and_applied_edit -- --ignored --nocapture 2>&1 \| tee target/test-output/live-lanes/tui_adapter_direct_file.log` | Active model/provider config must resolve to a live OpenRouter-capable model. | `target/test-output/live-lanes/tui-adapter/live-tui-adapter-direct-file/{report.json,headless-evidence.json,headless-events.json,workspace.diff}` and `target/test-output/live-lanes/tui_adapter_direct_file.log`. | Monthly during broad edit-surface work; before releases that touch headless TUI edit application. |
| `live_tui_adapter_canary_uses_indexed_context_before_applied_edit` | `PLOKE_EVAL_LIVE_TUI_CANARY_DIR=target/test-output/live-lanes/tui-adapter OPENROUTER_API_KEY=$OPENROUTER_API_KEY cargo test -p ploke-eval live_tui_adapter_canary_uses_indexed_context_before_applied_edit -- --ignored --nocapture 2>&1 \| tee target/test-output/live-lanes/tui_adapter_indexed_context.log` | Active model/provider config must resolve to a live OpenRouter-capable model. | `target/test-output/live-lanes/tui-adapter/live-tui-adapter-indexed-context/{report.json,headless-evidence.json,headless-events.json,workspace.diff}` and `target/test-output/live-lanes/tui_adapter_indexed_context.log`. | Monthly during RAG/edit-surface work; before changes to `request_code_context` or broad edit tools. |
| `live_intervention_synthesis_fans_out_replacement_candidates` | `OPENROUTER_API_KEY=$OPENROUTER_API_KEY cargo test -p ploke-eval live_intervention_synthesis_fans_out_replacement_candidates -- --ignored --nocapture 2>&1 \| tee target/test-output/live-lanes/openrouter_intervention_synthesis.log` | Active model and provider config must load; expected route source is OpenRouter. | `target/test-output/live-lanes/openrouter_intervention_synthesis.log`. | As-needed after intervention synthesis prompt/schema changes. |
| `live_eval_embedding_selection_preflight_uses_openrouter_env` | `OPENROUTER_API_KEY=$OPENROUTER_API_KEY cargo test -p ploke-eval --features live_api_tests live_eval_embedding_selection_preflight_uses_openrouter_env -- --ignored --nocapture 2>&1 \| tee target/test-output/live-lanes/openrouter_eval_embedding_preflight.log` | `OPENROUTER_API_KEY`. | `target/test-output/live-lanes/openrouter_eval_embedding_preflight.log`. | Monthly; also after eval embedding defaults or model registry changes. |
| `flakey_openrouter_models` | `cargo test -p ploke-llm flakey_openrouter_models -- --ignored --nocapture 2>&1 \| tee target/test-output/live-lanes/openrouter_models_fixture_names.log` | No live API key; reads existing provider model fixture data. | `target/test-output/live-lanes/openrouter_models_fixture_names.log`; input fixture/golden files under `crates/ploke-llm` provider data. | Fixture-maintenance only; prefer default non-ignored provider fixture tests for regression coverage. |
| `test_simple_query_embedding_models` | `cargo test -p ploke-llm test_simple_query_embedding_models -- --ignored --nocapture 2>&1 \| tee target/test-output/live-lanes/openrouter_embedding_models_fixture_refresh.log` | Network access to OpenRouter embedding models endpoint; auth is not required by the source. | `target/test-output/live-lanes/openrouter_embedding_models_fixture_refresh.log`; generated fixture `fixtures/openrouter/embeddings_models.json`. | Fixture-refresh only, when OpenRouter embedding model metadata changes. Review generated fixture before committing. |
| `live_openrouter_agent_turn_records_provider_attempt_report` | `OPENROUTER_API_KEY=$OPENROUTER_API_KEY PLOKE_LIVE_AGENT_MODEL=${PLOKE_LIVE_AGENT_MODEL:-x-ai/grok-4-fast} PLOKE_LIVE_AGENT_PROVIDER=${PLOKE_LIVE_AGENT_PROVIDER:-xai} cargo test -p ploke-tui --features live_api_tests live_openrouter_agent_turn_records_provider_attempt_report -- --ignored --nocapture 2>&1 \| tee target/test-output/live-lanes/openrouter_agent_turn_report.log` | `OPENROUTER_API_KEY`; optional `PLOKE_LIVE_AGENT_MODEL`, `PLOKE_LIVE_AGENT_PROVIDER`. | `target/test-output/live-lanes/openrouter_agent_turn_report.log`. | Monthly; also after provider-attempt reporting, LLM manager, or chat session changes. |
| `live_request_code_context_matrix_uses_production_tool_payload` | `OPENROUTER_API_KEY=$OPENROUTER_API_KEY cargo test -p ploke-tui --features test_harness,typed_type_graph live_request_code_context_matrix_uses_production_tool_payload -- --ignored --nocapture 2>&1 \| tee target/test-output/live-lanes/openrouter_request_code_context_matrix.log` | `OPENROUTER_API_KEY`; compiles only with `test_harness` and `typed_type_graph`. | `target/test-output/live-lanes/openrouter_request_code_context_matrix.log`. | Monthly while typed type-graph/RAG payload work is active; before changing production tool payload shape. |

## Operator-workspace canaries (6)

These are manual canaries. They should stay ignored/operator-only until they are
converted into hermetic fixture-backed tests or scheduled with a documented
operator workspace. Their evidence is the run log plus any printed workspace,
prompt, diagnostics, or submitted result paths.

| Test | Command | Env gate | Evidence artifact path | Suggested cadence |
| --- | --- | --- | --- | --- |
| `live_tui_runtime_setup_uses_sparse_child_db` | `PLOKE_EVAL_LIVE_TUI_CANARY_DIR=target/test-output/live-lanes/tui-adapter cargo test -p ploke-eval live_tui_runtime_setup_uses_sparse_child_db -- --ignored --nocapture 2>&1 \| tee target/test-output/live-lanes/operator_sparse_child_db_runtime_setup.log` | No external API key; prepares a local child workspace fixture and exercises runtime setup only. | `target/test-output/live-lanes/tui-adapter/live-tui-adapter-sparse-child-db/setup-error.txt` on setup failure; `target/test-output/live-lanes/operator_sparse_child_db_runtime_setup.log` for the run log. | Monthly while sparse/BM25 runtime setup is changing; before changing child-local parse/transform DB behavior. |
| `live_tui_runtime_setup_existing_workspace_from_env` | `PLOKE_EVAL_EXISTING_TUI_WORKSPACE=${PLOKE_EVAL_EXISTING_TUI_WORKSPACE:?workspace root} cargo test -p ploke-eval live_tui_runtime_setup_existing_workspace_from_env -- --ignored --nocapture 2>&1 \| tee target/test-output/live-lanes/operator_existing_workspace_runtime_setup.log` | `PLOKE_EVAL_EXISTING_TUI_WORKSPACE` must point at an existing workspace root. | `target/test-output/live-lanes/operator_existing_workspace_runtime_setup.log`. | Weekly while broad TUI workspace setup is changing; otherwise before release candidates. |
| `live_tui_initial_prompt_ploke_workspace_includes_rag_parts` | `PLOKE_EVAL_EXISTING_TUI_WORKSPACE=${PLOKE_EVAL_EXISTING_TUI_WORKSPACE:-$(pwd)} cargo test -p ploke-eval live_tui_initial_prompt_ploke_workspace_includes_rag_parts -- --ignored --nocapture 2>&1 \| tee target/test-output/live-lanes/operator_initial_prompt_rag_on.log` | Optional `PLOKE_EVAL_EXISTING_TUI_WORKSPACE`; defaults in source to the Ploke workspace root. | `target/test-output/live-lanes/operator_initial_prompt_rag_on.log`. | Monthly; also after prompt context assembly or BM25 diagnostics changes. |
| `live_tui_initial_prompt_off_skips_rag_parts` | `PLOKE_EVAL_EXISTING_TUI_WORKSPACE=${PLOKE_EVAL_EXISTING_TUI_WORKSPACE:-$(pwd)} cargo test -p ploke-eval live_tui_initial_prompt_off_skips_rag_parts -- --ignored --nocapture 2>&1 \| tee target/test-output/live-lanes/operator_initial_prompt_rag_off.log` | Optional `PLOKE_EVAL_EXISTING_TUI_WORKSPACE`; defaults in source to the Ploke workspace root. | `target/test-output/live-lanes/operator_initial_prompt_rag_off.log`. | Monthly; also after context-mode Off prompt behavior changes. |
| `live_tui_request_code_context_ploke_workspace_returns_results` | `PLOKE_EVAL_EXISTING_TUI_WORKSPACE=${PLOKE_EVAL_EXISTING_TUI_WORKSPACE:-$(pwd)} PLOKE_EVAL_TUI_SEARCH_TERMS=${PLOKE_EVAL_TUI_SEARCH_TERMS:-setup_workspace_tui_runtime,run_broad_headless_tui_attempt} cargo test -p ploke-eval live_tui_request_code_context_ploke_workspace_returns_results -- --ignored --nocapture 2>&1 \| tee target/test-output/live-lanes/operator_request_code_context_workspace.log` | Optional `PLOKE_EVAL_EXISTING_TUI_WORKSPACE`; optional `PLOKE_EVAL_TUI_SEARCH_TERMS`. | `target/test-output/live-lanes/operator_request_code_context_workspace.log`. | Weekly while RAG/indexing behavior is changing; otherwise monthly. |
| `live_broad_headless_tui_attempt_from_published_request_env` | `PLOKE_EVAL_BROAD_HARNESS_REQUEST_PATH=${PLOKE_EVAL_BROAD_HARNESS_REQUEST_PATH:?published request json} cargo test -p ploke-eval live_broad_headless_tui_attempt_from_published_request_env -- --ignored --nocapture 2>&1 \| tee target/test-output/live-lanes/operator_broad_headless_published_request.log` | `PLOKE_EVAL_BROAD_HARNESS_REQUEST_PATH`; optional `PLOKE_EVAL_HEADLESS_TUI_MODEL_ID`, `PLOKE_EVAL_HEADLESS_TUI_PROVIDER`. | `target/test-output/live-lanes/operator_broad_headless_published_request.log`; test prints prompt, workspace, diagnostics, and submitted-result paths derived from the published request. | Manual splice/debug only, whenever replaying a specific published broad harness request. |

Maintenance notes

- If a canary is moved into or out of this live lane, update this file and the
  ignored-test inventory/report that motivated the change.
- If a live-gated test returns success without exercising the live path while the
  relevant gate is enabled, treat that as a test bug and fix the gate before
  trusting the result.
- Use `target/test-output/live-lanes/` for run logs so final task/PR summaries
  can cite durable evidence without committing artifacts.
