# Eval Closure Formal Sketch

- date: 2026-04-16
- task title: Eval Closure Formal Sketch
- task description: Compact formal planning sketch for registry, eval-artifact, and protocol-artifact closure over the Rust Multi-SWE-bench slice, with a minimal feedback loop and suggested persisted state surfaces.
- related planning files:
  - [CURRENT_FOCUS.md](../CURRENT_FOCUS.md)
  - [2026-04-15_clap-baseline-eval-orchestration.md](2026-04-15_clap-baseline-eval-orchestration.md)
  - [2026-04-15_protocol-aggregate-cli.md](2026-04-15_protocol-aggregate-cli.md)

## Purpose

Use a compact formal sketch as the durable planning surface instead of a long prose handoff when the real object is a staged closure problem with explicit state transitions.

## Current Implementation Slice

The first faithful implementation slice has landed in `ploke-eval` as:

- `ploke-eval registry recompute`
- `ploke-eval registry status`
- `ploke-eval closure recompute`
- `ploke-eval closure status`

What is implemented:

- reduced `target-registry.json` semantics under `~/.ploke-eval/registries/multi-swe-bench-rust.json`
- a typed local representation of `T` for the materialized Rust Multi-SWE-bench slice
- reduced `closure-state.json` snapshots under `~/.ploke-eval/campaigns/<campaign>/`
- closure derivation from the persisted registry `T`, plus run-local artifacts, batch summaries, and protocol artifacts
- human-facing compact status projection over the reduced state

What is still deferred:

- `closure-events.jsonl`
- live semantic event emission from producer commands
- deeper `closure inspect` views beyond the current compact status surface

## Core Objects

Let:

- `E_r`
  - the Rust target-instance universe drawn from the local `multi-swe-bench` benchmark checkout
- `T`
  - the local benchmark-mapping surface in `ploke-eval`
- `C`
  - the eval configuration space
- `A`
  - persisted eval artifact sets
- `X = {x_1, ..., x_n}`
  - the required protocol procedures exposed by `ploke-eval protocol ...`
- `M_i`
  - persisted outputs/artifacts for procedure `x_i`

## Current Code Fit

The important current-code distinction is:

- the detailed per-run and per-procedure persisted artifacts already exist
- the missing layer is campaign-level closure accounting over those artifacts

Current concrete carriers in `ploke-eval`:

- benchmark-instance preparation and dataset loading:
  - `msb.rs`
- builtin dataset mapping:
  - `registry.rs`
- per-run artifact persistence:
  - `runner.rs`
  - `record.rs`
- protocol artifact persistence:
  - `protocol_artifacts.rs`
- protocol aggregate reduction:
  - `protocol_aggregate.rs`
- human-facing inspection:
  - `cli.rs`

Operationally, this means:

- `closure-state.json` should not duplicate `record.json.gz`
- `closure-state.json` should not duplicate persisted protocol outputs
- it should instead point at and classify the already-persisted artifacts

### Current reality of `T`

`T` is now better specified as:

- the explicit persisted local target registry for the materialized Rust benchmark slice
- written under `~/.ploke-eval/registries/multi-swe-bench-rust.json`
- populated from the local dataset JSONL files, but distinct from them in role

This is an upward correction of the earlier looser definition.

Concretely:

- dataset JSONLs remain the materialized benchmark-universe source artifacts
- `T` is the canonical local typed registry derived from that universe
- per-run `run.json` manifests are no longer the right semantic carrier for `T`

The remaining gap is no longer “`T` is missing” or “closure reconstructs `T` on demand”.

The new state is:

- `T` is explicit and persisted
- closure consumes `T` directly
- the remaining work is downstream:
  - event emission
  - deeper inspection surfaces
  - completion of eval and protocol closure over the full local slice

## Layered Closure

### 1. Registry Closure

There exists a representation map:

`rho : E_r -> T?`

Desired property:

`forall e in E_r_admissible, exists t in T such that rho(e) = t`

Operationally:

- benchmark Rust instances should have explicit local registry representation
- absence should be classified, not silently tolerated

### 2. Eval Closure

For a chosen config `c in C`, there exists:

`Eval_c : T -> A?`

Desired property:

`forall t in T_admissible, exists a in A or exists f_eval(t)`

where:

- `a`
  - a completed persisted eval artifact set
- `f_eval(t)`
  - an explicit eval failure state for `t`

Operationally:

- every admissible registry element should have either:
  - completed eval artifacts
  - or an explicit failed-eval classification

### 3. Protocol Closure

For each required procedure `x_i in X`, there exists:

`x_i : A -> M_i?`

Desired property:

`forall a in A_complete, forall x_i in X_required, exists m_i in M_i or exists f_proto(a, x_i)`

where:

- `m_i`
  - persisted protocol output/artifact for procedure `x_i`
- `f_proto(a, x_i)`
  - an explicit protocol failure or incompatibility state

Operationally:

- every completed eval artifact set should have full required protocol follow-through
- absence, failure, and incompatibility must remain distinct states

## State Classes

Every expected element at each layer should classify into one of:

- `complete`
- `failed`
- `missing`
- `ineligible`
- `incompatible`
- `partial`

Notes:

- `failed` means an attempted producer returned an explicit failure state
- `missing` means no realized artifact/state is present yet
- `incompatible` means an artifact exists but is not admissible under the current reference frame
- `partial` means some but not all required downstream products exist

## Minimal Feedback Loop

For a chosen layer `L` with producer `P_L`:

1. Enumerate expected elements at `L`.
2. Compare expected versus realized.
3. Classify each gap.
4. Run the producer for the missing admissible set.
5. Recompute coverage.
6. Repeat at the next layer.

In compact notation:

`enumerate -> compare -> classify -> produce -> recompute -> advance`

## Concise State Surfaces

The smallest useful durable state surfaces are:

### `registry-coverage`

Tracks closure of `E_r -> T`.

Suggested fields:

- `benchmark_family`
- `expected_total`
- `registered_total`
- `missing_total`
- `ambiguous_total`
- `status_by_instance`

### `eval-coverage`

Tracks closure of `T -> A`.

Suggested fields:

- `config_key`
- `expected_total`
- `complete_total`
- `failed_total`
- `missing_total`
- `partial_total`
- `status_by_target`

### `protocol-coverage`

Tracks closure of `A -> M`.

Suggested fields:

- `config_key`
- `required_procedures`
- `expected_total`
- `full_total`
- `partial_total`
- `failed_total`
- `missing_total`
- `incompatible_total`
- `status_by_run`
- `status_by_procedure`

## Existing Source Artifacts Versus New Closure State

The current source-of-truth artifacts already on disk include:

- run-local preparation and execution artifacts:
  - `run.json`
  - `execution-log.json`
  - `repo-state.json`
  - `indexing-status.json`
  - `snapshot-status.json`
  - `record.json.gz`
  - agent-mode extras such as:
    - `agent-turn-trace.json`
    - `agent-turn-summary.json`
    - `llm-full-responses.jsonl`
- batch-level artifacts:
  - `batch-run-summary.json`
  - `multi-swe-bench-submission.jsonl`
- protocol-local artifacts:
  - `protocol-artifacts/*.json`

The proposed new surfaces are different in kind:

- `closure-state.json`
  - reduced current campaign state
- `closure-events.jsonl`
  - sparse semantic transition history

These should be derived from and refer to the source artifacts above, not replace them.

## Recommended Persistence Shape

Use two layers, not one:

### 1. Canonical snapshot

One bounded machine-readable state file per active campaign.

Good fit:

- `json`

Why:

- deterministic current state
- cheap to read/update
- easy for CLI inspection
- good restart surface
- can classify partial/failure/missing state without rescanning every view

### 2. Append-only event log

