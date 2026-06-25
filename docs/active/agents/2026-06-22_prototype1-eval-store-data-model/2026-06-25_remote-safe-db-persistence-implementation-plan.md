# 2026-06-25 — Remote-safe DB persistence implementation plan

Status: active restart / implementation contract.

Short description: corrected plan for moving Prototype 1 loop persistence from scattered shared-filesystem records toward owner-scoped, queryable database persistence without weakening Channel, MessageBox, History/Crown, invocation/bootstrap, or artifact/worktree authority. The immediate acceptance target is a `dual-strict` live loop canary that completes two generations across a parent/successor handoff.

Related planning files:

- [`README.md`](README.md)
- [`2026-06-25_cleanup-before-db-parity-decision.md`](2026-06-25_cleanup-before-db-parity-decision.md)
- [`storage-plan.md`](storage-plan.md)
- [`persistence-port-map.md`](persistence-port-map.md)
- [`relational-data-model.md`](relational-data-model.md)
- [`codegraph-eval-join-model.md`](codegraph-eval-join-model.md)
- [`slice-by-slice-implementation-plan.md`](slice-by-slice-implementation-plan.md)
- [`slice-11-db-only-readiness-audit.md`](slice-11-db-only-readiness-audit.md)
- [`transition-persistence-dependency-matrix.md`](transition-persistence-dependency-matrix.md)
- [`crates/ploke-eval/docs/prototype1/persistence-inventory-and-cozo-map.md`](../../../../crates/ploke-eval/docs/prototype1/persistence-inventory-and-cozo-map.md)
- [`docs/workflow/evalnomicon/src/prototype1/history-crown.md`](../../../workflow/evalnomicon/src/prototype1/history-crown.md)
- [`docs/workflow/evalnomicon/drafts/runtime/parent-child-channel.md`](../../../workflow/evalnomicon/drafts/runtime/parent-child-channel.md)

## Why this document exists

The previous implementation produced useful owner-scoped DB rows, but it drifted into an eval DB mirror and then into confusing language. The intended goal is larger and more specific:

1. consolidate scattered Prototype 1 loop evidence into queryable database relations;
2. move away from assuming parent/child/successor runtimes share a filesystem;
3. keep parent-visible facts limited to what the parent produced or explicitly received/imported;
4. preserve difficult, typed access to the safety-critical domains that gate long-lived successor process spawning.

This document is the corrected implementation contract. Future agents should prefer it when older slice notes imply that current DB work has already achieved DB-backed parity or DB-only readiness.

## Correct vocabulary

Use these terms consistently:

- **`fs`**: current filesystem-backed runtime behavior.
- **`db-mirror`**: files remain runtime authority; owner eval DB gets query/audit rows.
- **`database`**: current compatibility spelling for mirror behavior until typed DB read paths exist. Do not call the intended DB-backed architecture legacy.
- **`dual-strict`**: write file authority plus DB rows for a migrated surface and fail loudly on expected DB write/hash/semantic mismatch.
- **DB-backed read path**: a future state where a specific ordinary eval surface reads typed DB rows in production. This is per-surface and does not imply DB authority for Channel, MessageBox, History, bootstrap, or artifact mutation.
- **Remote-safe**: parent decisions depend on parent-owned facts and validated channel/import edges, not direct reads of child-local files.

## User intent captured

The ploke-eval Prototype 1 loop runs a parent generation, creates child candidates, validates safety surfaces, chooses a successor, starts that successor as a detached next parent, and self-terminates. Detached successor spawning is the exceptional long-lived background process path, so the handoff must remain observable and hard to fake.

The database work is meant to clean up scattered persistence while supporting a future where parent, child, and successor may run on different VMs or machines. The parent database is in the parent runtime scope. A child may have its own local database. Cozo is not the transport layer between parent and child.

Parent-visible child evidence must therefore arrive through one of these explicit surfaces:

- typed `Channel<R, T: Transport>` payloads or channel-carried verifiable refs;
- typed `MessageBox` lock/unlock edges such as `ChildPlan`;
- sealed History material and successor validation bundles;
- explicit import/admission rows naming producer/runtime/artifact, hashes, and validation status.

The database should hold almost everything that is sensible to query, but it must hold protected-domain facts in domain-aware relations/ports instead of generic record blobs.

## Confirmed current state from source/docs

Source/docs checked before writing this plan:

- `eval_store/api.rs` currently exposes a narrow `EvalStore::put_parent_started` with `Fs`, `DbMirror`, and `DualStrict` wrappers.
- Most later DB writes are direct helpers such as `write_channel_message_to_owner_db`, `write_invocation_to_owner_db`, `write_evaluation_to_owner_db`, `write_artifact_provenance_to_owner_db`, and `write_record_ref_to_owner_db`.
- `EvalStorageBackend::Database` is already documented in `profile.rs` as compatibility spelling for current mirror mode.
- `Channel<R, T: Transport>` validates direction, campaign, node, runtime, and body hash; tests prove DB rows cannot synthesize missing channel envelopes.
- `MessageBox`, `Open`, `Locked`, and `Received` preserve child-plan lock/unlock authority; tests prove DB record refs cannot replace `ChildPlanFile`.
- successor handoff goes through `Parent<Selectable>::seal_block_with_artifact`, `FsBlockStore::append`, successor invocation, channel ready wait, and successor startup validation.
- `AgentTurnTraceRecord`, `AgentTurnSummaryRecord`, and `RawFullResponseRecord` already exist as typed persisted shapes. Treating turn-live/full-response files as opaque blobs is not correct.
- `loop walk audit` already reports file vs DB surfaces, but it mostly counts rows/files rather than proving semantic per-surface parity.

## Non-negotiable invariants

Do not implement any fix that weakens these invariants:

1. **History/Crown authority stays typed.** Generic eval rows must not replace sealed block append, block hash verification, state-root checks, Crown locking, or successor startup validation.
2. **Channel authority stays transport/session-shaped.** DB rows may mirror messages/receipts/imports; they must not make parent receive a message that did not cross the configured `Transport`.
3. **MessageBox authority stays lock/unlock-shaped.** A DB child-plan payload row cannot replace `Open<ChildPlan> -> Locked<ChildPlan> -> Received<ChildPlan>` unless a dedicated `MessageBoxBacking` implements equivalent semantics.
4. **Invocation/bootstrap stays executable.** DB invocation rows do not launch child/successor processes. Bootstrap delivery needs its own port before file bootstrap can be retired.
5. **Artifact/worktree authority stays backend-shaped.** DB artifact rows do not prove worktree existence, checkout install, selected artifact admission, or binary executability.
6. **Parent DB visibility is explicit.** Child-local DB/codegraph facts do not become parent-visible unless admitted through channel/import semantics.
7. **Dual-strict fails loudly.** No opportunistic best-effort mirror writes for a surface that claims dual-strict parity.
8. **Use existing domain types at boundaries.** Avoid inventing parallel DTOs that merely mirror current persisted JSON. Internal row structs are acceptable inside DB modules, but public/store APIs should use existing semantic types where possible.

## Acceptance target

The first end-to-end success criterion for this plan is:

```text
A live Prototype 1 loop run using storage.eval.backend = "dual-strict"
completes two generations and crosses a parent -> successor -> second-parent
handoff successfully.
```

This proves, at minimum:

- first parent writes expected dual-strict eval DB rows;
- child planning/fanout/evaluation/selection/handoff still work;
- the selected successor starts as a detached runtime;
- successor validates sealed History and reaches parent readiness;
- second generation runs to its configured completion boundary;
- `loop walk audit --format json --verbose --with-note` shows the expected DB/file surfaces and no hidden downgrade to fs-only behavior.

The canary should be recorded in `implementation-log.md` with command, profile path/hash, provider/model, campaign id, audit path, outcome, and any retained artifact paths.

## Implementation strategy overview

Do not jump straight to DB-only. Implement in two tracks:

### Track A: strict live parity for the current local filesystem execution

This is the immediate path to the two-generation `dual-strict` canary. It keeps file authority for protected domains but requires every migrated ordinary-evidence surface on the live path to produce strict DB rows.

### Track B: remote-safe read/import model

After Track A is green, migrate ordinary eval consumers so parent reconstruction/audit/query can use DB rows rather than shared child paths where appropriate. Protected domains receive separate ports only when ready.

The two tracks intentionally overlap: Track A proves the DB contains the right facts; Track B removes shared-filesystem assumptions from consumers.

## Concrete work slices

### Slice 0 — Guardrail cleanup and terminology

Goal: make current behavior unambiguous before more schema changes.

Tasks:

- Ensure profile/docs/CLI/audit output consistently describe `database` as compatibility mirror mode until DB-backed reads exist.
- Prefer `db-mirror` in new profiles and docs.
- Keep `dual-strict` wording strict: no silent downgrade.
- Add or update tests that assert `EvalStorageBackend::Database` maps to mirror behavior, not DB-only.

Verification:

- profile parse/default tests;
- audit output mentions mirror semantics where relevant;
- no runtime behavior changes in `fs` mode.

### Slice 1 — Centralize owner eval DB policy

Goal: stop ad hoc “if owner DB exists then maybe mirror” semantics from leaking into strict modes.

Tasks:

- Introduce a small owner eval DB policy/context used by migrated writers:
  - backend mode;
  - campaign manifest path;
  - owner DB path;
  - strictness behavior.
- Keep high-level loop signatures narrow, but avoid each helper rediscovering DB path/strictness from raw file paths.
- In mirror modes, opportunistic file-adjacent helpers may skip when no DB exists only for surfaces explicitly classified as passive compatibility.
- In `dual-strict`, expected live-path surfaces must fail if their DB write is missing, invalid, or mismatched.

Verification:

- unit tests for policy decisions;
- forced DB failure after file write returns a repairable error for strict surfaces;
- existing negative authority tests still pass.

### Slice 2 — Trait/port cleanup without overgeneralizing

Goal: make the implementation less “direct-helper soup” without creating one giant persistence trait.

Tasks:

- Keep `EvalStore` Domain-C only.
- Add focused methods only when a production callsite needs them, using existing types where possible.
- Classify direct DB helper functions as either:
  - internal backend methods for a specific port; or
  - explicit mirror/import helpers for compatibility surfaces.
- Do not add `put_blob`, `put_json`, or `put_record_any` as a generic escape hatch.

Candidate method families:

- parent lifecycle evidence;
- baseline/evaluation/selection/continuation summaries;
- record refs/log refs;
- trace events;
- attempt/invocation/channel receipt/import mirrors;
- artifact/build/operation refs.

Verification:

- compile and focused tests after each method move;
- no change to protected-domain read paths.

### Slice 3 — Make the live dual-strict path strict for all already-mirrored surfaces

Goal: current DB rows should not be only best-effort once `dual-strict` is selected.

Surfaces on the two-generation live path to review:

- R0 setup context: campaign, profile commitment, closure ref;
- R4c -> R5 parent started/resource;
- R5 -> R6 baseline;
- child-plan trace/mirror after `Received<ChildPlan>` only;
- node/request/result compatibility refs;
- child materialization/artifact/build/operation refs;
- child invocation rows;
- channel message/receipt/import rows, including terminal `ToParent::Result`;
- runtime stream `eval_log_ref` rows;
- headless-TUI turn-live files and LLM full-response refs/rows;
- evaluation, selection, continuation rows;
- successor invocation/channel/handoff log/artifact refs.

Tasks:

- For each surface, decide: strict DB row required, passive compatibility mirror, protected-domain authority, or intentionally file-only.
- Add per-surface strict-mode checks with good error messages.
- Ensure `dual-strict` failures surface as transition errors, not warnings.

Verification:

- focused unit tests per surface;
- authority-negative tests remain decisive;
- no direct DB row can satisfy Channel/MessageBox/History/bootstrap/artifact gates.

### Slice 4 — Normalize agent-turn and LLM trace persistence

Goal: stop treating turn-live and full-response traces as opaque blobs.

Use existing types:

- `AgentTurnTraceRecord`
- `AgentTurnSummaryRecord`
- `AgentTurnArtifactRecord`
- `ObservedTurnEventRecord`
- `RequestMessageRecord`
- `ProviderToolCallRecord`
- `ToolRequestRecord`
- `ToolCompletedRecord`
- `ToolFailedRecord`
- `RawFullResponseRecord`

Relations to add or populate:

- `eval_agent_turn`
- `eval_agent_turn_event`
- `eval_model_exchange`
- `eval_message_event`
- `eval_tool_event`
- `eval_edit_event` where proposal/apply data is available
- `eval_cost_event` when usage/cost data is available
- `eval_log_ref` / `eval_blob_ref` for raw provider payload refs with hashes/sensitivity

Rules:

- Preserve raw full-response JSONL as out-of-line evidence initially.
- DB rows should provide ordered, queryable conversation/tool timeline.
- Include tool calls in both prompt/request messages and response/tool event rows.
- Do not inline full provider payloads without explicit sensitivity policy.

Verification:

- parse existing turn-live bundle into rows;
- query events in source order;
- count tool requested/completed/failed rows;
- raw response hashes match sidecar lines;
- replay tests that consume files still pass while DB queries are added.

### Slice 5 — Parent-visible child evidence via Channel/import rows

Goal: align DB model with the future remote-runtime architecture.

Tasks:

- On parent read of child channel messages, write/import parent-owned facts:
  - validated message row;
  - receipt row;
  - import row;
  - derived attempt/evaluation/result summaries when applicable.
- Terminal result facts in the parent DB should come from `ToParent::Result` or explicitly imported channel refs, not direct shared child result path reads.
- Keep compatibility reads only for reconstruction paths that are explicitly marked as such.

Verification:

- terminal `ToParent::Result` message is mirrored/imported in DB;
- parent-visible DB facts include source runtime, target scope, validation status, and content hashes;
- corrupt/missing channel envelope still fails even when DB rows exist.

### Slice 6 — Child-plan MessageBox DB model, not replacement

Goal: make child-plan payload/query facts available without erasing lock/unlock authority.

Tasks:

- Add `eval_message_box` / `eval_message_box_event` rows or equivalent.
- Populate after lock/unlock transitions:
  - box kind;
  - lock transition;
  - unlock transition;
  - backing kind/ref;
  - payload hash/ref;
  - event outcome.
- Optional payload mirror after `Received<ChildPlan>` may populate candidate set/member rows.
- Do not allow DB rows to construct `Received<ChildPlan>` unless a future dedicated backing returns `Locked<ChildPlan>` and passes existing receiver validation.

Verification:

- lock/unlock events queryable;
- wrong/missing child-plan file still fails in current file-backed MessageBox;
- DB payload row cannot bypass `ChildPlanFiles::validate_receiver`.

### Slice 7 — Ordinary DB read migration for reconstruction/audit

Goal: begin removing shared-filesystem assumptions from consumers where it is safe.

Migrate only ordinary eval evidence consumers first:

- audit/report queries;
- candidate/evaluation/selection summaries;
- baseline summaries;
- trace/log/turn timeline inspection;
- parent-visible channel/import summaries.

Do not migrate these as generic DB-only runtime authorities yet:

- Channel transport;
- MessageBox receipt;
- History/Crown;
- invocation/bootstrap execution;
- artifact/worktree mutation;
- transition-journal replay if the replay contract still requires JSONL.

Verification:

- audit can answer semantic questions from DB rows where available;
- missing DB row in dual-strict is reported as failure/degraded for that surface;
- missing protected-domain file still fails the protected gate.

### Slice 8 — History refs now, HistoryStore later

Goal: make History queryable without making eval DB rows authority.

Immediate:

- Add/populate `eval_history_ref` rows for sealed block refs used by handoff/audit.
- Include block hash, height, lineage, selected runtime/artifact, store ref.
- Keep `FsBlockStore` authority for live successor handoff.

Later, only after explicit approval:

- implement `CozoBlockStore` or `HistoryStore` behind `BlockStore`;
- store raw block and entry JSON preimages;
- preserve `verify_hash`, `entries_root`, `HistoryStateRoot`, stale-head checks, and append transaction semantics;
- cross-store tests compare FS vs Cozo block hashes and roots.

Verification:

- DB history ref helps audit but cannot make successor startup pass without sealed History authority;
- existing handoff negative tests continue passing.

### Slice 9 — Codegraph overlay preparation

Goal: prepare for same-DB joins without merging child-local graphs into parent scope.

Tasks:

- Record parent codegraph snapshot identity when available.
- Add `eval_code_snapshot`, `eval_code_ref`, `eval_code_link`, and `eval_code_touch` only after ordinary evidence rows are stable.
- Link edits/patches/tool calls/validations to code refs with artifact/snapshot scope.
- Child-local codegraph facts remain child-local unless imported with artifact/runtime/hash context.

Verification:

- query “which tool call touched which path/code ref”;
- no base codegraph relation is mutated by eval overlay writes;
- selected successor must reindex/inherit/validate overlays under its artifact before relying on them.

### Slice 10 — Two-generation dual-strict live canary

Goal: prove the local loop works across parent/successor handoff with strict DB persistence.

Preconditions:

- profile uses `storage.eval.backend = "dual-strict"`;
- provider credentials and live opt-ins are present;
- strict surface list from Slice 3 is implemented;
- audit can report expected file/DB status.

