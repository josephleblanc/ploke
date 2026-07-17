# Prototype 1 Run Profile

The Prototype 1 run profile is the admitted loop configuration, usually stored
as `run-profile.toml` under a campaign. It owns target selection, child fanout,
candidate generation, successor selection, execution stops, and local parent
control. Control lives in the same file under `[control]`; Prototype 1 no
longer reads a separate `run-control.toml`.

## Validation Surfaces

`prototype1-setup --preview` builds a non-mutating, versioned configuration
plan. It requires an explicit profile and an already-prepared `--batch` or
`--batch-id`, then resolves the campaign manifest, exact selected slice,
normalized profile, model/provider routes, storage policy, search policy, and
effective concurrency. The JSON/table output includes SHA-256 commitments for
the full plan and each exact persisted payload.

Preview is not complete admission or live-provider readiness validation. It
does not create or validate the closure, owner database rows, scheduler/root
node, Git branch, parent identity, identity commit, or final checkout. Those
checks remain deferred until admission; provider requests remain explicit
`prototype1-doctor` preflights after admission. Preview writes no campaign,
closure, run, scheduler, Git, parent-identity, or monitor evidence.

Validation happens at the command boundaries that already read the config:

- `./target/debug/ploke-eval loop prototype1-setup --preview --batch "${P1_BATCH:?set P1_BATCH to a prepared batch manifest}" --campaign "${P1_CAMPAIGN:?set P1_CAMPAIGN}" --profile "${P1_PROFILE:?set P1_PROFILE to a profile name or TOML path}" --format json`
  reads, parses, validates, normalizes, and digests the operator profile and
  resolves the configuration plan without admission writes. Inline dataset
  selection is intentionally rejected because preparing a batch can create
  directories or download data.
- To bind a later command to reviewed inputs, run the same setup command without
  `--preview` and add
  `--expect-plan-sha256 <plan_sha256-from-preview>`. Setup replans from the
  current batch, dataset slice, profile, registry, preferences, and environment,
  then fails before its first admission write if any committed input changed.
  Setup without `--expect-plan-sha256` remains available for one-shot operation
  and inline batch preparation.
- `./target/debug/ploke-eval loop prototype1-doctor --repo-root "${P1_PARENT_ROOT:?set P1_PARENT_ROOT to the active parent checkout}"`
  reads the active parent identity, loads the admitted `run-profile.toml`,
  verifies its digest, and reports the effective `[control]` state. This is the
  safest existing post-admission check for profile/control conflicts.
- `prototype1-continue`, `prototype1-step`, and `prototype1-state` also fail
  while loading invalid admitted config, but they are execution commands rather
  than parse-only validators.

Setup and the control commands require an explicit/admitted run profile. An
admitted profile is one fail-closed pair:

- `prototype1/run-profile.toml` contains the exact normalized profile bytes.
- `prototype1/run-profile.commitment.json` is the final admission marker. It
  must use the current commitment schema, name the exact campaign-local profile
  path, and match the profile's SHA-256.

If both files are absent, the campaign has no admitted profile. If exactly one
exists, or schema/path/digest validation fails, the state is a partial or
corrupt admission and the loader stops. It never synthesizes or repairs a
commitment while reading. Start a fresh campaign or use an explicit repair
workflow rather than treating a profile-only legacy artifact as admitted.

Setup configuration authority is explicit in preview output. The prepared
batch owns its cohort and eval budget; `--instance` can select only the primary
Parent(0) identity from that cohort. The run profile owns search, generation,
selection, execution, storage, and control. Legacy `prototype1` search and
`--stop-after` flags are rejected as setup overrides instead of being silently
ignored.

### Current admission-recovery boundary

Plan comparison fails before admission writes, but the matched setup operation
is not yet a filesystem/DB/Git transaction. After the campaign/profile pair is
written, a later closure, owner-DB, node, branch, identity, commit, or checkout
failure can leave a partial campaign, and the ordinary setup command does not
yet carry a resumable admission receipt. Preserve that evidence and use a fresh
campaign rather than deleting artifacts or weakening validation. This is the
remaining Stage 1 admission-recovery work and is a hard gate before enabling UI
Start/Retry controls; a successful setup report still means every listed stage
completed.

