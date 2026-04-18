# ploke-eval Canonical Artifact Set

- date: 2026-04-18
- task title: ploke-eval canonical artifact set
- task description: reduced stored artifact policy for the `ploke-eval` rewrite, defining the canonical per-run records, durable attachments, convenience views to eliminate, and the authority surface downstream systems should consult
- related planning files: `docs/active/CURRENT_FOCUS.md`, `docs/active/agents/2026-04-18_ploke-eval-procedure-model.md`, `docs/active/agents/2026-04-18_ploke-eval-pipeline-recon/README.md`, `crates/ploke-eval/src/inner/recon-reports/patch-pipeline.md`, `docs/workflow/evalnomicon/drafts/formal-procedure-notation.md`

## Purpose

This note defines the reduced stored artifact set the `ploke-eval` rewrite
should target before implementation begins.

The point is not to rename the current files one by one. The point is to stop
duplicated stored meaning from proliferating again under a cleaner code layout.

This note therefore answers:

- which artifacts are canonical durable records
- which artifacts survive only as durable attachments
- which current files should disappear and be rendered on demand instead
- which authority surface downstream systems should consult for run identity,
  configuration, and discoverability

## Inputs To This Audit

This reduction is based on:

- the eval procedure model in
  [2026-04-18_ploke-eval-procedure-model.md](./2026-04-18_ploke-eval-procedure-model.md)
- the pipeline recon packet in
  [2026-04-18_ploke-eval-pipeline-recon/README.md](./2026-04-18_ploke-eval-pipeline-recon/README.md)
- direct writer inventory from `crates/ploke-eval/src`
- direct reader/consumer inventory from `crates/ploke-eval/src`

The strongest current facts are:

- `record.json.gz` is already the closest thing to a canonical run/evidence
  record, but it is incomplete in important ways such as patch-phase capture
- many current sidecars are strict subsets, locators, or convenience summaries
  over data that belongs in a richer canonical record
- the final snapshot DB is the one non-JSON attachment with a real canonical
  role for time-travel query and replay
- per-run submission JSONL is currently treated as both an export payload and a
  stored run artifact, but that meaning can be derived from a canonical
  packaging result instead of being stored twice

## Canonical Stored Surface

Per run, the rewrite should target exactly three classes of persisted surface:

1. one authoritative registration record
2. one canonical run/evidence record
3. a minimal attachment set

### 1. Authoritative Registration Record

Canonical path:

```text
registries/runs/<run_id>.json
```

Canonical type:

```text
RunRegistration
```

Role:

- shared source of truth for run identity
- shared source of truth for recorded `RunIntent`
- shared source of truth for `FrozenRunSpec`
- lifecycle/discoverability surface for operators and downstream systems
- artifact manifest root for the run

This is the authority surface other systems should consult when they need to
know:

- which runs exist
- which configuration each run used
- which runs completed, failed, or are still in progress
- where the canonical run record and attachments live
- which cache fingerprints are associated with the run

The registration record should own the authoritative copy of:

- `run_id`
- recorded `RunIntent`
- `FrozenRunSpec`
- schema/version identity
- lifecycle status and timestamps
- artifact references
- cache-relevant fingerprints or digests

Consequence:

- the rewrite should not require downstream systems to infer run identity or
  configuration from a pile of per-run sidecar files
- discoverability should come from the registry layer first, not from scanning
  run directories heuristically

### 2. Canonical Run/Evidence Record

Canonical path:

```text
runs/<run_id>/record.json.gz
```

Canonical type:

```text
RunRecord
```

Role:

- authoritative execution/evidence record for the run
- primary source for setup, inquiry, patch, validation, packaging, and timing
  evidence
- primary source for downstream inspect/replay/protocol views that do not
  require full DB state

Target contents:

- run id and registration reference
- setup phase
- turn spine
- patch phase
- validation phase if attempted
- packaging phase if attempted
- timing and outcome summary
- structured evidence needed by downstream analysis

Important tightening relative to the current crate:

- patch phase must be filled canonically
- validation must be filled canonically if the run performs validation
- per-turn timing must stop using placeholder timestamps
- the run record should be rich enough that current sidecars like repo state,
  indexing status, parse failure, execution log, and turn summary no longer
  need to exist as stored files

### 3. Minimal Attachment Set

Canonical per-run attachments should be limited to payloads that are either too
large, too raw, or too operationally specialized to inline into `RunRecord`.

Default attachment set:

```text
runs/<run_id>/state-final.db
```

Role:

- canonical queryable DB state for replay and time-travel inspection

Optional attachments only when capture policy explicitly calls for them:

```text
runs/<run_id>/attachments/llm-full-responses.jsonl
runs/<run_id>/attachments/failure/<...>
runs/<run_id>/attachments/debug/<...>
```

Role:

- raw provider responses
- failure-only dumps
- debugging payloads too large or too noisy for the canonical run record

Important discipline:

- optional attachments are not part of the canonical meaning of a successful
  run unless the policy explicitly says so
- absence of an optional attachment must not make the run semantically
  ambiguous

## Proposed Directory Shape

Minimal target layout:

```text
~/.ploke-eval/
  registries/
    runs/
      <run_id>.json
  runs/
    <run_id>/
      record.json.gz
      state-final.db
      attachments/
        ...
```

If batch workflows remain first-class, the same principle should apply:

```text
registries/batches/<batch_id>.json
```

with batch summaries derived from run registrations and run records rather than
stored as a second canonical truth surface.

## What Should Not Be Stored By Default

The rewrite should stop storing these by default as first-class run artifacts:

- execution logs
- repo-state sidecars
- indexing-status sidecars
- parse-failure sidecars
- snapshot-status sidecars
- agent-turn-summary sidecars
- per-run submission JSONL sidecars
- batch-run summary sidecars
- replay-only JSON dumps

These should instead be:

- fields inside `RunRecord`
- fields inside `RunRegistration`
- renderable views over canonical records
- optional attachments under an explicit capture policy

## Legacy Artifact Fate Matrix

### Keep As Canonical

- `record.json.gz`
  Keep and strengthen as the canonical run/evidence record.
- `final-snapshot.db`
  Keep, but rename conceptually to the canonical final DB attachment. The exact
  filename may change to `state-final.db`.

### Replace With New Canonical Surface

- `run.json`
  Replace as a standalone canonical surface with `RunRegistration` under
  `registries/runs/<run_id>.json`.
  The recorded `RunIntent` and `FrozenRunSpec` live there.
- `batch.json`
  Replace, if batch remains first-class, with a batch registration surface
  under `registries/batches/<batch_id>.json`.

### Collapse Into `RunRecord`

- `execution-log.json`
  Collapse into run timing, phase transitions, and operator-facing renderers
  derived from `RunRecord`.
- `repo-state.json`
  Collapse into setup-phase repo state in `RunRecord`.
- `indexing-status.json`
  Collapse into setup-phase indexing status in `RunRecord`.
- `parse-failure.json`
  Collapse into setup-phase parse-failure evidence in `RunRecord`.
- `agent-turn-summary.json`
  Collapse into the turn and patch sections of `RunRecord`.
- per-run packaging payload summary
  Collapse into the packaging phase of `RunRecord`.

### Eliminate As Stored Locators Or Redundant Views

- `snapshot-status.json`
  Eliminate. Snapshot path should live in `RunRegistration.artifacts` and, if
  helpful, in `RunRecord`.
- `multi-swe-bench-submission.jsonl` under each run directory
  Eliminate as a stored run artifact. Store packaging result canonically in
  `RunRecord` and render export JSONL on demand.
- `batch-run-summary.json`
  Eliminate as a canonical stored surface. Render from batch registration plus
  per-run registrations and run records.
- `replay-batch-###.json`
  Eliminate from the default stored surface. Replays may emit ephemeral or
  explicit operator outputs, but not canonical run artifacts.

### Downgrade To Optional Attachments

- `llm-full-responses.jsonl`
  Keep only as an optional raw attachment when explicit response capture is
  enabled.
- `agent-turn-trace.json`
  Do not keep as a default first-class stored surface. Either fold the required
  turn evidence into `RunRecord` or emit a compressed optional attachment if raw
  event volume later proves too large.
- `indexing-checkpoint.db`
  Do not keep by default. If useful for debugging or resumability, treat it as
  an optional intermediate attachment, not a canonical run artifact.
- `indexing-failure.db`
  Do not keep by default. If retained, keep only as a failure attachment.

### Keep Outside The Canonical Run Surface

- `last-run.json`
  Keep only as a global convenience pointer, not a canonical run artifact.
- `cache/starting-dbs/<hash>.sqlite` and metadata
  Keep only as cache infrastructure, not as canonical run state.
- `protocol-artifacts/*.json`
  Do not treat as part of the canonical eval run surface. They belong to a
  downstream protocol persistence question and should not shape the initial eval
  artifact set.

## Consumer Migration Targets

The canonical set above implies these reader migrations:

- `closure::classify_eval_status`
  should use `RunRegistration` lifecycle plus `RunRecord`, not repo-state,
  indexing-status, execution-log, or snapshot-status sidecars
- run-history helpers
  should use `RunRegistration` or `RunRecord` to find `state-final.db`, not
  `snapshot-status.json`
- campaign export
  should derive benchmark export rows from canonical packaging results in
  `RunRecord`, not from per-run submission JSONL sidecars
- inspect surfaces
  should render from `RunRecord` and `state-final.db`, with optional raw
  attachments only when requested

## Elimination Rules

A stored artifact should be removed if any of the following hold:

- it is a strict subset of a canonical stored record
- it is a lossless reformat of a canonical stored record
- it exists only to point at another canonical artifact
- it exists only because an older consumer took the filesystem layout as its API
- it can be rendered deterministically from `RunRegistration`, `RunRecord`, and
  attachments

An artifact may remain only if:

- it is the authoritative home of a distinct kind of information
- it is too large or too raw to inline cleanly
- it is required for replay/query semantics
- it is an explicit export product requested by an operator rather than a stored
  canonical run artifact

## Canonical Defaults

Default rule set for the rewrite:

- every run gets a `RunRegistration`
- every executed run gets a `RunRecord`
- every completed run that reaches queryable workspace state gets a final DB
  attachment
- raw provider responses are opt-in attachments
- debug and failure dumps are opt-in or failure-only attachments
- exports are rendered on demand, not stored as canonical run artifacts

## Open Design Pressure

One deliberate tension remains:

- whether `RunRecord` should continue to inline very rich turn artifacts, or
  whether some event-level payload should move into an optional compressed
  attachment

The current recommendation is:

- keep the canonical structured turn spine in `RunRecord`
- do not commit to a stored raw turn-trace attachment by default
- add an attachment only if actual event volume makes the canonical record
  impractical

That keeps the default stored surface minimal while preserving room for an
explicit capture policy later.

## Short Take

The reduced per-run canonical artifact set should be:

```text
registries/runs/<run_id>.json
runs/<run_id>/record.json.gz
runs/<run_id>/state-final.db
```

with optional attachments under:

```text
runs/<run_id>/attachments/
```

Everything else should either move into those canonical surfaces, become a
downstream store outside the eval run core, or be rendered on demand.