One append-only event stream for transitions that produced the current snapshot.

Good fit:

- `jsonl`

Why:

- preserves history
- supports replay/debugging
- lets worker/sub-agent actions emit semantic transitions without rewriting the whole state model each time

Important constraint:

- the event stream should stay sparse
- it should not become a tracing-style dump of every low-level step
- low-level execution detail already belongs in run-local artifacts

## Recommended Division Of Labor

Use:

- `json`
  - for the canonical closure state
- `jsonl`
  - for transition/event tracing
- `cli`
  - for human-facing projection of the current state

In other words:

- do not make the CLI the source of truth
- do not make raw `jsonl` the only readable surface
- derive CLI views from the canonical state

Recommended placement:

- source artifacts remain under the current run/batch layout in `~/.ploke-eval`
- campaign closure state can live under a campaign-scoped directory such as:
  - `~/.ploke-eval/campaigns/<campaign-id>/closure-state.json`
  - `~/.ploke-eval/campaigns/<campaign-id>/closure-events.jsonl`

## Sketch Of The Runtime Interface

Conceptually:

`track_layer(Enumerator, Realizer, Classifier, Producer) -> { reducer, event_writer }`

or more concretely:

`track_eval_closure(registry, config) -> closure_state`

where worker activity appends events such as:

- `campaign.started`
- `registry.enumerated`
- `registry.instance.mapped`
- `eval.run.started`
- `eval.run.completed`
- `eval.run.failed`
- `protocol.intent_segmentation.completed`
- `protocol.tool_call_review.completed`
- `protocol.tool_call_segment_review.completed`
- `protocol.run.partial`
- `protocol.run.complete`

Avoid event shapes such as:

- low-level checkout steps
- snapshot writes
- internal artifact-copy steps
- detailed tracing noise already preserved elsewhere

## Current Live Instantiation

At time of writing, the active concrete slice is approximately:

- `E_r`
  - Rust Multi-SWE-bench target instances on disk
- `T`
  - partial local mapping surface, currently spread across:
    - builtin dataset entries
    - local dataset files
    - prepared manifests and run directories
- `Eval_c`
  - baseline run under `x-ai/grok-4-fast` on `xai`
- `X`
  - the three currently required `ploke-eval protocol` procedures:
    - `tool-call-intent-segments`
    - `tool-call-review`
    - `tool-call-segment-review`

Current known frontier:

- baseline eval closure is nearly complete for the Rust slice
- protocol closure is partial and stopped at the first `clap` frontier

Concrete operational note:

- the current code already persists the detailed run and protocol artifacts needed for closure
- the missing operational layer is explicit campaign reduction plus progress classification

## Minimal Human-Facing Status Projection

The CLI surface over closure state should be concise and suitable both for:

- human inspection
- orchestrator/sub-agent monitoring

Desired style:

`<layer> <campaign-or-target> [<status>] | progress <done>/<total> | success <x>/<total> | fail <y>/<total> | partial <z>/<total>`

Examples:

- `eval rust-baseline [active] | progress 130/132 | success 130/132 | fail 2/132`
- `protocol rust-baseline [active] | progress 40/130 | full 39/130 | partial 1/130 | missing 90/130`

This view should be derived from `closure-state.json`, not by ad hoc filesystem scans every time.

## Proposed `closure-state.json` Schema

`closure-state.json` should be a reduced campaign snapshot, not a dump of raw run data.

Top-level shape:

```json
{
  "schema_version": "closure-state.v1",
  "campaign_id": "rust-baseline-grok4-xai",
  "updated_at": "2026-04-16T12:34:56Z",
  "config": {
    "model_id": "x-ai/grok-4-fast",
    "provider_slug": "xai"
  },
  "registry": { "...": "..." },
  "eval": { "...": "..." },
  "protocol": { "...": "..." },
  "instances": [
    { "...": "..." }
  ]
}
```

### Top-level fields

