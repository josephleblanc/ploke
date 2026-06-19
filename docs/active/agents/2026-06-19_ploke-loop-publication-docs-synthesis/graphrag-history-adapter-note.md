# GraphRAG / prototype History adapter evidence note

Task: `t_a22c3bd9`.

Purpose: identify where Ploke's GraphRAG-style code graph/retrieval pipeline and the Prototype 1 History/lineage runtime can be adapted or bridged for a publication-oriented description. This is a reference note for downstream synthesis, not a source patch.

## Short answer

Ploke already has two mostly separate but compatible structures:

1. A code graph / RAG pipeline that parses Rust workspaces into typed graph facts, stores them in Cozo, indexes nodes for BM25+dense retrieval, and assembles token-budgeted code context for the TUI/LLM loop.
2. A Prototype 1 runtime that treats parent/child runs, candidate selection, History blocks, custody, artifacts, evidence, and successor handoff as typed transition facts.

The likely publication bridge is not a new parser. It is an adapter layer that exports selected RAG/code-graph observations as History evidence/candidate payloads, then lets the existing History projection and successor-selection path reason over those payloads with provenance.

## Current code graph / RAG implementation surfaces

### Rust parsing and graph construction

- `crates/ingest/syn_parser/src/lib.rs`
  - Main parser entrypoints are documented at the crate root: `parse_workspace_with_config`, `parse_workspace`, `try_run_phases_and_resolve`, and related `try_run_phases_and_merge*` flows.
  - The crate performs discovery, parallel parsing, and module-tree construction, returning `ParsedWorkspace`, `ParserOutput`, `ParsedCodeGraph`, and `ModuleTree` structures.
  - It re-exports `GraphAccess`, `ParsedCodeGraph`, `ModuleTree`, `CrateContext`, `TargetSelector`, and node/test-id helpers used by downstream crates.

- `crates/ingest/syn_parser/src/resolve/type_resolution_v2.rs`
  - Contains the typed type-relation resolver. The module-level comment describes the v2 pass as running after `ModuleTree` construction and producing typed relation facts once source/target endpoint proof succeeds.
  - Important public shapes include `TypeRelationReport`, `TypeRelationSummary`, and `TypeRelationResolver::resolve_type_relations`.

### Cozo-backed graph database and retrieval schema

- `crates/ploke-db/src/database.rs`
  - `Database` wraps a Cozo in-memory store and tracks `active_embedding_set`.
  - It owns namespace import/export/removal types (`NamespaceInventory`, `NamespaceExportArtifact`, `NamespaceImportResult`, etc.) and snippet/context-node query helpers.
  - `snippet_context_nodes` turns query rows into `EmbeddingData` plus `NodePaths` with ids, names, file paths, byte spans, hashes, namespace, and canonical path.
  - Typed type-graph relations are recognized by `is_typed_type_graph_relation`, including `type_relation`, `type_use`, `type_contains`, and slot relations.

- `crates/ploke-db/src/lib.rs`
  - Re-exports `Database`, query/result types, HNSW helpers, BM25, type graph types, observability, and proof graph rows.

- `crates/ploke-db/src/multi_embedding/*`
  - Multi-embedding support is used by `Database` and `EmbeddingRuntime` to register embedding sets, active vector relations, and HNSW indices.

### Embeddings and indexing

- `crates/ingest/ploke-embed/src/runtime.rs`
  - `EmbeddingRuntime` is the active embedding-set and embedder handle. It supports `current_active_set`, `current_processor`, `activate`, `generate_embeddings`, dimensions, and snippet batch size.
  - `activate` updates database embedding-set relations before publishing the new active embedder.

- `crates/ingest/ploke-embed/src/indexer/mod.rs`
  - `EmbeddingProcessor` abstracts local, HuggingFace, OpenAI, OpenRouter, and mock/Cozo embedding backends.
  - `IndexerTask` binds `Database`, `IoManagerHandle`, `EmbeddingRuntime`, cancellation, optional BM25 command channel, node-type cursors, and processed counters.
  - Mock embeddings are deterministic and useful for tests.

- `crates/ploke-llm/src/router_only/openrouter/embed.rs`
  - Implements OpenRouter embeddings, model registry discovery, dimension validation, and error-rich response parsing.
  - `OpenRouterEmbedEnv::from_env` resolves the API key and optional `OPENROUTER_EMBEDDINGS_URL`. Do not surface credential values in reports.

### RAG service and context assembly

- `crates/ploke-rag/src/lib.rs`
  - Crate docs state the intended architecture: BM25 sparse retrieval, HNSW dense retrieval, RRF/MMR fusion, and token-budgeted context assembly.
  - Re-exports `RagService`, `RetrievalStrategy`, `RagConfig`, `TokenBudget`, `AssemblyPolicy`, `Bm25Status`, and fusion utilities.