## `control`

The admitted `run-profile.toml` owns both policy and local control. The
`[control]` section may narrow execution, but it must not widen the admitted
search policy in the same file.

```toml
[control]
mode = "continuous"
parallel_cap = 3
```

- `mode`: Accepted values are `continuous` and `step`. The explicit
  `prototype1-continue` and `prototype1-step` commands still choose the command
  execution mode; this field is retained in the admitted config as the typed
  operator preference.
- `parallel_cap`: Optional cap on concurrent child phase execution. If omitted,
  it defaults from `[search]`. For `search.schedule = "full-batch"`, the
  derived cap is the effective `search.children.parallel_targets` (explicit or
  `min(3, children.max)`). For `adaptive-batch`, it is the smaller of
  `search.children.min` and that effective patch-generation cap.

`control.parallel_cap` does not conflict when it is nonzero and less than or
equal to the derived cap. For example, a full-batch profile with
`children = { min = 1, max = 3 }` accepts `parallel_cap = 1`, `2`, or `3`.

`control.parallel_cap` conflicts when it is `0` or greater than the derived cap.
For example, a full-batch profile with `children = { min = 1, max = 3 }`
rejects `parallel_cap = 4` because the control section would widen the admitted
child fanout.

Legacy `prototype1/run-control.toml` files are ignored by the current control
path. Move any still-needed `parallel_cap` into `[control]` in
`prototype1/run-profile.toml`.

## Parent Patch Generation

`[search.children]` owns both the child admission budget and the parent-side
broad-harness patch-generation concurrency used to try to fill that budget.

```toml
[search.children]
min = 2
max = 3
parallel_targets = 3
```

- `parallel_targets`: Optional cap on concurrent parent-side broad-harness
  patch-generation slots. If omitted, it defaults to `min(3, max)`. This
  controls provisional edit-harness workspaces and model patch attempts, not
  child self-evaluation.

Parent patch generation may attempt more total slots than `max` over time, but
it must not run more slot attempts concurrently than the maximum number of
child candidates that can be admitted.

The per-slot headless TUI retry budget belongs under `[execution.broad_tui]`:

```toml
[execution.broad_tui]
max_attempts = 2
fresh_slots_per_child = 2
timeout_secs = 900
```

- `max_attempts`: Optional maximum attempts inside each headless TUI patch
  generation slot. If omitted, the harness request contract supplies the
  attempt budget.
- `fresh_slots_per_child`: Optional count of fresh broad-harness slots to
  publish per desired child. If omitted, the current runtime default is used.
- `timeout_secs`: Optional per-slot headless TUI turn timeout. If omitted, the
  harness request contract supplies the timeout budget.

## Profile Conflicts

The profile rejects internally conflicting settings:

- `target.instance` must be nonempty and, when `target.instances` is also set,
  must be included in `target.instances`.
- `target.instances` must not contain empty or duplicate ids.
- `model.id`, when present, must be a valid model id.
- `model.provider`, when present, must be a valid provider slug.
- `model.route_source = "direct-google"` only accepts `model.provider =
  "google"` or no provider. OpenRouter provider pins such as
  `google-ai-studio` are valid only with `model.route_source = "openrouter"`.
  Warning: this is a profile-level sentinel, not a campaign provider pin. After
  setup, direct Google campaigns serialize `provider_slug` as absent/null and
  preserve the route with `route_source = "direct_google"`.
- `search.children.min` and `search.children.max` must be nonzero, and `min`
  must not exceed `max`.
- `search.children.parallel_targets`, when present, must be nonzero and no
  greater than `search.children.max`.
- `generation.source = "legacy"` currently requires exactly one target
  instance.
- `execution.mbe.enabled = true` requires at least one configured target
  instance, a nonempty `execution.mbe.python`, and nonzero
  `execution.mbe.workers`.
- `execution.observe_child_stale_after_secs` must be nonzero.
- `execution.broad_tui.max_attempts`, `fresh_slots_per_child`, and
  `timeout_secs`, when present, must be nonzero.
