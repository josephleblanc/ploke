# Collaborator mdBook strategy for Ploke-loop documentation

Date: 2026-06-19
Task: `t_104dd26c`
Status: planning artifact for future mdBook writers; not final collaborator prose
Primary inputs: `README.md` source inventory, `implementation-invariants-source-slice.md`, and `graphrag-history-adapter-note.md` in this directory

## Purpose

This document turns the current Ploke-loop source inventory into a concrete mdBook writing plan. It is for future human and AI collaborators who need to generate chapters from source evidence rather than memory.

The plan covers two related books or book sections:

1. release-facing collaborator docs for the current Ploke code-understanding and TUI pipeline; and
2. implementation/proof-facing docs for Prototype 1 runtime succession, History/Crown authority, passive projections, and future GraphRAG-to-History evidence bridges.

The key rule for every chapter is: quote or summarize current source anchors first, then write prose. Do not promote stale notes, implementation sketches, run-local examples, or home-machine typestate assumptions into current implementation claims.

## Authority labels to preserve

Use these labels in chapter source notes and in any generated book front matter:

- `canonical/current`: current docs or code comments intended as the public or implementation source of truth.
- `implementation-grounded`: current code or bounded audit evidence; safe for "what this VM checkout does" only after nearby source is rechecked.
- `stale-but-useful`: historical explanation, migration note, run report, or older draft. Use for motivation/background only.
- `speculative/target`: proof target, adapter proposal, or future implementation design. Mark clearly as target work.
- `blocked`: known source gap. The main blocked item is unpushed home-machine typestate completion for `prototype1_state` transitions.

## Proposed mdBook table of contents

This is a proposed structure under `docs/book/src/`. Existing chapters can be updated in place where they already exist; new chapters should be added only after the source anchors below are refreshed.

```text
Introduction
Quick Start

User Guide
  First Run
  Indexing a Rust Workspace
  Commands and Modes
  Code Context Requests
  Message and Tool-call Lifecycle

Architecture
  Architecture Overview
  Workspace Map
  Code Understanding Pipeline
  Parsing and Code Graph Construction
  Database, Schemas, and Projections
  Embeddings and Indexing
  RAG and GraphRAG Context Assembly
  TUI Runtime and Event Paths
  Config and State Persistence
  Eval and Projection Plane
  Prototype 1 Runtime Succession
  History, Crown, and Authority
  Parent, Child, and Successor Handoffs
  Mutable and Immutable Surfaces
  Inductive Runtime Invariants

Crate Guide
  Runtime Crates
  Ingest and Indexing Crates
    syn-parser
    ploke-transform
    ploke-embed
  Retrieval and LLM Crates
    ploke-db
    ploke-rag
    ploke-llm
  TUI and Operator Crates
    ploke-tui
  Protocol and Projection Crates
    ploke-protocol
    ploke-records
    ploke-tree
    ploke-eval

Reference
  Glossary
  Source Authority Map
  Schema and Record Index
  Maintenance and Drift Checks
  Documentation Roadmap
```

## Chapter plan and source matrix

### 1. Architecture Overview

Audience: new human collaborators, AI coding agents, paper/proof writers who need the same vocabulary.

Purpose: explain Ploke as a Rust-first code-understanding system with a separate eval/projection plane. Keep the top-level path simple: parse Rust workspace -> transform code graph -> store/query in Cozo -> embed/index -> retrieve/assemble context -> route through TUI/LLM/tool loop -> optionally record eval evidence.

Source dependencies:

- `docs/book/src/architecture/index.md` and `workspace-map.md` for current public overview.
- `docs/book/src/architecture/code-understanding-pipeline.md` for the existing six-stage pipeline.
- `docs/book/src/architecture/eval-and-projection-plane.md` for the evidence-not-authority rule.
- `PROPOSED_ARCH_V3.md` only as stale-but-useful mission/background.
- This directory's `README.md` executive source authority map for source priority.

Canonical code anchors to refresh before writing:

- `crates/ploke-tui/src/lib.rs` module docs and `try_main` subsystem wiring.
- `crates/ingest/syn_parser/src/lib.rs` parser entrypoints.
- `crates/ploke-db/src/database.rs` DB initialization/import/query boundary.
- `crates/ingest/ploke-embed/src/runtime.rs` active embedding-set runtime.
- `crates/ploke-rag/src/core/mod.rs` `RagService`, `RagConfig`, and retrieval strategies.

