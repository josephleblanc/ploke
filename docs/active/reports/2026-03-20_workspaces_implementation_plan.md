# Workspaces Implementation Plan 2026-03-20

This plan supersedes the earlier draft in response to `(2026-03-20/3)` in
[docs/active/todo/2026-03-20_workspaces.md](/home/brasides/code/ploke/docs/active/todo/2026-03-20_workspaces.md).

It follows:

- [2026-03-20_workspaces_report.md](/home/brasides/code/ploke/docs/active/reports/2026-03-20_workspaces_report.md)
- [ingestion-pipeline-survey.md](/home/brasides/code/ploke/docs/active/agents/2026-03-workspaces/ingestion-pipeline-survey.md)
- [2026-03-20_db-rag-workspace-survey.md](/home/brasides/code/ploke/docs/active/reports/2026-03-20_db-rag-workspace-survey.md)
- [tui-workspace-survey.md](/home/brasides/code/ploke/docs/active/reports/tui-workspace-survey.md)
- [phase1-readiness-ingest-fixtures.md](/home/brasides/code/ploke/docs/active/agents/2026-03-workspaces/phase1-readiness-ingest-fixtures.md)
- [db-rag-acceptance-survey.md](/home/brasides/code/ploke/docs/active/agents/2026-03-workspaces/db-rag-acceptance-survey.md)
- [tui-phase-plan-survey.md](/home/brasides/code/ploke/docs/active/agents/2026-03-workspaces/tui-phase-plan-survey.md)
- [indexing-state-coherence-survey.md](/home/brasides/code/ploke/docs/active/agents/2026-03-workspaces/indexing-state-coherence-survey.md)
- [retrieval-scope-proof-survey.md](/home/brasides/code/ploke/docs/active/agents/2026-03-workspaces/retrieval-scope-proof-survey.md)
- [save-load-update-correctness-survey.md](/home/brasides/code/ploke/docs/active/agents/2026-03-workspaces/save-load-update-correctness-survey.md)

The exhaustive companion acceptance document is:

- [2026-03-20_workspaces_acceptance_criteria.md](/home/brasides/code/ploke/docs/active/reports/2026-03-20_workspaces_acceptance_criteria.md)

Current implementation status should be tracked in:

- [2026-03-20_workspaces_progress_tracker.md](/home/brasides/code/ploke/docs/active/agents/2026-03-workspaces/2026-03-20_workspaces_progress_tracker.md)

Update that tracker whenever a readiness item or phase changes status. Keep it
as the current source of truth for implementation progress, evidence, and next
blocking steps.

## Scope

Target this pass at basic workspace-wide behavior in `ploke-tui`:

- bare `/index` and `/index <path>` with target resolution that prefers an
  exact crate root and otherwise falls back to the nearest ancestor workspace
- transform/import of all workspace members into one loaded DB
- workspace-wide embeddings and HNSW indexing
- optional BM25 indexing over the loaded workspace
- workspace-aware retrieval by default, with optional crate restriction
- workspace save/load/status/update commands
- safe read/edit tooling inside the loaded workspace

Do not include inter-crate dependency graphs or dependency-link analysis in
this pass.

## Recommended implementation boundary

The first shippable version should still be:

- one loaded workspace per TUI session
- many crates inside that workspace
- one focused crate for ambiguous tool actions
- workspace-wide retrieval by default

This keeps the initial workspace rollout aligned with the current single-DB
runtime and avoids conflating basic workspace support with arbitrary multi-DB
merging.

## Key findings

The ingestion primitives already exist:

- `syn_parser::parse_workspace(...)` validates selected members and returns a
  `ParsedWorkspace`; see
  [crates/ingest/syn_parser/src/lib.rs#L66](/home/brasides/code/ploke/crates/ingest/syn_parser/src/lib.rs#L66).
- workspace manifest discovery already normalizes member and exclude paths; see
  [crates/ingest/syn_parser/src/discovery/workspace.rs#L144](/home/brasides/code/ploke/crates/ingest/syn_parser/src/discovery/workspace.rs#L144).
- `transform_parsed_workspace(...)` already writes `workspace_metadata` and then
  transforms each crate graph; see
  [crates/ingest/ploke-transform/src/transform/workspace.rs#L14](/home/brasides/code/ploke/crates/ingest/ploke-transform/src/transform/workspace.rs#L14).
- full schema creation already includes `WorkspaceMetadataSchema`; see
  [crates/ingest/ploke-transform/src/schema/mod.rs#L84](/home/brasides/code/ploke/crates/ingest/ploke-transform/src/schema/mod.rs#L84)
  and
  [crates/ingest/ploke-transform/src/schema/crate_node.rs#L17](/home/brasides/code/ploke/crates/ingest/ploke-transform/src/schema/crate_node.rs#L17).

The main gaps are:

- fixture readiness for real workspace-member and backup-DB validation
- `ploke-tui` workspace state and manifest-driven indexing orchestration
- scope-aware retrieval contracts in `ploke-rag` and `ploke-db`
- namespace-scoped DB export/import/remove primitives for crate-subset
  operations

The earlier draft put too much implied search complexity near the front of the
plan. This revision separates readiness, workspace indexing/state, and
workspace-scoped retrieval into distinct phases.

The additional acceptance review for `(2026-03-20/4)` found that this phase
ordering is logically coherent, but the earlier criteria were only necessary,
not sufficient, because they left several cross-phase proof obligations
implicit.

## Cross-phase proof obligations

Every phase below now assumes the following global properties.

- Successful workspace-mutating commands preserve one coherent session state
  across TUI loaded-workspace state, DB metadata, active embedding metadata,
  HNSW/BM25 state, and IO roots.
- Workspace membership is authoritative and explicit: it comes from parsed or
  restored workspace snapshot data, not from `current_dir()` or one focused
  crate, and manifest drift is surfaced rather than silently absorbed.
- Retrieval scope for later phases must be enforced before candidate
  truncation/fusion, not by late caller-side filtering after `top_k`.

These obligations are spelled out formally in
[2026-03-20_workspaces_acceptance_criteria.md](/home/brasides/code/ploke/docs/active/reports/2026-03-20_workspaces_acceptance_criteria.md).

## Phase 1 readiness

Phase 1 implementation should not start until the following readiness items are
complete.

### Fixture readiness

1. Add at least one committed multi-member workspace fixture under
   `tests/fixture_workspace/` with two or more members and at least one nested
   member path.
2. Keep `ws_fixture_00` as the minimal inherited-metadata fixture, but do not
   treat it as sufficient coverage for multi-member workspace behavior; it has
   only one member today; see
   [tests/fixture_workspace/ws_fixture_00/Cargo.toml#L1](/home/brasides/code/ploke/tests/fixture_workspace/ws_fixture_00/Cargo.toml#L1).
3. Do not count `ws_fixture_01` as coverage until it is populated; it is empty
   today; see
   [tests/fixture_workspace/ws_fixture_01/Cargo.toml](/home/brasides/code/ploke/tests/fixture_workspace/ws_fixture_01/Cargo.toml).
4. Add a canonical workspace backup fixture to `tests/backup_dbs/` and register
   it in
   [crates/test-utils/src/fixture_dbs.rs](/home/brasides/code/ploke/crates/test-utils/src/fixture_dbs.rs)
   and
   [docs/testing/BACKUP_DB_FIXTURES.md](/home/brasides/code/ploke/docs/testing/BACKUP_DB_FIXTURES.md).
5. Add a local-embeddings workspace backup fixture if the first workspace
   retrieval phase is expected to run on fixtures rather than on ad hoc local
   indexing.

### Behavior readiness

1. Add an assertion-level transform test for `workspace_metadata`; the existing
   transform test only proves the call returns `Ok(())`; see
   [crates/ingest/ploke-transform/src/transform/workspace.rs#L158](/home/brasides/code/ploke/crates/ingest/ploke-transform/src/transform/workspace.rs#L158).
2. Preserve strict backup import semantics. Do not weaken fixture loading to
   tolerate missing relations or schema drift; see
   [docs/testing/BACKUP_DB_FIXTURES.md](/home/brasides/code/ploke/docs/testing/BACKUP_DB_FIXTURES.md)
   and
   [AGENTS.md](/home/brasides/code/ploke/AGENTS.md).
3. `cargo xtask verify-backup-dbs` must pass with the new workspace fixtures
   before workspace indexing work begins.

The detailed readiness criteria are in
[2026-03-20_workspaces_acceptance_criteria.md](/home/brasides/code/ploke/docs/active/reports/2026-03-20_workspaces_acceptance_criteria.md).

## Phase 1: Fixture and ingestion baseline

Goal: make workspace behavior and workspace-snapshot coherence testable on
committed fixtures before changing TUI runtime behavior.

Primary files:

- [crates/ingest/syn_parser/src/lib.rs](/home/brasides/code/ploke/crates/ingest/syn_parser/src/lib.rs)
- [crates/ingest/syn_parser/src/discovery/workspace.rs](/home/brasides/code/ploke/crates/ingest/syn_parser/src/discovery/workspace.rs)
- [crates/ingest/ploke-transform/src/transform/workspace.rs](/home/brasides/code/ploke/crates/ingest/ploke-transform/src/transform/workspace.rs)
- [crates/test-utils/src/fixture_dbs.rs](/home/brasides/code/ploke/crates/test-utils/src/fixture_dbs.rs)
- [docs/testing/BACKUP_DB_FIXTURES.md](/home/brasides/code/ploke/docs/testing/BACKUP_DB_FIXTURES.md)

Deliverables:

- committed multi-member workspace fixture target
- registered canonical workspace backup fixture
- optional local-embedding workspace backup fixture
- transform test that queries `workspace_metadata` contents
- fixture-backed proof that `workspace_metadata.members` and restored
  `crate_context` rows agree for one registered multi-member workspace snapshot

Acceptance summary:

- `parse_workspace(...)` is validated on a committed multi-member workspace, not
  only on tempdir fixtures.
- `transform_parsed_workspace(...)` is validated by asserting persisted
  `workspace_metadata` fields, not only success/failure.
- new workspace fixtures load through the existing strict registry-backed
  fixture path.
- one registered workspace snapshot is proven to round-trip into mutually
  consistent `workspace_metadata` and `crate_context` membership data.

## Phase 2: Explicit loaded-workspace state in `ploke-tui`

Goal: stop treating focused crate and loaded workspace as the same thing.

Primary files:

- [crates/ploke-tui/src/app_state/core.rs](/home/brasides/code/ploke/crates/ploke-tui/src/app_state/core.rs)
- [crates/ploke-core/src/workspace.rs](/home/brasides/code/ploke/crates/ploke-core/src/workspace.rs)

Recommended new structures:

- `LoadedWorkspaceState { workspace_id, name, root_path, crates, focused_crate, active_embedding_set, bm25_state }`
- `LoadedCrateState { crate_id, name, root_path, namespace, version, stale_status, backup_ref }`

Required invariants:

- loaded workspace identity is derived from canonical root path using
  `WorkspaceId::from_root_path(...)`; see
  [crates/ploke-core/src/workspace.rs#L28](/home/brasides/code/ploke/crates/ploke-core/src/workspace.rs#L28).
- focused crate is either `None` or a member of the loaded workspace.
- read roots may cover the loaded workspace, but edit/write resolution still
  requires explicit crate/file targeting.

Acceptance summary:

- TUI state can represent one workspace with multiple crates without losing
  focused-crate behavior.
- path policy derives from loaded workspace state rather than only from
  `focused_crate_root()`.
- command handlers that update loaded workspace state also update IO roots
  atomically.
- loaded workspace membership is not reconstructed from focus-only state.

## Phase 3: Manifest-driven workspace indexing

Goal: replace the current single-directory crate flow with explicit workspace
orchestration.

Primary files:

- [crates/ploke-tui/src/app_state/handlers/indexing.rs](/home/brasides/code/ploke/crates/ploke-tui/src/app_state/handlers/indexing.rs)
- [crates/ploke-tui/src/app/commands/parser.rs](/home/brasides/code/ploke/crates/ploke-tui/src/app/commands/parser.rs)
- [crates/ploke-tui/src/app/commands/mod.rs](/home/brasides/code/ploke/crates/ploke-tui/src/app/commands/mod.rs)
- [crates/ploke-tui/src/app/commands/exec.rs](/home/brasides/code/ploke/crates/ploke-tui/src/app/commands/exec.rs)

Changes:

- add structured workspace indexing commands instead of growing legacy raw
  parsing further
- resolve the requested Cargo target before indexing: exact crate root first,
  otherwise nearest ancestor workspace
- use `parse_workspace(...)`
- use `transform_parsed_workspace(...)`
- record loaded workspace membership from parsed metadata, not from
  `current_dir()`
- publish focus, IO roots, active embedding metadata, and any ready BM25 state
  only when the workspace indexing transition commits successfully
- do not let a failed/cancelled indexing pass leave BM25 ahead of DB embedding
  state

Acceptance summary:

- bare `/index` inside a crate root indexes that crate; otherwise it indexes
  the nearest ancestor workspace.
- `/index <path>` uses the same target rule: exact crate root first, otherwise
  nearest ancestor workspace.
- non-Cargo targets fail explicitly with recoverable guidance.
- TUI state records all loaded member crates after success.
- success is atomic across workspace state, IO roots, and index state; failure
  does not publish partial workspace state.

## Phase 4: Workspace status and update

Goal: make freshness tracking and change detection operate per loaded crate.

Primary files:

- [crates/ploke-tui/src/app_state/database.rs](/home/brasides/code/ploke/crates/ploke-tui/src/app_state/database.rs)
- [crates/ploke-tui/src/app_state/core.rs](/home/brasides/code/ploke/crates/ploke-tui/src/app_state/core.rs)

Changes:

- generalize `scan_for_change(...)` from focused-crate-only behavior to loaded
  workspace membership
- add `/workspace status`
- add `/workspace update`
- report stale/fresh state per member crate
- detect member-set drift relative to the loaded workspace snapshot or current
  manifest and surface it explicitly

Recommended first implementation:

- reparse the whole workspace on update, then rely on existing tracking-hash and
  re-embedding behavior to avoid inventing fine-grained partial-update machinery
  too early

Acceptance summary:

- `/workspace status` reports every loaded crate, not only the focused one.
- `/workspace update` converges stale crates back to a fresh state without
  dropping unchanged embeddings.
- stale detection keys off persisted crate roots and DB file hashes, not
  `current_dir()`.
- added or removed workspace members are reported as drift or require re-index;
  they are not silently ignored.

## Phase 5: Workspace save/load registry with whole-workspace snapshots

Goal: make one loaded workspace restorable from user config.

Primary files:

- [crates/ploke-tui/src/app_state/database.rs](/home/brasides/code/ploke/crates/ploke-tui/src/app_state/database.rs)
- [crates/ploke-tui/src/user_config.rs](/home/brasides/code/ploke/crates/ploke-tui/src/user_config.rs)

Recommended storage layout:

- keep `~/.config/ploke/config.toml` as main config
- keep `~/.config/ploke/data/` as DB backup storage
- add either `~/.config/ploke/workspaces.toml` or `~/.config/ploke/workspaces/`
  for workspace registry state

Recommended first cut:

- `/save db` writes one workspace DB snapshot plus one workspace registry entry
- `/load <workspace>` restores that snapshot and restores workspace state and
  active embedding metadata
- the registry acts as the locator for the snapshot, while restored snapshot/DB
  metadata reconstructs runtime workspace membership

Acceptance summary:

- workspace restore resolves by workspace identity, not crate-name prefix.
- restore updates DB contents, active embedding set, workspace state, focused
  crate, and IO roots together.
- success is not reported if the snapshot exists but registry state is missing,
  or vice versa.
- restored workspace membership comes from consistent snapshot metadata, and a
  registry/snapshot mismatch fails explicitly.

## Phase 6: Workspace-scoped retrieval contracts in `ploke-db` and `ploke-rag`

Goal: make retrieval scope explicit and shared across dense, BM25, and hybrid
search.

Primary files:

- [crates/ploke-rag/src/core/mod.rs](/home/brasides/code/ploke/crates/ploke-rag/src/core/mod.rs)
- [crates/ploke-db/src/multi_embedding/hnsw_ext.rs](/home/brasides/code/ploke/crates/ploke-db/src/multi_embedding/hnsw_ext.rs)
- [crates/ploke-db/src/bm25_index/mod.rs](/home/brasides/code/ploke/crates/ploke-db/src/bm25_index/mod.rs)
- [crates/ploke-db/src/bm25_index/bm25_service.rs](/home/brasides/code/ploke/crates/ploke-db/src/bm25_index/bm25_service.rs)

Changes:

- introduce a shared retrieval-scope model, for example
  `LoadedWorkspace | SpecificCrate(CrateId)`
- thread scope through dense, BM25, hybrid, and context-assembly entrypoints
- add namespace-based filtering in DB search helpers
- enforce scope before dense `:limit`, BM25 `top_k`, fallback, and hybrid
  fusion; do not rely on late caller-side post-filtering
- preserve current failure behavior for BM25 fallback and missing HNSW state

Acceptance summary:

- dense, BM25, and hybrid retrieval accept the same scope model.
- optional crate restriction narrows results to one crate namespace.
- workspace-scoped retrieval does not leak hits from crates outside the loaded
  workspace.
- current BM25 fallback behavior remains unchanged except for added scope
  filtering.
- scope correctness is proved before candidate truncation/fusion, not only
  after returned hits are filtered.

## Phase 7: Namespace-scoped DB subset operations

Goal: support `/load crates ...` and `/workspace rm <crate>` safely.

Primary files:

- [crates/ploke-db/src/database.rs](/home/brasides/code/ploke/crates/ploke-db/src/database.rs)
- [crates/ploke-db/src/multi_embedding/db_ext.rs](/home/brasides/code/ploke/crates/ploke-db/src/multi_embedding/db_ext.rs)

Missing primitives today:

- export one crate namespace from a loaded DB
- import one crate namespace into an already populated DB
- remove one crate namespace from a loaded DB
- reconcile HNSW and BM25 state after subset mutation

Acceptance summary:

- no crate-subset merge occurs without explicit conflict validation.
- remove/load-subset commands operate on namespace-scoped DB primitives, not on
  whole-DB replacement disguised as crate operations.
- index state is rebuilt or refreshed after subset mutation.

## Phase 8: Workspace-aware tools and prompt/context behavior

Goal: allow workspace-wide read/retrieval while preserving strict edit safety.

Primary files:

- [crates/ploke-tui/src/rag/context.rs](/home/brasides/code/ploke/crates/ploke-tui/src/rag/context.rs)
- [crates/ploke-tui/src/llm/manager/mod.rs](/home/brasides/code/ploke/crates/ploke-tui/src/llm/manager/mod.rs)
- [crates/ploke-tui/src/tools/ns_read.rs](/home/brasides/code/ploke/crates/ploke-tui/src/tools/ns_read.rs)
- [crates/ploke-tui/src/tools/code_edit.rs](/home/brasides/code/ploke/crates/ploke-tui/src/tools/code_edit.rs)

Acceptance summary:

- prompts describe the loaded workspace rather than only one focused crate.
- read tooling can operate over workspace scope.
- edit tooling still requires explicit resolved crate/file targets before write.
- ambiguous symbol names across crates surface as explicit disambiguation, not
  silent best-guess edits.

## Command implementation order

The recommended command order is now:

1. Phase 1 readiness and fixture work
2. `/index` and `/index <path>`
3. `/workspace status`
4. `/workspace update`
5. `/save db`
6. `/load <workspace>`
7. workspace-scoped retrieval
8. workspace-aware tool gating
9. `/load crates ...`
10. `/workspace rm <crate>`

## Test strategy

By phase:

- Phase 1: parser/transform/fixture-registry tests on committed workspace
  fixtures, `cargo xtask verify-backup-dbs`, and fixture-backed assertions that
  `workspace_metadata.members` matches restored workspace crate rows
- Phases 2-5: `ploke-tui` unit/integration tests for workspace state,
  non-blocking indexing, save/load, stale detection, and IO-root updates
- Phase 6: fixture-backed `ploke-rag` and `ploke-db` tests proving scope
  filtering for dense, BM25, and hybrid retrieval before per-mode truncation
- Phase 7: `ploke-db` tests for namespace export/import/remove and index
  reconciliation
- Phase 8: TUI tool and prompt tests using multi-member workspaces with
  cross-crate symbol-name collisions

The exhaustive acceptance/test matrix is in
[2026-03-20_workspaces_acceptance_criteria.md](/home/brasides/code/ploke/docs/active/reports/2026-03-20_workspaces_acceptance_criteria.md).

## Guardrails to preserve

- Do not relax workspace member validation in `syn_parser`.
- Do not weaken backup import strictness to make workspace fixtures load.
- Do not keep prefix-based backup lookup for workspace restore.
- Do not let workspace-wide retrieval implicitly widen write/edit permissions.
- Do not implement crate-subset load/remove by silently replacing the whole DB.
- Do not key workspace identity or stale detection off `current_dir()`.

## Bottom line

The revised plan starts with readiness, not runtime changes. The parser and
transform layers are already capable enough for basic workspace ingestion, but
the repo still needs committed workspace fixtures, explicit `workspace_metadata`
assertions, registry-backed backup coverage, and a proof that workspace
snapshots round-trip into consistent membership data before the TUI-side
workspace rollout is safe to implement.

After that, the work breaks cleanly into three tracks:

- `ploke-tui` state and manifest-driven indexing
- workspace-scoped retrieval contracts in `ploke-db` and `ploke-rag`
- later namespace-scoped subset DB primitives for crate-level save/load/remove