- `selection.oracle.mode = "relative-score"` with
  `selection.oracle.require_evidence = true` requires MBE to be enabled and at
  least one configured target instance.
- `selection.oracle.gate = "all-resolved"` requires
  `selection.oracle.require_evidence = true`, MBE to be enabled, and at least
  one configured target instance.
- `selection.metrics.imp_at_k.enabled = true` requires
  `selection.metrics.imp_at_k.budget_k` to be nonzero.
- `selection.metrics.persist = false` conflicts with metric-driven scoring:
  `score_points_per_imp_point != 0` or `require_for_score = true`.
- `protocol.max_tokens` must be nonzero.
- `protocol.reasoning.mode = "effort"` requires
  `protocol.reasoning.effort`; `effort` is invalid for `omit` or `disabled`.

## Top Level

```toml
schema_version = "prototype1-run-profile.v1"
name = "smoke-broad-harness-1x3"
```

- `schema_version`: Required schema marker. Current value is
  `prototype1-run-profile.v1`.
- `name`: Operator-facing profile name. It is descriptive; campaign identity is
  still assigned by setup/admission.

## `storage`

```toml
[storage]
worktree_root = "~/.ploke-eval/worktrees"
```

- `worktree_root`: Root directory for managed Prototype 1 worktrees.

## `target`

```toml
[target]
dataset_key = "multi-swe-bench-rust"
instance = "BurntSushi__ripgrep-2209"
instances = ["BurntSushi__ripgrep-2209"]
```

- `dataset_key`: Optional registry dataset selector.
- `instance`: Primary benchmark instance id. If `instances` is absent, this is
  the evaluated target set.
- `instances`: Explicit target set. If both `instance` and `instances` are set,
  `instance` must be included in `instances`.

## `model`

```toml
[model]
id = "google/gemini-3.5-flash"
route_source = "direct-google"
provider = "google"
```

- `id`: Optional default model id for Prototype 1 setup. This is the eval
  model default. Protocol uses it only when neither `[protocol.model]` nor
  `--protocol-*` setup flags supply a protocol-specific override.
- `route_source`: Optional default router for ambiguous model ids. Accepted
  values are `direct-google` and `openrouter`. This disambiguates ids such as
  `google/gemini-3.5-flash`, which can be valid through both routers.
- `provider`: Optional provider slug. For `route_source = "direct-google"`,
  `provider = "google"` is accepted as the direct Google sentinel and resolves
  to no OpenRouter provider pin in the campaign manifest. For
  `route_source = "openrouter"`, this is an OpenRouter provider pin.

Warning: the direct Google serialization rule is intentionally asymmetric in
the current implementation. The admitted profile may say
`provider = "google"`, but `campaign.json` should then say
`"provider_slug": null` or omit the field. The campaign's route authority is
`"route_source": "direct_google"`. Do not "repair" a direct Google campaign by
adding `"provider_slug": "google"`; that value is only accepted as a setup-time
sentinel and is normalized away before campaign admission. OpenRouter provider
slugs belong only with `route_source = "openrouter"`.

CLI flags keep normal precedence over this section. `--model-id`,
`--route-source`, and `--provider` override the eval defaults. If no
protocol-specific model is configured, setup uses the resolved eval tuple for
protocol as well.

## `search`

```toml
[search]
max_generations = 2
max_total_nodes = 13
children = { min = 1, max = 3, parallel_targets = 3 }
schedule = "full-batch"
stop_on_first_keep = false
require_keep_for_continuation = false
explore_from_rejected = true
```

- `max_generations`: Maximum parent generations before the loop stops.
- `max_total_nodes`: Maximum total Prototype 1 nodes admitted for the campaign.
- `children.min`: Minimum children to reserve for a complete generation.
- `children.max`: Maximum children to reserve for a complete generation.
- `children.parallel_targets`: Optional parent-side patch-generation
  concurrency cap. If omitted, it defaults to `min(3, children.max)`.
- `schedule`: Child execution schedule. `full-batch` plans/runs the reserved
  cohort together; `adaptive-batch` can run narrower batches.