Suggested command shape to record exactly when run:

```text
PLOKE_EVAL_LIVE_API_TESTS=1 \
PLOKE_RUN_LIVE_TESTS=1 \
<provider env> \
cargo run -p ploke-eval -- loop walk ... --profile <dual-strict-profile>
```

The exact command depends on current CLI/profile options and should be copied from the working live-run procedure at execution time, not guessed in this doc.

Required evidence:

- campaign id;
- profile path and SHA-256;
- provider/model/route;
- first parent generation start/end;
- selected successor runtime id and pid;
- successor ready acknowledgement;
- second parent generation start/end;
- final outcome/completion boundary;
- `loop walk audit --format json --verbose --with-note` output;
- DB relation counts and high-signal semantic checks;
- notes for any file-only protected surfaces.

Pass criteria:

- live loop completes two generations;
- no dual-strict DB write silently degrades to warning;
- no protected-domain validation is weakened;
- audit shows expected rows for live path surfaces;
- History handoff and successor startup validation succeed through existing hard gates.

## Test command families

Use sub-agents for test execution per repository instruction.

Local/focused:

```text
cargo test -p ploke-eval prototype1_eval_store
cargo test -p ploke-eval prototype1_storage_authority_negative
cargo test -p ploke-eval prototype1_transition_contract_r4c_to_r5
cargo test -p ploke-eval prototype1_eval_store_record_ref
cargo test -p ploke-eval prototype1_eval_store_channel
cargo test -p ploke-eval prototype1_eval_store_runtime_streams
```

Provider/live gates remain explicit and env-gated:

```text
PLOKE_EVAL_LIVE_API_TESTS=1 PLOKE_RUN_LIVE_TESTS=1 \
cargo test -p ploke-eval --features live_api_tests -- --ignored prototype1_live_transition
```

Full two-generation canary command must be documented in `implementation-log.md` after selecting a concrete run profile.

## GitNexus / editing workflow for implementers

Before editing Rust symbols, run impact analysis and record blast radius in the handoff/summary, e.g.:

```text
npx gitnexus impact -r /home/brasides/code/ploke <symbol>
```

Likely high-impact symbols:

- `EvalStore`
- `ConfiguredEvalStore`
- `write_*_to_owner_db`
- `r4c_to_r5`
- `r5_to_r6`
- `Channel::recv_from_child`
- `mirror_parent_channel_imports`
- `write_broad_headless_tui_turn_live_bundle`
- `spawn_and_handoff_prototype1_successor`
- `validate_prototype1_successor_continuation`

Before any commit, run:

```text
npx gitnexus detect-changes -r /home/brasides/code/ploke
git status --short
git diff --stat
```

## Current uncommitted-work warning

At the time this document was written, the working tree already had uncommitted edits in:

- `AGENTS.md`
- `CLAUDE.md`
- `crates/ploke-eval/src/cli/prototype1_state/eval_store/setup.rs`
- `crates/ploke-eval/src/cli/prototype1_state/live_edges.rs`
- `crates/ploke-eval/src/cli/prototype1_state/profile.rs`
- `crates/ploke-eval/src/cli/prototype1_state/run/core.rs`

Future implementation should inspect and preserve those changes unless the operator says they are ours to modify.

## Stop conditions

Stop and ask before implementing if a proposed change would:

- make DB rows authoritative for History, Channel, MessageBox, invocation/bootstrap, or artifact/worktree mutation without a domain-specific port and negative tests;
- weaken validation, hash checks, state-root checks, surface checks, or receiver checks;
- silently tolerate missing expected DB evidence in `dual-strict`;
- merge child-local codegraph facts into the parent DB by default;
- require treating provider-facing tests as successful without real provider calls;
- require destructive fixture/backup DB updates without following fixture-registry rules.

## Open questions

1. Should the immediate owner eval DB remain a backup-file-backed in-memory Cozo at `prototype1/eval-store.cozo.sqlite`, or should the loop obtain a long-lived `ploke_db::Database` handle that is also the parent codegraph DB?
2. Which live profile should be the canonical two-generation `dual-strict` canary profile?
3. Which surfaces are strict requirements for the first canary, and which are intentionally file-only protected domains reported by audit?
4. Should child-plan DB support stop at box/event/query rows for now, or should a `MessageBoxBacking` trait be introduced before DB-backed read work?
5. How much raw LLM/provider payload should be referenced vs imported into DB, given sensitivity and size?
