# Prototype 1 Run Profile

The Prototype 1 run profile is the admitted loop configuration, usually stored
as `run-profile.toml` under a campaign. It owns target selection, child fanout,
candidate generation, successor selection, execution stops, and local parent
control. Control lives in the same file under `[control]`; Prototype 1 no
longer reads a separate `run-control.toml`.

## Validation Surfaces

There is currently no standalone `ploke-eval loop ... validate-config` command
that validates an arbitrary Prototype 1 config file without otherwise operating
on a campaign.

Validation happens at the command boundaries that already read the config:

- `ploke-eval loop prototype1-setup --profile <NAME_OR_PATH>` reads, parses,
  validates, admits, and digests the operator profile into the campaign as
  `prototype1/run-profile.toml`. This is the pre-admission validation surface
  for a profile file.
- `ploke-eval loop prototype1-doctor --repo-root <PARENT>` reads the active
  parent identity, loads the admitted `run-profile.toml`, verifies its digest,
  and reports the effective `[control]` state. This is the safest existing
  post-admission check for profile/control conflicts.
- `prototype1-continue`, `prototype1-step`, and `prototype1-state` also fail
  while loading invalid admitted config, but they are execution commands rather
  than parse-only validators.

The control commands require an admitted run profile. A campaign created without
`--profile` may still have legacy loop artifacts, but the current
doctor/continue/step path expects `prototype1/run-profile.toml` and its
commitment to exist.

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
  derived cap is `search.children.max`; for `adaptive-batch`, it is
  `search.children.min`.

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

## Profile Conflicts

The profile rejects internally conflicting settings:

- `target.instance` must be nonempty and, when `target.instances` is also set,
  must be included in `target.instances`.
- `target.instances` must not contain empty or duplicate ids.
- `search.children.min` and `search.children.max` must be nonzero, and `min`
  must not exceed `max`.
- `generation.source = "legacy"` currently requires exactly one target
  instance.
- `execution.mbe.enabled = true` requires at least one configured target
  instance, a nonempty `execution.mbe.python`, and nonzero
  `execution.mbe.workers`.
- `selection.oracle.mode = "relative-score"` with
  `selection.oracle.require_evidence = true` requires MBE to be enabled and at
  least one configured target instance.
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

## `search`

```toml
[search]
max_generations = 2
max_total_nodes = 13
children = { min = 1, max = 3 }
schedule = "full-batch"
stop_on_first_keep = false
require_keep_for_continuation = false
explore_from_rejected = true
```

- `max_generations`: Maximum parent generations before the loop stops.
- `max_total_nodes`: Maximum total Prototype 1 nodes admitted for the campaign.
- `children.min`: Minimum children to reserve for a complete generation.
- `children.max`: Maximum children to reserve for a complete generation.
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

`selection.oracle` is deliberately separate from `selection.evidence`.
Operational/protocol metrics decide the hard successor outcome; oracle policy
only controls whether recorded MBE results are used as a relative traversal
signal.

## `protocol`

```toml
[protocol]
max_tokens = 4000

[protocol.reasoning]
mode = "omit"
# effort = "low"
```

- `max_tokens`: Completion token budget for Prototype 1 protocol adjudication
  requests admitted from this profile. This applies to the campaign-driven
  baseline protocol path, including tool-call intent segmentation, tool-call
  review, and segment review. The default is `4000`; set it higher when a
  provider spends part of the completion budget on hidden or reported reasoning
  tokens before emitting JSON.
- `reasoning.mode`: Request-body policy for protocol adjudication. `omit`
  sends no reasoning control, `effort` sends the configured effort, and
  `disabled` sends `reasoning.effort = "none"`. The default is `omit`, so the
  protocol layer does not silently disable reasoning for every provider.
- `reasoning.effort`: Required only when `reasoning.mode = "effort"`. Accepted
  values follow the shared LLM enum, such as `low`, `medium`, `high`, or
  `xhigh`.

For a live route/request-shape check before spending a full loop advance, run:

```bash
./target/debug/ploke-eval loop prototype1-doctor --repo-root <parent> --live-protocol-preflight
```

That check sends a tiny JSON prompt through the admitted protocol
model/provider/reasoning tuple and reports a bounded error class without
printing credentials.

## `execution`

```toml
[execution]
stop_after = "complete"
trace_jsonl = "auto"
debug_tools = false
mbe = { enabled = true, python = "python3", workers = 2 }
```

- `stop_after`: Parent execution stop. `materialize` stops after child workspace
  materialization; `build` stops after child binary build; `spawn` stops after
  spawning the child; `complete` runs evaluation, selection, and handoff.
- `trace_jsonl`: Trace recording behavior. `inherit` follows the command or
  environment default; `auto` enables the standard trace artifact; `off`
  disables trace JSONL.
- `debug_tools`: Enables extra execution debug logging.
- `mbe.enabled`: Run Multi-SWE-bench Evaluation as the oracle recording pass.
  When enabled, `target.instance` or `target.instances` must be nonempty and the
  final MBE report must cover exactly those configured target instances.
- `mbe.python`: Python executable used to run the MBE harness.
- `mbe.workers`: Worker count applied to the MBE harness worker pools. Must be
  nonzero when `mbe.enabled = true`.