- `stop_on_first_keep`: Stop the generation as soon as a kept child exists.
- `require_keep_for_continuation`: Continue only from a kept child when true.
- `explore_from_rejected`: Allow rejected children to become exploration
  successors when no accepted keep is available.

## `generation`

```toml
[generation]
source = "broad-harness-request"
```

- `source`: Candidate generator. `legacy` is the old single-target path;
  `broad-harness-request` uses the published broad harness request/plan surface;
  `deterministic-tui-tools` uses the deterministic TUI tools fixture path.

## `selection`

```toml
[selection]
strategy = "history-score-child-prop"
evidence = "operational-and-protocol"
seed = 0
```

- `strategy`: Successor traversal strategy. `generation-local` considers only
  current-generation candidates; `history-frontier-max` traverses admitted
  history and picks the maximum frontier score; `history-score-child-prop`
  samples admitted history with child-count exploration pressure.
- `evidence`: Non-oracle scoring inputs. `operational` uses run metrics only;
  `operational-and-protocol` also includes protocol aggregate deltas.
- `seed`: Deterministic sampling seed for stochastic traversal strategies.

## `selection.metrics`

```toml
[selection.metrics]
persist = true
score_profile = "operational-quality-v1"

[selection.metrics.imp_at_k]
enabled = true
budget_k = 50
archive_scope = "selection-scope"
score_points_per_imp_point = 0
require_for_score = false
```

- `persist`: When true, successor selection seals metric evidence into
  `SelectionDecisionEntry` as part of the admitted History payload. Persisted
  metric sets are downstream Graph/UI evidence, not console-only logging.
- `score_profile`: Scoring profile used by the metric calculator. Current value
  is `operational-quality-v1`.
- `imp_at_k.enabled`: Enables selection-time `imp@k` evidence. The metric is
  computed after History candidates and current-generation candidates are merged
  and before the traversal strategy chooses a successor.
- `imp_at_k.budget_k`: Candidate budget used for the `imp@k` row. It must be
  nonzero when `imp_at_k.enabled = true`.
- `imp_at_k.archive_scope`: Archive boundary for the metric. Current value is
  `selection-scope`, meaning the archive visible at the selection point.
- `imp_at_k.score_points_per_imp_point`: Additive traversal-score multiplier.
  Set this to `0` to record metric evidence without letting `imp@k` influence
  successor selection.
- `imp_at_k.require_for_score`: When false, incomplete `imp@k` rows contribute
  zero and persist their incomplete reason. When true, candidates with
  incomplete `imp@k` are excluded from decision-grade metric scoring.

The normal validation profile for new metric plumbing should keep
`score_points_per_imp_point = 0` and `require_for_score = false`. That preserves
the existing operational/protocol successor behavior while producing persisted
metric fixtures for later `ploke-tree` and `ploke-egui` work.

## `selection.oracle`

```toml
[selection.oracle]
mode = "record-only"
require_evidence = true
gate = "disabled"
```

- `mode`: Oracle selection policy. `record-only` records oracle evidence when
  MBE runs but does not use it for successor scoring. `relative-score` uses the
  candidate resolved-rate as a relative ranking signal inside the selected
  traversal pool.
- `require_evidence`: Missing-evidence policy for `relative-score`. When true,
  selection errors if a configured target instance lacks oracle evidence. When
  false, missing oracle evidence does not block selection; it contributes no
  resolved point, so the score is still `resolved / configured`. Duplicate,
  mismatched, or unknown oracle evidence remains invalid in both modes.
- `gate`: Successor-admission policy, independent of ranking. `disabled`
  preserves the existing mode behavior. `all-resolved` requires both sealed
  evidence and selection-input evidence to cover exactly the admitted profile
  targets, requires the two carriers to agree, and excludes any candidate whose
  oracle verdicts are not all `resolved`. Missing, duplicate, unknown,
  mismatched, or internally inconsistent evidence fails closed.

`selection.oracle` is deliberately separate from `selection.evidence`.
Operational/protocol metrics determine the candidate disposition and ranking
inputs. Oracle `mode` controls relative traversal scoring, while oracle `gate`
separately controls whether a candidate may enter that ranking pool.

