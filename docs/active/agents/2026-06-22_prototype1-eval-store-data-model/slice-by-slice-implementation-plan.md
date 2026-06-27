# 2026-06-22 — Prototype 1 Eval Store Slice-by-Slice Implementation Plan

Status: active implementation plan / code-checked against current workspace.

Short description: consolidated implementation plan for moving Prototype 1 ordinary eval evidence behind an owner-scoped `EvalStore` with `fs | database | dual-strict`, starting with the fixed parent-owned `R4c -> R5` ParentStarted/resource evidence slice.

Related planning files:

- [`README.md`](README.md)
- [`implementation-plan.md`](implementation-plan.md)
- [`storage-plan.md`](storage-plan.md)
- [`persistence-port-map.md`](persistence-port-map.md)
- [`database-planning-notes.md`](database-planning-notes.md)
- [`relational-data-model.md`](relational-data-model.md)
- [`typestate-persistence-ledger.md`](typestate-persistence-ledger.md)
- [`transition-persistence-dependency-matrix.md`](transition-persistence-dependency-matrix.md)
- [`live-transition-test-plan.md`](live-transition-test-plan.md)
- [`implementation-log.md`](implementation-log.md)

## Scope and assumptions

- This plan covers Domain-C ordinary eval evidence only. It does not replace sealed History, channel transport, MessageBox authority, invocation/bootstrap delivery, artifact/worktree mutation, transition-journal replay, or logs/blob storage authority.
- Default behavior remains `backend = fs` until parity is proven.
- The first production writer is fixed: `R4c -> R5` parent-start evidence in `crates/ploke-eval/src/cli/prototype1_state/live_edges.rs:374`.
- The first filesystem evidence covered by parity is:
  - `JournalEntry::ParentStarted(ParentStartedEntry)` from `r4c_to_r5`;
  - the `journal::resource::Phase::ParentStart` sample currently produced by `append_parent_target_sample`.
- `records/mirror.cozo.sqlite` and `JsonRecordFile::emit` remain passive compatibility surfaces until later slices.
- Code graph overlay relations are follow-on work after ordinary eval evidence parity.

## Source files read for this plan

Planning packet read in full: every file under `docs/active/agents/2026-06-22_prototype1-eval-store-data-model/`.

Implementation code read for the planned change surfaces:

- First writer and final report: `crates/ploke-eval/src/cli/prototype1_state/live_edges.rs:374`, `:995`, `:1117`.
- Parent-start/resource record shapes and journal append path: `crates/ploke-eval/src/cli/prototype1_state/journal.rs:209`, `:346`, `:676`.
- Resource-sample helper: `crates/ploke-eval/src/cli/prototype1_state/cli_facing.rs:7993`.
- Run-profile parse/admission shape: `crates/ploke-eval/src/cli/prototype1_state/profile.rs:63`, `:123`, `:899`, `:930`, `:973`.
- Passive run-profile DTO mirror: `crates/ploke-records/src/run_profile.rs:18`, `:46`, `:520`.
- Run-shape handoff into live edges: `crates/ploke-eval/src/cli/prototype1_state/cli_facing.rs:1025`, `:1071`; typed context: `typestate/context.rs:139`, `:160`.
- Batch driver and walk phase inventory: `driver/advance.rs:25`, `walk/phase.rs`, `typestate/shape.rs`.
- Existing authority boundaries: `history/stored/mod.rs:23`, `channel.rs:183`, `channel.rs:815`, `inner.rs:41`, `parent.rs:127`, `parent.rs:430`, `backend/mod.rs`.
- Current passive record writer and mirror: `record_emission.rs:34`, `:73`, `:170`, `:197`.
- Later record writers: `intervention/scheduler.rs:778`, `:851`; `intervention/branch_registry.rs:28`; `cli_facing.rs:5505`.
- Invocation/bootstrap writer: `invocation.rs:529`, `:558`, `:578`.
- Trace/log source: `observe.rs`, `tracing_setup.rs:42`, `:540`.
- DB API surface: `crates/ploke-db/src/database.rs:1503`, `:2042`, `:2055`, `:2068`.

## How this plan preserves the live/API matrix

The time-efficient live/API plan is still present and canonical in [`live-transition-test-plan.md`](live-transition-test-plan.md) and [`transition-persistence-dependency-matrix.md`](transition-persistence-dependency-matrix.md). This consolidated plan now carries that matrix forward explicitly instead of replacing it.

Each implementation slice below names:

- **Producer transition(s):** the writer(s) whose persisted output changes.
- **Live API:** whether the producer itself must call a provider in the confidence suite.
- **Checkpoint:** the nearest fixture/checkpoint to restore or create so downstream consumers do not rerun provider work.
- **Downstream consumers:** later transitions that must be tested from checkpointed evidence.
- **Authority-negative:** the negative test proving DB/projection rows did not replace the real authority surface.
- **Canary:** when to reserve a full live loop.

Checkpoint ladder from the matrix:

| Checkpoint | Boundary | Main use | Live API to create/regenerate? |
| --- | --- | --- | --- |
| `F0_setup` | campaign/profile admitted, root parent identity ready or init path prepared | R0/R1/R2/R3 startup and profile/config tests | **No provider API.** Create locally. |
| `F1_ready_parent` | after `R4b -> R4c` or predecessor `R4a -> R4c`, before `R4c -> R5` | parent-start and baseline tests | **No provider API.** Genesis F1 is local. Predecessor F1 is derived from `F8_handoff_committed`, which may spawn a successor locally but must not call an LLM/provider at F1 time. |
| `F2_baseline_complete` | after `R5 -> R6` | policy/schedule/fanout tests | **Yes for genesis baseline regeneration when the baseline closure/eval/protocol producer is missing or intentionally refreshed.** **No** when restoring an existing verified closure baseline or promoting a selected-child baseline from upstream evidence. If F2 is absent and the path is genesis/default-live, regenerate it with the live suite instead of faking it from projection rows. |
| `F3_child_plan_received` | after `R7 -> R8` | schedule/fanout/C1 tests | **Yes for the default broad/provider planning producer.** **No only for an explicitly deterministic fixture profile** such as deterministic TUI tools. Storage-migration confidence for the default path must include a real provider-produced F3 checkpoint. |
| `F4_child_materialized_built` | after `C2 -> C3` plus artifact commit | spawn/ready tests | **No provider API.** Derive locally from verified `F3_child_plan_received`. If F3 is missing for the default path, regenerate F3 live first. |
| `F5_child_terminal_result` | after child terminal result with channel/result/treatment evidence | observe/compare/selection tests | **Yes for provider-dependent treatment execution / terminal result production.** Deterministic/local child fixtures may skip provider calls only when explicitly labeled as deterministic and not used as the provider confidence checkpoint. |
| `F6_child_compared` | after `C5 -> ParentCompared` | report/selection/continuation tests | **No provider API.** Derive locally from verified `F5_child_terminal_result`. If F5 is absent or stale for provider confidence, regenerate F5 with live provider calls first, then derive F6 locally. |
| `F7_selection_ready` | after `R11 -> R12`, before `R12 -> R13` | stopped/handoff branch tests | **No provider API.** Derive locally from verified `F6_child_compared`. If F6 is missing, rebuild upstream; provider calls occur only while regenerating F5, not at F7. |
| `F8_handoff_committed` | after `R12 -> R13` handoff with sealed History and successor invocation | successor startup/finalization tests | **No provider API.** Derive locally from verified `F7_selection_ready`; this may spawn/exec the successor process, seal History, and mutate/install selected artifact state, but it must not call an LLM/provider at F8. |

Live provider command shape remains explicit and opt-in:

```text
PLOKE_EVAL_LIVE_API_TESTS=1 PLOKE_RUN_LIVE_TESTS=1 cargo test -p ploke-eval --features live_api_tests -- --ignored prototype1_live_transition
```

Provider-facing producer transitions use real provider calls. Checkpointed outputs are for downstream consumer coverage and time savings, not for pretending the provider producer ran.

Live-call intent rule: when the operator says to “go ahead”, “proceed”, or otherwise authorizes implementation, and the live opt-in variables/credentials are present, autonomous implementation should run the required live producer gates for provider-facing slices. Do not substitute deterministic checkpoints for `F3_child_plan_received` or `F5_child_terminal_result` provider confidence. If credentials or opt-in variables are missing, report the live gate as blocked rather than silently treating a skipped test as success.

Transition-contract rule: each storage migration slice must add or preserve a focused test for the exact typestate edge being changed. Checkpoints accelerate setup and downstream consumer coverage; they do not replace per-edge contract tests. A transition-contract test should prove preconditions, output typestate, expected writes, no unrelated authority mutation, and loud failure for unsupported backends or missing authority.

## Validation-critical details for autonomous implementation

### Transition inventory artifact

Before any production writer moves behind `EvalStore`, Slice 0 must create a source-derived inventory artifact and test assertion.

Recommended generated artifact:

```text
docs/active/agents/2026-06-22_prototype1-eval-store-data-model/transition-inventory.generated.md
```

Required fields per transition/outcome:

```text
edge_id                  -- e.g. r4c_to_r5
source_anchor            -- file:path/line or rust path
from_phase
to_phase_or_branch
producer_surfaces        -- files/relations/log refs written
consumer_surfaces        -- later reads/gates
authority_classes        -- evidence/projection, channel, MessageBox, History, artifact, bootstrap
live_api_required        -- yes/no, with provider reason
checkpoint_in
checkpoint_out
downstream_consumers
authority_negative_case
```

The corresponding test must derive the inventory/count from source metadata where possible and fail when a transition split/merge changes the expected count. The generated count belongs in the generated inventory and test snapshot, not in prose here.

### Checkpoint registry and manifest

Implement checkpoints as declared fixtures with a small manifest before relying on them for downstream tests.

Recommended non-backup fixture root for campaign/checkpoint trees:

```text
crates/ploke-eval/tests/fixtures/prototype1-checkpoints/<checkpoint-name>/
```

If a checkpoint includes schema-coupled Cozo backup DBs under `tests/backup_dbs/`, follow `docs/testing/BACKUP_DB_FIXTURES.md` before changing them.

Each checkpoint must include a manifest, for example:

```json
{
  "schema_version": "prototype1-checkpoint-manifest.v1",
  "checkpoint": "F1_ready_parent",
  "created_by": "test|live_api|manual_import",
  "campaign_id": "...",
  "parent_id": "...",
  "boundary": "after R4b -> R4c, before R4c -> R5",
  "requires_live_api_to_regenerate": false,
  "files": [
    {
      "path": "prototype1/transition-journal.jsonl",
      "sha256": "...",
      "required": true,
      "authority_class": "evidence/projection"
    }
  ],
  "db_snapshots": [
    {
      "path": "eval-store.cozo.sqlite",
      "sha256": "...",
      "schema_version": "...",
      "required": false
    }
  ],
  "authority_surfaces": {
    "history": [],
    "channels": [],
    "message_boxes": [],
    "artifacts": []
  }
}
```

Restore tests must verify hashes before executing a consumer transition. A checkpoint with missing required authority surfaces must not be silently repaired from DB rows.