Freshness risks:

- `PROPOSED_ARCH_V3.md` is older and aspirational.
- The TUI command and RAG service initialization paths have been changing.
- Do not collapse release TUI runtime and Prototype 1 eval runtime into one authority plane.

Maintenance plan:

- Keep this chapter short and link to pipeline chapters for implementation detail.
- On each release-doc refresh, diff `docs/book/src/SUMMARY.md`, `crates/ploke-tui/src/lib.rs`, and crate-guide pages to ensure crate responsibilities still match.
- If a new runtime/projection crate appears, update both the overview and crate-guide cross-links in the same commit.

### 2. Parsing and Code Graph Construction

Audience: implementation collaborators, AI agents generating parser/graph docs, researchers reading GraphRAG claims.

Purpose: describe how Rust source becomes a typed graph before storage and retrieval.

Source dependencies:

- `graphrag-history-adapter-note.md` section "Rust parsing and graph construction".
- `docs/book/src/architecture/code-understanding-pipeline.md` stages 1-2.
- `docs/testing/TYPE_RESOLUTION_COVERAGE.md` for coverage/gap claims.
- `docs/workflow/evalnomicon/drafts/formal/callgraph-implementation-design-for-detached-process-proof.md` only for proof-target gaps.

Canonical code anchors to refresh before writing:

- `crates/ingest/syn_parser/src/lib.rs` for `parse_workspace_with_config`, `parse_workspace`, `try_run_phases_and_resolve`, and returned graph/module structures.
- `crates/ingest/syn_parser/src/resolve/type_resolution_v2.rs` for typed type-relation resolver claims.
- `crates/ingest/ploke-transform/**` for database-ready relation transformation.
- `crates/ploke-records/src/proof_facts.rs` and `proof_authority.rs` for proof-useful call/effect fact DTOs, but only in a proof-gaps subsection.

Freshness risks:

- Compiler-grade call/effect extraction is target work, not current parser behavior.
- Macro expansion, build domains, proc macro effects, and build-script effects remain proof gaps unless current code proves otherwise.

Maintenance plan:

- Every parser chapter update must include a mini source table: parser entrypoints, transform outputs, DB relation consumers, and known unsupported Rust/macro cases.
- Keep proof-target material under a clearly labeled "Future proof-grade extraction" section.

### 3. Database, Schemas, and Projections

Audience: implementers, DB/RAG maintainers, proof/projection collaborators.

Purpose: document Cozo-backed graph storage, embedding vector relations, passive records, proof graph rows, and the projection/authority boundary.

Source dependencies:

- `implementation-invariants-source-slice.md` rows 76-88 and 63-74.
- `docs/book/src/crate-guide/ploke-db.md` and `ploke-tree.md`.
- `docs/book/src/architecture/eval-and-projection-plane.md`.
- `docs/active/agents/2026-06-09_prototype1-state-api-surface-migration/migration.md` for DTO/projection migration notes.

Canonical code anchors to refresh before writing:

- `crates/ploke-db/src/database.rs`: `DEFAULT_EMBEDDING_SET`, type-relation import mode, typed type graph relation whitelist, namespace import/export/query helpers.
- `crates/ploke-db/src/multi_embedding/schema.rs`: `EmbeddingVector`, vector relation schema, `validate_embedding_vec`.
- `crates/ploke-db/src/proof_graph.rs`: proof graph store and invariant status rows.
- `crates/ploke-records/src/history.rs`: passive record mirror boundary.
- `crates/ploke-tree/src/browser.rs`, `graph/artifact_tree.rs`, and projection tests in `crates/ploke-tree/src/tests.rs`.

Freshness risks:

- Schema details drift quickly; line-level claims must be regenerated from current source.
- Cozo vector values are read as `DataValue::Vec(cozo::Vector::F32(_))` or F64, not plain list values; do not write examples that assert list-shaped readback.
- Passive records are not authority and must not be described as advancing History or scheduler state.

Maintenance plan:

- Add a `Source Authority Map` reference page with a table of relation families, owning crate, writer, reader, and authority treatment.
- For schema changes, require a docs checklist item: update schema chapter, crate guide, tests/fixture docs if fixtures are affected, and any projection diagrams.

### 4. Embeddings and Indexing

Audience: operators, TUI users, embedding/RAG implementers, AI agents debugging indexing.

Purpose: explain active embedding-set selection, vector relation creation, indexing tasks, provider backends, BM25 companion indexing, and cancellation/failure surfaces.

Source dependencies:

- `graphrag-history-adapter-note.md` sections "Embeddings and indexing" and "TUI / LLM integration".
- `implementation-invariants-source-slice.md` rows for `ploke-db/src/multi_embedding/schema.rs`, `ploke-embed`, and TUI initialization.
- Existing `docs/book/src/crate-guide/ploke-embed.md` and `docs/book/src/user-guide/indexing.md`.
- `crates/ploke-tui/docs/data-flows/select-embedding-model.md` only as a known doc gap/stub.

Canonical code anchors to refresh before writing:

- `crates/ingest/ploke-embed/src/runtime.rs`: `EmbeddingRuntime`, `activate`, active set sharing.
- `crates/ingest/ploke-embed/src/indexer/mod.rs`: `EmbeddingProcessor`, `IndexerTask`, batching, cancellation, BM25 channel.
- `crates/ingest/ploke-embed/src/config.rs` for local/provider configuration.
- `crates/ploke-llm/src/router_only/openrouter/embed.rs` for OpenRouter embedding environment behavior.
- `crates/ploke-tui/src/lib.rs` for setup path from config to DB to embedding runtime to RAG.

Freshness risks:

- Provider-specific docs can leak credentials if copied from local environment output. Never include env values.
- Existing select-embedding data-flow doc is a stub; do not treat it as implementation authority.
- Active embedding-set behavior may change with config overlay work.

Maintenance plan:

- Include a safe example with placeholder environment variable names only.
- Keep a troubleshooting table keyed by observable state: active set mismatch, vector relation missing, index task canceled, BM25 unavailable, provider dimension mismatch.
- Refresh command examples from `/help` and `app_state/commands.rs`, not from memory.

### 5. RAG and GraphRAG Context Assembly

Audience: TUI users, RAG implementers, AI collaborators that need to request code context effectively.

Purpose: explain sparse, dense, and hybrid retrieval; type-context expansion; token-budgeted assembly; tool/request provenance; and how GraphRAG evidence can later become History candidate payloads without authorizing runtime succession.

Source dependencies:

- `graphrag-history-adapter-note.md` sections "RAG service and context assembly", "Candidate adapter seam", and "Why this seam is safer".
- `crates/ploke-rag/docs/point2_context_assembly_review.md` as stale/proposal material.
- `docs/book/src/crate-guide/ploke-rag.md`.
- `crates/ploke-tui/src/tools/request_code_context.rs` for user/model tool schema.

Canonical code anchors to refresh before writing:

- `crates/ploke-rag/src/lib.rs` crate docs and re-exports.
- `crates/ploke-rag/src/core/mod.rs`: `RetrievalStrategy::{Dense,Sparse,Hybrid}`, `RagConfig`, `TypeContextConfig`, `RagService::new_full`.
- `crates/ploke-rag/src/context/mod.rs`: `assemble_context`, `assemble_context_with_type_context`, `ContextPart`.
- `crates/ploke-tui/src/rag/context.rs`: `process_with_rag`, scan wait, `RetrievalScope::LoadedWorkspace`, context snapshot emission, no-context fallback.
- `crates/ploke-tui/src/tools/request_code_context.rs`: request fields and degraded type-context notes.
- `crates/ploke-eval/src/cli/prototype1_state/history/projection/mod.rs` only for the future adapter's read-side seam.

Freshness risks:

- Current RAG may not implement all context assembly review recommendations.
- Type-context expansion is configurable/degradable; do not describe it as always available.
- Adapter-to-History is target/speculative unless a future implementation writes or reads those payloads.

Maintenance plan:

- Split the chapter into "Current RAG path" and "Future GraphRAG-to-History adapter".
- Require each code-context example to state its source: automatic RAG, explicit tool call, or future sealed History candidate.
- Update examples when `request_code_context` schema changes.

