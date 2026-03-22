# Workspace Acceptance Criteria 2026-03-20

Backlink:
[2026-03-20_workspaces_implementation_plan.md](/home/brasides/code/ploke/docs/active/reports/2026-03-20_workspaces_implementation_plan.md)

This document is the exhaustive acceptance-criteria companion for the workspace
implementation plan. It records:

- the concrete properties that must be shown for each readiness item or phase
- the new or changed data structures implied by the plan
- invariants and cross-crate contracts
- failure states to catch
- required fixtures
- already present tests that partially validate the area
- properties that are not presently provable, with reasons

## Method and limits

The criteria below are intentionally phrased as disprovable statements.

They are engineering acceptance obligations backed by direct tests, fixtures,
and explicit runtime evidence. They are not presented as a formal proof in the
Howard-Curry sense that every future execution of the system is correct.

The plan's "Phase 1: Fixture and ingestion baseline" is the first positive
evidence phase after readiness. The runtime phases remain numbered 2-8 to stay
aligned with the implementation plan.

Two important corrections to example properties from the todo:

1. The current schema does not create explicit edges from crate nodes to a
   workspace node. `workspace_metadata` stores a `members` list field instead;
   see
   [crates/ingest/ploke-transform/src/schema/crate_node.rs#L17](/home/brasides/code/ploke/crates/ingest/ploke-transform/src/schema/crate_node.rs#L17)
   and
   [crates/ingest/ploke-transform/src/transform/workspace.rs#L60](/home/brasides/code/ploke/crates/ingest/ploke-transform/src/transform/workspace.rs#L60).
   The direct evidence target for the current plan is therefore
   membership-list consistency, not graph-edge existence.
2. Exact global cross-crate ranking is not fully provable in general for the
   workspace search rollout because dense retrieval uses embeddings plus
   approximate HNSW search; see
   [crates/ploke-rag/src/core/mod.rs#L489](/home/brasides/code/ploke/crates/ploke-rag/src/core/mod.rs#L489)
   and
   [crates/ploke-db/src/multi_embedding/hnsw_ext.rs#L337](/home/brasides/code/ploke/crates/ploke-db/src/multi_embedding/hnsw_ext.rs#L337).
   The correct acceptance target is scope correctness plus expected hits on
   controlled fixtures, not universal ranking proofs.

## Definitions and predicates

Unless a later phase states a narrower witness, the following terms are used in
the precise senses below.

- `loaded workspace identity`: `WorkspaceId::from_root_path(...)` over the
  canonicalized workspace root path. `WorkspaceId::from_root_path(...)` and
  `WorkspaceInfo::from_root_path(...)` both derive identity from
  `canonicalize_best_effort(path)`; see
  [crates/ploke-core/src/workspace.rs#L28](/home/brasides/code/ploke/crates/ploke-core/src/workspace.rs#L28),
  [crates/ploke-core/src/workspace.rs#L39](/home/brasides/code/ploke/crates/ploke-core/src/workspace.rs#L39),
  and
  [crates/ploke-core/src/workspace.rs#L80](/home/brasides/code/ploke/crates/ploke-core/src/workspace.rs#L80).
- `runtime member set`: the canonical runtime projection of loaded membership.
  In current code the only runtime carrier is `SystemStatus.workspace_roots` and
  its `crates` vector, while `crate_focus` selects one member from that set; see
  [crates/ploke-tui/src/app_state/core.rs#L383](/home/brasides/code/ploke/crates/ploke-tui/src/app_state/core.rs#L383),
  [crates/ploke-tui/src/app_state/core.rs#L425](/home/brasides/code/ploke/crates/ploke-tui/src/app_state/core.rs#L425),
  and
  [crates/ploke-core/src/workspace.rs#L97](/home/brasides/code/ploke/crates/ploke-core/src/workspace.rs#L97).
  Phase 2 may replace the carrier with `LoadedWorkspaceState.crates`, but the
  canonical projection remains the set of loaded crate root paths in runtime
  state, not `crate_focus` alone.
- `snapshot member set`: the member roots represented by
  `workspace_metadata.members` and by restored `crate_context.root_path` rows
  for the loaded snapshot. `try_parse_manifest(...)` converts manifest members
  to workspace-root-joined paths, and `transform_workspace_metadata(...)`
  persists those paths verbatim; see
  [crates/ingest/syn_parser/src/discovery/workspace.rs#L203](/home/brasides/code/ploke/crates/ingest/syn_parser/src/discovery/workspace.rs#L203),
  [crates/ingest/ploke-transform/src/transform/workspace.rs#L60](/home/brasides/code/ploke/crates/ingest/ploke-transform/src/transform/workspace.rs#L60),
  and
  [docs/active/agents/2026-03-workspaces/save-load-update-correctness-survey.md#L12](/home/brasides/code/ploke/docs/active/agents/2026-03-workspaces/save-load-update-correctness-survey.md#L12).
- `member list canonicalized`: for this document, canonicalization means the
  workspace-root-joined member paths produced by parser discovery and persisted
  by transform. Membership-equality checks compare those canonical root paths as
  a set; order-sensitive persistence checks may additionally assert that the
  manifest order is preserved because the transform layer iterates parser output
  directly. It does not mean caller-side sorting or deduplication; see
  [crates/ingest/syn_parser/src/discovery/workspace.rs#L210](/home/brasides/code/ploke/crates/ingest/syn_parser/src/discovery/workspace.rs#L210),
  [crates/ingest/ploke-transform/src/transform/workspace.rs#L66](/home/brasides/code/ploke/crates/ingest/ploke-transform/src/transform/workspace.rs#L66),
  and
  [crates/ingest/syn_parser/src/discovery/workspace.rs#L395](/home/brasides/code/ploke/crates/ingest/syn_parser/src/discovery/workspace.rs#L395).
- `authoritative membership`: a phase-relative rule.
  - during `/index`, parsed target output is authoritative for loaded
    membership: crate-target indexing may authoritatively load one crate, while
    workspace-target indexing uses parsed workspace membership from
    `parse_workspace(...)`; see
    [crates/ingest/syn_parser/src/lib.rs#L66](/home/brasides/code/ploke/crates/ingest/syn_parser/src/lib.rs#L66)
    and
    [docs/active/reports/2026-03-20_workspaces_implementation_plan.md#L88](/home/brasides/code/ploke/docs/active/reports/2026-03-20_workspaces_implementation_plan.md#L88)
  - during `/load`, restored snapshot/DB metadata is authoritative and the
    registry is only a locator for the snapshot; see
    [docs/active/reports/2026-03-20_workspaces_implementation_plan.md#L298](/home/brasides/code/ploke/docs/active/reports/2026-03-20_workspaces_implementation_plan.md#L298)
    and
    [docs/active/agents/2026-03-workspaces/save-load-update-correctness-survey.md#L24](/home/brasides/code/ploke/docs/active/agents/2026-03-workspaces/save-load-update-correctness-survey.md#L24)
  - during `/workspace status`, `/workspace update`, and subset commands, the
    loaded snapshot member set remains authoritative while live manifest
    inspection is used only to detect drift; see
    [docs/active/agents/2026-03-workspaces/save-load-update-correctness-survey.md#L25](/home/brasides/code/ploke/docs/active/agents/2026-03-workspaces/save-load-update-correctness-survey.md#L25)
    and
    [docs/active/reports/2026-03-20_workspaces_implementation_plan.md#L347](/home/brasides/code/ploke/docs/active/reports/2026-03-20_workspaces_implementation_plan.md#L347)
  - neither `current_dir()` nor `crate_focus` is authoritative for membership;
    see
    [crates/ploke-tui/src/app_state/handlers/indexing.rs#L52](/home/brasides/code/ploke/crates/ploke-tui/src/app_state/handlers/indexing.rs#L52)
    and
    [crates/ploke-tui/src/app_state/core.rs#L425](/home/brasides/code/ploke/crates/ploke-tui/src/app_state/core.rs#L425)
- `coherent session state` / `same loaded dataset`: the tuple
  `(workspace_id, workspace_root, member_roots, active_embedding_set,
  hnsw_registration(active_embedding_set), bm25_status, focused_crate,
  io_roots)` published after a successful mutating command. Workspace identity
  and member roots come from loaded workspace state plus snapshot metadata;
  active embedding state is restored via `restore_embedding_set(...)`; HNSW
  registration is checked by `is_hnsw_index_registered(...)`; BM25 readiness is
  exposed by `Bm25Status`; focus and IO roots come from `SystemStatus` and
  `derive_path_policy(...)`; see
  [crates/ploke-db/src/database.rs#L974](/home/brasides/code/ploke/crates/ploke-db/src/database.rs#L974),
  [crates/ploke-db/src/multi_embedding/hnsw_ext.rs#L448](/home/brasides/code/ploke/crates/ploke-db/src/multi_embedding/hnsw_ext.rs#L448),
  [crates/ploke-db/src/bm25_index/bm25_service.rs#L9](/home/brasides/code/ploke/crates/ploke-db/src/bm25_index/bm25_service.rs#L9),
  [crates/ploke-tui/src/app_state/core.rs#L500](/home/brasides/code/ploke/crates/ploke-tui/src/app_state/core.rs#L500),
  and
  [crates/ploke-tui/src/app_state/database.rs#L234](/home/brasides/code/ploke/crates/ploke-tui/src/app_state/database.rs#L234).
- `registered HNSW state`: the active embedding set's HNSW relation exists in
  Cozo and passes `::indices`, as implemented by
  `is_hnsw_index_registered(...)`; see
  [crates/ploke-db/src/multi_embedding/hnsw_ext.rs#L448](/home/brasides/code/ploke/crates/ploke-db/src/multi_embedding/hnsw_ext.rs#L448).
- `ready BM25 state`: `Bm25Status::Ready { docs }` with `docs > 0`; see
  [crates/ploke-db/src/bm25_index/bm25_service.rs#L9](/home/brasides/code/ploke/crates/ploke-db/src/bm25_index/bm25_service.rs#L9).
- `explicitly reported unavailable`: a user-visible or runtime-visible state in
  which search readiness is not claimed. In current code this includes BM25
  non-ready states (`Uninitialized`, `Building`, `Empty`, `Error(...)`),
  strict-BM25 errors such as `"bm25 index not ready"` and `"bm25 index empty"`,
  and `/load` surfacing `"embedding searches will be unavailable"` when no
  populated embedding set can be restored; see
  [crates/ploke-rag/src/core/mod.rs#L421](/home/brasides/code/ploke/crates/ploke-rag/src/core/mod.rs#L421),
  [crates/ploke-rag/src/core/mod.rs#L471](/home/brasides/code/ploke/crates/ploke-rag/src/core/mod.rs#L471),
  [crates/ploke-db/src/bm25_index/bm25_service.rs#L9](/home/brasides/code/ploke/crates/ploke-db/src/bm25_index/bm25_service.rs#L9),
  and
  [crates/ploke-tui/src/app_state/database.rs#L392](/home/brasides/code/ploke/crates/ploke-tui/src/app_state/database.rs#L392).
  For HNSW, the acceptance meaning is the same: missing registration or
  search-path errors must surface as unavailable rather than being reported as
  ready; see
  [crates/ploke-db/src/index/hnsw.rs#L35](/home/brasides/code/ploke/crates/ploke-db/src/index/hnsw.rs#L35),
  [crates/ploke-db/src/index/hnsw.rs#L291](/home/brasides/code/ploke/crates/ploke-db/src/index/hnsw.rs#L291),
  and
  [docs/active/agents/2026-03-workspaces/db-rag-acceptance-survey.md#L25](/home/brasides/code/ploke/docs/active/agents/2026-03-workspaces/db-rag-acceptance-survey.md#L25).
- `canonical plain workspace backup fixture`: for this acceptance document, this
  means the same fixture class as the current plain backup fixtures: plain
  backup import, no embedding-model contract assumed, and primary HNSW index
  created after import by the caller. This is an explicit acceptance definition
  inferred from the current fixture taxonomy; see
  [docs/testing/BACKUP_DB_FIXTURES.md#L73](/home/brasides/code/ploke/docs/testing/BACKUP_DB_FIXTURES.md#L73)
  and
  [docs/testing/BACKUP_DB_FIXTURES.md#L133](/home/brasides/code/ploke/docs/testing/BACKUP_DB_FIXTURES.md#L133).
- `current BM25 fallback semantics are preserved`: lenient BM25 search retries
  on `Uninitialized`/`Building`, falls back to dense search when BM25 is not
  ready or empty, and does not fall back when `Ready { docs > 0 }` returns zero
  hits. Strict BM25 search returns explicit errors for `Uninitialized`,
  `Building`, `Empty`, or `Error`, and returns an empty result only when BM25 is
  `Ready`; see
  [crates/ploke-rag/src/core/mod.rs#L229](/home/brasides/code/ploke/crates/ploke-rag/src/core/mod.rs#L229),
  [crates/ploke-rag/src/core/mod.rs#L296](/home/brasides/code/ploke/crates/ploke-rag/src/core/mod.rs#L296),
  [crates/ploke-rag/src/core/mod.rs#L310](/home/brasides/code/ploke/crates/ploke-rag/src/core/mod.rs#L310),
  [crates/ploke-rag/src/core/mod.rs#L421](/home/brasides/code/ploke/crates/ploke-rag/src/core/mod.rs#L421),
  and
  [crates/ploke-rag/src/core/mod.rs#L471](/home/brasides/code/ploke/crates/ploke-rag/src/core/mod.rs#L471).

## Global G1: successful workspace commands preserve one coherent session state

Why this exists:

- component-local tests do not imply that TUI state, DB contents, index state,
  and IO policy still describe the same loaded workspace after a mutating
  command succeeds

New or changed structures:

- no new single production type is required, but the plan assumes a coherent
  relation among loaded workspace state, `workspace_metadata`, `crate_context`,
  active embedding metadata, HNSW state, BM25 state, and IO roots

Required invariants:

- after any successful workspace-mutating command (`/index`, `/load`,
  `/workspace update`, and later subset commands), there exists one loaded
  workspace identity and one coherent session tuple, as defined above, such
  that:
  - loaded workspace membership in TUI state matches the canonical runtime
    member set
  - `workspace_metadata.members` and restored `crate_context.root_path` rows
    describe that same loaded member set
  - active embedding-set metadata, registered HNSW state for the active set, and
    BM25 readiness all describe that same loaded dataset, or are explicitly
    reported unavailable
  - focused crate and IO roots are members or subpaths of that loaded workspace
- failure paths leave the previous coherent state intact; they do not publish a
  mixed success state

Cross-crate contracts:

- `ploke-tui` command handlers, `ploke-db` metadata restore, and
  `ploke-embed`/BM25 finalization must agree on when a workspace transition is
  committed

Failure states:

- focus or IO roots change before parse/index success and remain changed after
  failure
- DB rows are updated for one workspace while active embedding metadata, HNSW,
  or BM25 still point at the previous dataset
- BM25 is reported ready for a batch whose corresponding embedding commit did
  not complete

Existing relevant code/tests:

- current `/index` flow mutates focus and roots before parse success; see
  [crates/ploke-tui/src/app_state/handlers/indexing.rs#L52](/home/brasides/code/ploke/crates/ploke-tui/src/app_state/handlers/indexing.rs#L52)
  and
  [crates/ploke-tui/src/app_state/handlers/indexing.rs#L70](/home/brasides/code/ploke/crates/ploke-tui/src/app_state/handlers/indexing.rs#L70)
- current path policy is still focus-derived; see
  [crates/ploke-tui/src/app_state/core.rs#L500](/home/brasides/code/ploke/crates/ploke-tui/src/app_state/core.rs#L500)
- current runtime membership is represented only by `workspace_roots` plus one
  `crate_focus`; see
  [crates/ploke-tui/src/app_state/core.rs#L383](/home/brasides/code/ploke/crates/ploke-tui/src/app_state/core.rs#L383)
  and
  [crates/ploke-tui/src/app_state/core.rs#L438](/home/brasides/code/ploke/crates/ploke-tui/src/app_state/core.rs#L438)
- current `/load` restores one active embedding set, recreates HNSW for that
  set, and then resets focus and IO roots from one `crate_context` row; see
  [crates/ploke-tui/src/app_state/database.rs#L290](/home/brasides/code/ploke/crates/ploke-tui/src/app_state/database.rs#L290),
  [crates/ploke-tui/src/app_state/database.rs#L392](/home/brasides/code/ploke/crates/ploke-tui/src/app_state/database.rs#L392),
  and
  [crates/ploke-tui/src/app_state/database.rs#L411](/home/brasides/code/ploke/crates/ploke-tui/src/app_state/database.rs#L411)
- HNSW registration is currently relation-level, not workspace-state-level; see
  [crates/ploke-db/src/multi_embedding/hnsw_ext.rs#L448](/home/brasides/code/ploke/crates/ploke-db/src/multi_embedding/hnsw_ext.rs#L448)
- BM25 readiness is currently exposed through `Bm25Status`; see
  [crates/ploke-db/src/bm25_index/bm25_service.rs#L9](/home/brasides/code/ploke/crates/ploke-db/src/bm25_index/bm25_service.rs#L9)
- BM25 seeding/finalization is not obviously atomic with DB embedding commit in
  the current indexer path; see
  [crates/ingest/ploke-embed/src/indexer/mod.rs#L550](/home/brasides/code/ploke/crates/ingest/ploke-embed/src/indexer/mod.rs#L550)
  and
  [crates/ingest/ploke-embed/src/indexer/mod.rs#L770](/home/brasides/code/ploke/crates/ingest/ploke-embed/src/indexer/mod.rs#L770)

Untestable or not yet provable:

- current code does not yet expose a single assertion point proving whole-session
  coherence across state, DB, and search services

Acceptance statement:

- no later phase passes merely by proving one subsystem in isolation; each
  successful mutating command must preserve one coherent loaded-workspace state
  across TUI state, DB metadata, search state, and IO policy

## Global G2: membership authority and manifest drift are explicit

Why this exists:

- later save/load/status/update phases are only meaningful if "loaded workspace
  membership" has one authoritative source and if drift is surfaced instead of
  silently absorbed

New or changed structures:

- registry-backed workspace snapshot metadata
- loaded workspace membership in TUI state
- fixture-backed assertions comparing `workspace_metadata.members` with
  restored `crate_context` rows

Required invariants:

- during `/index`, parsed workspace output is authoritative for membership;
  during `/load`, restored snapshot metadata is authoritative and registry data
  is only the snapshot locator
- workspace membership is established from parsed or restored workspace
  snapshot data, not from `current_dir()` and not from a single focused-crate
  lookup
- `workspace_metadata.members` agrees with the canonical root-path set of
  restored `crate_context` rows for the loaded workspace snapshot
- manifest drift means that the live manifest's canonical member set differs
  from the authoritative loaded snapshot member set
- if the live manifest later adds or removes members relative to the loaded
  snapshot, the system reports drift or requires re-index; it does not silently
  merge, drop, or reuse a stale member set

Cross-crate contracts:

- `syn_parser`, `ploke-transform`, backup/restore, and `ploke-tui` runtime
  state must preserve the same workspace identity and member set

Failure states:

- only the focused crate is restored or reported as if it were the whole
  workspace
- a snapshot restores successfully even though `workspace_metadata.members` and
  `crate_context` disagree
- added or removed manifest members are ignored during status/update/load

Existing relevant code/tests:

- the persisted membership record today is `workspace_metadata.members`; see
  [crates/ingest/ploke-transform/src/transform/workspace.rs#L60](/home/brasides/code/ploke/crates/ingest/ploke-transform/src/transform/workspace.rs#L60)
- current `scan_for_change(...)` and focused-crate restore paths still derive
  behavior from one crate at a time; see
  [crates/ploke-tui/src/app_state/database.rs#L499](/home/brasides/code/ploke/crates/ploke-tui/src/app_state/database.rs#L499)
  and
  [crates/ploke-tui/src/app_state/database.rs#L407](/home/brasides/code/ploke/crates/ploke-tui/src/app_state/database.rs#L407)

Untestable or not yet provable:

- current tests do not yet prove member-set equality between restored workspace
  metadata and all persisted crate rows on a committed multi-member fixture

Acceptance statement:

- later phases are not accepted until workspace membership has one authoritative
  path from parse/restore into runtime state and any manifest drift is surfaced
  explicitly

## Readiness R1: committed multi-member workspace fixture exists

Why this exists:

- current committed fixture coverage is too shallow for workspace-member
  behavior

New or changed structures:

- a new committed fixture under `tests/fixture_workspace/`
- at least two member crates
- at least one nested member path

Required invariant:

- the fixture manifest is a real Cargo workspace on disk with normalized member
  paths that `parse_workspace(...)` and `locate_workspace_manifest(...)` can
  consume directly

Cross-crate contracts:

- `syn_parser` manifest discovery and parser tests must be able to consume the
  same fixture without ad hoc tempdir generation

Failure states:

- only single-member workspaces are represented
- nested member paths are absent
- fixture is present but not buildable enough for parsing

Required fixtures:

- a new multi-member committed workspace fixture

Existing relevant code/tests:

- `parse_workspace(...)` already validates member selection and aggregates
  parsing across members; see
  [crates/ingest/syn_parser/src/lib.rs#L66](/home/brasides/code/ploke/crates/ingest/syn_parser/src/lib.rs#L66)
- parser tests currently cover multi-member behavior only through tempdirs; see
  [crates/ingest/syn_parser/src/lib.rs#L392](/home/brasides/code/ploke/crates/ingest/syn_parser/src/lib.rs#L392)
- manifest discovery already normalizes workspace members and nested paths; see
  [crates/ingest/syn_parser/src/discovery/workspace.rs#L186](/home/brasides/code/ploke/crates/ingest/syn_parser/src/discovery/workspace.rs#L186)
  and
  [crates/ingest/syn_parser/src/discovery/workspace.rs#L395](/home/brasides/code/ploke/crates/ingest/syn_parser/src/discovery/workspace.rs#L395)
- the only committed workspace fixture with assertions today is single-member
  `ws_fixture_00`; see
  [tests/fixture_workspace/ws_fixture_00/Cargo.toml#L1](/home/brasides/code/ploke/tests/fixture_workspace/ws_fixture_00/Cargo.toml#L1)
  and
  [crates/ingest/syn_parser/src/discovery/mod.rs#L447](/home/brasides/code/ploke/crates/ingest/syn_parser/src/discovery/mod.rs#L447)

Untestable or not yet provable:

- workspace-member behavior on committed fixtures is not yet provable for
  multiple members because no such committed fixture is registered today

Acceptance statement:

- Phase 1 readiness is not met until the repo contains at least one committed
  multi-member workspace fixture that is used by parser/discovery tests.

## Readiness R2: registered workspace backup fixture exists

Why this exists:

- workspace save/load and workspace-scoped retrieval need schema-coupled backup
  fixtures that contain workspace metadata

New or changed structures:

- new `FixtureDb` registry entry or entries for workspace backups in
  [crates/test-utils/src/fixture_dbs.rs](/home/brasides/code/ploke/crates/test-utils/src/fixture_dbs.rs)
- corresponding inventory entry in
  [docs/testing/BACKUP_DB_FIXTURES.md](/home/brasides/code/ploke/docs/testing/BACKUP_DB_FIXTURES.md)

Required invariants:

- workspace backup fixtures load through the same strict registry-backed paths
  as existing fixtures
- no permissive import behavior is added to compensate for stale schema

Cross-crate contracts:

- `ploke-db`, `ploke-rag`, and `ploke-tui` tests must be able to consume the new
  fixture through shared registry constants and documented import modes

Failure states:

- workspace backup exists on disk but is not registered
- fixture doc inventory is stale relative to registry
- workspace fixture only works via ad hoc load path

Required fixtures:

- canonical plain workspace backup fixture
- local-embeddings workspace backup fixture if search tests will depend on a
  pre-embedded snapshot

Existing relevant code/tests:

- current registry entries are crate-centric only; see
  [crates/test-utils/src/fixture_dbs.rs#L145](/home/brasides/code/ploke/crates/test-utils/src/fixture_dbs.rs#L145)
- current fixture inventory is crate-centric only; see
  [docs/testing/BACKUP_DB_FIXTURES.md#L63](/home/brasides/code/ploke/docs/testing/BACKUP_DB_FIXTURES.md#L63)
- update instructions already require registry and doc updates together; see
  [docs/testing/BACKUP_DB_FIXTURES.md#L162](/home/brasides/code/ploke/docs/testing/BACKUP_DB_FIXTURES.md#L162)

Untestable or not yet provable:

- workspace backup roundtrip behavior is not yet provable on registered
  fixtures because no active workspace fixture is registered today

Acceptance statement:

- Phase 1 readiness is not met until at least one workspace backup fixture is
  registered, documented, and verifiable via `cargo xtask verify-backup-dbs`.

## Readiness R3: `workspace_metadata` transform is asserted, not only smoke-tested

Why this exists:

- the current transform tests do not prove that persisted workspace metadata is
  correct

New or changed structures:

- no production structure change required
- new transform assertions against `workspace_metadata`

Required invariants:

- persisted `workspace_metadata.id` and `.namespace` are derived from
  `WorkspaceId::from_root_path(...)`
- `root_path`, `members`, `exclude`, `resolver`, and `package_version` in the
  relation match the parsed manifest

Cross-crate contracts:

- `syn_parser` workspace output must remain compatible with
  `ploke-transform` workspace insert logic

Failure states:

- transform returns `Ok(())` but omits or miswrites workspace metadata fields
- member list is not canonicalized as workspace-root-joined member paths
- package version inheritance is lost

Required fixtures:

- tempdir fixture is sufficient for the assertion test
- committed multi-member fixture is preferred for regression coverage

Existing relevant code/tests:

- `transform_parsed_workspace(...)` inserts workspace metadata before crate
  graphs; see
  [crates/ingest/ploke-transform/src/transform/workspace.rs#L14](/home/brasides/code/ploke/crates/ingest/ploke-transform/src/transform/workspace.rs#L14)
- `process_workspace_metadata(...)` shows the exact relation fields that should
  be asserted; see
  [crates/ingest/ploke-transform/src/transform/workspace.rs#L60](/home/brasides/code/ploke/crates/ingest/ploke-transform/src/transform/workspace.rs#L60)
- current tests only prove transform success; see
  [crates/ingest/ploke-transform/src/transform/workspace.rs#L140](/home/brasides/code/ploke/crates/ingest/ploke-transform/src/transform/workspace.rs#L140)
- deterministic `WorkspaceId` derivation already exists; see
  [crates/ploke-core/src/workspace.rs#L28](/home/brasides/code/ploke/crates/ploke-core/src/workspace.rs#L28)

Untestable or not yet provable:

- there is currently no direct test proving that every crate-context row in the
  transformed DB is represented in `workspace_metadata.members`

Acceptance statement:

- readiness is not met until a transform test queries `workspace_metadata` and
  proves field correctness, not only transform success.

## Readiness R4: strict backup verification passes for workspace fixtures

Why this exists:

- workspace work cannot begin on a backup foundation that only works under
  relaxed import assumptions

Required invariants:

- backup import remains strict
- schema drift is handled by regeneration or the documented repair path, not by
  silent tolerance

Failure states:

- import silently tolerates missing `workspace_metadata`
- fixture regeneration is skipped in favor of permissive loading

Required fixtures:

- the new workspace backup fixture set

Existing relevant code/tests:

- backup-fixture policy explicitly requires strictness; see
  [docs/testing/BACKUP_DB_FIXTURES.md#L162](/home/brasides/code/ploke/docs/testing/BACKUP_DB_FIXTURES.md#L162)
- repo guardrails say not to weaken import semantics without approval; see
  [AGENTS.md](/home/brasides/code/ploke/AGENTS.md)

Untestable or not yet provable:

- no workspace fixture exists yet to run through the current verification path

Acceptance statement:

- Phase 1 readiness is not met until the workspace fixture set passes
  `cargo xtask verify-backup-dbs`.

## Phase 1 C0: workspace snapshot coherence has fixture-backed witness evidence

Why this exists:

- later runtime phases assume that one workspace snapshot represents one
  concrete member set and one corresponding set of persisted crate rows

New or changed data structures:

- no new runtime structure is required beyond the registered workspace fixture
- new fixture-backed assertions comparing `workspace_metadata` with restored
  crate rows

Required invariants:

- on at least one committed multi-member workspace fixture,
  `workspace_metadata.members` equals the loaded member set represented by
  restored `crate_context` rows
- restored workspace identity and root path agree with the
  `WorkspaceId::from_root_path(...)` derivation used during transform
- backup/restore does not change the workspace member set represented by the
  snapshot

Cross-crate contracts:

- `syn_parser` member selection, `ploke-transform` workspace insertion, and
  registry-backed backup/restore preserve the same workspace identity and
  member set

Failure states:

- `workspace_metadata.members` lists a crate with no corresponding persisted
  crate row
- a persisted crate row exists for the loaded workspace but is omitted from
  `workspace_metadata.members`
- backup/restore changes workspace identity or member-set contents

Required fixtures:

- registered multi-member workspace backup fixture
- committed multi-member workspace source fixture

Existing relevant code/tests:

- workspace metadata is inserted before crate graphs; see
  [crates/ingest/ploke-transform/src/transform/workspace.rs#L14](/home/brasides/code/ploke/crates/ingest/ploke-transform/src/transform/workspace.rs#L14)
- current transform tests still stop at smoke coverage; see
  [crates/ingest/ploke-transform/src/transform/workspace.rs#L140](/home/brasides/code/ploke/crates/ingest/ploke-transform/src/transform/workspace.rs#L140)
- backup verification already provides the strict path on which the fixture must
  travel; see
  [docs/testing/BACKUP_DB_FIXTURES.md#L162](/home/brasides/code/ploke/docs/testing/BACKUP_DB_FIXTURES.md#L162)

Untestable or not yet provable:

- the repo does not yet contain a fixture-backed equality assertion between
  `workspace_metadata.members` and restored multi-member crate rows
- one witness fixture does not by itself prove transform/restore correctness for
  all possible workspaces; it is regression evidence for the intended
  transform/restore path

Acceptance statement:

- Phase 1 passes only when committed fixture tests provide direct witness
  evidence that one registered workspace snapshot round-trips into mutually
  consistent `workspace_metadata` and `crate_context` membership data.
- This witness is necessary regression evidence for the planned
  transform/restore path; it is not, by itself, a universal proof over all
  possible workspaces.

## Phase 2 C1: loaded workspace state is first-class in `ploke-tui`

Why this exists:

- current `SystemStatus` models one focused crate, not one loaded workspace with
  many crates

New or changed data structures:

- add `LoadedWorkspaceState`
- add `LoadedCrateState`
- keep `crate_focus`, but scope it to loaded-workspace membership rather than
  treating it as the only source of truth

Required invariants:

- workspace identity is derived from canonical root path using
  `WorkspaceId::from_root_path(...)`
- focused crate is either absent or belongs to the loaded workspace
- per-crate root path and namespace are retained in state
- path policy can derive read roots from loaded workspace membership, not only
  focused crate root
- loaded workspace membership is derived from parsed/restored workspace state,
  not inferred ad hoc from `current_dir()` or a single focused crate

Cross-crate contracts:

- `ploke-core` identity helpers remain the source of truth for crate/workspace
  IDs; see
  [crates/ploke-core/src/workspace.rs#L7](/home/brasides/code/ploke/crates/ploke-core/src/workspace.rs#L7)
- `ploke-tui` IO policy must stay synchronized with loaded workspace state; see
  [crates/ploke-tui/src/app_state/core.rs#L500](/home/brasides/code/ploke/crates/ploke-tui/src/app_state/core.rs#L500)

Failure states:

- focused crate points outside the loaded workspace
- workspace state updates but IO roots remain on the old crate
- loaded workspace exists but members are absent from state
- loaded membership is silently reconstructed from focus-only state

Required fixtures:

- tempdir workspace fixture is enough for state-model tests
- committed multi-member fixture is preferred for integration tests

Existing relevant code/tests:

- current `SystemStatus` only stores `workspace_roots` plus one `crate_focus`;
  see
  [crates/ploke-tui/src/app_state/core.rs#L383](/home/brasides/code/ploke/crates/ploke-tui/src/app_state/core.rs#L383)
- `derive_path_policy(...)` currently uses only `focused_crate_root()`; see
  [crates/ploke-tui/src/app_state/core.rs#L500](/home/brasides/code/ploke/crates/ploke-tui/src/app_state/core.rs#L500)
- `load_db_crate_focus` proves absolute root paths from DB are used directly;
  see
  [crates/ploke-tui/tests/load_db_crate_focus.rs#L10](/home/brasides/code/ploke/crates/ploke-tui/tests/load_db_crate_focus.rs#L10)
- `no_workspace_fallback` proves current no-workspace behavior is a fallback tip,
  not true workspace state; see
  [crates/ploke-tui/tests/no_workspace_fallback.rs#L28](/home/brasides/code/ploke/crates/ploke-tui/tests/no_workspace_fallback.rs#L28)

Untestable or not yet provable:

- current code has no workspace-state structure to assert against

Acceptance statement:

- Phase 2 passes only when TUI state can represent one workspace with multiple
  crates, a focused crate that is guaranteed to be one of those members, and a
  member set that is not inferred from focus alone.

## Phase 3 C2: manifest-driven indexing resolves the explicit Cargo target

Why this exists:

- current indexing still risks mixing command target resolution, loaded-state
  authority, and crate-only parse orchestration

New or changed data structures:

- structured command variants for workspace indexing
- workspace-aware indexing handler result/state updates

Required invariants:

- bare `/index` indexes the exact crate root when pwd is a crate root;
  otherwise it resolves the nearest ancestor workspace manifest
- `/index <path>` follows the same rule: exact crate root first, otherwise
  nearest ancestor workspace
- repo-relative or other relative targets must not be silently re-resolved from
  process cwd when loaded app state already supplies the authoritative absolute
  target
- non-Cargo targets fail explicitly with recoverable guidance
- successful indexing records the loaded workspace and its member crates from
  parsed metadata, or records the single loaded crate when the resolved target
  is crate-scoped
- successful indexing publishes focus, IO roots, active embedding state, and
  any ready BM25 state atomically for that loaded workspace
- failed indexing leaves the previously coherent workspace state in place
- strict `WorkspaceSelectionMismatch` behavior from `syn_parser` is preserved

Cross-crate contracts:

- `ploke-tui` calls `parse_workspace(...)` and
  `transform_parsed_workspace(...)`; it does not reimplement workspace member
  discovery independently

Failure states:

- only one crate is indexed from a multi-member workspace
- non-Cargo directories are silently accepted as valid indexing roots
- state still records only the focused crate after success
- IO roots stay on the previous focus after indexing
- `/index <path>` ignores the explicit target or re-resolves it from process
  cwd instead of app-state authority
- focus or IO roots mutate before parse success and are not rolled back
- BM25 readiness advances for a workspace whose corresponding embedding/DB
  commit did not finish

Required fixtures:

- committed multi-member workspace fixture
- optional local-embedding workspace fixture for full end-to-end indexing tests

Existing relevant code/tests:

- `resolve_index_target(...)` already distinguishes crate-root, ancestor-
  workspace, and no-target cases; see
  [crates/ploke-tui/src/parser.rs#L43](/home/brasides/code/ploke/crates/ploke-tui/src/parser.rs#L43)
- current `index_workspace(...)` resolves a target, parses it, and publishes
  loaded state only after parse success; see
  [crates/ploke-tui/src/app_state/handlers/indexing.rs#L36](/home/brasides/code/ploke/crates/ploke-tui/src/app_state/handlers/indexing.rs#L36)
- current command surface still exposes legacy `index start [directory]`; see
  [crates/ploke-tui/src/app/commands/mod.rs#L18](/home/brasides/code/ploke/crates/ploke-tui/src/app/commands/mod.rs#L18)
  and
  [crates/ploke-tui/src/app/commands/exec.rs#L852](/home/brasides/code/ploke/crates/ploke-tui/src/app/commands/exec.rs#L852)
- parser/discovery behavior already supports workspace manifests and selection
  mismatch failure; see
  [crates/ingest/syn_parser/src/lib.rs#L94](/home/brasides/code/ploke/crates/ingest/syn_parser/src/lib.rs#L94)

Untestable or not yet provable:

- `C2` now has direct committed-fixture handler witnesses, but stronger
  whole-session coherence evidence across embedding/HNSW/BM25 still belongs to
  later phases and global `G1`

Acceptance statement:

- Phase 3 passes only when `/index` and `/index <path>` both resolve the
  intended Cargo target explicitly, commit loaded crate/workspace state
  atomically on success, and fail loudly on invalid targets without publishing
  partial state.

## Phase 4 C3: workspace status and update are per-crate, not focus-only

Why this exists:

- current change detection is hard-coded to one focused crate

New or changed data structures:

- per-crate stale/fresh status inside loaded workspace state
- command result payloads for `/workspace status` and `/workspace update`

Required invariants:

- every loaded crate appears in status output
- stale detection is computed from DB file metadata and real file state for each
  loaded crate
- update converges stale crates back to fresh state without dropping unchanged
  embeddings
- status/update re-compare the loaded member set against current manifest
  membership
- added or removed members are surfaced as drift or require re-index; they are
  not silently ignored

Cross-crate contracts:

- `ploke-tui` change detection depends on `ploke-db::get_crate_files(...)`; see
  [crates/ploke-db/src/database.rs#L390](/home/brasides/code/ploke/crates/ploke-db/src/database.rs#L390)

Failure states:

- status reports only the focused crate
- fresh crates are marked stale or stale crates are missed
- update clears embeddings for untouched crates
- update leaves stale markers behind after success
- added or removed workspace members are silently merged, dropped, or ignored

Required fixtures:

- multi-member workspace fixture with at least one changed crate and one
  unchanged crate
- local-embedding workspace backup fixture if update tests restore from backup

Existing relevant code/tests:

- `scan_for_change(...)` currently errors without focus and scans only one
  crate; see
  [crates/ploke-tui/src/app_state/database.rs#L499](/home/brasides/code/ploke/crates/ploke-tui/src/app_state/database.rs#L499)
- `test_update_embed` already proves the single-crate path can detect changes,
  retract changed embeddings, and reindex them; see
  [crates/ploke-tui/src/app_state/database.rs#L1476](/home/brasides/code/ploke/crates/ploke-tui/src/app_state/database.rs#L1476)

Untestable or not yet provable:

- current test suite has no multi-crate stale-detection or per-crate status
  assertions

Acceptance statement:

- Phase 4 passes only when status/update behavior is defined and tested over all
  loaded crates, not just the focused one, and workspace-member drift is
  surfaced explicitly.

## Phase 5 C4: workspace save/load uses registry-backed workspace identity

Why this exists:

- earlier save/load was keyed by focused crate name and prefix-based file
  lookup, which was insufficient for workspace identity and explicit mismatch
  handling

New or changed data structures:

- workspace registry manifest or registry file
- workspace snapshot metadata including member crates and active embedding set
- explicit runtime search-availability markers when restored HNSW/BM25 state is
  not ready

Required invariants:

- `/save db` persists both snapshot data and registry metadata
- `/load <workspace>` resolves by workspace identity, not prefix-only file name
- restore updates DB contents, active embedding set, HNSW registration or
  explicit HNSW unavailability, BM25 readiness or explicit BM25 unavailability,
  workspace state, focus, and IO roots together
- registry chooses the workspace snapshot to restore, but restored runtime
  membership comes from workspace snapshot/DB metadata rather than from one
  focused-crate lookup
- after restore, `workspace_metadata.members` and restored `crate_context` rows
  agree for the loaded workspace
- active embedding-set metadata must round-trip from workspace snapshot metadata;
  the legacy crate-backup fallback to the first populated set is not sufficient
  acceptance evidence for workspace restore
- disagreement between registry metadata and restored snapshot metadata fails
  explicitly

Cross-crate contracts:

- DB backup/import remains whole-DB in this phase
- active embedding set metadata survives backup/restore; see
  [crates/ploke-db/src/database.rs#L2383](/home/brasides/code/ploke/crates/ploke-db/src/database.rs#L2383)
  and
  [crates/ploke-db/src/database.rs#L2438](/home/brasides/code/ploke/crates/ploke-db/src/database.rs#L2438)
- HNSW registration after restore is currently recreated per active embedding
  set; see
  [crates/ploke-tui/src/app_state/database.rs#L392](/home/brasides/code/ploke/crates/ploke-tui/src/app_state/database.rs#L392)
- BM25 readiness is separate actor state and must therefore be restored or
  marked unavailable explicitly; see
  [crates/ploke-db/src/bm25_index/bm25_service.rs#L9](/home/brasides/code/ploke/crates/ploke-db/src/bm25_index/bm25_service.rs#L9)

Failure states:

- registry points at a missing snapshot
- snapshot exists without matching registry state
- wrong backup is chosen because names share a prefix
- DB loads but TUI state and IO roots still point at the previous workspace
- DB loads while HNSW or BM25 still reflect the previous dataset, or are not
  marked unavailable explicitly
- load succeeds while restored workspace membership and persisted crate rows
  disagree
- load succeeds by silently using the legacy `FirstPopulated` embedding-set
  fallback instead of authoritative workspace snapshot metadata
- stale registry metadata is silently preferred over restored snapshot metadata

Required fixtures:

- registered workspace backup fixture
- save/load integration fixture exercising non-default embedding-set metadata

Existing relevant code/tests:

- `save_db(...)` now writes a registry-backed workspace snapshot keyed by
  `WorkspaceInfo::from_root_path(...)` identity rather than by focused crate
  name; see
  [crates/ploke-tui/src/app_state/database.rs#L513](/home/brasides/code/ploke/crates/ploke-tui/src/app_state/database.rs#L513)
- `load_db(...)` now resolves exact workspace registry entries, validates the
  restored snapshot against registry metadata, and rejects legacy
  `FirstPopulated` fallback for workspace restore; see
  [crates/ploke-tui/src/app_state/database.rs#L723](/home/brasides/code/ploke/crates/ploke-tui/src/app_state/database.rs#L723)
- restore currently recreates HNSW for the restored active set, reports
  embedding-search unavailability when no active set is restored, reports BM25
  unavailability explicitly, and hydrates loaded workspace membership from the
  restored snapshot metadata; see
  [crates/ploke-tui/src/app_state/database.rs#L809](/home/brasides/code/ploke/crates/ploke-tui/src/app_state/database.rs#L809),
  [crates/ploke-tui/src/app_state/database.rs#L915](/home/brasides/code/ploke/crates/ploke-tui/src/app_state/database.rs#L915),
  and
  [crates/ploke-tui/src/app_state/database.rs#L939](/home/brasides/code/ploke/crates/ploke-tui/src/app_state/database.rs#L939)
- `restore_embedding_set(...)` still permits legacy `FirstPopulated` fallback;
  see
  [crates/ploke-db/src/database.rs#L974](/home/brasides/code/ploke/crates/ploke-db/src/database.rs#L974)
- `load_db_restores_saved_embedding_set_and_index`,
  `load_db_requires_workspace_registry_entry_instead_of_prefix_lookup`,
  `load_db_rejects_first_populated_embedding_fallback_for_workspace_registry_loads`,
  and `load_db_fails_when_registry_metadata_disagrees_with_restored_snapshot`
  now provide direct witness coverage for the core `C4` restore invariants; see
  [crates/ploke-tui/src/app_state/database.rs#L1649](/home/brasides/code/ploke/crates/ploke-tui/src/app_state/database.rs#L1649)
- BM25 status is actor state and is not restored by `load_db(...)` today; see
  [crates/ploke-db/src/bm25_index/bm25_service.rs#L9](/home/brasides/code/ploke/crates/ploke-db/src/bm25_index/bm25_service.rs#L9)
  and
  [docs/active/agents/2026-03-workspaces/db-rag-acceptance-survey.md#L27](/home/brasides/code/ploke/docs/active/agents/2026-03-workspaces/db-rag-acceptance-survey.md#L27)

Untestable or not yet provable:

- whole-session `G1` atomicity across DB mutation, runtime state publication,
  HNSW registration, BM25 availability, and IO roots is still not provable from
  one assertion point in current code

Acceptance statement:

- Phase 5 passes only when workspace save/load is registry-backed and no longer
  depends on prefix-based crate backup discovery, with restored membership
  reconstructed from consistent snapshot metadata rather than focus-only state.
- The accepted restore path must leave HNSW and BM25 coherent with the restored
  dataset, or mark them unavailable explicitly; it may not silently accept
  legacy embedding-set fallback as workspace-correct behavior.

## Phase 6 C5: dense, BM25, and hybrid retrieval share one scope model

Why this exists:

- current retrieval entrypoints are unscoped and operate over the full loaded DB

New or changed data structures:

- retrieval scope type, such as `LoadedWorkspace | SpecificCrate(CrateId)`
- scoped DB query helpers for dense and BM25 search

Required invariants:

- `search`, `search_bm25`, `search_bm25_strict`, `hybrid_search`, and
  `get_context` all accept the same scope model
- optional crate restriction narrows results to one crate namespace
- scope is enforced before any dense `:limit`, BM25 `top_k`, dense fallback, or
  RRF fusion truncation
- context assembly only materializes IDs already admitted by scope-filtered
  retrieval
- existing BM25 fallback/retry semantics are preserved after scoping

Cross-crate contracts:

- `RagService` must pass scope through to `ploke-db` helpers rather than
  post-filtering snippets late
- caller-side filtering after candidate truncation is not sufficient to satisfy
  the acceptance claim
- DB search results must continue projecting `namespace` so tests can assert
  scope correctness; see
  [crates/ploke-db/src/multi_embedding/hnsw_ext.rs#L390](/home/brasides/code/ploke/crates/ploke-db/src/multi_embedding/hnsw_ext.rs#L390)

Failure states:

- dense search leaks hits from crates outside the requested scope
- BM25 search leaks hits outside scope or changes fallback semantics
- hybrid search combines differently scoped result sets
- context assembly reintroduces out-of-scope IDs
- scope is applied only after dense `:limit`, BM25 `top_k`, fallback, or RRF
  fusion

Required fixtures:

- local-embedding multi-member workspace fixture
- at least two crates with intentionally overlapping symbol names or nearby
  lexical content so scope leaks are detectable

Existing relevant code/tests:

- current BM25 search has retry/fallback semantics and no scope argument; see
  [crates/ploke-rag/src/core/mod.rs#L234](/home/brasides/code/ploke/crates/ploke-rag/src/core/mod.rs#L234)
- current dense search has no scope parameter and fans out across all primary
  node types; see
  [crates/ploke-rag/src/core/mod.rs#L489](/home/brasides/code/ploke/crates/ploke-rag/src/core/mod.rs#L489)
- current `get_context(...)` assumes retrieval already selected the right IDs;
  see
  [crates/ploke-rag/src/core/mod.rs#L547](/home/brasides/code/ploke/crates/ploke-rag/src/core/mod.rs#L547)
  and
  [crates/ploke-rag/src/context/mod.rs#L164](/home/brasides/code/ploke/crates/ploke-rag/src/context/mod.rs#L164)
- dense DB search is embedding-set scoped, not workspace scoped; see
  [crates/ploke-db/src/multi_embedding/hnsw_ext.rs#L337](/home/brasides/code/ploke/crates/ploke-db/src/multi_embedding/hnsw_ext.rs#L337)
- BM25 actor commands currently carry only `query` and `top_k`; see
  [crates/ploke-db/src/bm25_index/bm25_service.rs#L19](/home/brasides/code/ploke/crates/ploke-db/src/bm25_index/bm25_service.rs#L19)
- existing search tests already cover dense, BM25, fallback, and hybrid search
  behavior on crate fixtures; see
  [crates/ploke-rag/src/core/unit_tests.rs#L237](/home/brasides/code/ploke/crates/ploke-rag/src/core/unit_tests.rs#L237)
  and
  [crates/ploke-rag/src/core/unit_tests.rs#L324](/home/brasides/code/ploke/crates/ploke-rag/src/core/unit_tests.rs#L324)
- existing DB tests already assert dense neighbor behavior and namespace-bearing
  file data; see
  [crates/ploke-db/src/multi_embedding/hnsw_ext.rs#L926](/home/brasides/code/ploke/crates/ploke-db/src/multi_embedding/hnsw_ext.rs#L926)
  and
  [crates/ploke-db/src/database.rs#L1795](/home/brasides/code/ploke/crates/ploke-db/src/database.rs#L1795)

Untestable or not yet provable:

- exact ranking relationships across crates are not universally provable because
  HNSW is approximate and embedding models may change
- the acceptable proof target is inclusion/exclusion by scope plus expected hit
  presence on controlled fixtures

Acceptance statement:

- Phase 6 passes only when all retrieval modes share one scope model and tests
  prove namespace-accurate inclusion/exclusion on multi-crate fixtures, with
  scope enforced before candidate truncation in every retrieval path.

## Phase 7 C6: namespace-scoped subset export/import/remove exists before crate-subset commands

Why this exists:

- `/load crates ...` and `/workspace rm <crate>` are not safe on top of
  whole-database backup/restore

New or changed data structures:

- DB-level crate-subset export artifact format or API
- namespace-scoped import/remove operations
- conflict-report structure for duplicate names, roots, or namespaces
- loaded-workspace membership update result covering focus, IO roots, and
  snapshot metadata after subset mutation

Required invariants:

- subset import never silently replaces an existing loaded namespace
- subset removal removes all rows for the target namespace
- subset import/remove updates authoritative membership end-to-end: runtime
  loaded-workspace state, `workspace_metadata.members`, restored/remaining
  `crate_context` rows, focus, IO roots, and any persisted registry/snapshot
  metadata all describe the same post-mutation member set
- if the focused crate is removed, focus is moved to another loaded member or
  cleared explicitly before IO roots are published
- HNSW and BM25 state are reconciled after subset mutation, or are explicitly
  marked unavailable for the mutated dataset

Cross-crate contracts:

- TUI conflict validation depends on DB operations that can inspect and mutate
  namespaces safely
- subset commands must reuse the same membership-authority rule as `/load` and
  `/workspace update`; they may not introduce a second source of membership
  truth

Failure states:

- crate-subset load is implemented as whole-DB replace
- removal leaves dangling namespace data
- `workspace_metadata.members` or runtime membership still contains a removed
  crate after subset mutation
- focus or IO roots still point at a removed crate, or widen to roots outside
  the post-mutation workspace
- registry/snapshot metadata still describes the pre-mutation member set after a
  successful subset command
- index state is stale after subset mutation

Required fixtures:

- workspace fixture with multiple crates
- per-crate backup fixtures or exportable subset artifacts derived from that
  workspace

Existing relevant code/tests:

- current load path is whole-DB import only; see
  [crates/ploke-db/src/multi_embedding/db_ext.rs#L1169](/home/brasides/code/ploke/crates/ploke-db/src/multi_embedding/db_ext.rs#L1169)
- existing retract tests prove some index cleanup behavior but not namespace
  export/import/remove semantics; see
  [crates/ploke-db/src/database.rs#L2285](/home/brasides/code/ploke/crates/ploke-db/src/database.rs#L2285)
- current TUI command/state surveys already identify `/load crates ...` and
  `/workspace rm <crate>` as the motivating surface, and they call out the need
  to avoid dangling focus and IO roots; see
  [docs/active/agents/2026-03-workspaces/tui-phase-plan-survey.md#L29](/home/brasides/code/ploke/docs/active/agents/2026-03-workspaces/tui-phase-plan-survey.md#L29)
  and
  [docs/active/reports/2026-03-20_workspaces_implementation_plan.md#L347](/home/brasides/code/ploke/docs/active/reports/2026-03-20_workspaces_implementation_plan.md#L347)
- current `/load` already demonstrates that successful workspace mutation must
  update focus plus IO roots together, even though it does so for only one crate
  today; see
  [crates/ploke-tui/src/app_state/database.rs#L411](/home/brasides/code/ploke/crates/ploke-tui/src/app_state/database.rs#L411)

Untestable or not yet provable:

- crate-subset merge correctness is not provable today because the required DB
  primitives do not exist

Acceptance statement:

- Phase 7 passes only when crate-subset commands are implemented on
  namespace-scoped DB primitives with explicit conflict validation.
- The accepted subset path must also update authoritative membership,
  focus/IO policy, and search-state availability end to end; namespace mutation
  alone is not sufficient.

## Phase 8 C7: workspace-aware tools preserve strict edit safety

Why this exists:

- widening retrieval scope is not allowed to widen write permissions implicitly

New or changed data structures:

- workspace-aware prompt/context state
- explicit edit target resolution carrying crate/file identity

Required invariants:

- read tools may operate on loaded-workspace scope
- edit tools require explicit resolved crate/file targets before writing
- ambiguous symbol hits across crates surface as disambiguation, not silent
  choice

Cross-crate contracts:

- tool calls depend on retrieval returning namespace/file-aware results
- exact item resolution continues to use canonical file/module path helpers; see
  [crates/ploke-db/src/helpers.rs#L13](/home/brasides/code/ploke/crates/ploke-db/src/helpers.rs#L13)

Failure states:

- edit tool writes outside the loaded workspace
- read tool or edit tool uses stale focused-crate-only assumptions after
  workspace loading
- ambiguous cross-crate symbol names result in best-guess edits

Required fixtures:

- multi-member workspace fixture with symbol-name collisions across crates

Existing relevant code/tests:

- current prompt and tools are focused-crate-centric; see
  [crates/ploke-tui/src/llm/manager/mod.rs](/home/brasides/code/ploke/crates/ploke-tui/src/llm/manager/mod.rs),
  [crates/ploke-tui/src/rag/context.rs](/home/brasides/code/ploke/crates/ploke-tui/src/rag/context.rs),
  [crates/ploke-tui/src/tools/ns_read.rs](/home/brasides/code/ploke/crates/ploke-tui/src/tools/ns_read.rs),
  and
  [crates/ploke-tui/src/tools/code_edit.rs](/home/brasides/code/ploke/crates/ploke-tui/src/tools/code_edit.rs)
- exact path/module resolution helpers already exist in `ploke-db`; see
  [crates/ploke-db/src/helpers.rs#L13](/home/brasides/code/ploke/crates/ploke-db/src/helpers.rs#L13)

Untestable or not yet provable:

- LLM disambiguation quality is not fully provable as a semantic property
- the provable property is that ambiguous or out-of-scope edits are rejected or
  require explicit target resolution before mutation

Acceptance statement:

- Phase 8 passes only when workspace-aware retrieval does not weaken write-path
  safety and edit ambiguity is surfaced explicitly.

## Deterministic identity: what is provable now and what is not

Provable now:

- crate and workspace IDs are deterministic functions of canonical root paths;
  see
  [crates/ploke-core/src/workspace.rs#L19](/home/brasides/code/ploke/crates/ploke-core/src/workspace.rs#L19)
  and
  [crates/ploke-core/src/workspace.rs#L39](/home/brasides/code/ploke/crates/ploke-core/src/workspace.rs#L39)
- current tests prove unique tracking hashes for surveyed common-node sets, not
  universal node IDs; see
  [crates/ploke-db/src/database.rs#L1893](/home/brasides/code/ploke/crates/ploke-db/src/database.rs#L1893)

Not yet provable from the current survey:

- the stronger statement "all items in the database have a deterministically
  generated, unique identifier" is not established by the current workspace
  code/tests
- if that property becomes a formal requirement, it needs a separate audit and
  dedicated tests across all persisted node kinds

## Completion rule

The workspace plan is not accepted merely because implementation compiles. It is
accepted when:

- all readiness items above are satisfied
- each implemented phase satisfies its acceptance statement
- the global coherence and membership-authority propositions above continue to
  hold after every successful mutating command
- each new fixture is registry-backed and documented
- each direct-evidence target above is covered by direct tests or explicit
  fixture verification
- each limit or non-provable property remains documented as such, with the
  reason preserved in this file