### Live API suite contract

Live provider tests require explicit opt-in and credentials.

Required migration-suite opt-in:

```text
PLOKE_EVAL_LIVE_API_TESTS=1
```

Recommended strict mode so missing live config fails instead of skipping:

```text
PLOKE_RUN_LIVE_TESTS=1
```

Direct-Google suite variables:

```text
GOOGLE_PROJECT_ID=<project>      # code may default; log effective value
GOOGLE_REGION=<region>           # code may default; log effective value
PLOKE_EVAL_LIVE_GOOGLE_MODEL_ID=google/gemini-2.5-flash-lite
```

Google ADC auth must be available to the process. If OpenRouter-backed tests are later added, they must require `OPENROUTER_API_KEY` and a suite-specific opt-in instead of falling through from `live_api_tests` alone.

Provider-facing producer tests must run serially or under a bounded live-test lock. Test output and [`implementation-log.md`](implementation-log.md) must record provider, model, route source, profile path/hash, checkpoint produced, and retained artifact path.

### Concrete test target names

Use stable test name prefixes so autonomous runs can target a slice without grep archaeology:

```text
prototype1_transition_inventory_*
prototype1_transition_contract_*
prototype1_checkpoint_*
prototype1_eval_store_parent_start_fs_*
prototype1_eval_store_parent_start_db_*
prototype1_eval_store_parent_start_dual_strict_*
prototype1_storage_authority_negative_*
prototype1_live_transition_*          -- ignored + env-gated
```

Expected command families:

```text
cargo test -p ploke-eval prototype1_transition_inventory
cargo test -p ploke-eval prototype1_transition_contract
cargo test -p ploke-eval prototype1_checkpoint
cargo test -p ploke-eval prototype1_eval_store_parent_start_fs
cargo test -p ploke-eval prototype1_eval_store_parent_start_db
cargo test -p ploke-eval prototype1_eval_store_parent_start_dual_strict
cargo test -p ploke-eval prototype1_storage_authority_negative
PLOKE_EVAL_LIVE_API_TESTS=1 PLOKE_RUN_LIVE_TESTS=1 cargo test -p ploke-eval --features live_api_tests -- --ignored prototype1_live_transition
```

### Parent-owned DB handle gate

Do not implement production `database` or `dual-strict` by opening `records/mirror.cozo.sqlite`. The first DB-backed production slice needs an owner-scoped parent `ploke_db::Database` handle.

If no parent-owned persistent code-graph DB handle is available at Slice 5, implement one of these explicit paths instead of faking parity:

1. **Injected test DB only:** keep production `database`/`dual-strict` returning a clear configuration error; complete DB schema/unit tests with injected `Database`.
2. **Parent eval DB file:** create an owner-scoped eval DB under the campaign tree, e.g. `prototype1/eval-store.cozo.sqlite`, clearly separate from the passive mirror and relation-named as `DbEvalStore`.
3. **Existing parent code-graph DB handle:** if the live parent already owns a code graph `Database`, pass that handle into `ConfiguredEvalStore` at the loop boundary.

Document the chosen path in [`implementation-log.md`](implementation-log.md) before enabling production `dual-strict`.

### Partial-write and recovery validation

For `dual-strict`, filesystem-first means DB failure can leave valid JSONL evidence without matching DB rows. That is acceptable only if the transition fails loudly and emits enough recovery data.

Required error details:

```text
journal_path
parent_started_source_event_index
resource_source_event_index
parent_started_content_sha256
resource_content_sha256
expected_semantic_hash
db_error_or_mismatch
suggested_recovery = "re-run deterministic import for these source indices" | "remove failed checkpoint and retry slice"
```

Required recovery test:

1. Force DB failure after filesystem append.
2. Assert transition returns an error.
3. Assert journal lines exist and hashes match the error.
4. Run an explicit deterministic import/repair helper in test mode.
5. Assert DB rows then match the original semantic hash.

Do not add automatic production rollback that deletes journal lines; the journal is append-only compatibility evidence.

### Autonomous implementation and commit contract

When implementation starts, keep [`implementation-log.md`](implementation-log.md) updated at every slice boundary.

Autonomous implementation is pre-approved only within the planned slices and guardrails:

- make live API calls only for provider-facing producer transitions when the opt-in variables and credentials are present;
- commit after each slice passes its required local/checkpoint/live gates;
- keep commits slice-scoped and reversible;
- never commit secrets, unredacted provider payloads, large live artifacts, or generated checkpoint payloads without an explicit manifest/fixture decision;
- before editing Rust symbols, run the required impact analysis workflow available in the harness;
- before each commit, run change detection / `git status --short` / `git diff --stat` and record the result.

The implementation should still stop if a proposed change would weaken authority/correctness semantics, silently tolerate missing expected evidence, require treating the passive mirror as typed DB parity, or need a destructive fixture/backup-DB change not covered by the fixture registry rules.

## First-slice contract source

The canonical first-slice contract is in [`database-planning-notes.md`](database-planning-notes.md). This section is a working summary for implementers; if it drifts, update this section from the database-planning note rather than treating it as a competing source.

### Method and envelope

Add one narrow method, not a generic emitter:

```rust
trait EvalStore {
    fn put_parent_started(
        &mut self,
        evidence: ParentStartedEvidence,
    ) -> Result<ParentStartedReceipt, EvalStoreError>;
}
```

`ParentStartedEvidence` should be a semantic envelope containing the already-known current fields, not raw file paths as identity:

- `campaign_id`
- `parent_identity`
- `repo_root`
- `handoff_runtime_id`
- `pid`
- `parent_started_recorded_at`
- `resource_recorded_at`
- `resource_subject = cargo_target`
- `resource_phase = parent_start`
- `resource_path = repo_root/target`
- `resource_status`, `resource_bytes`, `resource_error`
- source stream context supplied by the filesystem backend receipt.

### Minimal DB rows

First DB slice should create only:

- `eval_transition_event` for the semantic parent-start transition;
- `eval_record_ref` for the `ParentStarted` JSONL source line and the resource-sample JSONL source line.

Do not add `eval_runtime` yet: genesis parent-start currently has no concrete parent `runtime_id`; `ParentStartedEntry` only has optional `handoff_runtime_id`. Do not add `eval_trace_event` yet unless a structured trace writer is included in the same slice.

### Minimal common-axis values

For this slice:

- `store_scope = parent`
- `producer_role = parent`
- `visibility_scope = parent_visible`
- `source_class = direct_write`
- `evidence_class = typed_transition` for `eval_transition_event`
- `evidence_class = diagnostic` for the resource-sample `eval_record_ref`
- `validation_status = valid`

### Deterministic ids and hashes

- `source_stream_id`: deterministic string for the transition journal, e.g. `prototype1-transition-journal:<campaign_id>` plus the journal path as `source_ref`.
- `source_event_index`: zero-based JSONL event index returned by a new journal append receipt.
- `source_line`: one-based JSONL source line.
- `content_sha256`: SHA-256 of the exact compact JSON line bytes, excluding trailing newline.
- `event_id`: `sha256("p1.eval.transition_event.v1" || campaign_id || parent_id || "r4c_to_r5" || source_stream_id || source_event_index || content_sha256)`.
- `record_ref_id`: same pattern, with `"p1.eval.record_ref.v1"` and the relation family.
- `semantic_hash`: SHA-256 over canonical JSON for the semantic envelope, independent of Cozo storage formatting.

### Dual-strict write order

Use filesystem-first for the first live dual-strict slice because the current journal is still the compatibility/replay source and DB ids depend on source stream receipt fields.

Failure behavior:

- parent-start journal append failure remains fatal as today;
- in `fs` mode, preserve current best-effort behavior for resource-sample append failure from `append_parent_target_sample`;
- in `dual-strict`, resource sample is expected evidence for this slice, so resource append failure must fail the transition loudly after emitting enough error context;
- DB write failure after filesystem success fails the transition loudly and reports the source stream/index/hash that was already written;
- duplicate DB insert with the same deterministic id and same semantic hash is idempotent;
- duplicate deterministic id with different semantic hash is an error;
- queried row hash mismatch after insert is an error.

## Slice 0 — Transition inventory and checkpoint harness, no storage behavior changes

Goal: make the migration testable without waiting for a full live loop after each narrow change.

Code to change:

- Add test/inventory support near `crates/ploke-eval/src/cli/prototype1_state/live_edges.rs` and/or a new `prototype1_state/transition_inventory.rs` test helper.
- Use current walk/phase metadata from `crates/ploke-eval/src/cli/prototype1_state/walk/phase.rs` and runtime shape metadata from `typestate/shape.rs` rather than hand-maintained prose.
- Add checkpoint helpers in the existing test area used by `crates/ploke-eval/src/cli/prototype1_state/tests/cli_tests.rs` or a focused new test module.

Tests and live/API matrix gates:

- **Producer transition(s):** none changed; this slice creates the harness for every source-derived transition/outcome.
- **Live API:** no API calls required for the harness smoke itself.
- **Checkpoint:** implement checkpoint metadata/restore/hash verification and seed at least `F0_setup` and `F1_ready_parent`.
- **Downstream consumers:** prove one local consumer edge can run from a restored checkpoint without advancing producer setup again.
- **Authority-negative:** one fixture where a DB/projection row is present but MessageBox/channel/History/artifact evidence is invalid must fail at the real gate.
- **Inventory:** derive the current edge/outcome set and assert the generated count; future source changes must force the test matrix to be updated intentionally.
- **Canary:** no full live loop for this slice.

Exit criteria:

- Every live edge/outcome is named by tests.
- The harness can separate producer failures from downstream consumer-contract failures.

## Slice 1 — Profile config shape only

Goal: admit storage selection without changing runtime behavior.

Code to change:

- `crates/ploke-eval/src/cli/prototype1_state/profile.rs:123`
  - Extend `Storage` from only `worktree_root` to include `eval: EvalStorage`.
  - Add `EvalStorageBackend` enum with serde kebab-case values: `fs`, `database`, `dual-strict`.
  - Keep omitted config defaulting to `fs`.
  - Call storage validation from `Prototype1RunProfile::validate` at `profile.rs:63`.
- `crates/ploke-records/src/run_profile.rs:46`
  - Mirror the passive DTO shape so admitted profiles still round-trip through `RunProfileRecord` tests.
- `crates/ploke-eval/src/cli/prototype1_state/cli_facing.rs:1025`
  - Add an `eval_storage_backend` field to `Prototype1StateRunShape` so live edges can choose the store without reloading the whole profile.
  - `from_command` defaults to `fs`; `from_profile` reads `profile.storage.eval.backend`.

Tests and live/API matrix gates:

- **Producer transition(s):** setup/admitted-profile writes: `run-profile.toml` and `run-profile.commitment.json`.
- **Live API:** no.
- **Checkpoint:** update/create `F0_setup` so its admitted profile includes the storage config and verified digest.
- **Downstream consumers:** profile/policy chain consumers from the matrix: `R0/R1` startup resolution, `R6 -> R7` policy budget, `R9 -> R10` selection strategy, and `R12 -> R13` continuation decisions must prefer the admitted profile over scheduler fallback.
- **Authority-negative:** alter/remove `scheduler.json` in a fixture and prove admitted profile still drives policy; separately prove missing admitted profile fails or falls back only where currently allowed.
- **Unit tests:** omitted `[storage.eval]` defaults to `fs`; `[storage.eval] backend = "fs"` parses and round-trips; unknown backend is rejected; `admitted_run_profile_carries_digest` includes storage config; `ploke-records` passive profile DTO round-trips the new shape.
- **Canary:** no full live loop.

Exit criteria:

- Existing profile TOML still parses.
- No runtime code changes behavior in `fs` default.

## Slice 2 — EvalStore module, receipts, and filesystem backend only

Goal: introduce the narrow Domain-C port without adding DB behavior.

Code to change:

- Add a new module, likely `crates/ploke-eval/src/cli/prototype1_state/eval_store.rs`, exported from `prototype1_state/mod.rs`.
- Add:
  - `EvalStore` trait with only `put_parent_started`;
  - `ConfiguredEvalStore::Fs` wrapper;
  - `FsEvalStore` over the existing `PrototypeJournal`;
  - `ParentStartedEvidence`, `ParentStartedReceipt`, `JournalAppendReceipt`, and `EvalStoreError`.
- `crates/ploke-eval/src/cli/prototype1_state/journal.rs:676`
  - Add a non-breaking `append_with_receipt` helper on `PrototypeJournal` that preserves `RecordStore::append` and returns path, source event index, source line, cursor/byte counts if available, and content SHA-256.
  - Keep `RecordStore::append` behavior unchanged for existing callers.
- `crates/ploke-eval/src/cli/prototype1_state/cli_facing.rs:7993`
  - Split resource-sample construction from appending: add a helper that returns `journal::resource::Sample`; keep `append_parent_target_sample` as the existing best-effort wrapper for non-migrated callsites.

Tests and live/API matrix gates:

- **Producer transition(s):** test-only parent-start writer shape; production `R4c -> R5` is not moved yet.
- **Live API:** no.
- **Checkpoint:** use `F1_ready_parent` as the fixture shape for parent-start evidence construction.
- **Downstream consumers:** replay/metrics/history-preview readers that consume transition-journal `ParentStarted` and `Resource(parent_start)` entries.
- **Authority-negative:** no new authority path is introduced; confirm `FsEvalStore` only writes journal evidence and does not construct History/channel/MessageBox/artifact facts.
- **Unit tests:** `append_with_receipt` writes the same compact JSONL bytes as `RecordStore::append`; `FsEvalStore::put_parent_started` writes the same two journal entries as current `r4c_to_r5`; existing journal replay tests still pass.
- **Canary:** no full live loop.

Exit criteria:

- New store exists but production code is not yet moved.
- `backend = fs` still has no DB dependency.

## Slice 3 — Move only `R4c -> R5` behind FsEvalStore

Goal: make the first writer use `EvalStore` in `fs` mode with no behavior change.

Code to change:

- `crates/ploke-eval/src/cli/prototype1_state/live_edges.rs:374`
  - Replace the direct `parts.journal.append(JournalEntry::ParentStarted(...))` plus direct `append_parent_target_sample(...)` calls with `ConfiguredEvalStore::from_run_shape(...).put_parent_started(...)`.
  - For this slice, only `Fs` is constructible in production; `database` and `dual-strict` should return explicit `DatabaseSetup`/configuration errors until Slice 5 wires DB construction.
- `crates/ploke-eval/src/cli/prototype1_state/driver/advance.rs:25` and `walk/controller.rs`
  - No direct change expected if the edge signature stays the same.

Tests and live/API matrix gates:

- **Producer transition(s):** `R4c -> R5` parent-start/resource evidence.
- **Live API:** no; this transition is parent-owned and provider-free.
- **Checkpoint:** restore `F1_ready_parent`, execute exactly `R4c -> R5`, and retain the post-R5 campaign tree for replay/metrics checks.
- **Downstream consumers:** walk/replay/metrics/final-review consumers of `ParentStarted` and parent-start resource evidence; no downstream provider work should run.
- **Authority-negative:** parent readiness must already be decided by startup/History/artifact validation before the writer; a parent-start record alone must not let a bad `R4a/R4b` fixture enter `R5`.
- **Focused tests:** `prototype1_transition_contract_r4c_to_r5_fs_records_parent_start` asserts the exact `R4c -> R5` output typestate and `ParentStarted`/`Resource(parent_start)` journal writes; `prototype1_transition_contract_r4c_to_r5_db_backends_fail_loudly_without_writes` asserts unsupported production DB backends fail before writing; walk/controller step from `R4c` to `R5` advances exactly one edge; `driver/replay.rs`, `driver/reconstruct.rs`, and `history_preview.rs` consumers still pass.
- **Canary:** no full live loop.

Exit criteria:

- One production writer is behind the narrow store.
- Filesystem bytes and paths are preserved.

## Slice 4 — First DB schema/backend for tests and injected stores

Goal: prove typed DB rows for the first slice without changing authority or requiring code-graph overlays.

Code to change:

- In the new eval-store module, add `DbEvalStore<D>` and a narrow `EvalDb` trait implemented for `ploke_db::Database` using:
  - `Database::raw_query_mut_params` at `crates/ploke-db/src/database.rs:2068`;
  - `Database::raw_query_params` at `database.rs:2055`.
- Add schema installation functions for only:
  - `eval_transition_event`;
  - `eval_record_ref`.
- Add relation-exists/schema tests using `ploke_db::Database::new_init()` at `database.rs:1503`.
- Do not touch `record_emission.rs` yet; the passive mirror remains compatibility-only.

