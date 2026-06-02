# Prototype1-state phase-specific model routing analysis (2026-06-01)

Status: analysis/audit only. This note proposes routing seams; it does not implement Rust changes, does not run live providers, and does not start benchmark campaigns.

## 1. Short verdict

The main seam should live in the admitted Prototype1 run profile plus the campaign manifest, with a small role-aware resolver that runs before existing runner/protocol/headless-TUI execution receives a concrete model/provider/route.

Why:

- `prototype1 loop` already resolves a single campaign-level eval model in `crates/ploke-eval/src/cli/prototype1_state/cli_facing.rs::prepare_prototype1_loop_campaign`, then persists it in `CampaignManifest` (`crates/ploke-eval/src/campaign.rs`). Parent baseline eval, protocol closure, and child treatment campaigns all currently inherit from this campaign-level choice.
- Low-level eval execution already wants concrete values. `advance_eval_closure` passes `config.model_id` and provider into `execute_batch_eval_for_manifest` (`crates/ploke-eval/src/cli.rs`), and `runner.rs`/`inner/core.rs` freeze the concrete `model_id` and `provider_slug` for reproducibility. Do not make the runner infer phase policy.
- Protocol execution is already structurally separable: `protocol_llm_config` accepts `model_id`, `route_source`, and `provider`, and `StoredProtocolArtifact` already records `model_id` and `provider_slug`. The current coupling is a policy/persistence choice, not a protocol engine limitation.
- Broad parent patch generation is already a separate runtime family. `BroadTuiAttemptOptions` chooses a `tui_adapter::ModelSelection`, and `load_parent_patcher_model_selection()` has an explicit temporary comment saying parent patching currently reads a split parent-patcher selection while eval/protocol read the active model. This is the strongest code-level hint that the intended destination is profile/campaign plumbing.

Recommended shape:

1. Add stable phase roles and a shared `ModelDefaults`-like role selection object.
2. Persist configured/resolved role selections in `CampaignManifest` or a campaign-adjacent role manifest, and admit them from `Prototype1RunProfile`.
3. Resolve each phase to concrete `{model_id, route_source, provider_slug}` at phase boundaries:
   - before baseline eval batch execution,
   - before treatment/child self-eval campaign creation,
   - before protocol `JsonLlmConfig` creation,
   - before broad headless-TUI attempt execution/published request creation.
4. Keep runner/protocol/headless-TUI adapters as concrete executors; they should record the model actually used, not decide policy.

## 2. Current routing map

### 2.1 Registry and provider preference layer

Current facts:

- `crates/ploke-eval/src/model_registry.rs` stores model selections as `ActiveModelSelection { model_id }` and exposes:
  - `load_active_model()` / `load_active_model_at()` for the general active model.
  - `load_parent_patcher_model()` / `load_parent_patcher_model_at()` for the broad parent patcher split selection.
  - `resolve_model_for_run(explicit_model_id, use_default_model)` which loads the registry and returns a `ResponseItem` selected by explicit id, default model key, or active model.
- `crates/ploke-eval/src/provider_prefs.rs` stores `ProviderPrefs { selected_providers: BTreeMap<String, ProviderKey> }` and exposes `load_provider_for_model(model_id)`.
- In the current CLI/prototype paths inspected here, route parsing accepts `openrouter` and `direct-google` spellings:
  - `crates/ploke-eval/src/cli.rs::parse_model_route_source`
  - `crates/ploke-eval/src/cli/prototype1_state/profile.rs::optional_profile_route_source`
- Direct-Google constraints are enforced in several places. For example `cli.rs::headless_model_selection` rejects an OpenRouter provider when the registry/request implies direct Google, and `cli.rs::resolve_protocol_route` does the same for protocol.

Implication:

The registry/prefs layer is a good backing source for defaults, but it is not enough for phase-specific reproducibility. Active-model files and provider prefs are mutable global state. A run should persist the resolved phase selections in campaign/run/protocol artifacts.

### 2.2 Prototype1 loop CLI, profile, and campaign manifest

Current facts:

- `crates/ploke-eval/src/cli.rs::Prototype1LoopCommand` has a campaign-level model path:
  - `model_id`
  - `provider`
  - `route_source`
  - `use_default_model`
  - protocol-specific `protocol_model_id`, `protocol_provider`, and `protocol_route_source`
- `crates/ploke-eval/src/cli.rs::Prototype1StateCommand` does not expose general model/provider/route flags for normal state advancement. State advancement resolves the existing campaign and admitted profile instead.
- `crates/ploke-eval/src/cli/prototype1_state/profile.rs::Prototype1RunProfile` has one `model: ModelDefaults` field. `ModelDefaults` has:
  - `id: Option<String>`
  - `route_source: Option<ModelRouteSource>`
  - `provider: Option<String>`