## `protocol`

```toml
[protocol]
# Keep at or above 4096 unless the live protocol preflight and direct-Google
# malformed-call repro both prove a lower budget is safe for the chosen route.
max_tokens = 4096
tool_review_parallelism = 8

[protocol.model]
id = "google/gemini-2.5-flash-lite"
route_source = "direct-google"
provider = "google"

[protocol.reasoning]
mode = "omit"
# effort = "low"
```

- `model.id`: Optional protocol-only model id. This lets the loop use a
  smaller or cheaper model for protocol JSON adjudication while leaving eval and
  parent patch generation on their own model routes.
- `model.route_source`: Optional protocol router. Accepted values are
  `direct-google` and `openrouter`. Direct Google is used for the Google API
  endpoint directly; OpenRouter remains supported.
- `model.provider`: Optional protocol provider setting. For
  `model.route_source = "direct-google"`, use `provider = "google"` or omit the
  provider. OpenRouter provider pins are valid with
  `model.route_source = "openrouter"`.
- `max_tokens`: Completion token budget for Prototype 1 protocol adjudication
  requests admitted from this profile. This applies to the campaign-driven
  baseline protocol path, including tool-call intent segmentation, tool-call
  review, and segment review. The default is `4096`; set it higher when a
  provider spends part of the completion budget on hidden or reported reasoning
  tokens before emitting JSON. Run `prototype1-doctor
  --live-protocol-preflight` before a full live loop; the doctor now includes a
  protocol-shaped budget canary and blocks budgets below the 4096 safe floor.
- `tool_review_parallelism`: Maximum number of tool-call review adjudications
  that may be in flight at once within a single protocol run. This is distinct
  from campaign-level run concurrency. Lower it, for example to `1` or `2`, for
  providers that return quota/rate-limit errors during the review fanout. The
  default is `8`.
- `reasoning.mode`: Request-body policy for protocol adjudication. `omit`
  sends no reasoning control, `effort` sends the configured effort, and
  `disabled` sends `reasoning.effort = "none"`. The default is `omit`, so the
  protocol layer does not silently disable reasoning for every provider.
- `reasoning.effort`: Required only when `reasoning.mode = "effort"`. Accepted
  values follow the shared LLM enum, such as `low`, `medium`, `high`, or
  `xhigh`.

For a live route/request-shape check before spending a full loop advance, run:

```bash
P1_PARENT_ROOT="${P1_PARENT_ROOT:?set P1_PARENT_ROOT to the active parent checkout}"
./target/debug/ploke-eval loop prototype1-doctor \
  --repo-root "$P1_PARENT_ROOT" \
  --live-protocol-preflight
```

That check sends a tiny JSON prompt through the admitted protocol
model/provider/reasoning tuple and reports a bounded error class without
printing credentials.

## `execution`

```toml
[execution]
stop_after = "complete"
observe_child_stale_after_secs = 1200
trace_jsonl = "auto"
debug_tools = false
mbe = { enabled = true, python = "python3", workers = 2 }
```

- `stop_after`: Parent execution stop. `materialize` stops after child workspace
  materialization; `build` stops after child binary build; `spawn` stops after
  spawning the child; `complete` runs evaluation, selection, and handoff.
- `observe_child_stale_after_secs`: Maximum time the parent waits in
  `observe_child` for child result evidence before treating the child as stale
  or hung. Defaults to `1200` seconds. Must be nonzero.
- `trace_jsonl`: Trace recording behavior. `inherit` follows the command or
  environment default; `auto` enables the standard trace artifact; `off`
  disables trace JSONL.
- `debug_tools`: Enables extra execution debug logging.
- `mbe.enabled`: Run Multi-SWE-bench Evaluation as the oracle recording pass.
  When enabled, `target.instance` or `target.instances` must be nonempty and the
  final MBE report must cover exactly those configured target instances. It is
  required when `selection.oracle.gate = "all-resolved"`.
- `mbe.python`: Python executable used to run the MBE harness.
- `mbe.workers`: Worker count applied to the MBE harness worker pools. Must be
  nonzero when `mbe.enabled = true`.