First-slice relation sketch:

```text
eval_transition_event {
  event_id: String =>
  campaign_id: String,
  parent_id: String,
  runtime_id: String?,
  node_id: String,
  generation: Int,
  transition: String,
  phase: String,
  outcome: String,
  store_scope: String,
  producer_role: String,
  visibility_scope: String,
  source_class: String,
  evidence_class: String,
  validation_status: String,
  source_stream_id: String,
  source_event_index: Int,
  source_line: Int,
  source_ref: String,
  content_sha256: String,
  semantic_hash: String,
  recorded_at: Int,
  ingested_at: String
}

eval_record_ref {
  record_ref_id: String =>
  campaign_id: String,
  family: String,
  schema_version: String,
  store_scope: String,
  producer_role: String,
  producer_id: String?,
  source_class: String,
  evidence_class: String,
  visibility_scope: String,
  validation_status: String,
  source_stream_id: String,
  source_event_index: Int,
  source_line: Int,
  source_ref: String,
  content_sha256: String,
  payload_json: String?,
  recorded_at: Int?,
  ingested_at: String
}
```

Tests and live/API matrix gates:

- **Producer transition(s):** no live producer moved; DB writer is exercised through injected/test stores for the `R4c -> R5` envelope.
- **Live API:** no.
- **Checkpoint:** use fixed `F1_ready_parent`-derived sample evidence for deterministic ids/hashes.
- **Downstream consumers:** DB query round-trips only; filesystem replay remains the consumer authority until dual-strict is wired.
- **Authority-negative:** fabricated DB row cannot satisfy MessageBox/channel/History/artifact startup gates.
- **DB tests:** schema install is idempotent; insert/query round-trip works; duplicate deterministic id with identical semantic hash is idempotent; duplicate id with different semantic hash fails; missing axes/hash/source fields fail before write.
- **Canary:** no full live loop.

Exit criteria:

- DB rows work for one narrow evidence slice in tests.
- No legacy scheduler/node/latest-result authority-shaped tables are introduced.

## Slice 5 — Production DB construction and dual-strict for first slice

Goal: enable `database` and `dual-strict` modes for the first writer only.

Code to change:

- `eval_store.rs`
  - Add `ConfiguredEvalStore::Db` and `ConfiguredEvalStore::DualStrict`.
  - Add construction that either receives an owner-scoped `Arc<ploke_db::Database>` or fails explicitly if a production parent DB handle is not available.
- `cli_facing.rs:1025` / `live_edges.rs:374`
  - Pass enough storage config and DB handle/context to `r4c_to_r5` without broadening other transition signatures unnecessarily.
- If the current parent loop still has no owner-scoped parent `Database` handle, stop before faking one with `records/mirror.cozo.sqlite`; add the explicit handle-plumbing slice rather than treating the passive mirror as `DbEvalStore`.

Tests and live/API matrix gates:

- **Producer transition(s):** `R4c -> R5` parent-start/resource evidence in `dual-strict`.
- **Live API:** no.
- **Checkpoint:** restore `F1_ready_parent`, execute exactly `R4c -> R5` in `dual-strict`, and retain the post-R5 journal plus DB snapshot.
- **Downstream consumers:** same as Slice 3 plus DB row/hash queries; replay/metrics must still work from JSONL.
- **Authority-negative:** DB row present but invalid/missing startup History/channel/MessageBox/artifact evidence still fails the relevant authority path; DB row cannot make `R4a/R4b` valid.
- **Dual tests:** successful dual write leaves the journal usable and creates expected DB rows; DB failure after filesystem write fails loudly; forced semantic mismatch fails; repeated write/import behavior matches deterministic-id spec.
- **Canary:** no full live loop; at most a local no-provider walk smoke from `F1_ready_parent` through `R6` if parent-start evidence consumers changed.

Exit criteria:

- `dual-strict` proves parity for the first writer.
- `database` mode is clearly experimental and cannot claim DB-only readiness for file-dependent reads.

## Slice 6 — Trace/log/ref evidence lane

Goal: add queryable observability evidence without changing transition authority.

Code to change:

- `crates/ploke-eval/src/cli/prototype1_state/observe.rs`
  - Map `TransitionBuilder`, `Step`, `command_output`, and result/future helpers into `eval_trace_event` rows where the store is available, or ingest their existing observation JSONL output.
- `crates/ploke-eval/src/tracing_setup.rs:42`, `:540`
  - Use `current_prototype1_observation_log_path` / `PLOKE_PROTOTYPE1_TRACE_JSONL` as source refs for ingestion.
- `eval_store.rs`
  - Add `put_trace_event` only after the first parent-start method is stable.
  - Add `eval_log_ref` for log refs/hashes; do not inline full LLM payloads.

Tests and live/API matrix gates:

- **Producer transition(s):** whichever trace/log writer is touched. Local first target should be parent-owned `observe::TransitionBuilder` / `observe::Step` events around non-provider transitions.
- **Live API:** no for local trace/log import; yes only if this slice starts capturing provider-facing producer traces (`R7 -> R8` broad planning or child treatment execution).
- **Checkpoint:** local trace tests can use `F1_ready_parent` and post-R5 evidence; provider-facing trace capture must create/review `F3_child_plan_received` for broad planning or `F5_child_terminal_result` for treatment results.
- **Downstream consumers:** trace/log report queries from restored checkpoints; do not rerun provider calls for downstream replay, compare, selection, or report consumers.
- **Authority-negative:** trace/log rows cannot replace channel terminal results, MessageBox receipts, or History evidence.
- **Import tests:** observation JSONL import produces deterministic trace ids; missing/invalid JSONL line yields explicit rejected/degraded import error; `fs` mode still emits the same logs/traces.
- **Canary:** only when a trace/log writer changes a cross-chain producer contract; otherwise no full live loop.