- `crates/ploke-rag/src/core/mod.rs`
  - `RetrievalStrategy` has `Dense`, `Sparse { strict }`, and `Hybrid { rrf, mmr }` variants.
  - `RagConfig` controls BM25 timeouts/retries, strictness, fusion, dense params per node type, token counter, reranker, and type-context expansion.
  - `RagService` holds `Arc<Database>`, `Arc<EmbeddingRuntime>`, a BM25 actor sender, config, optional IO manager, and a type-context-degraded flag.
  - Constructors include `new`, `new_with_config`, `new_with_io`, and `new_full`.

- `crates/ploke-rag/src/context/mod.rs`
  - `assemble_context` and `assemble_context_with_type_context` convert ranked node ids into `AssembledContext`.
  - It deduplicates IDs, fetches snippet context nodes from the DB, batch-fetches snippets through `ploke-io`, applies token budgets, and emits `ContextPart` values with file/canonical paths and optional type context.

### TUI / LLM integration

- `crates/ploke-tui/src/lib.rs`
  - Runtime initialization wires user config into an `EmbeddingRuntime`, initializes `Database::init_with_schema`, calls `setup_multi_embedding`, shares the runtime active set with the DB, and initializes `RagService::new_full` with DB, embed runtime, IO manager, and config-derived `RagConfig`.

- `crates/ploke-tui/src/rag/context.rs`
  - `process_with_rag` is the main prompt-construction path. It waits for scan completion, snapshots chat/config state, calls `rag.get_context(...)` using `RetrievalScope::LoadedWorkspace`, emits a `ContextPlanSnapshot`, annotates the user message with context stats, and sends the augmented prompt to the LLM manager.
  - If no RAG context is available, it falls back to conversation-only prompt construction and emits user-facing no-workspace/context tips.

- `crates/ploke-tui/src/app_state/commands.rs`
  - `/index` and `/load` command semantics live here. `IndexCmd::resolve` translates command mode and target into a concrete `IndexResolution`; `LoadCmd` and `LoadResolution` protect loaded-state transitions.
  - The command architecture is mid-migration toward typed command groups (`WorkspaceCmd`, planned `DbCmd`, `IndexCmd`, `ChatCmd`, `LlmCmd`, `RagCmd`).

- `crates/ploke-tui/src/tools/request_code_context.rs`
  - Tool surface for models/users to request deeper code context beyond the automatically attached RAG snippets.

## Prototype 1 History / lineage implementation surfaces

### Parent/runtime authority and transition spine

- `crates/ploke-eval/src/cli/prototype1_state/run/mod.rs`
  - The module doc contains a live execution path inventory and explicitly says the extraction target is `prototype1_state/run/core.rs`.
  - It identifies the intended typed core shape:
    - `Parent<Ready>`
    - `ChildPlan<Admitted>`
    - `ChildAttempt<Observed>*`
    - `SuccessorSelection<SealedEvidence>`
    - `Continuation<Allowed | Stopped>`
    - `Parent<Retired> | Parent<Ready>`
  - The key safety boundary is post-child continuation: `Parent<Selectable> + SuccessorSelection + AdmittedRunPolicy + PersistedRunInventory -> Continuation<Allowed | Stopped>`.

- `crates/ploke-eval/src/cli/prototype1_state/run/core.rs`
  - Contains the typed operational runtime around diagnosis/control. Important shapes include `EffectiveRunControl`, `DiagnosedPhase`, `ActiveParentStatus`, `RuntimeContext`, `PromptPreflight`, `ProtocolLivePreflight`, `HeadlessTuiSetupPreflight`, and `ChildSnapshot`.
  - `doctor`, `prompt`, `resume`, and `step` are re-exported from `run/mod.rs`.
  - It currently imports core live-path functions from `cli_facing`, so not all authority has been extracted yet.

- `crates/ploke-eval/src/cli/prototype1_state/parent.rs`
  - Defines the typed parent states and transition guards around startup, baseline/child planning, selectable parent state, and child-plan files.
  - Useful anchors from the run-module inventory: `Parent<Unchecked>`, `Parent<Checked>`, `Parent<Ready>`, `Parent<Planned>`, and `Parent<Selectable>`.

- `crates/ploke-eval/src/cli/prototype1_state/c1.rs`, `c2.rs`, `c3.rs`, `c4.rs`
  - Child typestate path: C1 materializes child artifact, C2 builds child runtime, C3 spawns child runtime, C4 observes child terminal result, C5 carries terminal treatment evidence for parent comparison.

- `crates/ploke-eval/src/cli/prototype1_state/cli_facing.rs`
  - Still contains much of the live path: baseline closure, child-plan resolution, candidate generation, child fanout, selection, continuation disposition, and CLI-facing output. Treat it as the current source of truth but not the desired long-term authority boundary.

- `crates/ploke-eval/src/cli/args/loop_args.rs`
  - CLI arguments expose the runtime modes. Notable `Prototype1StateCommand` fields: `--repo-root`, `--handoff-invocation`, `--stop-after`, `--successor-selection`, `--successor-selection-seed`, `--successor-selection-metrics`, and `--candidate-generator`.
  - Candidate generators are `legacy`, `broad-harness-request`, and `deterministic-tui-tools`.

### History projection and candidate extraction

