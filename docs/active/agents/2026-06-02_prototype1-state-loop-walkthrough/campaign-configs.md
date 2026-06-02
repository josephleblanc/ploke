# Prototype 1 campaign and run-profile config companion

Status: final-pass draft after source audit
Related walkthrough: [`README.md`](README.md)
Related terminology note: [`terminology-conflicts.md`](terminology-conflicts.md)
Command family: `ploke-eval loop prototype1-setup`, then `ploke-eval loop prototype1-state`

## Purpose

This companion expands the main walkthrough's config section. It answers the concrete `model_id` vs `model` question, maps each campaign and run-profile field to its source and use site, and calls out overlapping config surfaces that can cause bugs while stabilizing the Prototype 1 loop.

The short answer is:

- `CampaignManifest.model_id` is the campaign's persisted model id string. After setup, this is the value that `resolve_campaign_config` materializes into `ResolvedCampaignConfig.model_id`, and both eval closure and protocol closure read the resolved campaign config rather than re-reading profile `[model]` directly.[^campaign-fields] [^resolve-config] [^eval-protocol-routing]
- Run-profile `[model].id` is an operator-profile default used during setup/admission. If CLI setup flags do not override it, setup may copy this value into the generated campaign manifest as `model_id`. After that, the live `prototype1-state` turn uses the campaign manifest/resolved campaign for model routing, not the profile `[model]` section.[^profile-model] [^setup-model-precedence] [^runtime-runshape]
- `--protocol-model-id` is a setup CLI override for the baseline protocol arm. In the current Prototype 1 setup path, baseline eval and baseline protocol must resolve to the same model, route source, and provider; if they differ, setup errors before writing the campaign as a runnable shared baseline.[^loop-cli-model-flags] [^setup-shared-model]

## Pass log

### Draft pass

The raw source audit found two config planes:

1. Campaign config: persisted as `campaign.json`, resolved to `ResolvedCampaignConfig`, and used by closure/eval/protocol execution.
2. Run-profile config: operator TOML admitted into the campaign root as `prototype1/run-profile.toml` plus `prototype1/run-profile.commitment.json`, then used for runtime search/generation/selection/protocol/execution controls.

### Consistency pass

This doc uses these conventions:

- **Manifest Model ID** means `CampaignManifest.model_id`, the optional persisted JSON field.
- **Resolved Model ID** means `ResolvedCampaignConfig.model_id`, the non-optional runtime field after defaults and validation.
- **Profile Model ID** means `[model].id` in the run-profile TOML.
- **Protocol Model ID** means the argument to `protocol_llm_config` or setup `--protocol-model-id`; it is not a separate run-profile field.
- **Provider slug** means the OpenRouter provider slug when using OpenRouter. Direct Google route is special: config/display may say `google`, but resolved direct-Google campaign/provider state stores `provider_slug = None` unless an intermediate display helper formats it as `google`.[^resolve-config] [^json-llm-config]
- **Route source** means `ModelRouteSource`: current important values are OpenRouter and direct Google.

Terminology is intentionally structural rather than by long flat identifiers. For example, this doc says “Manifest Model ID” in prose instead of proposing a longer generated field name; the type that contains the field should carry the relationship.

### Final pass

Every implementation claim below has an end-note citation to current code, except for explicitly labeled “source-search note” observations. Source-search notes state what was not found by content search and should be rechecked after refactors.

## 1. Config plane overview

### 1.1 Campaign config plane

The campaign plane is the durable evaluation scope. It says which dataset slice is being evaluated, which model/provider/route the campaign uses, which roots store instances/batches, and which eval/protocol policies should be applied.[^campaign-fields]

The persisted type is `CampaignManifest`. It is optional/default-heavy: `model_id`, `provider_slug`, `route_source`, `instances_root`, and `batches_root` can be omitted from JSON.[^campaign-fields] The runtime type is `ResolvedCampaignConfig`. It replaces those optionals with required runtime values for `model_id`, `route_source`, `instances_root`, and `batches_root`.[^resolved-fields]

For Prototype 1 setup, `prepare_prototype1_loop_campaign` creates a new `CampaignManifest`, writes the selected instance slice dataset, fills model/provider/route from setup/profile resolution, writes the manifest, and immediately resolves it with `resolve_campaign_config`.[^setup-model-precedence] [^setup-manifest-write]

### 1.2 Run-profile config plane

The run-profile plane is the operator-approved behavior contract for a Prototype 1 campaign. It is a TOML profile with schema `prototype1-run-profile.v1`, name, and these sections: `storage`, `target`, `model`, `search`, `generation`, `selection`, `protocol`, `execution`, and `control`.[^profile-root]

Setup loads an operator profile from an explicit path or from `~/.ploke-eval/profiles/prototype1/<name>.toml`, validates it, uses parts of it while creating the batch/campaign, and then admits it into the campaign root.[^profile-load-admit] Admission writes two files: `prototype1/run-profile.toml` and `prototype1/run-profile.commitment.json`.[^profile-admit-write]

At runtime, `prototype1-state` tries to load the admitted run-profile from the campaign manifest path. If it exists, it uses profile-derived run-shape values; otherwise it falls back to CLI command flags/defaults.[^runtime-runshape]

## 2. The model/provider/route precedence chain

### 2.1 Setup-time eval model selection

During `prototype1-setup`, model selection is decided before the campaign manifest is written.[^setup-model-precedence]