### 6. TUI Runtime and Event Paths

Audience: TUI contributors, operators, AI agents making UI or command changes.

Purpose: document the runtime subsystems, user input path, command routing, EventBus priorities, and high-value commands.

Source dependencies:

- `graphrag-history-adapter-note.md` TUI/LLM integration bullets.
- Existing `docs/book/src/user-guide/commands-and-modes.md`.
- `docs/book/src/crate-guide/ploke-tui.md`.
- `crates/ploke-tui/AGENTS.md` for crate-specific workflow and test commands.

Canonical code anchors to refresh before writing:

- `crates/ploke-tui/src/lib.rs`: subsystem startup, `StateCommand` channel, `EventBus`, `AppEvent`, priorities, `GenerateContext`.
- `crates/ploke-tui/src/app_state/commands.rs`: `/index`, `/load`, command resolution, typed command group migration status.
- `crates/ploke-tui/src/app/mod.rs` and `src/app/input/**`: input handling and channel sends.
- `crates/ploke-tui/src/observability.rs`: event persistence and tool persistence worker boundaries.
- `crates/ploke-tui/src/chat_history.rs`: hierarchical messages and update-failure events.

High-value command examples to keep current:

```text
/help
/model search <query>
/model list
/model info
/model use <name>
/model refresh
/index start [path]
/index pause
/index resume
/index cancel
/embedding search <query>
/save db
/load <crate-or-workspace-name>
```

Also include RAG/tool examples with the current schema:

```text
Ask a code question after loading/indexing a workspace.
If automatic context is insufficient, request code context using identifiers, module names, file names, type names, or concise code terms.
```

Freshness risks:

- `commands-and-modes.md` currently says `/help` is the most current command surface; command examples must be rechecked against code or live help before release.
- Command architecture is mid-migration toward typed command groups.
- TUI user-message hierarchy is separate from Prototype 1 parent/child runtime vocabulary; do not reuse `parent` or `child` without qualification.

Maintenance plan:

- Add a docs test/checklist that greps command strings from `commands.rs` or captures `/help` output before command chapter updates.
- Treat command screenshots/transcripts as per-release artifacts, not stable source authority.

### 7. Lifetime of a User Message

Audience: TUI contributors, LLM/tool implementers, AI collaborators debugging context or message updates.

Purpose: trace a user-entered message from terminal input through chat history, optional RAG context generation, LLM request, tool call/request-code-context paths, message update events, and persistence/observability.

Source dependencies:

- `implementation-invariants-source-slice.md` row for `crates/ploke-tui/src/chat_history.rs`.
- `graphrag-history-adapter-note.md` TUI/LLM integration and RAG path bullets.
- `docs/book/src/architecture/sequence-diagrams.md` if current diagrams exist.

Canonical code anchors to refresh before writing:

- `crates/ploke-tui/src/app/input/**` for input-to-command/message decisions.
- `crates/ploke-tui/src/chat_history.rs`: `Message`, hierarchical parent/child message docs, update failure event.
- `crates/ploke-tui/src/rag/context.rs`: RAG prompt construction and context annotations.
- `crates/ploke-tui/src/llm/**` and `crates/ploke-llm/**`: provider request/response boundaries.
- `crates/ploke-tui/src/tools/**`: tool request/response and persistence.
- `crates/ploke-tui/src/observability.rs`: persisted event/tool observation.

Freshness risks:

- The exact async task/channel order can drift; write sequence diagrams from code immediately before publication.
- Tool-call surfaces differ between user-visible TUI behavior and internal eval/protocol artifacts.

Maintenance plan:

- Maintain the chapter as a sequence diagram plus a table of message states, not a long prose description.
- Every update should include one checked example path: conversation-only message, RAG-augmented message, and tool-call message.

### 8. Config and State Persistence

Audience: operators, maintainers, AI agents diagnosing mismatched provider/model/workspace behavior.

Purpose: map TUI user config, model/provider routing, embedding config, workspace registry, save/load behavior, eval campaign config, and Prototype 1 run/profile config without merging them into one state model.

Source dependencies:

- `docs/active/agents/2026-06-02_prototype1-state-loop-walkthrough/campaign-configs.md`.
- `docs/active/agents/2026-06-02_prototype1-state-loop-walkthrough/model-api-brief.md`.
- `implementation-invariants-source-slice.md` rows for `user_config.rs`, TUI config overlay, and `profile.rs`.
- `docs/book/src/user-guide/first-run.md`, `indexing.md`, and command chapters.

Canonical code anchors to refresh before writing:

- `crates/ploke-tui/src/user_config.rs`: user configuration types, provider/model/editing/embedding fields, workspace registry env.
- `crates/ploke-tui/src/app/view/components/config_overlay.rs` and `src/app/input/config_overlay.rs`: overlay behavior.
- `crates/ploke-tui/src/app_state/**`: state manager and config state.
- `crates/ploke-eval/src/cli/prototype1_state/profile.rs`: run profile policy for Prototype 1.
- `crates/ploke-eval/src/cli/args/loop_args.rs`: `Prototype1StateCommand` fields.

Freshness risks:

- Local machine paths, model names, run ids, and API key locations in active-agent notes are examples, not canonical project configuration.
- Eval campaign config and TUI user config have different authority scopes.

Maintenance plan:

- Organize this chapter by config plane: TUI user config, provider/model routing, workspace/indexing state, eval campaign/run profile, and persisted records.
- Include a "never document secrets" box and use placeholder env values only.
- Refresh after any config schema or overlay UI changes.

### 9. Prototype 1 Runtime Succession

Audience: Prototype 1 implementers, proof-track authors, researchers, advanced collaborators.

Purpose: explain why Prototype 1 is runtime succession rather than a flat eval rerun: the parent runtime evaluates child artifacts/runtimes, chooses evidence, and may hand off to a successor runtime under constrained authority.

Source dependencies:

- `docs/workflow/evalnomicon/src/prototype1/runtime-loop.md`.
- `docs/workflow/evalnomicon/src/prototype1/artifact-runtime-model.md`.
- `docs/workflow/evalnomicon/src/prototype1/selection-and-evaluation.md`.
- `docs/active/agents/2026-06-02_prototype1-state-loop-walkthrough/README.md`.
- `graphrag-history-adapter-note.md` section "Parent/runtime authority and transition spine".

Canonical code anchors to refresh before writing:

- `crates/ploke-eval/src/cli/prototype1_state/mod.rs`: vocabulary and design constraints.
- `crates/ploke-eval/src/cli/prototype1_state/run/mod.rs` and `run/core.rs`: live execution inventory and typed core extraction target.
- `crates/ploke-eval/src/cli/prototype1_state/cli_facing.rs`: current live source of truth for broad path until extraction completes.
- `crates/ploke-eval/src/cli/args/loop_args.rs`: CLI mode and candidate generator fields.

Freshness risks:

- Current VM typestate is incomplete relative to reported home-machine work. This chapter must explicitly say final typestate transitions are blocked until pushed/supplied.
- `cli_facing.rs` is current live behavior but not the desired final boundary.

Maintenance plan:

- Keep a visible "Current VM status" callout with the commit/branch and known typestate caveat.
- Do not publish line-specific command flow until line refs are refreshed against current HEAD.

### 10. Parent, Child, and Successor Handoffs

Audience: Prototype 1 implementers, proof authors, run-review agents.

Purpose: document parent startup, child plan realization, child build/run/observation, parent-side selection, successor bootstrap, successor-ready handshake, and continuation disposition.

Source dependencies:

- `implementation-invariants-source-slice.md` sections "Prototype 1 authority, History, Crown, and handoff".
- `docs/workflow/evalnomicon/drafts/runtime/parent-child-channel.md` as stale-but-useful rationale.
- `docs/active/agents/2026-06-06_parent-successor-handoff-regression/**` as bounded negative evidence, not happy-path authority.

Canonical code anchors to refresh before writing:

- `crates/ploke-eval/src/cli/prototype1_state/parent.rs`: `Startup`, `Parent` states, child-plan packing/validation.
- `crates/ploke-eval/src/cli/prototype1_state/c1.rs` through `c4.rs`: transition scaffolds.
- `crates/ploke-eval/src/cli/prototype1_state/channel.rs`: role-indexed channel markers and filesystem transport.
- `crates/ploke-eval/src/cli/prototype1_state/successor.rs`: successor record states.
- `crates/ploke-eval/src/cli/prototype1_process.rs`: current process-spawn/handoff implementation.
- `crates/ploke-eval/src/intervention/scheduler.rs`: selection/continuation helper, marked as not necessarily live authority path by run docs.