- `crates/ploke-eval/src/cli/prototype1_state/history/projection/mod.rs`
  - Defines the narrow `History::candidates(scope)` read-only projection over verified sealed blocks.
  - It scans verified History blocks, filters selection decisions by scope, verifies selection observation hash, considered-order hash, and candidate-set commitment, then returns `HistoryCandidates` with payloads and provenance.
  - `HistoryCandidate` carries source block hash/height/lineage/entry id, decision scope, selected-by-decision bool, payload hash, optional candidate-set root, optional candidate-set membership, and the `EvaluationPayload` itself.
  - This is the best existing seam for feeding past evidence into traversal policy without expanding authority.

- `crates/ploke-eval/src/cli/prototype1_state/history/mod.rs`
  - Broad History domain definitions, including actor/subject/procedure/evidence/artifact references and block/entry structures.
  - Use this for serialization/provenance vocabulary if writing paper diagrams.

- `crates/ploke-eval/src/cli/prototype1_state/history/seal/mod.rs`
  - Sealing logic for verified History blocks. Important if a future adapter needs to write GraphRAG observations as History entries rather than just read History projections.

- `crates/ploke-eval/src/cli/prototype1_state/history/stored/mod.rs`
  - Filesystem block-store layer for persisted History segments.

### Selection, successor, and continuation

- `crates/ploke-eval/src/successor_selection/traversal.rs`
  - Holds History traversal logic referenced by `History::candidates` via `HISTORY_TRAVERSAL_PROCEDURE_ID`.

- `crates/ploke-eval/src/cli/prototype1_state/successor.rs`
  - Successor-related state and handoff types.

- `crates/ploke-eval/src/cli/prototype1_process.rs`
  - Current successor handoff and process-spawn implementation: validating continuation, preparing/installing successor artifacts, spawning/execing successor, waiting for ready, and recording ready/completion.

- `crates/ploke-eval/src/intervention/scheduler.rs`
  - Contains `decide_continuation_with_selection`, but `run/mod.rs` notes this currently exists outside the live authority path.

## Candidate adapter seam

A publication-friendly adapter can be described as a three-stage bridge:

1. Observe code graph evidence from existing RAG/query outputs.
   - Inputs: `ParsedCodeGraph`/`ModuleTree`, `Database` query rows, `EmbeddingData`, `AssembledContext`, `ContextPart`, BM25/dense scores, type-context expansion facts, and tool/request provenance from the TUI.

2. Normalize those observations into History-compatible payload/evidence records.
   - Use `SubjectRef` for code node/candidate identity, `ProcedureRef` for retrieval/scoring procedure identity, `EvidenceRef` for content-addressed snippet/query/run artifacts, and existing artifact/lineage identifiers where possible.
   - Do not mint continuation authority here. The adapter should produce evidence/candidate facts only.

3. Feed selected candidate payloads through the existing History projection / successor-selection boundary.
   - Read-side seam: `History::candidates(scope)` already validates sealed decision provenance before returning candidates.
   - Write-side seam, if needed later: History sealing in `history/seal/mod.rs`, but this requires stricter authority review.

## Why this seam is safer than changing the parser or TUI prompt path

- The parser/DB/RAG path already yields deterministic, content-addressed-ish code observations with spans, hashes, canonical paths, namespaces, scores, and snippets.
- The History path already protects lineage semantics with block hashes, candidate-set commitments, and decision verification.
- Bridging at the evidence/payload boundary avoids making automatic RAG retrieval synonymous with successor authority.
- The existing runtime docs warn that continuation authority belongs behind `Continuation<Allowed | Stopped>`, not in selection, CLI output, or child execution.

## Paper/report phrasing candidate

"Ploke separates code-understanding evidence from evolutionary authority. The GraphRAG layer parses Rust workspaces into typed code graph facts, indexes graph nodes for sparse and dense retrieval, and assembles provenance-bearing code context for model turns. The Prototype 1 History layer records parent/child attempts, candidate selection, and successor handoff as sealed transition evidence. A narrow adapter can convert retrieval observations into History candidate payloads while preserving the existing continuation gate, so retrieved code context can inform successor selection without authorizing lineage progress by itself."

## Known cautions for downstream implementation work

- `cli_facing.rs` remains a large live-path implementation surface. Reports should not imply authority is fully isolated in `run/core.rs` yet.
- `run/mod.rs` explicitly warns that `decide_continuation_with_selection` in `intervention/scheduler.rs` is not currently the live authority path.
- OpenRouter embedding code reads credentials from environment; do not include raw env values in docs or test fixtures.
- The TUI command architecture is in transition; future changes should prefer the typed command-group direction rather than adding more flat `StateCommand` cases.
- RAG context assembly currently notes some placeholder/range-normalization limitations; avoid overclaiming precise range stitching.

## Verification performed for this note

- Read repository guidance in `/home/team_ploke_dev/code/ploke/AGENTS.md` and `crates/ploke-tui/AGENTS.md`.
- Inspected the files listed above using repository-local paths.
- Checked current git status before writing; pre-existing unrelated untracked file remains: `.hermes/plans/2026-06-15_225532-ploke-call-graph-typed-plan.md`.