Exit criteria:

- Trace/log evidence is queryable as evidence, not treated as authority.

## Slice 7 — Compatibility record refs, not generic authority

Goal: ingest ordinary legacy/projection record refs with scope/source/evidence axes.

Code to change:

- `crates/ploke-eval/src/record_emission.rs:34`, `:73`
  - Do not replace `JsonRecordFile::emit` first. After Slice 6, add an optional typed `eval_record_ref` lane beside the passive mirror.
- `crates/ploke-eval/src/intervention/scheduler.rs:778`, `:851`
  - Move node projection and runner result refs through `EvalStore` methods or typed record-ref helpers.
  - Split child-local attempt results from parent-imported terminal channel facts.
- `crates/ploke-eval/src/intervention/branch_registry.rs:28`
  - Treat `branches.json` append records as compatibility refs until evaluation/selection summaries are normalized.

Tests and live/API matrix gates:

- **Producer transition(s):** depends on record family: node/request projections from `R7 -> R8`, `C1 -> C2`, `C2 -> C3`; runner results from child `ResultWritten`; branch registry refs from comparison/selection paths.
- **Live API:** no for deterministic/local node/request/build refs; yes only when touching provider-facing broad planning outputs (`R7 -> R8`) or provider-dependent child terminal result production.
- **Checkpoint:** use `F3_child_plan_received` for node/request consumers, `F4_child_materialized_built` for build/spawn consumers, and `F5_child_terminal_result` for runner-result consumers.
- **Downstream consumers:** for node/request refs run `C1`, `C2`, and `C3` from restored child-plan fixtures; for result refs run `C4`, `C5`, selection, and next-generation baseline promotion from `F5/F6`.
- **Authority-negative:** scheduler/node/latest-result refs cannot drive selection or History admission; terminal child success still requires channel-carried result/treatment evidence.
- **Compatibility tests:** existing `JsonRecordFile` mirror tests still pass; `eval_record_ref` includes store scope, producer role, source class, evidence class, visibility, validation status, and payload hash.
- **Canary:** only if terminal result or branch comparison refs affect the treatment/evaluation/selection chain; otherwise checkpoint consumers are sufficient.

Exit criteria:

- Legacy files are retained as refs/projections, not promoted into authority-shaped schema.

## Slice 8 — Attempt/invocation/channel mirrors

Goal: make runtime attempts and channel receipts queryable without replacing bootstrap or transport.

Code to change:

- `crates/ploke-eval/src/cli/prototype1_state/invocation.rs:529`, `:558`, `:578`
  - Mirror invocation metadata as eval evidence after the executable invocation file is written.
- `crates/ploke-eval/src/cli/prototype1_state/channel.rs:183`, `:520`, `:815`
  - Add mirror/import rows after channel envelopes are sent/received.
  - Keep `Channel<R, T: Transport>` and `FileTransport` as the hard authority.

Relations:

- `eval_attempt`
- `eval_invocation`
- `eval_channel_message`
- `eval_channel_receipt`
- `eval_import_event`

Tests and live/API matrix gates:

- **Producer transition(s):** `C3 -> C4` invocation/channel/spawn, child `Ready/Evaluating/Result`, successor ready/completion handoff messages.
- **Live API:** no for spawn/ready/invocation mirrors; yes for child terminal `Result` if this slice changes provider-dependent treatment execution.
- **Checkpoint:** use `F4_child_materialized_built` for spawn/ready, `F5_child_terminal_result` for terminal-result consumers, and `F8_handoff_committed` for successor startup/finalization.
- **Downstream consumers:** `C4 -> C5` observe, `C5 -> ParentCompared`, `R10/R11` selection, `R12/R13` continuation/handoff, and successor startup validation from `F8`.
- **Authority-negative:** DB mirror present but channel envelope missing/invalid fails; invocation mirror present but executable invocation file missing/invalid cannot launch child/successor; channel body-hash mismatch still fails.
- **Focused tests:** parent readiness/result still requires valid channel envelope and body hash; parent-visible imports name source/target scope and validation status.
- **Canary:** run a full live loop only when channel/import behavior changes the child runtime or successor handoff chain.

Exit criteria:

- Parent-visible child evidence crosses channel/import edges explicitly.

## Slice 9 — Evaluation, candidate, selection, and continuation summaries

Goal: replace manual joins over branch/evaluation projections with typed queryable summaries.

Code to change:

- `crates/ploke-eval/src/cli/prototype1_state/cli_facing.rs:5505`
  - Add `eval_evaluation` / `eval_evaluation_instance` rows when `compare_observed_child_treatment` writes `prototype1/evaluations/<branch-id>.json` and appends `branches.json`.
- `cli_facing.rs:7178`, `:6892`
  - Add selection and continuation rows after `select_successor_for_profile` / `live_successor_continuation_decision`.
- `live_edges.rs` R12/R13 journal writes
  - Mirror successor selected/stopped/handoff evidence but do not replace sealed History.

Relations:

- `eval_candidate_set`
- `eval_candidate_member`
- `eval_child`
- `eval_evaluation`
- `eval_evaluation_instance`
- `eval_selection_decision`
- `eval_selection_candidate`
- `eval_continuation_decision`

Tests and live/API matrix gates:

- **Producer transition(s):** `C5 -> ParentCompared`, `R10 -> R11` rejected-only/selection paths, `R11 -> R12`, and `R12 -> R13` continuation decision evidence.
- **Live API:** no when starting from `F5_child_terminal_result`; live API is needed only to regenerate `F5` if the provider-facing child result producer changed.
- **Checkpoint:** restore `F5_child_terminal_result`, produce `F6_child_compared`, then `F7_selection_ready` for continuation/handoff tests.
- **Downstream consumers:** same-generation selection, stopped/no-selection final report, handoff branch from `F7`, and next-generation `R5 -> R6` baseline promotion from selected child evaluation evidence.
- **Authority-negative:** DB evaluation/selection rows cannot replace sealed History append, active checkout advancement, or successor startup validation.
- **Focused tests:** same-generation selection can be audited from rows plus source refs; next-generation baseline promotion still reads valid evaluation evidence.
- **Canary:** full live loop canary before declaring this slice complete, because this is a critical treatment/evaluation/selection chain.

Exit criteria:

- Selection audit queries no longer depend on mutable scheduler/node projections.

## Slice 10 — Operation/artifact provenance refs

Goal: preserve provenance for artifact/build/patch operations while keeping workspace/backend authority separate.

Code to change:

- `crates/ploke-eval/src/cli/prototype1_state/backend/mod.rs` and `backend/git_worktree.rs`
  - Add refs/hashes around artifact surfaces, child workspace materialization, and selected artifact install.
- `c1.rs`, `c2.rs`, `c3.rs`, `c4.rs`, and the `run_planned_child` path in `cli_facing.rs`
  - Mirror operation/build/artifact refs after existing artifact/backend actions succeed.

Relations:

- `eval_artifact_ref`
- `eval_artifact_surface`
- `eval_operation`
- `eval_patch`
- `eval_apply_event`
- `eval_build_event`
- `eval_binary_ref`

Tests and live/API matrix gates:

- **Producer transition(s):** `C1 -> C2` materialize, `C2 -> C3` build/artifact commit, `C3 -> C4` spawn binary refs, and `R12 -> R13` selected-artifact active-checkout install.
- **Live API:** no for materialize/build/artifact ref writes; provider API is only needed to regenerate earlier candidate/terminal checkpoints if the candidate/result producer changed.
- **Checkpoint:** use `F3_child_plan_received` for materialize, `F4_child_materialized_built` for spawn/ready, `F7_selection_ready` for handoff, and `F8_handoff_committed` for successor startup.
- **Downstream consumers:** `C2/C3/C4` build-spawn consumers, selection/handoff review, History sealing/handoff negative tests, and successor startup validation from persisted checkout/invocation/History.
- **Authority-negative:** DB artifact refs cannot prove workspace materialization, binary existence, active checkout install, or selected artifact admission without workspace/backend validation.
- **Focused tests:** build/spawn downstream tests run from checkpoints rather than full loops.
- **Canary:** full live loop canary before declaring this slice complete if handoff or selected-artifact provenance changes.

Exit criteria:

- Selection/handoff review can query artifact provenance without weakening artifact authority.

## Slice 11 — DB-only readiness review, not enablement by default

Goal: decide whether any surface can run DB-only.

Code/read audit before enabling DB-only:

- `scheduler.json` reads: `intervention/scheduler.rs`, `cli_facing.rs` fallback paths.
- latest `runner-result.json` reads: scheduler/result helpers and stored-child-outcome recovery.
- `branches.json` reads: `intervention/branch_registry.rs` and selection/recovery consumers.
- `node.json`, invocation JSON, channel JSONL, child-plan MessageBox files, transition journal JSONL, sealed History files.

Tests and live/API matrix gates:

- **Producer transition(s):** none by default; this is an audit gate over read paths.
- **Live API:** no by default; use existing checkpoints to prove read-path parity. Regenerate live checkpoints only when a provider-facing producer changed.
- **Checkpoint:** audit from the full ladder: `F0` for profile/startup, `F3/F4/F5` for child chains, `F6/F7` for selection/continuation, and `F8` for successor startup.
- **Downstream consumers:** every consumer listed in the dependency matrix for migrated surfaces must either read typed DB rows or be explicitly classified as filesystem authority/compatibility.
- **Authority-negative:** DB-only must not be claimed for History, Channel, MessageBox, bootstrap, artifact mutation, or transition-journal replay unless that domain has its own explicit backend and negative tests.
- **Canary:** one full live loop only after isolated producer/consumer checkpoint coverage passes.

Exit criteria:

- DB-only is enabled only for surfaces whose production reads have migrated or remain explicitly filesystem-authority dependencies.
- No DB-only claim is made for History, Channel, MessageBox, bootstrap, artifact mutation, or transition-journal replay unless those domains receive their own ports.

## Deferred follow-on: code graph overlays

After ordinary eval evidence parity, add code graph join slices from `codegraph-eval-join-model.md`:

1. `eval_code_snapshot`.
2. `eval_code_ref`.
3. `eval_code_link`.
4. `eval_code_touch`.
5. `eval_validation_event` / `eval_validation_cover`.
6. `eval_retrieval` / `eval_retrieval_hit`.
7. `eval_refresh_event`.

Do not merge child-local code graph facts into the parent database by default. Parent-visible imports must name artifact/runtime/import scope and validate hashes.

## Stop conditions

Stop and ask before implementing if a slice would:

- make DB rows authoritative for History, channels, MessageBoxes, invocation/bootstrap, or artifact mutation;
- weaken missing-evidence detection or silently tolerate schema/source drift;
- treat the passive mirror as typed DB parity;
- require full live loops as the inner test cycle instead of using checkpoints;
- make provider-facing confidence tests pass without real provider calls.