1. `--model-id` is parsed first if present.[^setup-model-precedence]
2. `--use-default-model` also forces command/default-model behavior.[^setup-model-precedence]
3. If neither of those is set, setup may use run-profile `[model].id`.[^setup-model-precedence]
4. The chosen optional id is passed to `resolve_model_for_run`; if no explicit/default selection exists, `resolve_model_for_run` falls back to `active-model.json`.[^setup-model-precedence] [^active-model-selection]

This is the critical `model_id` / `[model].id` distinction. Profile `[model].id` is only a source candidate at setup; the campaign manifest receives a plain `model_id` string after model resolution.[^setup-model-precedence] [^setup-manifest-write]

### 2.2 Setup-time route source and provider selection

After selecting a model, setup resolves route/provider like this:

1. Route source precedence: setup `--route-source`, then profile `[model].route_source`, then the selected model registry item's `route_source`.[^setup-model-precedence]
2. Provider precedence: setup `--provider`, then profile `[model].provider`.[^setup-model-precedence]
3. Provider is validated against the chosen route. Direct Google accepts no OpenRouter provider; the only accepted direct-Google provider spelling is effectively `google`, and resolved campaign config stores direct-Google provider as `None`.[^resolve-config] [^profile-model]

The generated manifest then stores `model_id`, `provider_slug`, and `route_source` from the eval model route.[^setup-manifest-write]

### 2.3 Setup-time baseline protocol model selection

If setup will reach baseline protocol, it checks protocol model/route/provider. A setup `--protocol-model-id` can override the protocol model candidate, but the current baseline arm requires protocol to share the same model, route source, and provider as eval. A mismatch returns an error in `prepare_prototype1_loop_campaign`.[^setup-shared-model]

This means `--protocol-model-id` is not currently a free independent protocol route for Prototype 1 baseline setup. It is a compatibility/override surface that must still collapse to the same resolved route as eval for the shared baseline campaign.[^setup-shared-model]

### 2.4 Runtime campaign resolution

At runtime, `resolve_campaign_config` loads the manifest and resolves defaults.[^resolve-config]

- Dataset sources come from overrides if provided, otherwise from the manifest.[^resolve-config]
- Resolved Model ID comes from override `model_id`, then manifest `model_id`, then active model selection.[^resolve-config]
- Route source comes from explicit override/manifest value, otherwise from the model registry, otherwise `ModelRouteSource::default()`.[^resolve-config] [^route-helper]
- Provider slug comes from override/manifest provider, except direct Google converts `google`/none to `None` and rejects non-`google`; OpenRouter can fall back to the provider preference store.[^resolve-config]
- Instances and batches roots come from overrides, then manifest, then layout defaults.[^resolve-config] [^route-helper]

Prototype 1 setup usually writes `model_id`, `provider_slug`, `route_source`, `instances_root`, and `batches_root` into the manifest, so the most dangerous defaults are less likely after normal setup than for hand-authored campaign manifests.[^setup-manifest-write]

## 3. Campaign config field inventory

| Field | Persisted in | Runtime use | Overlap / footgun |
| --- | --- | --- | --- |
| `schema_version` | `CampaignManifest` | Persisted and deserialized during manifest load; the cited load path validates campaign id consistency, not schema version equality. | Versioned JSON surface; not a run-profile schema. [^campaign-load] |
| `campaign_id` | `CampaignManifest` and `ResolvedCampaignConfig` | Directory identity under campaigns root and closure/eval/protocol identity. | Also appears in parent identity, node records, invocations, and reports; do not infer authority from the string alone. [^campaign-paths] |
| `benchmark_family` | `CampaignManifest`, default `MultiSweBenchRust` | Closure/eval registry recompute and execution. | Profile does not currently choose benchmark family; profile `target` chooses dataset/instances. [^campaign-defaults] |
| `dataset_sources` | `CampaignManifest` | Registry recompute, closure selection, eval batch preparation. | Setup writes a campaign-local `slice.jsonl` source for selected instances. [^setup-manifest-write] |
| `model_id` | `CampaignManifest` optional; `ResolvedCampaignConfig` required | Eval closure passes it into batch eval; protocol closure passes it into JSON adjudication. | Confuses easily with profile `[model].id`; after setup, campaign value is the runtime source of truth. [^resolved-fields] [^eval-protocol-routing] |
| `provider_slug` | `CampaignManifest` optional; `ResolvedCampaignConfig` optional | Eval route resolution and protocol JSON config. | Direct Google may display as `google` but resolve to `None`; OpenRouter may fall back to provider preferences if omitted. [^resolve-config] [^json-llm-config] |
| `route_source` | `CampaignManifest` optional; `ResolvedCampaignConfig` required | Chooses OpenRouter vs direct Google in eval/protocol routing. | If omitted in hand-authored campaigns, registry/default fallback can matter. [^resolve-config] [^route-helper] |
| `required_procedures` | `CampaignManifest` | Protocol closure coverage requirements. | Aliases normalize to canonical kebab-case ids; docs should use canonical ids. [^campaign-fields] [^procedure-normalize] |
| `instances_root` | `CampaignManifest` optional; `ResolvedCampaignConfig` required | Where prepared instance/run directories live. | CLI alias `runs_root` exists for compatibility; profile `storage.worktree_root` does not feed this root. [^campaign-fields] [^resolve-config] |
| `batches_root` | `CampaignManifest` optional; `ResolvedCampaignConfig` required | Where batch manifests and summaries are written. | Setup nests under `batches/prototype1/<campaign>`. [^setup-manifest-write] |
| `eval` | `EvalCampaignPolicy` | Controls eval closure batch selection/execution: partials, stop-on-error, limits, labels, budget, batch prefix. | Setup fills budget from prepared batch; run-profile search budget is not eval budget. [^campaign-fields] [^setup-manifest-write] |
| `protocol` | `ProtocolCampaignPolicy` | Controls protocol row selection, max run fanout, tool review fanout, max tokens, reasoning, stop-on-error. | Profile maps only max tokens/tool review/reasoning into this policy; profile cannot currently set `max_concurrency`. [^campaign-fields] [^profile-protocol] [^protocol-fanout] |
| `framework` | `FrameworkConfig` | Carried through resolved config and closure recompute request. | Not a run-profile behavior field. [^resolved-closure-request] |