Freshness risks:

- `channel.rs` is staged/dead-code-allowed in current inventory; do not imply full live migration.
- `c2.rs` and `c3.rs` note live controller gaps in this VM checkout.
- Regression docs are stale-but-useful unless revalidated.

Maintenance plan:

- Use a state-transition diagram with each edge annotated by source file and status label.
- Keep happy path, failure/timeout path, and successor handoff path separate.
- Add an explicit "blocked pending home-machine typestate changes" note until pushed changes are reviewed.

### 11. History, Crown, and Authority

Audience: all collaborators who make or review authority claims.

Purpose: define History and Crown as local lineage authority surfaces, distinguish sealed History from projections/logs/reports, and explain what Crown does not prove.

Source dependencies:

- `docs/workflow/evalnomicon/src/prototype1/history-crown.md`.
- `docs/workflow/evalnomicon/src/prototype1/runtime-authority.md`.
- `docs/workflow/evalnomicon/src/prototype1/invariant-ledger.md`.
- `docs/workflow/evalnomicon/src/prototype1/persistence-and-observability.md`.
- `implementation-invariants-source-slice.md` authority and History rows.

Canonical code anchors to refresh before writing:

- `crates/ploke-eval/src/cli/prototype1_state/history/mod.rs`: domain definitions, projection warnings, startup-to-Parent authority sequence.
- `crates/ploke-eval/src/cli/prototype1_state/history/seal/mod.rs`: typed entries/open blocks/sealing comments.
- `crates/ploke-eval/src/cli/prototype1_state/history/projection/mod.rs`: read-only candidate projection.
- `crates/ploke-eval/src/cli/prototype1_state/history/stored/mod.rs`: append-only block store and head projection boundary.
- `crates/ploke-eval/src/cli/prototype1_state/invocation.rs`: bootstrap role contract.
- `crates/ploke-records/src/history.rs`: passive mirror boundary.

Freshness risks:

- Formal detached-process proof docs are proof targets, not proof of current implementation.
- Crown is local authority, not distributed consensus, whole-environment trust, or proof that a process path/branch is safe.

Maintenance plan:

- Require every authority sentence to include a source label and a non-claim where appropriate.
- Keep a table of "authority", "evidence", "projection", and "cache" surfaces and update it whenever records/projection crates change.

### 12. Mutable and Immutable Surfaces

Audience: edit-surface implementers, TUI adapter contributors, proof authors.

Purpose: document the bounded edit surface: what is immutable evidence, what can be mutated, what is ambient context, and how eval binds proposals to checked surfaces before admission.

Source dependencies:

- `docs/workflow/evalnomicon/src/prototype1/edit-surface.md`.
- `implementation-invariants-source-slice.md` edit surface rows.
- `docs/active/agents/2026-06-09_prototype1-state-api-surface-refactor-proposal.md` as speculative/proposal.
- `docs/active/agents/2026-06-09_prototype1-state-api-surface-migration/migration.md` as migration log.

Canonical code anchors to refresh before writing:

- `crates/ploke-eval/src/cli/prototype1_state/edit_surface/tui.rs`: eval-owned adapter boundary and generator surface version.
- `crates/ploke-eval/src/cli/prototype1_state/backend/surface_admission.rs`: checked candidate validation.
- `crates/ploke-eval/src/cli/prototype1_state/edit_surface/harness_request.rs`: broad harness request/admission binding.
- `crates/ploke-eval/src/cli/prototype1_state/evidence.rs`: child-evidence grouping invariant.
- `crates/ploke-eval/src/cli/prototype1_state/evidence_inventory.rs`: projection/sealed/ingress lanes.

Freshness risks:

- Multiple overlapping edit-surface models exist during migration; do not flatten them into one completed architecture.
- Submitted result persistence is not promotion authority.

Maintenance plan:

- Put current implementation and migration target in separate subsections.
- Include an admission-check table: path normalization, surface policy membership, expected hashes, span bounds, generator surface equality, all-applied vs rejected evidence.

### 13. Inductive Runtime Invariants