- `schema_version`
  - snapshot schema id
- `campaign_id`
  - stable identifier for the tracked campaign
- `updated_at`
  - last reducer update time
- `config`
  - the configuration under which closure is being tracked
- `registry`
  - campaign-level registry closure summary
- `eval`
  - campaign-level eval closure summary
- `protocol`
  - campaign-level protocol closure summary
- `instances`
  - reduced per-instance closure rows

### `config`

Suggested fields:

- `model_id`
- `provider_slug`
- `dataset_keys`
- `required_procedures`
- `notes`

This should describe the closure frame, not every runtime detail.

### `registry`

Suggested fields:

- `expected_total`
- `mapped_total`
- `missing_total`
- `ambiguous_total`
- `status`

Where `status` is one of:

- `complete`
- `partial`
- `missing`

### `eval`

Suggested fields:

- `expected_total`
- `complete_total`
- `failed_total`
- `missing_total`
- `partial_total`
- `in_progress_total`
- `status`
- `started_at`
- `last_transition_at`

### `protocol`

Suggested fields:

- `expected_total`
- `full_total`
- `partial_total`
- `failed_total`
- `missing_total`
- `incompatible_total`
- `in_progress_total`
- `status`
- `required_procedures`
- `started_at`
- `last_transition_at`

### `instances[]`

Each row should be reduced and pointer-oriented:

```json
{
  "instance_id": "clap-rs__clap-3521",
  "dataset_key": "clap",
  "repo_family": "clap-rs__clap",
  "registry_status": "mapped",
  "eval_status": "complete",
  "protocol_status": "partial",
  "eval_failure": null,
  "protocol_failure": null,
  "artifacts": {
    "run_manifest": "/.../run.json",
    "record_path": "/.../record.json.gz",
    "execution_log": "/.../execution-log.json",
    "protocol_anchor": "/.../protocol-artifacts/....json"
  },
  "protocol_procedures": {
    "tool-call-intent-segments": "complete",
    "tool-call-review": "partial",
    "tool-call-segment-review": "missing"
  },
  "last_event_at": "2026-04-15T22:53:00Z"
}
```

Suggested per-instance fields:

- `instance_id`
- `dataset_key`
- `repo_family`
- `registry_status`
- `eval_status`
- `protocol_status`
- `eval_failure`
- `protocol_failure`
- `artifacts`
- `protocol_procedures`
- `last_event_at`

Per-instance status enums should stay small and explicit:

- registry:
  - `mapped`
  - `missing`
  - `ambiguous`
  - `ineligible`
- eval:
  - `complete`
  - `failed`
  - `missing`
  - `in_progress`
  - `partial`
- protocol:
  - `full`
  - `partial`
  - `failed`
  - `missing`
  - `incompatible`
  - `in_progress`

## Proposed `closure-events.jsonl` Vocabulary

The event stream should record semantic transitions only.

Canonical shape:

```json
{
  "schema_version": "closure-event.v1",
  "campaign_id": "rust-baseline-grok4-xai",
  "timestamp": "2026-04-16T12:34:56Z",
  "kind": "eval.run.completed",
  "instance_id": "clap-rs__clap-3521",
  "payload": {
    "record_path": "/.../record.json.gz"
  }
}
```

Required fields:

- `schema_version`
- `campaign_id`
- `timestamp`
- `kind`
- `instance_id`
- `payload`

Recommended event kinds:

- `campaign.started`
- `registry.enumerated`
- `registry.instance.mapped`
- `registry.instance.missing`
- `eval.run.started`
- `eval.run.completed`
- `eval.run.failed`
- `protocol.intent-segmentation.completed`
- `protocol.tool-call-review.completed`
- `protocol.tool-call-segment-review.completed`
- `protocol.run.partial`
- `protocol.run.complete`
- `campaign.recomputed`

Recommended payload examples:

- `eval.run.completed`
  - `record_path`
  - `execution_log`