## 4. Run-profile field inventory

| Section / field | Runtime use | Overlap / footgun |
| --- | --- | --- |
| `schema_version` | Validated against `prototype1-run-profile.v1`. | Different from campaign manifest schema and commitment schema. [^profile-root] |
| `name` | Persisted operator label. | Not the campaign id. [^profile-root] |
| `[storage].worktree_root` | Defined with default `~/.ploke-eval/worktrees`. | Source-search note: no direct `profile.storage` or `.storage.worktree_root` consumption was found in `crates/ploke-eval/src` during this audit. Current live worktree paths come from repo/current checkout, node workspace roots, campaign roots, and backend worktree operations, not this profile field. [^profile-storage] |
| `[target].dataset_key` | Used by setup batch preparation when CLI `--dataset-key` is absent. | Campaign manifest stores dataset sources after setup; runtime does not reselect target from profile. [^profile-target] [^setup-target] |
| `[target].instance` / `[target].instances` | Used by setup batch preparation when CLI `--instance` is empty; `target.instance` is also the primary instance fallback. | Legacy generation rejects multiple instances; non-legacy profile is required for multi-instance child self-eval setup. [^profile-target] [^setup-admission] |
| `[model].id` | Optional setup-time source for model selection. | Not the persisted campaign `model_id` field, though setup may copy its resolved value into the manifest. [^profile-model] [^setup-model-precedence] |
| `[model].route_source` | Optional setup-time source for route source. | Accepts aliases like `google` for direct Google; campaign stores a `ModelRouteSource`, not the original spelling. [^profile-route-serde] |
| `[model].provider` | Optional setup-time provider slug. | For direct Google, only `google` is accepted; non-`google` provider with direct-Google route is rejected. [^profile-model] |
| `[search].max_generations` | Complete `prototype1-state` runs load admitted profile search policy and enforce generation hard stop before planning children. | Runtime CLI `prototype1-state` does not expose this directly; setup CLI has legacy/search flags used only when no profile is admitted. [^runtime-search-policy] [^loop-cli-search] |
| `[search].max_total_nodes` | Complete runs enforce total-node hard stop before planning. | Same profile-vs-scheduler split as `max_generations`. [^runtime-search-policy] |
| `[search].children.min/max/parallel_targets` | Child budget and fanout. `parallel_targets()` defaults to 3, caps at max, and bottoms at 1; schedule converts this into fanout width. | This is the current live config for child fanout and broad-harness patch-generation parallelism. [^child-budget] [^runtime-search-policy] [^broad-patch-cap] |
| `[search].schedule` | Selects full-batch vs adaptive-batch child scheduling. | Adaptive batch can intentionally serialize batches to allow early successor selection. [^child-budget] [^child-fanout] |
| `[search].stop_on_first_keep` | Continuation gate in successor decision. | Not a child-run stop condition; it affects whether the parent continues to next generation. [^runtime-successor-gates] |
| `[search].require_keep_for_continuation` | Continuation gate requiring selected branch disposition keep unless explore-from-rejected applies. | Can make a selected best child non-continuing. [^runtime-successor-gates] |
| `[search].explore_from_rejected` | Allows exploration from rejected child when policy permits. | Needs clear docs because it can continue from evidence whose branch disposition is not keep. [^runtime-successor-gates] |
| `[generation].source` | Maps to candidate generator: legacy, broad harness request, deterministic TUI tools. | Live complete runs reject legacy candidate generation before child planning. [^profile-generation] [^runtime-search-policy] |
| `[selection].strategy` | Maps to `Prototype1SuccessorSelection`. | Profile-driven when admitted; CLI-driven only without admitted profile. [^profile-selection] [^runtime-runshape] |
| `[selection].evidence` | Maps to traversal metrics input. | Name says evidence, but runtime type is traversal-metrics selector. [^profile-selection] |
| `[selection].metrics` | Maps to selection metrics policy. | Validation prevents imp@k scoring when persistence is disabled. [^profile-selection] |
| `[selection].oracle` | Maps to oracle mode and evidence requirement. | Relative-score + required evidence requires MBE enabled and target instances. [^profile-selection] |
| `[selection].seed` | Seeds successor selection. | Replay/selection determinism depends on this plus the considered set. [^profile-selection] |
| `[protocol].max_tokens` | Copied into `ProtocolCampaignPolicy.max_tokens`, then `JsonLlmConfig.max_tokens`. | Applies to protocol JSON calls, not eval chat/tool run budget. [^profile-protocol] [^protocol-llm-config] |
| `[protocol].tool_review_parallelism` | Copied into `ProtocolCampaignPolicy.tool_review_parallelism`, then used as a semaphore for tool-call reviews. | Segment reviews are still sequential inside each protocol run task. [^profile-protocol] [^protocol-fanout] [^review-calls] |
| `[protocol].reasoning` | Copied into protocol policy and `JsonLlmConfig`; auto reasoning is disabled for direct Google route. | Recent direct-Google bugs can hide here because `auto` becomes `disabled` only after route resolution. [^profile-protocol] [^protocol-llm-config] |
| `[execution].stop_after` | Maps to `Prototype1StateStopAfter` for the parent turn. | Different enum from setup's legacy `Prototype1LoopStopAfter`. [^profile-execution] [^loop-cli-model-flags] |
| `[execution].observe_child_stale_after_secs` | Parent C4/fanout stale timeout. | Zero is rejected. Long values affect live monitoring expectations. [^profile-execution] |
| `[execution].trace_jsonl` / `debug_tools` | Persisted profile fields. | Source-search note: this audit did not trace these into the live `cli_facing.rs` parent-turn path; treat as partially wired until proven otherwise. [^profile-execution] |
| `[execution].mbe` | Enables Multi-SWE-Bench oracle execution settings and interacts with selection oracle validation. | `mbe.workers` is distinct from child fanout and protocol fanout. [^profile-execution] [^profile-selection] |
| `[control].mode` | Persisted run-control field. | Source-search note: the extracted `prototype1_state/run/core.rs` reports effective control, but current live `prototype1-state` path in `cli_facing.rs` does not appear to consume `profile.control.mode`. [^profile-control] [^run-core-control] |
| `[control].parallel_cap` | Validation says it may narrow but not widen derived fanout. | Source-search note: in current live `cli_facing.rs`, this audit found no `control.parallel_cap` consumption; child fanout uses `search.children.parallel_targets` through `Prototype1ChildBudget`. This overlap is a footgun. [^profile-control] [^child-budget] [^runtime-search-policy] |

