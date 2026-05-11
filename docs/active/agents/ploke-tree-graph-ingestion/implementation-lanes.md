# Implementation Lanes

This file describes how to split work across sub-agents without losing the
single-graph model.

The graph is comprehensive by design. The lane split is not deciding whether a
persisted loop-relevant file "belongs" in the graph; it decides which layer
loads it, which typed shape owns it, and how it is represented: core relation,
evidence, metadata, locator, digest, diagnostic, secondary ordering, or
explicit ambiguity.

Target current emitted run shapes only. Historical drift is recorded in the
persisted surface survey so workers do not rediscover it, but older path
variants do not create migration work. If a file is absent from the current
emitted run shape, treat it as unsupported unless the user explicitly asks for
legacy support.

## Target Shape

Keep one public assembly path:

```rust
impl Graph {
    pub fn from_records(records: &RunRecordSet) -> Self {
        // orchestration only
    }
}
```

Implementation modules may be numerous, but they all mutate one internal
builder and finish into one `Graph`.

```text
crates/ploke-tree/src/
  store/
    mod.rs
    fs.rs
    record_set.rs
    forest.rs
    history.rs
    journal.rs
    passive/
      mod.rs
      branch.rs
      channel.rs
      evaluation.rs
      protocol.rs

  graph/
    mod.rs
    types/
      mod.rs
      history.rs
      selection.rs
      artifact.rs
      runtime.rs
      evidence.rs
      warning.rs
    build/
      mod.rs
      history.rs
      selection.rs
      scheduler.rs
      journal.rs
      passive.rs
      evaluation.rs
      protocol.rs
      patch.rs
      tool.rs
      provider.rs
      database.rs
```

This is a direction, not a requirement to create every file immediately.

## File Families By Lane

Use this table as the default assignment map. If a worker encounters a file not
listed here, it should update the survey or report the missing row before
adding a loader.

| Family or pattern | Lane | Representation target |
|---|---:|---|
| `scheduler.json` | 1, then 4 | loaded run metadata/status evidence |
| `nodes/*/node.json` | 1, then 4 | node/runtime metadata and labels |
| configured `parent_identity.json` | 1, then 4 | parent/opening identity evidence |
| `nodes/*/successor-ready/*.json` | 1, then 4 | successor handoff evidence |
| `nodes/*/successor-completion/*.json` | 1, then 4 | successor completion evidence |
| `history/blocks/segment-*.jsonl` | 1, 2, 3 | core History spine and authority ordering |
| `transition-journal.jsonl` | 1, then 4 | append-only transition evidence, secondary to History |
| `branches.json` | 1, then 4 | branch registry/comparison evidence |
| `nodes/*/channels/*/{parent-to-child,child-to-parent}.jsonl` | 1, then 4 | communication evidence and channel metadata |
| `evaluations/branch-*.json` | 1, 3 | evaluation evidence attached to candidates/branches |
| configured `protocol-artifacts/*.json` | 1, 3, then 5 | protocol evidence; align configured dir with current discovery |
| `nodes/*/runner-request.json` | 5 | runner request evidence; passive owner exists |
| `nodes/*/runner-result.json` | 5 | runner result evidence; passive owner exists |
| `nodes/*/invocations/*.json` | 5 | runtime invocation/handoff boundary evidence |
| `nodes/*/results/*.json` | 5 | result evidence; passive shape unresolved |
| `messages/child-plan/node-*.json` | 5 | passive owner and store loader exist; graph currently attaches summary-only evidence |
| `agent-turn-summary.json`, `agent-turn-trace.json` | 5 | blocked on nested tool UI/error payload typing before passive owner can be added |
| `llm-full-responses.jsonl`, `prototype1_observation_*.jsonl` | 5 | provider/observation evidence after writer/type resolution |
| `record.json.gz` | 5 | replay/provenance metadata or locator |
| `run-profile.toml`, `run-profile.commitment.json` | 5 | passive owner, store loader, and graph metadata evidence exist |
| `history/index/*` | 6 | rebuildability/check metadata derived from History |
| `slice.jsonl`, current projection/debug artifacts | 6 | weak evidence or diagnostics derived from typed inputs |
| `stdout.log`, `stderr.log`, console logs | 6 | log locator/metadata and drilldown surface |
| `nodes/*/bin/ploke-eval` | 6 | binary metadata/digest/provenance |
| worktrees, repo caches, cargo caches, `target/**` | 6 | locator/size/digest metadata only; no recursive ingestion |
| model/provider/target registries and dataset caches | 6 | environment/context metadata only when tied to a run |
| stale older-only files such as `prototype1-loop-trace.json` | unsupported | record in survey only; no loader unless explicitly reauthorized |