- The profile protocol block is not a model selection block. `profile.rs::Protocol` currently configures protocol execution parameters only:
  - `max_tokens`
  - `tool_review_parallelism`
  - `reasoning`
- `crates/ploke-eval/src/campaign.rs::CampaignManifest` persists one campaign-level `model_id`, `provider_slug`, and `route_source`, plus `eval: EvalCampaignPolicy` and `protocol: ProtocolCampaignPolicy`.
- `crates/ploke-eval/src/campaign.rs::ResolvedCampaignConfig` resolves one campaign-level `model_id`, `provider_slug`, and `route_source`.
- `crates/ploke-eval/src/cli/prototype1_state/cli_facing.rs::Prototype1LoopControllerInput::from_command` loads/adopts a profile, prepares the batch, calls `prepare_prototype1_loop_campaign`, then admits the run profile beside the campaign manifest.
- `cli_facing.rs::prepare_prototype1_loop_campaign` resolves a single eval model from command/profile/defaults, writes it into `CampaignManifest`, and, when the loop will reach baseline protocol, currently rejects divergence between eval and protocol selections with this policy error: the baseline arm delegates to closure/campaign and currently requires one shared model/route/provider.

Implication:

The campaign manifest is already the right persistence anchor, but its schema only represents one shared model choice. The profile has exactly one model block, so it cannot express phase roles without extension.

### 2.3 Parent baseline eval

Current facts:

- Parent baseline eval is advanced through `crates/ploke-eval/src/cli/prototype1_state/run/core.rs::advance_baseline_eval`, which calls `crate::cli::advance_eval_closure(&context.resolved_campaign, &context.resolved_campaign.eval, false, None)`.
- `crates/ploke-eval/src/cli.rs::advance_eval_closure`:
  - recomputes closure state,
  - selects eval rows,
  - prepares MBE batch manifests,
  - attaches `campaign_context_from_config(config)` to prepared runs,
  - parses `config.provider_slug`,
  - calls `execute_batch_eval_for_manifest(..., Some(config.model_id.clone()), provider.clone(), ...)`.
- `crates/ploke-eval/src/spec.rs::PreparedCampaignContext` stores campaign-local `model_id` and `provider_slug` on prepared run manifests.
- `crates/ploke-eval/src/runner.rs` propagates selected concrete values into run records/metadata. `inner/core.rs::RunIntent` and `FrozenRunSpec` carry `model_id` and `provider_slug`; `RunIntent::freeze()` copies them.

Current behavior:

Baseline eval uses the campaign-level `ResolvedCampaignConfig.model_id`, `provider_slug`, and `route_source`. There is no separate `baseline_eval_model` role.

### 2.4 Child self-eval / treatment campaigns

Current facts:

- Child execution is entered through `crates/ploke-eval/src/cli/prototype1_process.rs::execute_prototype1_runner_invocation`, which loads a child invocation, builds child runtime state, then calls `run_prototype1_resolved_branch_treatment`.
- `run_prototype1_resolved_branch_treatment` prepares a treatment campaign by calling `cli_facing.rs::prepare_prototype1_treatment_campaign(&baseline_resolved, branch_id)`.
- `prepare_prototype1_treatment_campaign` creates a new campaign id and copies the baseline campaign selection:
  - `manifest.model_id = Some(baseline.model_id.clone())`
  - `manifest.provider_slug = baseline.provider_slug.clone()`
  - `manifest.route_source = Some(baseline.route_source)`
  - `manifest.eval = baseline_manifest.eval.clone()`
  - `manifest.protocol = baseline_manifest.protocol.clone()`
- The child then advances eval closure for the treatment campaign via `advance_eval_closure(&treatment_campaign.resolved, &eval_policy, false, Some(instance_repo_cache.as_path()))`.
- The child also advances protocol closure for the treatment campaign via `advance_protocol_closure(&treatment_campaign.resolved, &protocol_policy, false)`.

Current behavior:

Child self-eval is a new campaign that inherits the same model/provider/route as the parent baseline campaign. There is no separate `child_self_eval_model` role. Parent baseline and child self-eval share by construction.

### 2.5 Protocol / LLM adjudication

Current facts:

- Protocol CLI config is built in `crates/ploke-eval/src/cli.rs::protocol_llm_config(model_id, route_source, provider, timeout_secs, max_attempts, max_tokens, reasoning)`.
- `protocol_llm_config` resolves the model using `resolve_protocol_model_id`, resolves route/provider with `resolve_protocol_route`, adjusts reasoning via `effective_protocol_reasoning`, and returns `JsonLlmConfig`.
- `resolve_protocol_model_id(None)` still reads `load_active_model()`. The code comment explicitly says this is a temporary split: eval/protocol defaults read active model while broad parent patching reads `load_parent_patcher_model_selection()`, and both should collapse onto admitted profile/campaign config once plumbing exists.
- `crates/ploke-eval/src/cli.rs::advance_protocol_closure` currently passes campaign-level values into `execute_protocol_run_tasks`:
  - `config.model_id.clone()`
  - `config.route_source`
  - `config.provider_slug.clone()`
  - protocol policy execution settings (`max_concurrency`, `tool_review_parallelism`, `max_tokens`, `reasoning`)
- `crates/ploke-eval/src/protocol_artifacts.rs::StoredProtocolArtifact` already stores `model_id: Option<String>` and `provider_slug: Option<String>` for each persisted protocol artifact.
- `crates/ploke-eval/src/cli.rs::print_protocol_artifact_detail` renders stored model/provider.

Current behavior:

Protocol/adjudication can technically accept a distinct config, and artifacts can record it, but Prototype1 campaign/closure policy currently feeds the same campaign-level model and route into protocol. `prepare_prototype1_loop_campaign` explicitly prevents baseline eval/protocol divergence in the loop path.

### 2.6 Broad headless-TUI parent patch generation

Current facts:

- Parent child planning uses `crates/ploke-eval/src/cli/prototype1_state/cli_facing.rs::run_parent_target_selection`.
- Depending on `CandidateGenerationConfig`, broad generation publishes a broad-harness request via `publish_broad_harness_child_plan_request` / `publish_broad_edit_harness_request` instead of using the legacy target-selection loop path.
- `crates/ploke-eval/src/cli/prototype1_state/edit_surface/harness_request.rs::BroadHarnessRequest` contains parent/workspace/edit-policy/budget/evidence/instructions, but no model/provider/route field.
- `cli_facing.rs::BroadTuiAttemptOptions` carries an optional `tui_adapter::ModelSelection`. Its constructors currently choose:
  - explicit CLI attempt model/provider via `from_cli(model_id, provider, ...)`, or
  - `crate::cli::load_parent_patcher_model_selection()` by default.
- `crates/ploke-eval/src/cli.rs::load_parent_patcher_model_selection()` loads a dedicated parent-patcher model if present, otherwise falls back to `load_active_model()`, then loads provider prefs and returns a `tui_adapter::ModelSelection`.
- `crates/ploke-eval/src/cli/prototype1_state/edit_surface/tui_adapter.rs::run_headless_with_model` accepts `Option<ModelSelection>`, starts a headless `ploke-tui` runtime, and calls runtime model/provider selection when a model is supplied.
- `cli_facing.rs::BroadHarnessAttemptProjection` records `model_id` and `provider` for attempt projection, along with request/workspace/result/diagnostic paths and status.
- `crates/ploke-eval/src/cli/prototype1_state/edit_surface/harness_result.rs` submitted result/admission structures do not appear to persist the executor model/provider/route; they focus on request binding, return evidence, admission, workspace/artifact evidence, executor run/attempt id, and result paths.

Current behavior:

Broad parent patch generation is already operationally split from eval/protocol through the parent-patcher selection file and attempt options. But the split is outside the admitted profile/campaign model policy, and the published request/result schema does not make the desired/actual model route explicit enough for reproducible external harness execution.

## 3. Phase / role taxonomy

Proposed stable role names:

### `parent_patch_generation`

Purpose: generate candidate edits for a parent state, especially broad headless-TUI patch attempts.

Current surfaces:

- `BroadTuiAttemptOptions`
- `tui_adapter::ModelSelection`
- `load_parent_patcher_model_selection()`
- `BroadHarnessRequest` / `SubmittedBroadHarnessResult` / attempt projection

Capabilities to require/preflight:

- edit-surface/tool compatibility for `ploke-tui`, not just plain chat completion;
- adequate context length for workspace prompt and evidence roots;
- route that supports the TUI request path being used;
- ability to produce usable edits, with admission handled separately by Ploke.

### `protocol_adjudicator`

Purpose: run procedure-based LLM adjudication and JSON/structured protocol artifact generation.

Current surfaces:

- `ProtocolCampaignPolicy`
- `protocol_llm_config`
- `JsonAdjudicator`
- `execute_protocol_run_tasks`
- `StoredProtocolArtifact`

Capabilities to require/preflight:

- structured JSON output reliability or procedure-compatible JSON retry support;
- procedure-artifact compatibility for tool-call-intent/tool-call-review/segment-review;
- reasoning policy compatibility, especially direct-Google cases where auto reasoning is currently disabled;
- appropriate max tokens and parallelism.

### `baseline_eval_model`

Purpose: run parent baseline benchmark/eval turns for the campaign's selected instances.

Current surfaces:

- `CampaignManifest.model_id/provider_slug/route_source`
- `ResolvedCampaignConfig`
- `advance_eval_closure`
- `execute_batch_eval_for_manifest`
- `PreparedCampaignContext`
- `RunIntent` / `FrozenRunSpec` / run records

Capabilities to require/preflight:

- benchmark-turn compatibility;
- tool-use behavior expected by MBE harness;
- cost/concurrency constraints;
- route reachability and provider availability.

### `child_self_eval_model`

Purpose: evaluate a child treatment campaign after a candidate patch is materialized and run against the same benchmark family/instances.

Current surfaces:

- `prepare_prototype1_treatment_campaign`
- child invocation/runtime path in `prototype1_process.rs`
- treatment campaign `ResolvedCampaignConfig`
- treatment `advance_eval_closure`

Recommended default:

Keep `child_self_eval_model` separate as a named override point, but default it to `baseline_eval_model`.

Rationale:

- Sharing by default preserves current behavior and makes baseline-vs-treatment comparison fair unless Joseph explicitly opts into a different child evaluator.
- A separate override is still useful. Child self-eval may later need a cheaper model for high fanout, a stricter model for final confirmation, or a deliberately independent judge model to reduce self-confirmation bias.
- Persisting the default alias matters. Reports should distinguish `child_self_eval_model = inherited baseline_eval_model` from `child_self_eval_model = independently configured but equal values`.

Optional umbrella role:

A convenience role named `eval_model` can be supported as an alias/default source for both `baseline_eval_model` and `child_self_eval_model`, but the persisted/read-side view should still render the concrete phase roles.

## 4. Implementation seam options

### Option A: CLI flags only

Possible fields:

- `--baseline-eval-model-id`, `--baseline-eval-provider`, `--baseline-eval-route-source`
- `--child-self-eval-model-id`, `--child-self-eval-provider`, `--child-self-eval-route-source`
- `--parent-patch-model-id`, `--parent-patch-provider`, `--parent-patch-route-source`
- existing `--protocol-model-id`, `--protocol-provider`, `--protocol-route-source`

Pros:

- Direct and testable with CLI parsing snapshots.
- Good for one-off operator overrides.

Cons / migration risk:

- Bad as the primary seam: Prototype1 state advancement usually happens through an existing campaign and admitted profile, and `Prototype1StateCommand` lacks general model flags.
- CLI-only choices are easy to lose on detached successor/child turns.
- Does not naturally persist default/inherited role resolution.

Use CLI flags as overrides into the role resolver, not as the core policy store.

Structs/functions affected:

- `cli.rs::Prototype1LoopCommand`
- optionally specific broad-TUI attempt commands/options
- `cli_facing.rs::prepare_prototype1_loop_campaign`
- parser/snapshot tests in `prototype1_state/tests/cli_tests.rs`

### Option B: run-profile TOML / admitted profile

Pros:

- Best operator-facing config seam.
- Profiles are already admitted beside the campaign in `Prototype1LoopControllerInput::from_command`.
- Existing `Prototype1RunProfile.model: ModelDefaults` is already validated and close to the needed shape.

Cons / migration risk:

- Profile is an input policy, not necessarily the final resolved route. Persist resolved values separately in the campaign to make runs reproducible after global registry/prefs change.
- Need a backwards-compatible schema so existing profiles with `[model]` continue to mean the shared/default model.

Suggested schema shape:

```toml
[model]
id = "..."              # legacy/shared default
provider = "..."
route_source = "openrouter"

[model_roles.parent_patch_generation]
id = "..."
provider = "..."
route_source = "openrouter"

[model_roles.protocol_adjudicator]
id = "..."
route_source = "direct-google"

[model_roles.baseline_eval_model]
id = "..."
provider = "..."

[model_roles.child_self_eval_model]
inherits = "baseline_eval_model" # optional; default behavior
```

A map is more extensible than hard-coded fields, but explicit typed fields may be easier for first implementation. If using a map, validate unknown role names loudly.

Structs/functions affected:

- `profile.rs::Prototype1RunProfile`
- `profile.rs::ModelDefaults` or a new `RoleModelDefaults`
- `profile.rs` validation and round-trip tests
- `cli_facing.rs::prepare_prototype1_loop_campaign`
- `run/core.rs` runtime context if role config needs read-side access