## 5. Config-related persistence and provenance types

| Persisted type | Originating type / source | Writer | Meaning and use |
| --- | --- | --- | --- |
| `CampaignManifest` | Built by setup from batch, CLI flags, profile defaults, and policy defaults | `save_campaign_manifest` after `prepare_prototype1_loop_campaign` | Durable campaign contract. Runtime resolves this into `ResolvedCampaignConfig`. [^setup-manifest-write] |
| Selected slice dataset JSONL | Selected lines from prepared dataset | `write_prototype1_slice_dataset` | Campaign-local dataset source used by the generated manifest. [^slice-dataset] |
| `Prototype1RunProfile` | Operator TOML profile | `admit_run_profile` writes `prototype1/run-profile.toml` | Campaign-local admitted operator behavior. Runtime run-shape/search/protocol policy use this when present. [^profile-admit-write] [^runtime-runshape] |
| `RunProfileCommitment` | Admitted profile text + source path + SHA-256 + admission timestamp | `write_commitment` writes `prototype1/run-profile.commitment.json` | Integrity/provenance guard; admitted profile load rejects digest mismatch. [^profile-admit-write] |
| `ActiveModelSelection` | CLI/model-selection state outside this command family | `save_active_model` writes `models/active-model.json` | Fallback model source if campaign/profile/CLI do not specify a model. [^active-model-selection] |
| `ProviderPrefs` | Per-model provider preference map | `set_provider_for_model` / `save_provider_prefs` write `models/provider-preferences.json` | Fallback provider source for OpenRouter routes when no explicit provider is set. [^provider-prefs] |
| `ModelRegistry` | OpenRouter and Google model-list endpoints | `refresh_model_registry` writes `models/registry.json` | Source for model existence, route source, and model metadata fallback. [^model-registry] |
| `BatchRunSummary` | Eval batch execution | `run_batch` writes `batch-run-summary.json` | Persists selected model/provider and per-instance eval results. [^eval-run-batch] |
| `RunIntent` / registration | Per-run eval attempt setup | `register_run_attempt` persists registration | Stores selected model/provider in run registry intent for eval provenance. [^run-intent] |
| `StoredProtocolArtifact` / artifact write record | Protocol procedure input/output/artifact and optional model/provider | `write_protocol_artifact` writes protocol artifact JSON | Persists protocol model/provider provenance and typed protocol evidence. [^protocol-artifact] |

## 6. Live API call boundaries and where the model is set

### 6.1 Eval/headless-TUI chat calls

The eval/headless-TUI path gets its model from campaign config:

1. `advance_eval_closure` passes `Some(config.model_id.clone())` and a parsed provider into `execute_batch_eval_for_manifest`.[^eval-protocol-routing]
2. `run_batch` parses the requested model id, resolves the selected model, loads provider preference if needed, resolves route endpoints, and records the selected provider.[^eval-run-batch]
3. Per instance, `run_batch` calls `RunMsbAgentSingleRequest::run()` sequentially.[^eval-run-batch]
4. `configure_headless_benchmark_chat` calls `configure_eval_model_runtime`, which sets `RuntimeConfig.active_model`, `RuntimeConfig.active_router`, and model-provider selection.[^eval-runtime-config]

The route-resolution step can itself make a live provider metadata API call. Direct-Google models return a direct route locally, while OpenRouter models call `OpenRouter::fetch_model_endpoints` and validate provider/tool-call support.[^eval-route-api]

### 6.2 Protocol JSON adjudication calls

The protocol path also gets model/provider/route from campaign config:

1. `advance_protocol_closure` passes `config.model_id`, `config.route_source`, `config.provider_slug`, and protocol policy values into `execute_protocol_run_tasks`.[^eval-protocol-routing]
2. `execute_protocol_run_tasks` fans out protocol run tasks up to `policy.max_concurrency` and shares a semaphore for tool-review requests.[^protocol-fanout]
3. Each protocol task builds `JsonLlmConfig` with `protocol_llm_config`.[^protocol-llm-config]
4. `JsonAdjudicator` executes protocol specs by calling `adjudicate_json`, which parses `cfg.model_id`, builds either a Google or OpenRouter JSON chat request, and sends it through `chat_step`.[^json-adjudicator] [^json-request]