Audience: proof-track authors, implementers reviewing safety claims, AI agents writing formal-ish docs.

Purpose: turn repeated runtime/projection rules into an inductive invariant ledger that future proofs and docs can cite.

Source dependencies:

- `docs/workflow/evalnomicon/src/prototype1/invariant-ledger.md`.
- `docs/workflow/evalnomicon/drafts/formal/detached-process-callgraph-proof-target.md`.
- `docs/workflow/evalnomicon/drafts/formal/callgraph-implementation-design-for-detached-process-proof.md`.
- `docs/active/agents/2026-06-19_detached-process-crown-proof-spine/traceability.md`, but revalidate because observed HEAD may differ.
- `implementation-invariants-source-slice.md` citation-priority section.

Canonical code anchors to refresh before writing:

- `crates/ploke-eval/src/cli/prototype1_state/history/**` for History/Crown facts.
- `crates/ploke-eval/src/cli/prototype1_state/parent.rs`, `successor.rs`, `invocation.rs`, `channel.rs` for role/state authority carriers.
- `crates/ploke-records/src/proof_facts.rs` and `proof_authority.rs` for proof-fact DTOs.
- `crates/ploke-db/src/proof_graph.rs` for proof graph and invariant status storage.
- `crates/ploke-tree/src/tests.rs` for executable projection invariants.

Candidate invariant families:

- History authority: only admitted/sealed History advances lineage authority.
- Projection safety: scheduler, registry, CLI reports, dashboards, DB side tables, and passive records are evidence/projections, not promotion authority.
- Runtime succession: a modified artifact does not change the already-running parent binary; successor behavior requires descendant runtime construction and handoff.
- Role/state authority: Parent, Child, and Successor capabilities are role/state/lineage scoped and should not be reconstructed from loose paths or JSON.
- Evidence grouping: persisted child evidence must deserialize through typed stored records; loose inference from paths or metrics must not feed selection/authority semantics.
- Surface admission: edits require checked surfaces, hashes, policies, spans, and all-applied/rejected evidence.
- Detached-process target: no process should outlive a runtime with Crown-equivalent authority except through admitted handoff; current proof extraction is incomplete.

Freshness risks:

- Strong detached-process claims are speculative until compiler-grade call/effect facts and handoff proof obligations are implemented and checked.
- Existing proof-spine docs can be stale relative to current HEAD.

Maintenance plan:

- Maintain this as a ledger table with columns: invariant, status, source anchors, executable tests/checks, proof blockers, last refreshed commit.
- Do not add a new invariant without a non-claim and a source anchor.

### 14. GraphRAG-to-History Evidence Adapter

Audience: researchers, implementation designers, proof-track collaborators.

Purpose: propose how existing code graph/RAG observations can become History-compatible evidence/candidate payloads while preserving the continuation authority boundary.

Status: `speculative/target`; publication-friendly, not implemented as a complete adapter in this VM checkout.

Source dependencies:

- `graphrag-history-adapter-note.md` sections "Candidate adapter seam" and "Paper/report phrasing candidate".
- Current RAG and History projection code anchors from chapters 5 and 11.
- `docs/workflow/evalnomicon/src/prototype1/selection-and-evaluation.md`.

Canonical code anchors to refresh before writing:

- RAG side: `ParsedCodeGraph`, `ModuleTree`, `EmbeddingData`, `AssembledContext`, `ContextPart`, BM25/dense scores, type-context expansion, TUI tool provenance.
- History side: `SubjectRef`, `ProcedureRef`, `EvidenceRef`, `EvaluationPayload`, and `History::candidates(scope)`.
- Continuation side: `run/mod.rs` warning about `Continuation<Allowed | Stopped>` and current live authority path.

Freshness risks:

- Adapter should not mint continuation authority.
- Write-side History sealing is stricter than read-side payload projection and needs separate authority review.

Maintenance plan:

- Keep this chapter under an "Experimental / Design Target" banner until a real adapter lands.
- Any implementation PR must add tests showing GraphRAG observations remain evidence-only and cannot bypass selection/continuation gates.

## Source-to-chapter writing workflow

For each chapter, future writers should follow this mechanical workflow:

1. Start from this plan's source dependencies and canonical code anchors.
2. Re-run source discovery for the exact chapter scope using `search_files`/`read_file` or equivalent code intelligence tools.
3. Record the current branch and commit in a short chapter source note.
4. Classify each source with the authority labels above.
5. Draft the chapter from canonical/current and implementation-grounded sources first.
6. Put stale/speculative material in explicitly labeled callouts.
7. Add or update diagrams only after source paths and state names are refreshed.
8. Run markdown checks/readback and, for code-adjacent examples, the narrowest relevant command or test.
9. Update `docs/book/src/SUMMARY.md` and the relevant crate-guide cross-links in the same commit as chapter additions.

## Drift and maintenance strategy

### Release-doc refresh checklist

Run this checklist before considering the collaborator docs current:

- Read `docs/book/src/SUMMARY.md` and confirm every linked chapter exists.
- Re-read `crates/ploke-tui/src/lib.rs`, `app_state/commands.rs`, and `/help` output if available before updating command examples.
- Re-read `crates/ploke-rag/src/core/mod.rs`, `context/mod.rs`, and `crates/ploke-tui/src/rag/context.rs` before updating RAG/GraphRAG claims.
- Re-read `crates/ploke-db/src/database.rs`, `multi_embedding/schema.rs`, and `proof_graph.rs` before updating DB/schema/vector/proof graph claims.
- Re-read `crates/ploke-eval/src/cli/prototype1_state/mod.rs`, `run/mod.rs`, `run/core.rs`, `parent.rs`, `successor.rs`, `channel.rs`, `history/**`, and `cli_facing.rs` before updating Prototype 1 runtime claims.
- Check whether `crates/ploke-eval/src/cli/prototype1_state/typestate/**` or equivalent home-machine typestate changes have been pushed. If not, keep typestate chapters blocked/speculative where noted.
- Re-run or inspect any executable invariant tests cited in the chapter, especially projection tests under `crates/ploke-tree/src/tests.rs`.
- Do not include raw credentials, local-only absolute paths, local run ids as requirements, or provider secrets.

### Documentation drift triggers

Update affected chapters when any of these change:

- `docs/book/src/SUMMARY.md` chapter layout.
- TUI command parser/resolution, config overlay, or user config schema.
- Embedding provider/runtime activation or vector relation schema.
- RAG retrieval strategy, context assembly, type-context expansion, or request-code-context tool schema.
- Cozo schema import/export, backup fixtures, proof graph rows, or passive record DTOs.
- Prototype 1 runtime state names, handoff files, History sealing/projection, successor selection, or continuation logic.
- Any merge of the user's home-machine typestate changes.

### Handling unpushed typestate changes

Until the missing/home-machine typestate work is pushed or supplied:

- Keep `Prototype 1 Runtime Succession`, `Parent, Child, and Successor Handoffs`, and `Inductive Runtime Invariants` marked as partially implementation-grounded and partially blocked.
- Use current VM files for current behavior only, not final intended typestate architecture.
- Preserve code comments that say typed states are partial, staged, or not consumed by the live controller.
- Avoid diagrams that show C1 -> C5 as fully live unless current source proves it.
- Add a source note saying: "Typestate completion is reported out-of-band but not present in this checkout; this chapter must be refreshed after those changes land."

## Immediate next writing slices

1. Update existing `docs/book/src/architecture/code-understanding-pipeline.md` from a short overview into a source-anchored pipeline chapter. Keep it release-facing and avoid Prototype 1 authority claims.
2. Add a new `docs/book/src/architecture/tui-runtime-and-event-paths.md` after refreshing command and message paths from `ploke-tui` source.
3. Add a new `docs/book/src/architecture/history-crown-and-authority.md` using evalnomicon `src/prototype1` pages plus `prototype1_state/history/**` source anchors.
4. Add a new `docs/book/src/reference/source-authority-map.md` generated from the inventory table in this directory.
5. Defer final typestate/handoff chapters until home-machine typestate changes are pushed or supplied.

## Non-goals for this plan

- It does not assert that the future GraphRAG-to-History adapter exists.
- It does not claim detached-process/Crown safety is proven.
- It does not replace source inventory or implementation notes; it routes future writers to them.
- It does not update the public mdBook directly. Public chapter edits should be separate, source-refreshed slices.