## Worker File Ownership

These ownership boundaries are stricter than the lane descriptions. They are
intended for concurrent implementation and review. Workers may read adjacent
files only when needed for exact type references, and should report the line
ranges they used.

| Worker | May edit | Must not edit | Inputs it may ingest into `Graph` |
|---|---|---|---|
| Builder scaffold | `crates/ploke-tree/src/graph/build.rs`; module declarations under `crates/ploke-tree/src/graph/build/**` | `ploke-records`; `ploke-egui`; `ploke-eval`; store loaders | none beyond preserving existing `Graph::from_records(&RunRecordSet)` behavior |
| Graph types | `crates/ploke-tree/src/graph/types.rs`; `crates/ploke-tree/src/graph/types/**` | builder logic; store loaders; `ploke-records` schemas | none; this is domain organization only |
| History builder | `crates/ploke-tree/src/graph/build/history.rs` | `selection.rs`; `passive.rs`; store loaders; `ploke-eval` | `RunRecordSet.history_blocks` and `ploke_records::history::*` |
| Selection builder | `crates/ploke-tree/src/graph/build/selection.rs` | History block loading; passive evidence loading; `ploke-eval` | `SelectionDecisionEntryRecord` payloads already admitted inside History entries |
| Passive evidence / metadata builder | `crates/ploke-tree/src/graph/build/passive.rs`; focused modules such as `scheduler.rs`, `handoff.rs`, `journal.rs`, `channel.rs`, `branch.rs` | filesystem discovery; `ploke-records` schemas unless explicitly assigned | already-loaded `RunRecordSet` / `PassiveEvidence` fields only |
| Missing passive shapes | focused `crates/ploke-records/src/**` modules for the assigned family | `ploke-tree` graph ingestion; `ploke-eval`; renderer code | none; this worker creates passive record owners only |
| Store loader split | `crates/ploke-tree/src/lib.rs`; new `crates/ploke-tree/src/store/**` | graph semantic ingestion; `ploke-records` schemas unless explicitly assigned | loader-only: typed files into `RunRecordSet`, after record ownership is known |

Do not make each source family return its own graph. Source-family modules add
facts to one internal builder that finishes into one `Graph`.

## Lane 1: Store Loader Split

Ownership:

- `crates/ploke-tree/src/lib.rs`
- new `crates/ploke-tree/src/store/**`

Goal:

- move filesystem loading and typed record loading out of `lib.rs`;
- keep public behavior and types stable;
- keep `RunRecordSet` as the loaded-run carrier.
- do this before adding more loader families, because `lib.rs` is already large
  enough that additional loader work will normalize the wrong file boundary.

Current status:

- `store/{mod.rs,record_set.rs,evidence.rs}` exists.
- `FsRunStore`, loader methods, loader errors, sorted file helpers, and
  protocol artifact key helpers have moved to `store/fs.rs`.
- Root re-exports preserve public `ploke_tree::FsRunStore` and
  `ploke_tree::RunRecordSet` paths.
- Store-focused tests still live in `lib.rs`; moving those tests is the next
  mechanical cleanup, not a semantic graph change.