OpenRouter protocol requests include provider preferences only when `provider_slug` is set; direct-Google protocol requests reject non-`google` provider slugs and otherwise use the Google router directly.[^json-request]

## 7. Conflicts, contradictions, and footguns

### 7.1 `model_id` vs `[model].id`

Risk: Profile authors may assume `[model].id` is read live on every `prototype1-state` turn. It is not. Setup may use it to fill `CampaignManifest.model_id`; runtime then uses campaign resolution.[^setup-model-precedence] [^runtime-runshape]

Recommendation: In operator docs, call `[model].id` “Profile Model ID” and campaign `model_id` “Manifest Model ID.” If refactoring, prefer a typed profile shape such as `ProfileModel { id, route, provider }` feeding a separate `CampaignModel { id, route, provider }` conversion boundary rather than renaming fields into longer flat names.

### 7.2 `provider`, `provider_slug`, and direct Google

Risk: The same semantic idea appears as CLI `--provider`, profile `[model].provider`, campaign `provider_slug`, provider preferences, and protocol artifact `provider_slug`. Direct Google makes this worse because the user-facing provider label can be `google`, while resolved/persisted provider slug is often `None`.[^loop-cli-model-flags] [^profile-model] [^resolve-config] [^protocol-artifact]

Recommendation: Use “provider slug” only for OpenRouter slugs. For direct Google, say “direct Google route” and avoid implying a provider slug is stored.

### 7.3 `route_source` fallback can drift for hand-authored campaigns

Risk: If a campaign manifest omits `route_source`, resolution consults the model registry and then defaults. A model registry refresh can change what a model appears to support, and a missing route can hide that drift until runtime or validation.[^resolve-config] [^route-helper] [^model-registry]

Recommendation: Prototype 1 setup should continue writing explicit `route_source` into generated manifests. Hand-authored campaigns should do the same.

### 7.4 Eval protocol and baseline protocol are currently tied

Risk: The CLI exposes `--protocol-model-id`, `--protocol-provider`, and `--protocol-route-source`, but Prototype 1 baseline setup currently rejects a protocol route that differs from the eval route.[^loop-cli-model-flags] [^setup-shared-model]

Recommendation: Either keep documenting this as a shared baseline invariant, or refactor into an explicit `BaselineRoute { eval, protocol }` type when the loop is ready to support split routes.

### 7.5 Profile protocol cannot set `max_concurrency`

Risk: Campaign protocol policy has both `max_concurrency` and `tool_review_parallelism`, but profile `[protocol]` exposes only `max_tokens`, `tool_review_parallelism`, and `reasoning`; `Prototype1RunProfile::protocol_policy` leaves other protocol fields at defaults.[^campaign-fields] [^profile-protocol]

Recommendation: If operators need to lower protocol run-level fanout, add it intentionally to the profile schema with validation. Do not overload `tool_review_parallelism`; it gates only tool-call review requests, not the number of protocol run tasks.[^protocol-fanout] [^review-calls]

### 7.6 `control.parallel_cap` vs `search.children.parallel_targets`

Risk: The profile contains `[control].parallel_cap` and validates it as a cap, but the current live `cli_facing.rs` parent turn appears to use `search.children.parallel_targets` for broad-harness admission and child fanout. The extracted `run/core.rs` does compute an `EffectiveRunControl`, but the main walkthrough already notes the live parent turn remains in `cli_facing.rs`.[^profile-control] [^broad-patch-cap] [^child-fanout] [^run-core-control]

Recommendation: Until refactored, document `search.children.parallel_targets` as the live fanout control for `prototype1-state`. Treat `[control].parallel_cap` as either future/extracted-controller config or wire it into the live path explicitly.

### 7.7 `storage.worktree_root` appears defined but not live-wired

Risk: A profile author can set `[storage].worktree_root`, but this audit found no direct `profile.storage` consumption in `crates/ploke-eval/src`. That makes it look authoritative while current worktree behavior is driven elsewhere.[^profile-storage]

Recommendation: Either remove/deprecate the field, wire it into worktree allocation, or document it as reserved. Do not silently pretend it controls child/parent checkout placement.

### 7.8 Sequential eval remains a major missed parallelism point

Risk: Profile/search parallelism can make child planning and child execution concurrent, but eval closure still executes selected batches sequentially, and each batch executes instances sequentially. The direct `prototype1-state` runtime CLI also remains a separate fallback/control surface from admitted run-profile execution.[^eval-protocol-routing] [^eval-run-batch] [^state-cli-flags]

Recommendation: If API quota and filesystem isolation allow it, bounded per-instance eval fanout is likely higher impact than micro-optimizing protocol artifact scans. It needs explicit caps and isolated run dirs before enabling.

## 8. Parallelism map from config to execution

