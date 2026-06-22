# 2026-06-22 — Prototype 1 Eval Store Data Model

Status: active planning folder.

Short description: planning packet for replacing Prototype 1 shared-filesystem assumptions with an owner-scoped, backend-agnostic persistence model that can use filesystem, database, transport, and artifact backends without weakening History, Channel, MessageBox, or policy authority.

## Files

- [`storage-plan.md`](storage-plan.md) — current plan for a Domain-C `EvalStore` while keeping History, Channel, and MessageBox authority separate.
- [`persistence-port-map.md`](persistence-port-map.md) — broader port/trait map for all persisted surfaces: `EvalStore`, `BlockStore`, `Transport`, `MessageBox`, `WorkspaceBackend`, profile/config, journals, bootstrap, logs, snapshots, and compatibility mirrors.
- [`implementation-plan.md`](implementation-plan.md) — phased implementation plan for config, `EvalStore`, first DB slice, dual-strict, and the tests/checkpoints required before each slice.
- [`typestate-persistence-ledger.md`](typestate-persistence-ledger.md) — source-checked transition ledger of current writes, reads, hard gates, passive mirror behavior, and trace/log sinks.
- [`relational-data-model.md`](relational-data-model.md) — first draft of entity/fact/ref relations for querying parent/child/successor lifetimes and agent traces.
- [`database-planning-notes.md`](database-planning-notes.md) — logical-to-physical schema planning notes: relation tiers, key strategy, Cozo considerations, normalization guidance, and first physical slices.
- [`cozo-feature-map.md`](cozo-feature-map.md) — source-checked Cozo feature review, including algorithms/fixed rules, transactions, indices, time travel, HNSW/FTS/LSH, backups, and backend trait implications.
- [`codegraph-eval-join-model.md`](codegraph-eval-join-model.md) — follow-on join model for making Prototype 1 eval evidence useful in the same database as the parsed Ploke code graph; intentionally kept separate from the first file-or-db storage migration pass.
- [`debugging-and-research-query-workloads.md`](debugging-and-research-query-workloads.md) — run-review/bug-derived workload review for debugging, trajectory reconstruction, benchmark aggregation, Cozo feature use, and schema pressure beyond file-shaped persistence.
- [`live-transition-test-plan.md`](live-transition-test-plan.md) — implementation gate for testing the full live typestate transition inventory in isolation before/after storage abstraction, including live provider/API confidence tests.
- [`transition-persistence-dependency-matrix.md`](transition-persistence-dependency-matrix.md) — producer/consumer matrix for persisted data created by earlier transitions and required by later transitions, plus checkpoint strategy to minimize live API wall time without lowering validity.
- [`schema-review.md`](schema-review.md) — source-checked review of relation gaps, query usefulness, and recommended model additions.
- [`record-usefulness-triage.md`](record-usefulness-triage.md) — cleanup/refactor ledger for legacy, projection-only, compatibility, or superseded records; critical authority surfaces and clear DB candidates are tracked elsewhere.
- [`open-questions.md`](open-questions.md) — modeling questions to resolve before implementation.

## Current decisions

- Do not treat “the database” as one ambient shared object. Stores are owner-scoped: parent, child runtime, successor runtime, treatment run, imported evidence, passive mirror, etc.
- `History` / sealed `Block` remains a dedicated authority path.
- `Channel<R, T: Transport>` remains runtime communication authority.
- `MessageBox` remains typed lock/unlock authority; payloads may be mirrored, but generic record writes do not replace the box.
- `records/mirror.cozo.sqlite` is a compatibility/debug mirror, not the future `DbEvalStore` schema.
- Structured traces and logs are persisted evidence and need first-class `eval_trace_event` / `eval_log_ref` treatment.
- Code-graph overlay relations are a follow-on capability after file/db parity for ordinary eval evidence; first pass should not require them.
- `EvalStore` is only one port in the broader persistence plan; other persisted surfaces stay tracked under their own authority ports or future port candidates.
- Parent-visible child evidence must cross a channel or explicit import/admission edge; child-local DB/codegraph facts must not be merged into the parent graph by default.
- Storage migration must be gated by isolated tests for the full current live typestate transition inventory; provider-facing transitions should use live API calls in the confidence suite.
- To avoid 30–35 minute full-loop waits between narrow changes, use persisted dependency checkpoints and producer/consumer contract tests before reserving full live loops for canaries.

## Working goal

Design relations that can answer lifecycle questions such as:

- What happened during this parent generation?
- Which children were planned, launched, observed, compared, and selected?
- Which runtime produced this record/log/artifact?
- Which data is parent-owned, child-local, successor-local, treatment-local, or imported?
- Which code items were protected/mutable under the active policy surface?
- Which LLM/tool-call trace led to a candidate patch or failed admission?