### Option C: campaign manifest / resolved campaign config

Pros:

- Best reproducibility seam.
- Existing `CampaignManifest` already records `model_id`, `provider_slug`, and `route_source`.
- Existing `ResolvedCampaignConfig` is already threaded into eval/protocol closure advancement.
- Child treatment campaigns already copy campaign manifest fields; this is exactly where `child_self_eval_model` inheritance/override should be resolved.

Cons / migration risk:

- Changing `CampaignManifest` affects closure-state recompute/read-side code.
- Need careful schema defaults so existing campaign manifests remain valid.
- If only the resolved role map is stored without the requested role map/source, operators lose visibility into whether a value was inherited, CLI-overridden, profile-overridden, or defaulted from active model/provider prefs.

Suggested campaign shape:

```json
{
  "model_id": "... legacy/shared eval model ...",
  "provider_slug": "...",
  "route_source": "openrouter",
  "model_roles": {
    "baseline_eval_model": {
      "model_id": "...",
      "provider_slug": "...",
      "route_source": "...",
      "source": "profile:model_roles.baseline_eval_model"
    },
    "child_self_eval_model": {
      "inherits": "baseline_eval_model",
      "model_id": "...",
      "provider_slug": "...",
      "route_source": "..."
    },
    "protocol_adjudicator": { "model_id": "...", "route_source": "..." },
    "parent_patch_generation": { "model_id": "...", "provider_slug": "..." }
  }
}
```

Keep top-level `model_id/provider_slug/route_source` during migration as the shared eval defaults and read-side compatibility fields.

Structs/functions affected:

- `campaign.rs::CampaignManifest`
- `campaign.rs::CampaignOverrides`
- `campaign.rs::ResolvedCampaignConfig`
- `campaign.rs::StoredClosureConfig` / closure adoption paths
- `closure.rs::render_closure_status`
- `cli_facing.rs::prepare_prototype1_loop_campaign`
- `cli_facing.rs::prepare_prototype1_treatment_campaign`

### Option D: per-run registry / frozen run specs only

Pros:

- Eval runs already freeze concrete model/provider values in `RunIntent` and `FrozenRunSpec`.
- Good for proving what actually ran.

Cons / migration risk:

- Too late for policy. By the time the run registry exists, the campaign/phase decision has already happened.
- Does not cover parent patch generation well unless broad harness attempts also become run-registry participants.
- Does not cover protocol completely unless protocol artifacts also get a role/route field.

Use this as an observability sink, not the primary config seam.

Structs/functions affected:

- `inner/core.rs::RunIntent`
- `inner/core.rs::FrozenRunSpec`
- `inner/registry.rs::RunRegistration`
- `runner.rs` request freezing and record metadata
- possibly `PreparedCampaignContext` with `model_role`

### Option E: environment fallback / global active model files

Pros:

- Already exists: active model, parent patcher model, provider prefs.
- Useful for local defaults and smoke tests.

Cons / migration risk:

- Not reproducible across detached turns or time.
- Mutating global state can change future phases of the same logical experiment if values are not persisted at campaign creation/admission time.
- Harder to explain in closure/read-side views.

Keep only as a fallback source for the resolver, then persist the resolved role selections.

## 5. Persistence and observability impact

### Campaign state

Record both requested and resolved role selections, or at least resolved selections plus a source label.

Minimum for each role:

- `role`: one of the stable role names.
- `model_id`: concrete model id used or intended.
- `route_source`: concrete route source.
- `provider_slug`: concrete provider when the route uses provider endpoints; null/omitted for direct route where appropriate.
- `source`: e.g. `cli`, `profile`, `campaign_manifest`, `active_model_fallback`, `parent_patcher_model_fallback`, `inherited:baseline_eval_model`.
- optional `capability_preflight`: pass/fail/warn summary.

### Run registry and eval records

The existing run registry records concrete model/provider, but it should also record the phase role to avoid ambiguity when the same campaign has multiple models.

Recommended additions:

- `PreparedCampaignContext.model_role` or `RunIntent.model_role` for eval runs.
- `FrozenRunSpec.model_role` copied from intent.
- record metadata path should continue to store selected concrete model/provider.

For baseline eval runs, `model_role = baseline_eval_model`.
For child treatment/self-eval runs, `model_role = child_self_eval_model`.

### Node records / child plan artifacts

Node records should not become the primary model policy store, but child artifacts should expose enough to audit candidate generation:

- planned child generated by `parent_patch_generation` role;
- desired model/route persisted in the request or child plan metadata;
- actual executor model/route persisted in submitted result or attempt projection;
- admission outcome stored separately from route outcome.

### Headless harness request/result

Current request/result schemas do not clearly persist model/provider/route. Add:

- request desired role selection: `model_role`, requested/resolved model route, and source.
- result actual executor selection: model id, route source, provider slug, executor runtime id/attempt id.
- projection already has `model_id` and `provider`; extend it with route source and role.

This matters for external harness execution: a published request with no model policy leaves the model choice to whoever runs the harness, which is not reproducible.

### Protocol artifacts

`StoredProtocolArtifact` already records `model_id` and `provider_slug`; extend it with:

- `model_role = protocol_adjudicator`
- `route_source`
- maybe `reasoning`, `max_tokens`, and `procedure_capability` snapshot if not already derivable from input/config.

Do not rely only on campaign state for protocol: protocol artifacts are the local proof of what adjudicated a specific procedure/subject.

### Closure/read-side views

`closure.rs::render_closure_status` currently prints a single model/provider/route for the campaign. With phase roles it should render something like:

```text
campaign ...
models:
  baseline_eval_model: <model> provider=<provider> route=<route>
  child_self_eval_model: inherited baseline_eval_model (<model> ...)
  protocol_adjudicator: <model> provider=<provider> route=<route>
  parent_patch_generation: <model> provider=<provider> route=<route>
```

### Avoid confusing route success with semantic phase success

Separate these concepts in every record/report:

- Route/preflight status: can the model/provider/route be resolved and reached?
- Execution status: did the provider call/tool session complete mechanically?
- Admission/protocol/eval status: did the phase semantically succeed?

For broad harness attempts, provider unavailable, timeout, no-edit, admission reject, and accepted child plan should be distinct statuses. For protocol, JSON retry exhaustion/provider failure should not be reported as a negative adjudication. For eval, model route failure should not be mixed with benchmark failure if the benchmark never ran.

## 6. Capability / preflight implications

### `protocol_adjudicator`

Protocol is more than chat completion. It needs procedure-compatible JSON outputs. Preflight should check or at least classify:

- route supports the target model;
- structured/JSON output support or retry policy is acceptable;
- reasoning policy is compatible with route/model;
- max token budget is sufficient for procedure artifacts;
- expected protocol procedures are supported by the adjudicator implementation.

Current hint: `effective_protocol_reasoning` disables auto reasoning for direct-Google routes, which means route/model capabilities already affect protocol config.

### `parent_patch_generation`

Parent patching through headless TUI is an edit/tool workflow. Preflight should check:

- TUI can select the model/provider route;
- model/tool path can use read/edit tools in the workspace-scoped policy;
- context window can hold prompt plus evidence roots;
- request/result schema records selected model;
- admission boundary is independent from model route.

A model that is fine for protocol JSON may be poor for interactive edit-surface tool use, and vice versa.

### `baseline_eval_model` and `child_self_eval_model`

Eval runs need benchmark-turn compatibility:

- task prompt/tool constraints expected by MBE runner;
- route/provider availability under campaign concurrency;
- comparable budget semantics across baseline and treatment;
- clear inheritance/default policy if baseline and child share the same model.

If Joseph intentionally uses different baseline and child evaluators, reports should call that out because it changes how treatment-vs-baseline comparisons are interpreted.

## 7. Smallest safe implementation slice

### Slice 0: no-behavior-change role model resolver/read-side refactor

Goal: introduce role names and make current behavior explicit without changing which model is used.

Tasks:

1. Add a `PhaseModelRole` enum with serde kebab-case names:
   - `parent_patch_generation`
   - `protocol_adjudicator`
   - `baseline_eval_model`
   - `child_self_eval_model`
2. Add a role selection struct reusing the existing `ModelDefaults` shape, e.g. `PhaseModelDefaults { id, route_source, provider, inherits }`, and a resolved struct, e.g. `ResolvedPhaseModel { role, model_id, route_source, provider_slug, source }`.
3. Add a resolver that preserves current behavior:
   - `baseline_eval_model` resolves from existing command/profile/campaign eval path.
   - `child_self_eval_model` inherits baseline.
   - `protocol_adjudicator` resolves to the existing campaign model in Prototype1 closure paths.
   - `parent_patch_generation` resolves via existing `load_parent_patcher_model_selection()` fallback.
4. Persist/render the resolved role map without using it to change executor inputs yet.
5. Add backwards-compatible parsing tests for manifests/profiles with no role map.

Why this is safe:

- It makes existing implicit behavior visible first.
- It gives tests/read-side code a stable contract before enabling divergence.