| Config knob | Current execution effect | Parallel opportunity |
| --- | --- | --- |
| `search.children.parallel_targets` | Feeds `Prototype1ChildBudget.parallel_targets()`, broad-harness request publication/admission caps, and `run_child_fanout` fanout width. [^child-budget] [^broad-patch-cap] [^broad-admit-parallel] [^child-fanout] | Keep as live child/patch-generation cap. |
| `search.schedule = full-batch` | Fanout width is min(parallel targets, planned children). [^child-budget] | Runs planned children in batches of that width. |
| `search.schedule = adaptive-batch` | Fanout width is min(children.min, parallel targets, planned children), then selection can stop after a batch. [^child-budget] [^adaptive-fanout] | Sequentiality is partly semantic: early stop can save work. |
| `protocol.max_concurrency` | Campaign policy controls number of protocol run tasks spawned by `execute_protocol_run_tasks`. [^protocol-fanout] | Expose in profile if operator-level control is needed. |
| `protocol.tool_review_parallelism` | Shared semaphore for tool-call review calls. [^protocol-fanout] [^review-calls] | Already parallelizes call reviews; does not parallelize segment reviews. |
| `execution.mbe.workers` | MBE oracle worker count when MBE is enabled. [^profile-execution] | Separate from LLM/API fanout. |
| `control.parallel_cap` | Validated/profiled but not observed in live `cli_facing.rs` fanout path in this audit. [^profile-control] [^run-core-control] | Wire or deprecate. |

## 9. Refactor candidates

1. Create a typed model-route carrier used at conversion boundaries:
   - `ModelRoute { id, route, provider }` for resolved campaign/eval/protocol routes.
   - Keep profile and manifest structures separate, but make the conversion explicit.
2. Rename in docs before code:
   - Profile Model ID: `[model].id`.
   - Manifest Model ID: `CampaignManifest.model_id`.
   - Resolved Model ID: `ResolvedCampaignConfig.model_id`.
3. Split direct-Google display from provider slug:
   - Direct Google should not be described as an OpenRouter provider slug.
   - Protocol artifacts should preserve route source if provider slug is absent.
4. Decide fate of profile storage/control fields:
   - Wire `storage.worktree_root` and `control.parallel_cap` into the live path, or mark them reserved/deprecated.
5. Add profile-level protocol `max_concurrency` only if operators need it:
   - It should be a separate field, not inferred from `tool_review_parallelism`.
6. Keep route source explicit in generated manifests:
   - Avoid hand-authored campaigns depending on model-registry fallback.

## End-note citations

