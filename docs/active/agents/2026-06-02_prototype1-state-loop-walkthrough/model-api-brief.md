# Prototype1-state model and live API brief

Date: 2026-06-03
Command: `ploke-eval loop prototype1-state`
Scope: source trace plus current local Prototype 1 model config.

## Short answer

`prototype1-state` does not hard-code one model for the full loop. The normal baseline and treatment eval/protocol calls use the resolved campaign model: `ResolvedCampaignConfig.model_id`, with `provider_slug` and `route_source` resolved from the campaign manifest and fallbacks.[^campaign-resolved] The automatic broad headless-TUI parent patcher path is the exception: it currently reads the separate parent-patcher model selection, falling back to the active model selection if the parent-patcher file is missing.[^parent-patcher-selection]

In this checkout, running `prototype1-state` without `--campaign` would infer `p1-occurrence-selection-overnight-20260509-1` from `.ploke/prototype1/parent_identity.json`.[^campaign-inference] That campaign manifest sets `model_id = "x-ai/grok-4-fast"` and `provider_slug = "xai"` and has no admitted run-profile.[^local-stale-campaign]

The current machine-local model-selection files under `/home/brasides/.ploke-eval/models/` set both `active-model.json` and `parent-patcher-model.json` to `google/gemini-3.5-flash`; provider preferences include a stale-looking `google/gemini-3.5-flash -> google-ai-studio` OpenRouter-style provider entry, but direct-Google resolution ignores non-direct provider pins when the registry route is direct Google.[^model-files] [^headless-selection] Recent profile-backed June 2 campaigns also set `[model].id = "google/gemini-3.5-flash"`, `route_source = "direct-google"`, `provider = "google"`, and persist campaign `model_id = "google/gemini-3.5-flash"` with `route_source = "direct_google"`.[^local-june-campaign]

## Execution path and live calls

1. CLI dispatch enters `LoopCommand::run`, dispatches `Prototype1State(cmd)`, then calls `Prototype1StateCommand::run()` and `run_turn()`.[^dispatch] `run_turn()` resolves repo root, campaign, run shape, resolved campaign config, parent identity, then establishes baseline, plans children, runs child fanout, and possibly launches a successor.[^state-run-turn]

2. Baseline eval/protocol live calls happen only for generation 0 parents, and only if the closure advance selects incomplete work. `establish_initial_parent_baseline()` calls `advance_eval_closure()` and `advance_protocol_closure()` using the resolved campaign config.[^baseline] Eval passes `Some(config.model_id.clone())` and parsed provider into `execute_batch_eval_for_manifest()`.[^eval-advance] Protocol passes `config.model_id`, `config.route_source`, and `config.provider_slug` into `execute_protocol_run_tasks()`.[^protocol-advance]

3. Eval/headless-TUI chat calls go through `RunMsbAgentBatchRequest::run()` to per-instance `RunMsbAgentSingleRequest::run()`. The single-run path resolves the requested model, resolves route/provider, then writes that route into `RuntimeConfig.active_model`, `active_router`, and provider selection before the headless TUI sends the prompt.[^batch-run] [^single-run] [^eval-runtime-config] The actual provider HTTP call is `ploke_llm::chat_step_with_attempts()`, which resolves the completion URL and bearer token, then POSTs the chat request with the request model.[^tui-manager] [^chat-step]

4. Protocol JSON calls use `protocol_llm_config()` to build `JsonLlmConfig` from the campaign model/route/provider plus protocol policy values.[^protocol-config] `JsonAdjudicator` calls `adjudicate_json()`, which parses `cfg.model_id`, builds either a Google or OpenRouter JSON chat request, and sends it through `chat_step()`.[^json-adjudicator] [^json-call]

5. Broad headless-TUI candidate generation can make live model calls before child fanout. When the child plan is absent and the candidate generator is `BroadHarnessRequest`, the parent publishes/admitts broad harness slots, runs `run_broad_headless_tui_attempt()`, and calls `tui_adapter::run_headless_with_model_capture_responses()`.[^broad-plan] [^broad-attempt] `BroadTuiAttemptOptions::for_parent_patcher_defaults()` uses `load_parent_patcher_model_selection()`, not the campaign manifest.[^broad-options] The adapter sets the headless runtime's active model/router/provider if a model selection is present, then submits the prompt into the normal TUI LLM manager.[^headless-adapter]

6. Child fanout itself is process orchestration in the parent. It materializes/builds/spawns child runtimes, then observes their terminal result.[^child-parent] The child process enters `prototype1-runner --execute`, loads the child invocation, then calls `run_prototype1_resolved_branch_treatment()`.[^runner-entry] That child treatment path creates a treatment campaign by copying baseline `model_id`, `provider_slug`, and `route_source`, then advances treatment eval and protocol closure, so treatment live calls use the same resolved model route as the baseline campaign.[^treatment-copy] [^child-treatment]

## Config setting sites

- Campaign manifest fields: `CampaignManifest` persists optional `model_id`, `provider_slug`, and `route_source`; `ResolvedCampaignConfig` materializes non-optional `model_id` and `route_source` for runtime use.[^campaign-types]
- Setup writes the campaign model: `prepare_prototype1_loop_campaign()` selects CLI/profile/default model, route, and provider, enforces shared eval/protocol route for the baseline arm, then writes `manifest.model_id`, `manifest.provider_slug`, and `manifest.route_source`.[^setup-model]
- Run-profile model fields: `[model].id`, `[model].route_source`, and `[model].provider` live in `ModelDefaults`; validation rejects direct-Google route with non-`google` provider.[^profile-model]
- Runtime run shape comes from the admitted profile when present, but the live eval/protocol model still comes from the resolved campaign config, not by re-reading profile `[model]` at each turn.[^run-shape]
- Active and parent-patcher model files are under `ploke_eval_home()/models`: `active-model.json`, `parent-patcher-model.json`, and `provider-preferences.json`.[^model-file-paths]