### Slice 1: enable one real override path: `protocol_adjudicator`

Protocol is the cleanest first nontrivial override because `protocol_llm_config` and `execute_protocol_run_tasks` already accept concrete model/route/provider, and `StoredProtocolArtifact` already records model/provider.

Tasks:

1. Extend `ProtocolCampaignPolicy` or campaign role map to carry `protocol_adjudicator` model defaults/resolved selection.
2. Update `advance_protocol_closure` to pass the resolved `protocol_adjudicator` values instead of `config.model_id/config.route_source/config.provider_slug`.
3. Update `prepare_prototype1_loop_campaign` so existing `--protocol-model-id/--protocol-provider/--protocol-route-source` populate the protocol role instead of being rejected when they differ from eval.
4. Add `model_role` and `route_source` to stored protocol artifacts, while preserving existing `model_id/provider_slug` compatibility.
5. Tests: no network. Use fake model registry/provider prefs and invoke resolver/protocol planning paths with dry-run or fake protocol task records.

### Slice 2: move `parent_patch_generation` into profile/campaign policy

Tasks:

1. Replace default `load_parent_patcher_model_selection()` calls in `BroadTuiAttemptOptions::for_parent_patcher_defaults` / `from_cli(None, None, ...)` with role resolver input from admitted profile/campaign.
2. Preserve existing parent-patcher model file as fallback source.
3. Add desired/actual model role fields to `BroadHarnessRequest`, submitted result, and `BroadHarnessAttemptProjection`.
4. Add tests proving external request/result round trip includes the selected role/model route.

### Slice 3: split `baseline_eval_model` and `child_self_eval_model`

Tasks:

1. Keep baseline eval wired to the campaign base eval model or `baseline_eval_model` role.
2. Update `prepare_prototype1_treatment_campaign` so treatment campaign model/provider/route resolves from `child_self_eval_model`, defaulting to inherited baseline.
3. Add a comparison/report warning when child self-eval differs from baseline eval.
4. Add route propagation tests proving baseline and treatment campaigns freeze the intended concrete role values into prepared runs and run records.

## 8. Test plan

### Unit/config parsing tests

- `profile.rs`: parse existing profile with only `[model]`; assert all roles default/inherit as expected.
- `profile.rs`: parse `[model_roles.protocol_adjudicator]`, `[model_roles.parent_patch_generation]`, etc.; validate model id, provider slug, route source.
- `campaign.rs`: parse old campaign manifest with top-level `model_id/provider_slug/route_source`; assert role map is synthesized/read-compatible.
- `campaign.rs`: parse new manifest with explicit role map; assert resolved values and inheritance.

### CLI snapshot / manifest tests

- `Prototype1LoopCommand` with legacy `--model-id/--provider/--route-source` still writes the same top-level campaign model fields.
- Existing `--protocol-model-id/--protocol-provider/--protocol-route-source` writes protocol role selection rather than failing on divergence once Slice 1 is enabled.
- Profile-admitted run writes role map into the campaign manifest and an admitted profile record.

### Route propagation tests with fake provider / no network

- Build isolated fake model registry with OpenRouter and direct-Google entries.
- Build fake provider prefs for an OpenRouter model.
- Assert resolver returns correct provider/null-provider behavior for direct-Google and OpenRouter.
- Assert invalid direct-Google + non-google provider still fails.
- Assert parent-patcher fallback still loads active model when parent-patcher selection is absent.

### Eval/run record assertions

- Baseline eval prepared manifests include `model_role = baseline_eval_model` and concrete model/provider.
- Treatment campaign prepared by `prepare_prototype1_treatment_campaign` includes inherited or overridden `child_self_eval_model` values.
- `RunIntent::freeze()` preserves role plus model/provider.
- Run record metadata reports selected actual model/provider, not just requested values.

### Protocol artifact assertions

- Protocol artifacts store `model_role = protocol_adjudicator`, `model_id`, `provider_slug`, and `route_source` for each procedure artifact.
- Artifact detail/read-side rendering shows the phase role and actual route.
- Provider/JSON retry failure is recorded as execution failure, not as semantic adjudication.

### Broad harness request/result assertions

- Published broad harness request includes desired `parent_patch_generation` selection.
- Submitted result includes actual executor selection.
- Attempt projection includes role, model id, provider, route source, and route/execution/admission statuses separately.
- Admission binding mismatch tests continue to pass; model metadata must not weaken admission authority.

## 9. Open questions / decisions for Joseph