- `eval.run.failed`
  - `error`
  - `run_manifest`
- `protocol.*.completed`
  - `artifact_path`
  - `procedure_name`
- `campaign.recomputed`
  - `source`
  - `notes`

Noise guardrail:

- if an event does not change campaign classification or materially improve pointer visibility, it should not be emitted

## Proposed CLI Boundary

The minimal new CLI surface should be:

- `ploke-eval closure init`
- `ploke-eval closure recompute`
- `ploke-eval closure status`
- `ploke-eval closure inspect`

Optional but useful later:

- `ploke-eval closure tail`

### `closure init`

Purpose:

- create the initial campaign directory and seed `closure-state.json`

Suggested shape:

```bash
ploke-eval closure init \
  --campaign rust-baseline-grok4-xai \
  --model x-ai/grok-4-fast \
  --provider xai \
  --required-procedure tool-call-intent-segments \
  --required-procedure tool-call-review \
  --required-procedure tool-call-segment-review
```

### `closure recompute`

Purpose:

- rebuild the reduced snapshot from current on-disk source artifacts
- recover from missed events or older campaigns

Suggested shape:

```bash
ploke-eval closure recompute --campaign rust-baseline-grok4-xai
```

This command is important because it keeps the system recoverable even if event emission is incomplete.

### `closure status`

Purpose:

- print the concise one-line or compact block status surface for orchestration

Suggested shape:

```bash
ploke-eval closure status --campaign rust-baseline-grok4-xai
ploke-eval closure status --campaign rust-baseline-grok4-xai --json
```

### `closure inspect`

Purpose:

- show one layer in more detail without dropping to raw artifacts

Suggested shape:

```bash
ploke-eval closure inspect registry --campaign rust-baseline-grok4-xai
ploke-eval closure inspect eval --campaign rust-baseline-grok4-xai
ploke-eval closure inspect protocol --campaign rust-baseline-grok4-xai
```

## Integration Boundary With Existing Commands

The lowest-friction integration is:

- existing producer commands gain optional `--campaign <id>`
- when set, they:
  - append one sparse semantic event
  - trigger a reducer update for `closure-state.json`

This should apply to:

- `run-msb-single`
- `run-msb-agent-single`
- `run-msb-batch`
- `run-msb-agent-batch`
- `protocol tool-call-intent-segments`
- `protocol tool-call-review`
- `protocol tool-call-segment-review`

Recommended behavior:

- if `--campaign` is absent:
  - current behavior remains unchanged
- if `--campaign` is present:
  - the command behaves normally
  - then emits one semantic transition
  - then updates the campaign snapshot

This keeps adoption incremental.

## Minimal Implementation Direction

The smallest plausible implementation slice is:

1. add campaign directory helpers under `~/.ploke-eval/campaigns/<campaign-id>/`
2. define `closure-state.v1` and `closure-event.v1`
3. implement `closure recompute` using existing filesystem artifacts
4. implement `closure status`
5. add optional `--campaign` hooks to:
   - `run-msb-agent-batch`
   - `protocol tool-call-intent-segments`
   - `protocol tool-call-review`
   - `protocol tool-call-segment-review`

Reason for this order:

- `closure recompute` gives recovery first
- `closure status` gives monitoring value early
- event emission can then be added without making correctness depend on it

## Next Useful Implementation Step

If this sketch is adopted, the next bounded implementation slice should be:

1. define one canonical closure-state JSON schema for the Rust baseline campaign
2. define one sparse append-only event JSONL schema
3. attach optional campaign/event emission to the existing eval and protocol-producing commands
4. add a small CLI inspection surface that renders the reduced campaign state
5. keep the detailed per-run and per-procedure artifacts as the underlying source of truth

This preserves:

- semantic meaning
- explicit state classes
- recoverable progress
- a compact planning artifact that can replace a large amount of prose
- a live orchestration surface without inventing a second copy of the raw run data