## Notes

- `JsonLlmConfig::default()` has `moonshotai/kimi-k2`, but the `prototype1-state` protocol closure path passes the campaign model explicitly. Treat the default as a generic protocol fallback, not the model chosen by a normal campaign-backed Prototype 1 state turn.[^json-default] [^protocol-advance]
- OpenRouter route resolution can make a provider metadata API call before a model completion: `resolve_route_for_model()` returns direct Google routes locally, but for OpenRouter models it calls `OpenRouter::fetch_model_endpoints()`.[^route-metadata]
- `stop_after` does not prevent generation-0 baseline establishment. A generation-0 parent can still advance baseline eval/protocol before stopping at a later child state boundary.[^state-run-turn] [^baseline]

[^dispatch]: `crates/ploke-eval/src/cli.rs:1445-1457`.
[^state-run-turn]: `crates/ploke-eval/src/cli/prototype1_state/cli_facing.rs:6531-7045`.
[^campaign-inference]: `crates/ploke-eval/src/cli/prototype1_state/cli_facing.rs:4212-4229`; current checkout `.ploke/prototype1/parent_identity.json`.
[^baseline]: `crates/ploke-eval/src/cli/prototype1_state/cli_facing.rs:424-453`.
[^eval-advance]: `crates/ploke-eval/src/cli.rs:6882-6952`.
[^protocol-advance]: `crates/ploke-eval/src/cli.rs:6973-7029`.
[^batch-run]: `crates/ploke-eval/src/runner.rs:3699-3768`.
[^single-run]: `crates/ploke-eval/src/runner.rs:3084-3108`.
[^eval-runtime-config]: `crates/ploke-eval/src/runner.rs:116-130` and `crates/ploke-eval/src/runner.rs:3236-3264`.
[^tui-manager]: `crates/ploke-tui/src/llm/manager/mod.rs:594-671` and `crates/ploke-tui/src/llm/manager/session.rs:949-1023`.
[^chat-step]: `crates/ploke-llm/src/manager/session.rs:291-349`.
[^protocol-config]: `crates/ploke-eval/src/cli.rs:1745-1777` and `crates/ploke-eval/src/cli.rs:7748-7768`.
[^json-adjudicator]: `crates/ploke-protocol/src/llm.rs:219-275`.
[^json-call]: `crates/ploke-protocol/src/llm.rs:537-647`.
[^broad-plan]: `crates/ploke-eval/src/cli/prototype1_state/cli_facing.rs:4334-4571`.
[^broad-attempt]: `crates/ploke-eval/src/cli/prototype1_state/cli_facing.rs:1423-1445` and `crates/ploke-eval/src/cli/prototype1_state/cli_facing.rs:1532-1612`.
[^broad-options]: `crates/ploke-eval/src/cli/prototype1_state/cli_facing.rs:646-715`.
[^headless-adapter]: `crates/ploke-eval/src/cli/prototype1_state/edit_surface/tui_adapter.rs:175-345`.
[^child-parent]: `crates/ploke-eval/src/cli/prototype1_state/cli_facing.rs:4910-5075` and `crates/ploke-eval/src/cli/prototype1_state/cli_facing.rs:5170-5289`.
[^runner-entry]: `crates/ploke-eval/src/cli.rs:1461-1480` and `crates/ploke-eval/src/cli/prototype1_process.rs:2977-3129`.
[^child-treatment]: `crates/ploke-eval/src/cli/prototype1_process.rs:3131-3306`.
[^treatment-copy]: `crates/ploke-eval/src/cli/prototype1_state/cli_facing.rs:7172-7208`.
[^campaign-types]: `crates/ploke-eval/src/campaign.rs:27-53` and `crates/ploke-eval/src/campaign.rs:123-138`.
[^campaign-resolved]: `crates/ploke-eval/src/campaign.rs:478-572`.
[^setup-model]: `crates/ploke-eval/src/cli/prototype1_state/cli_facing.rs:7596-7714`.
[^profile-model]: `crates/ploke-eval/src/cli/prototype1_state/profile.rs:130-180`.
[^run-shape]: `crates/ploke-eval/src/cli/prototype1_state/cli_facing.rs:963-1003`.
[^parent-patcher-selection]: `crates/ploke-eval/src/cli.rs:2924-2979`.
[^headless-selection]: `crates/ploke-eval/src/cli.rs:2924-2979` and `crates/ploke-eval/src/cli/prototype1_state/edit_surface/tui_adapter.rs:332-342`.
[^model-file-paths]: `crates/ploke-eval/src/layout.rs:130-147` and `crates/ploke-eval/src/model_registry.rs:212-287`.
[^json-default]: `crates/ploke-protocol/src/llm.rs:27-52`.
[^route-metadata]: `crates/ploke-eval/src/runner.rs:2501-2539`.
[^local-stale-campaign]: Current local files checked on 2026-06-03: `.ploke/prototype1/parent_identity.json` and `/home/brasides/.ploke-eval/campaigns/p1-occurrence-selection-overnight-20260509-1/campaign.json`.
[^model-files]: Current local files checked on 2026-06-03: `/home/brasides/.ploke-eval/models/active-model.json`, `/home/brasides/.ploke-eval/models/parent-patcher-model.json`, and `/home/brasides/.ploke-eval/models/provider-preferences.json`.
[^local-june-campaign]: Current local files checked on 2026-06-03: `/home/brasides/.ploke-eval/campaigns/p1-gemini35-flash-direct-3g2x3-par2-20260602-170054/campaign.json` and `prototype1/run-profile.toml`.