[^campaign-fields]: `crates/ploke-eval/src/campaign.rs:21-53` defines default required procedures and `CampaignManifest` fields, including optional `model_id`, `provider_slug`, `route_source`, `instances_root`, and `batches_root`; `crates/ploke-eval/src/campaign.rs:55-93` defines `EvalCampaignPolicy` and `ProtocolCampaignPolicy`.
[^resolved-fields]: `crates/ploke-eval/src/campaign.rs:123-138` defines `ResolvedCampaignConfig` with required runtime `model_id`, `route_source`, `instances_root`, and `batches_root` plus optional `provider_slug`.
[^resolve-config]: `crates/ploke-eval/src/campaign.rs:478-572` loads a manifest, applies overrides/defaults, parses model id, resolves route source, normalizes direct-Google provider handling, loads provider prefs for OpenRouter fallback, resolves roots, and returns `ResolvedCampaignConfig`.
[^route-helper]: `crates/ploke-eval/src/campaign.rs:894-916` resolves route source from explicit value or model registry route source, defaulting when registry lookup has no value; `crates/ploke-eval/src/campaign.rs:918-924` renders route labels.
[^campaign-defaults]: `crates/ploke-eval/src/campaign.rs:197-234` sets `BenchmarkFamily::MultiSweBenchRust` as default and initializes `CampaignManifest::new` with default procedures and empty optional model/provider/roots.
[^campaign-load]: `crates/ploke-eval/src/campaign.rs:287-313` reads and parses the campaign manifest and validates that the stored campaign id matches the requested campaign id. The source search for `schema_version` shows the field is set/deserialized but does not show a separate equality check against `CAMPAIGN_MANIFEST_SCHEMA_VERSION` in this load path.
[^campaign-paths]: `crates/ploke-eval/src/campaign.rs:277-285` derives `campaign.json` and `closure-state.json` paths under the campaign directory.
[^procedure-normalize]: `crates/ploke-eval/src/campaign.rs:842-879` normalizes required procedure aliases into canonical kebab-case procedure ids.
[^resolved-closure-request]: `crates/ploke-eval/src/campaign.rs:250-274` converts `ResolvedCampaignConfig` into a `ClosureRecomputeRequest`, carrying benchmark family, model, provider, route, dataset sources, required procedures, roots, and framework config.
[^profile-root]: `crates/ploke-eval/src/cli/prototype1_state/profile.rs:37-59` defines `Prototype1RunProfile` sections; `crates/ploke-eval/src/cli/prototype1_state/profile.rs:61-85` validates schema and all sections.
[^profile-storage]: `crates/ploke-eval/src/cli/prototype1_state/profile.rs:117-128` defines `Storage { worktree_root }` and its default. Source-search note: content search for `profile\.storage|\.storage\.worktree_root` in `crates/ploke-eval/src` returned no matches in this audit.
[^profile-model]: `crates/ploke-eval/src/cli/prototype1_state/profile.rs:130-179` defines `ModelDefaults { id, route_source, provider }`, parses model id, validates provider strings, and rejects direct-Google route with a non-`google` provider.
[^profile-route-serde]: `crates/ploke-eval/src/cli/prototype1_state/profile.rs:183-218` serializes/deserializes optional profile route source and accepts aliases such as `open-router`, `open_router`, `direct_google`, and `google`.
[^profile-target]: `crates/ploke-eval/src/cli/prototype1_state/profile.rs:221-278` defines target fields, validates duplicate/empty instances, requires `target.instance` to appear in `target.instances` when both are set, and exposes `eval_instances()` / `primary_instance()`.
[^profile-protocol]: `crates/ploke-eval/src/cli/prototype1_state/profile.rs:107-113` maps profile protocol settings into `ProtocolCampaignPolicy`; `crates/ploke-eval/src/cli/prototype1_state/profile.rs:577-611` defines and validates profile protocol `max_tokens`, `tool_review_parallelism`, and reasoning.
[^profile-execution]: `crates/ploke-eval/src/cli/prototype1_state/profile.rs:635-686` defines `Execution`, validates stale timeout, maps stop-after into `Prototype1StateStopAfter`, and exposes child stale timeout; `crates/ploke-eval/src/cli/prototype1_state/profile.rs:688-721` defines/validates MBE settings.
[^profile-control]: `crates/ploke-eval/src/cli/prototype1_state/profile.rs:747-790` defines `Control { mode, parallel_cap }` and validates that `parallel_cap` is nonzero and does not widen derived fanout.
[^profile-selection]: `crates/ploke-eval/src/cli/prototype1_state/profile.rs:375-456` defines selection strategy/evidence/metrics/oracle/seed mappings and validates MBE/oracle constraints; `crates/ploke-eval/src/cli/prototype1_state/profile.rs:486-568` defines metrics/imp@k defaults and validation.
[^profile-generation]: `crates/ploke-eval/src/cli/prototype1_state/profile.rs:336-373` defines generation source and maps it to `Prototype1CandidateGenerator`.
[^profile-load-admit]: `crates/ploke-eval/src/cli/prototype1_state/profile.rs:822-835` loads operator profiles; `crates/ploke-eval/src/cli/prototype1_state/profile.rs:922-930` resolves bare profile names under `~/.ploke-eval/profiles/prototype1/<name>.toml`.
[^profile-admit-write]: `crates/ploke-eval/src/cli/prototype1_state/profile.rs:837-865` writes admitted `run-profile.toml` and constructs `RunProfileCommitment`; `crates/ploke-eval/src/cli/prototype1_state/profile.rs:868-903` loads admitted profile and rejects digest mismatch; `crates/ploke-eval/src/cli/prototype1_state/profile.rs:948-954` writes the commitment JSON.
[^loop-cli-model-flags]: `crates/ploke-eval/src/cli.rs:1002-1163` defines `Prototype1LoopCommand`, including setup/batch/profile/root/model/provider/route/protocol/search/stop flags; `crates/ploke-eval/src/cli.rs:1072-1110` specifically defines default/eval/protocol model-provider-route flags.
[^loop-cli-search]: `crates/ploke-eval/src/cli.rs:1120-1154` defines setup CLI search and wrapper stop flags such as max generations, child limits, schedule mode, continuation gates, and wrapper stop-after.
[^state-cli-flags]: `crates/ploke-eval/src/cli.rs:609-661` defines the direct `Prototype1StateCommand` runtime CLI shape: campaign/node/repo/identity/handoff/stop-after/selection/generator/format.
[^setup-admission]: `crates/ploke-eval/src/cli/prototype1_state/cli_facing.rs:195-301` loads optional operator profile, prepares/loads batch, rejects unsupported multi-instance legacy profile setup, prepares campaign, admits profile, ensures baseline closure, registers root parent node, checks out parent branch, writes parent identity, commits, and returns setup report.
[^setup-target]: `crates/ploke-eval/src/cli/prototype1_state/cli_facing.rs:3826-3863` lets CLI dataset/instance values override profile target defaults, then prepares the batch request with repo/instance/batch roots and eval budget.
[^setup-model-precedence]: `crates/ploke-eval/src/cli/prototype1_state/cli_facing.rs:7553-7593` selects setup model/route/provider from command flags, profile model defaults, default/active model resolution, selected model registry route source, and provider resolution.
[^setup-shared-model]: `crates/ploke-eval/src/cli/prototype1_state/cli_facing.rs:7595-7633` resolves optional protocol model/route/provider and errors unless baseline eval and baseline protocol share the same model, route source, and provider.
[^setup-manifest-write]: `crates/ploke-eval/src/cli/prototype1_state/cli_facing.rs:7635-7714` builds campaign id/path, writes `slice.jsonl`, creates `CampaignManifest`, sets dataset/model/provider/route/roots/eval/protocol policy, saves the manifest, resolves it, and returns campaign paths.
[^slice-dataset]: `crates/ploke-eval/src/cli/prototype1_state/cli_facing.rs:7717-7771` filters selected dataset instances, creates the output parent directory, and writes the campaign-local slice JSONL file.
[^runtime-runshape]: `crates/ploke-eval/src/cli/prototype1_state/cli_facing.rs:946-999` defines `Prototype1StateRunShape`, maps CLI and profile values, and resolves by loading an admitted profile first, falling back to CLI/defaults when absent.
[^runtime-search-policy]: `crates/ploke-eval/src/cli/prototype1_state/cli_facing.rs:6670-6701` loads complete-run search policy from admitted profile or scheduler state, rejects complete live legacy generation, and reserves child budget or uses a one-child debug budget.
[^runtime-successor-gates]: `crates/ploke-eval/src/cli/prototype1_state/cli_facing.rs:5327-5400` enforces continuation gates including selected branch, direct child, generation, total nodes, require-keep, stop-on-first-keep, historical traversal, and explore-from-rejected outcomes.
[^child-budget]: `crates/ploke-eval/src/intervention/scheduler.rs:156-190` defines `Prototype1ChildBudget { min, max, parallel_targets }`, default values, explicit `with_parallel_targets`, and `parallel_targets()` default/cap behavior; `crates/ploke-eval/src/intervention/scheduler.rs:193-221` defines full-batch/adaptive-batch fanout width.
[^broad-patch-cap]: `crates/ploke-eval/src/cli/prototype1_state/cli_facing.rs:1241-1299` publishes broad-harness request slots and records `patch_generation_parallel_cap: child_budget.parallel_targets()` in `HarnessRequestBatch`.
[^child-fanout]: `crates/ploke-eval/src/cli/prototype1_state/cli_facing.rs:5123-5245` runs planned children in batches using `Prototype1ChildScheduleMode::fanout_width`, `JoinSet::spawn_blocking`, and sorted outcomes.
[^adaptive-fanout]: `crates/ploke-eval/src/cli/prototype1_state/cli_facing.rs:5248-5314` runs adaptive child fanout in batches, re-runs successor selection after each batch, and stops early when selection accepts/explores a successor.
[^broad-admit-parallel]: `crates/ploke-eval/src/cli/prototype1_state/cli_facing.rs:4412-4504` admits broad-harness slots with a `JoinSet` bounded by `batch.patch_generation_parallel_cap` and `child_budget.max`.
[^active-model-selection]: `crates/ploke-eval/src/model_registry.rs:22-25` defines `ActiveModelSelection`; `crates/ploke-eval/src/model_registry.rs:257-305` writes active/parent-patcher model selection JSON; `crates/ploke-eval/src/model_registry.rs:311-332` resolves explicit/default/active model to a registry item.
[^provider-prefs]: `crates/ploke-eval/src/provider_prefs.rs:11-14` defines `ProviderPrefs`; `crates/ploke-eval/src/provider_prefs.rs:43-64` writes provider preferences; `crates/ploke-eval/src/provider_prefs.rs:66-88` loads/sets provider for a model.
[^model-registry]: `crates/ploke-eval/src/model_registry.rs:52-61` refreshes and writes the merged model registry; `crates/ploke-eval/src/model_registry.rs:64-89` fetches OpenRouter and Google registries; `crates/ploke-eval/src/model_registry.rs:185-210` saves sorted `models/registry.json`.
[^eval-protocol-routing]: `crates/ploke-eval/src/cli.rs:6904-6955` advances eval closure, prepares batches, writes run/batch manifests, and calls `execute_batch_eval_for_manifest` with `config.model_id`; `crates/ploke-eval/src/cli.rs:6966-7022` advances protocol closure and calls `execute_protocol_run_tasks` with campaign model/route/provider and protocol policy.
[^eval-run-batch]: `crates/ploke-eval/src/runner.rs:3730-3904` parses requested model, resolves selected model/provider/route, writes batch submission aggregate, sequentially runs each prepared instance, and writes `BatchRunSummary` with selected model/provider.
[^eval-runtime-config]: `crates/ploke-eval/src/runner.rs:116-130` sets eval runtime active model/router/provider and applies benchmark chat policy for headless benchmark chat.
[^eval-route-api]: `crates/ploke-eval/src/runner.rs:2501-2602` validates tool support, handles direct-Google routes locally, calls `OpenRouter::fetch_model_endpoints` for OpenRouter models, validates requested provider support, and returns an `LlmRoute`.
[^run-intent]: `crates/ploke-eval/src/runner.rs:158-214` builds and persists run registration intent including selected model id and provider slug.
[^protocol-fanout]: `crates/ploke-eval/src/cli.rs:1745-1846` defines `execute_protocol_run_tasks`, bounds run task fanout by `max_concurrency`, and shares a semaphore sized by `tool_review_parallelism`.
[^protocol-llm-config]: `crates/ploke-eval/src/cli.rs:7741-7761` builds `JsonLlmConfig` from model/route/provider/timeout/attempt/token/reasoning inputs; `crates/ploke-eval/src/cli.rs:7764-7773` disables auto reasoning for direct-Google routes.
[^review-calls]: `crates/ploke-eval/src/cli.rs:7775-7834` spawns one tool-call review task per subject and gates each request with the shared semaphore.
[^json-llm-config]: `crates/ploke-protocol/src/llm.rs:27-62` defines `JsonLlmConfig` fields/defaults and `provider_display`, which displays direct Google as `google` and OpenRouter provider as provider slug or `auto/openrouter`.
[^json-adjudicator]: `crates/ploke-protocol/src/llm.rs:218-275` defines `JsonAdjudicator`, labels direct Google vs OpenRouter execution, calls `adjudicate_json`, and records `JsonLlmProvenance` with model, route, provider, content, reasoning, and full response.
[^json-request]: `crates/ploke-protocol/src/llm.rs:537-647` parses `JsonLlmConfig.model_id`, builds direct-Google or OpenRouter JSON chat requests, sets JSON response/max tokens/reasoning, applies OpenRouter provider preferences when set, rejects invalid direct-Google providers, and sends via `chat_step`.
[^protocol-artifact]: `crates/ploke-eval/src/protocol_artifacts.rs:21-35` defines `StoredProtocolArtifact` with optional model/provider; `crates/ploke-eval/src/protocol_artifacts.rs:292-352` writes protocol artifact JSON with procedure, subject, run id, model/provider, input/output/artifact, and syncs registration status.
[^run-core-control]: `crates/ploke-eval/src/cli/prototype1_state/run/core.rs:73-80` defines `EffectiveRunControl`; `crates/ploke-eval/src/cli/prototype1_state/run/core.rs:472-500` computes `parallel_cap` from admitted profile control and patch-generation cap from profile search children. The main walkthrough notes the live parent turn remains in `cli_facing.rs`.