1. Role naming: are the proposed names acceptable, especially `child_self_eval_model` versus `treatment_eval_model`?
2. Default policy: should `child_self_eval_model` inherit `baseline_eval_model` by default, or should both inherit an umbrella `eval_model` role?
3. Fairness policy: if child self-eval differs from baseline eval, should selection/branch comparison require an explicit override flag or emit only a warning?
4. Protocol first slice: is enabling `protocol_adjudicator` divergence the desired first behavior change, or should parent patch generation be first because a parent-patcher split already exists?
5. Parent patch request schema: should the model selection be part of `BroadHarnessRequest`, or part of a separate executor assignment envelope so the request remains model-agnostic?
6. Capability registry: should capabilities live in model registry metadata, provider prefs, or a separate `phase_capabilities` table/file?
7. Direct route support: Prototype1 CLI/profile parsing currently handles OpenRouter and direct-Google. Should direct Anthropic/OpenAI be supported in this same slice if the underlying `ModelRouteSource` supports them elsewhere?
8. Observability scope: should `closure status` render all phase roles by default, or hide inherited/equal roles unless verbose?
9. Backwards compatibility: how long should top-level campaign `model_id/provider_slug/route_source` remain the canonical eval fields before role map becomes mandatory?

## Evidence index

Key inspected files/functions:

- `crates/ploke-eval/src/cli.rs::Prototype1LoopCommand` — loop-level model/provider/route and protocol-specific flags.
- `crates/ploke-eval/src/cli.rs::parse_model_route_source` — OpenRouter/direct-Google CLI route parsing.
- `crates/ploke-eval/src/cli.rs::headless_model_selection` and `load_parent_patcher_model_selection` — broad parent patch model selection and current temporary split comment.
- `crates/ploke-eval/src/cli.rs::protocol_llm_config`, `resolve_protocol_model_id`, `resolve_protocol_route` — protocol/adjudicator routing config.
- `crates/ploke-eval/src/cli.rs::advance_eval_closure` — eval closure passes campaign model/provider into batch execution.
- `crates/ploke-eval/src/cli.rs::advance_protocol_closure` — protocol closure currently passes campaign model/route/provider into protocol tasks.
- `crates/ploke-eval/src/cli/prototype1_state/profile.rs::Prototype1RunProfile`, `ModelDefaults`, `Protocol` — admitted profile has one model block and protocol execution settings, not role-specific models.
- `crates/ploke-eval/src/cli/prototype1_state/cli_facing.rs::Prototype1LoopControllerInput::from_command` — profile/campaign admission path.
- `crates/ploke-eval/src/cli/prototype1_state/cli_facing.rs::prepare_prototype1_loop_campaign` — current single eval model resolution and protocol/eval equality gate.
- `crates/ploke-eval/src/cli/prototype1_state/cli_facing.rs::prepare_prototype1_treatment_campaign` — child treatment campaign copies baseline model/provider/route.
- `crates/ploke-eval/src/cli/prototype1_state/cli_facing.rs::BroadTuiAttemptOptions` and `BroadHarnessAttemptProjection` — broad TUI model option and attempt projection.
- `crates/ploke-eval/src/cli/prototype1_state/edit_surface/tui_adapter.rs::ModelSelection` and `run_headless_with_model` — concrete TUI model/provider execution hook.
- `crates/ploke-eval/src/cli/prototype1_state/edit_surface/harness_request.rs::BroadHarnessRequest` — broad request schema currently lacks model route fields.
- `crates/ploke-eval/src/cli/prototype1_state/edit_surface/harness_result.rs` — submitted/admission result schema currently focuses on request binding/evidence/executor ids, not model route.
- `crates/ploke-eval/src/cli/prototype1_process.rs::run_prototype1_resolved_branch_treatment` — child self-eval/treatment closure flow.
- `crates/ploke-eval/src/campaign.rs::CampaignManifest`, `CampaignOverrides`, `ResolvedCampaignConfig`, `ProtocolCampaignPolicy` — campaign persistence and resolution.
- `crates/ploke-eval/src/model_registry.rs::load_active_model`, `load_parent_patcher_model`, `resolve_model_for_run` — registry/default sources.
- `crates/ploke-eval/src/provider_prefs.rs::ProviderPrefs`, `load_provider_for_model` — provider preference source.
- `crates/ploke-eval/src/spec.rs::PreparedCampaignContext` — prepared run campaign context currently carries model/provider.
- `crates/ploke-eval/src/runner.rs` and `crates/ploke-eval/src/inner/core.rs::RunIntent/FrozenRunSpec` — concrete eval model/provider freeze and run metadata.
- `crates/ploke-eval/src/protocol_artifacts.rs::StoredProtocolArtifact` — protocol artifacts already record model/provider.
