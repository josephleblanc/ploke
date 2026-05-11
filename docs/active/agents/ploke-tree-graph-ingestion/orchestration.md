# Graph Ingestion Orchestration

This lane uses sub-agents to keep the main thread light while preserving one
semantic target: `ploke-tree::Graph` as the read-side index over
`RunRecordSet`.

The graph is the primary interface for investigating what happened in a loop.
Every persisted loop-relevant surface belongs in the graph boundary eventually:
History, scheduler state, runtime handoffs, channel communication, runner
requests/results, provider attempts, patch/proposal artifacts, logs, binaries,
profiles, and projection artifacts. That does not mean every file becomes a
rendered graph node or edge. Heavy or low-authority surfaces may enter as
typed metadata, evidence locators, digests, summaries, or explicit ambiguity
records.

## Main Thread Role

The main thread owns:

- the current task stack and this coordination packet;
- the semantic invariant that `Graph::from_records(&RunRecordSet)` is the
  assembly boundary;
- assignment of disjoint worker lanes;
- targeted verification of worker claims;
- deciding when a set of changes is commit-ready.

The main thread should not broadly read logs, run directories, generated JSON,
or many source files directly. Use a read-only explorer first, then inspect the
exact paths and line ranges it reports.

## Current Orchestration Shape

The near-term implementation is a multi-worker cleanup around one object:
`ploke-tree::Graph`. The graph should preserve the three related structures
described by Prototype 1 state docs:

- Artifact graph: durable Artifact states connected by patches or derivation
  evidence;
- Runtime derivation graph: which Artifact hydrated each Runtime;
- Operation graph: which Runtime operated over which Artifact to create a
  patch attempt or candidate.

History remains the primary authority-ordering spine. Scheduler, channel,
branch, invocation, result, log, profile, and projection records attach as
evidence, metadata, diagnostics, locators, or explicit ambiguity. They do not
replace History ordering and do not become active-loop authority.

Use up to six sub-agents, but keep roles distinct:

| Worker role | Mode | Write surface | Allowed graph input |
|---|---|---|---|
| Builder scaffold | write | `crates/ploke-tree/src/graph/build.rs`, module declarations under `crates/ploke-tree/src/graph/build/**` | existing `RunRecordSet` only; no new family ingestion |
| Graph types | write | `crates/ploke-tree/src/graph/types.rs`, `crates/ploke-tree/src/graph/types/**` | type organization for graph concepts only |
| History builder | write after scaffold | `crates/ploke-tree/src/graph/build/history.rs` | `RunRecordSet.history_blocks` / `ploke_records::history::*` |
| Selection builder | write after scaffold | `crates/ploke-tree/src/graph/build/selection.rs` | History selection payloads already present in `SealedBlockRecord` entries |
| Passive evidence / metadata builder | write after scaffold | `crates/ploke-tree/src/graph/build/passive.rs` and focused modules such as `scheduler.rs`, `handoff.rs`, `journal.rs` | already-loaded fields on `RunRecordSet` and `PassiveEvidence`; no filesystem discovery |
| Missing passive shapes | write, independent | focused `crates/ploke-records/src/**` modules only | passive schemas for current persisted files that lack record owners; no graph ingestion until the type boundary is clear |

Keep one read-only retainer explorer available when slots permit. Its job is to
answer narrow location questions, identify type definitions, find prior reviews,
and suggest exact line ranges. The retainer must not implement changes, read
large files, or dump run artifacts.

The main thread should periodically:

- close or recycle workers after their report is accepted;
- check file lengths with `wc -l` before assigning follow-up edits;
- prefer a new focused module over extending a file that is becoming a blob;
- run focused checks after each integrated wave;
- make periodic commits when a coherent wave passes checks and review.

## Hard Boundaries

Workers must not edit:

- `crates/ploke-eval/**`
- generated run directories under `/home/brasides/.ploke-eval/**`
- unrelated crates outside their assigned lane

Workers may read `ploke-eval` only when explicitly assigned a read-only
conceptual audit. Current graph ingestion work should avoid reading it unless
the main thread asks for a specific line-range check.

Workers must not:

- add production `serde_json::Value` walking for owned persisted files;
- create new graph-shaped DTOs in `ploke-egui` or browser code;
- make each source family return an independent mini-graph;
- turn scheduler, monitor, browser, or CLI projections into History authority;
- drop a persisted loop-relevant surface from the graph plan merely because it
  is not intended for visual rendering;
- commit directly unless the main thread explicitly delegates that action.

## Deserialization Ownership

There is one deserialization path for Prototype 1 run files:

```text
disk files
  -> ploke-records typed records
  -> ploke-tree::store / FsRunStore loads RunRecordSet
  -> ploke-tree::Graph::from_records(&RunRecordSet)
  -> browser / ploke-egui projections
```

Only `ploke-tree` store/loading code may discover run-file paths or deserialize
Prototype 1 run files. Graph builders consume `&RunRecordSet`. Renderer code
consumes `&Graph` or explicit graph-derived projections.

Directory-wide exclusions such as worktrees, target dirs, repo caches, cargo
caches, and logs mean "do not recursively deserialize or scan contents as
source records." They do not mean "omit from the graph forever." If a surface
matters to loop investigation, add a typed metadata/locator row and ingest that
summary through `RunRecordSet`.

Workers outside a store-loader lane must not call `fs::read_to_string`,
`serde_json::from_str`, JSONL readers, directory walkers, or path probes for
Prototype 1 run files.

Current file ownership, relative to a Prototype 1 run root unless noted:

| Persisted file or pattern | Typed record owner | Loader target | Graph status |
|---|---|---|---|
| `scheduler.json` | `ploke_records::scheduler::SchedulerStateRecord` | `RunForestInput.scheduler` | loaded, not ingested |
| `nodes/*/node.json` | `ploke_records::scheduler::NodeRecord` | `RunForestInput.node_records` | loaded, not ingested |
| configured `parent_identity.json` path | `ploke_records::identity::ParentIdentityRecord` | `RunForestInput.parent_identity` | loaded, not ingested |
| `nodes/*/successor-ready/*.json` | `ploke_records::invocation::SuccessorReadyRecord` | `RunForestInput.successor_ready` | loaded, not ingested |
| `nodes/*/successor-completion/*.json` | `ploke_records::invocation::SuccessorCompletionRecord` | `RunForestInput.successor_completion` | loaded, not ingested |
| `history/blocks/segment-*.jsonl` | `ploke_records::history::SealedBlockRecord` | `RunRecordSet.history_blocks` | core History spine |
| `transition-journal.jsonl` | `ploke_records::journal::JournalEntry` | `RunRecordSet.transition_journal` and passive count evidence | loaded, not ingested |
| `branches.json` object or JSONL log | `ploke_records::branch::{Prototype1BranchRegistry, BranchLogRecord}` | `PassiveEvidence.branch_registry` summary | loaded summary, not ingested |
| nested `channels/**/*.jsonl` | `ploke_records::channel::Envelope<ToParent/ToChild>` | `PassiveEvidence.channel_envelopes` summary | loaded summary, not ingested |
| `evaluations/*.json` | `ploke_records::evaluation::Artifact` | `PassiveEvidence.evaluations` | evidence attachment |
| configured protocol artifacts dir `*.json` | `ploke_records::protocol::Artifact` | `PassiveEvidence.protocol_artifacts` | evidence attachment |

If a worker needs a file not listed here, it should report the missing row
first. Do not add an alternate reader in a graph or renderer module.

## Worker Report Contract

Every worker report should include:

- files changed, or `none`;
- exact files and line ranges read;
- tests or checks run;
- whether the patch changes behavior or is mechanical organization only;
- any ambiguity preserved rather than resolved;
- recommended smallest next check.

For implementation workers, include a short commit candidate summary. The main
thread decides whether and when to stage or commit grouped changes.

## Retry Rule For Checks

If `cargo check`, `cargo test`, or a target-specific check fails in a way that
looks unrelated to the worker's assigned files:

1. wait about one minute;
2. run the same bounded command once more;
3. if it still fails, report the exact command, the short failure tail, and why
   it appears unrelated.

Do not broaden the patch to fix unrelated failures without a new assignment.
For `cargo test`, keep output bounded according to `AGENTS.md`.

## Retainer Explorer

Keep one cheap read-only explorer available for lookup tasks while
implementation workers run. Its role is to answer narrow questions such as:

- where is a type defined?
- which file owns this loader?
- what previous review already covered this family?
- what command verifies this exact claim?

The retainer explorer should not implement changes, read large files, or dump
logs. It reports paths, line ranges, and a compact answer.

## Acceptance Gate

A graph-ingestion patch is not accepted until:

- `Graph::from_records(&RunRecordSet)` remains the only assembly entry point;
- `History` ordering remains primary;
- new source families attach as core graph structure, evidence, secondary
  ordering, diagnostics, or explicit unresolved ambiguity;
- every newly encountered persisted loop surface is either loaded into the graph
  boundary now or tracked as a future metadata/evidence/locator input;
- the ingestion inventory is updated when graph coverage changes;
- focused checks for the touched crate pass or failures are clearly unrelated.