No-goals:

- no graph semantic changes;
- no new source families unless assigned separately.

Verification:

```bash
cargo check -p ploke-tree
```

## Lane 2: Graph Type Split

Ownership:

- `crates/ploke-tree/src/graph/types.rs`
- new `crates/ploke-tree/src/graph/types/**`

Goal:

- split graph domain types by concept: History, selection, artifacts,
  runtimes, evidence, warnings;
- preserve the same public exports initially.

No-goals:

- no loader changes;
- no ingestion behavior changes.

Verification:

```bash
cargo check -p ploke-tree
```

## Lane 3: Graph Builder Split

Ownership:

- `crates/ploke-tree/src/graph/build.rs`
- `crates/ploke-tree/src/graph/build/**`

Goal:

- keep `Graph::from_records(&RunRecordSet)` as the assembly entry point;
- keep one internal builder context;
- move History ingestion, selection ingestion, passive evidence ingestion, and
  warnings/evidence helpers into focused modules.

No-goals:

- no independent mini-graphs;
- no browser or egui projection work.

Verification:

```bash
cargo check -p ploke-tree
```

## Lane 4: Loaded-Not-Ingested Families

Ownership:

- graph builder modules for already-loaded inputs;
- inventory doc updates.

Candidate inputs:

- scheduler records;
- node records;
- parent identity;
- successor ready/completion;
- transition journal.

Goal:

- classify each loaded input as core graph structure, evidence, metadata,
  secondary ordering, diagnostics, or unresolved ambiguity;
- implement one family at a time after the builder split is stable.

No-goals:

- no edits to `ploke-eval`;
- no new persisted schemas unless required by missing typed data.

Verification:

```bash
cargo check -p ploke-tree
```

## Lane 5: Missing Loader Families

Ownership:

- `ploke-records` for passive persisted shapes;
- `ploke-tree` store loaders and graph ingestion;
- inventory doc updates.

Candidate families:

- tool calls/results;
- agent turn trace/summary;
- patch artifacts and MBE packaging;
- provider attempts/retries/timeouts/full responses;
- database context and prompt evidence.

Goal:

- first audit whether the family already has named passive record types;
- add `RunRecordSet` loading only after the passive type boundary is clear;
- then attach to `Graph` as evidence, metadata, locator, digest, secondary
  ordering, diagnostics, or unresolved ambiguity.

Current status notes:

- `messages/child-plan/node-*.json`: passive records, store loading, and
  summary-only graph evidence now exist.
- `run-profile.toml` and `run-profile.commitment.json`: passive records and
  store loading now exist; graph metadata evidence also exists.
- `agent-turn-summary.json` and `agent-turn-trace.json`: blocked before passive
  ownership because `ToolCompletedRecord` / `ToolFailedRecord` UI payloads can
  contain nested LLM retry JSON. Resolve the tool UI/error payload boundary
  before adding an `agent_turn` record module.

Projection follow-up:

- Browser/fine playback still carries bare `membership_id` and one path still
  aligns memberships by vector index. It should preserve
  `{ candidate_set_root, membership_id }` or project directly from
  `ploke-tree::Graph` membership keys before the UI treats membership IDs as
  stable detail keys.

No-goals:

- no live runtime logic;
- no active-loop authority changes;
- no `ploke-eval` edits.

Verification depends on the family, but starts with:

```bash
cargo check -p ploke-records
cargo check -p ploke-tree
```

## Lane 6: Projection Consumers

Ownership:

- browser projection modules;
- later `ploke-egui` graph import/view modules.

Goal:

- derive renderer-facing projections from `ploke-tree::Graph`;
- remove duplicated semantic reconstruction from browser/egui where practical.

No-goals:

- no independent graph source-family ingestion outside `ploke-tree`;
- no visual tuning until the semantic graph source is stable.

Verification:

```bash
cargo check -p ploke-tree-browser
cargo check -p ploke-egui
```
