# Remote Embedding Scratchpad

Created: 2025-11-14 19:20:54 UTC

## Inputs reviewed in this pass
- [`required-groundwork.md`](./required-groundwork.md)
- [`reference-docs.md`](./reference-docs.md) → [`remote_embedding_trait_system_design_report_2025-11-13.md`](../../reports/remote_embedding_trait_system_design_report_2025-11-13.md)
- [`remote_vector_indexing_support_report_2025-02-18.md`](../../reports/remote_vector_indexing_support_report_2025-02-18.md)
- [`deep-research_remote_embedding.md`](../../reports/deep-research_remote_embedding.md)

## Baseline readiness evidence
| Command | Timestamp (UTC) | Notes |
| --- | --- | --- |
| `cargo xtask verify-fixtures` | 2025-11-14 19:17 | Fixture DB backup + pricing json present. |
| `cargo test -p ploke-tui -q` | 2025-11-14 19:19 | `test result: ok. 132 passed; 0 failed; 4 ignored;` across unit/integration suites. |

> No evidence artifacts were emitted under `target/test-output/...` during these baseline checks, so live-gate readiness is **not** yet established.

## Blockers / missing decisions before implementation
1. **Embedding storage schema is still undefined for code** — The plan calls for a new `embedding_nodes` relation plus joins (`required-groundwork.md`, Section 1), but the concrete schema (columns, key format, relation names per node type, migration DDL) and IoManager hashing flow are not specified. We need a migration spec that covers:
   - Dual-write window semantics and verification queries before dropping legacy `<F32; 384>?` columns (`crates/ingest/ploke-transform/src/schema/primary_nodes.rs`).
   - Index naming + retention strategy for HNSW graphs keyed by `(node_type, embedding_set_id)` (`crates/ploke-db/src/index/hnsw.rs`).
   - Backfill tooling + artifact expectations so we can prove parity after the split.
2. **Embedding set activation contract is underspecified** — Versioned embedding sets and CLI verbs (`embedding list|use|drop|prune`) are mentioned, but we still lack:
   - A data model for `embedding_set` metadata (provider slug, model id, dimension, dtype, created_at, status flags) and how it links to RAG/indexing consumers.
   - Rules for switching active sets without re-indexing (e.g., how AppState + RAG caches observe the change, what happens to long-running `IndexerTask`s).
   - Error handling policies when the requested set is missing vectors for some node types.
3. **Trait stack + registry shape still open** — Reports mandate an `EmbeddingRouter`/`EmbeddingRequest` pattern, but there are unresolved questions (`remote_embedding_trait_system_design_report_2025-11-13.md`, "Open Questions"):
   - Whether we reuse the existing `Router` naming or introduce `EmbeddingProvider`.
   - How to share `WireRequest` plumbing between chat + embedding stacks.
   - How to surface pricing/cost metadata (needed for telemetry + UX) inside the registry.
   These decisions affect module layout (`crates/ploke-tui/src/embedding/...`), macro derivations, and typed request/response structs. We should lock the trait signatures and shared types before touching call sites.
4. **Runtime reconfiguration workflow is unplanned** — We know we need an `EmbeddingManager` that atomically swaps embedders (`required-groundwork.md`, Section 3), yet there is no design for:
   - Event sequencing between `/embedding use …` commands, `IndexerTask`, and RAG services to avoid stale `Arc<EmbeddingService>` handles.
   - Persistence strategy for user overrides (config vs. workspace state) and how IoManager stages the config write.
   - Hot-reload UX (overlay/notifications) so users see which provider/model/dimension is active post-switch.
5. **Telemetry + artifact format TBD** — Section 4 of the groundwork doc calls for structured tracing and JSON evidence under `target/test-output/embedding/...`, but no schema for those artifacts exists. We need to define:
   - File naming + directory layout (offline summaries vs. live provider traces) and minimum fields (pass/fail counts, provider slug, batch latency, tool-call traces).
   - How gating hooks assert that live paths actually executed (e.g., parsing recorded tool calls, verifying `EmbeddingSetId` propagation).
   - Ownership of artifact generation (IndexerTask vs. dedicated reporter) to guarantee every test run emits verifiable evidence.
6. **Provider credential + catalog sources** — Deep research collected API shapes, but we still must codify:
   - Canonical env vars/config key names per provider (OpenAI vs. HF vs. future Cohere/Azure) so the new registry can auto-detect secrets and warn when missing.
   - Source of truth for remote model catalogs (pulling OpenAI `/v1/models`, HF Hub tags, cached JSON). Without this, `/embedding list` cannot reliably show choices or dimensions.
7. **Cost/safety modeling unresolved** — Trait design notes call out cost tracking + rate limits, yet there is no plan for:
   - Sharing pricing tables (e.g., `crates/ploke-tui/data/models/all_pricing_parsed.json`) with the embedding registry.
   - Defining retry/backoff budgets and how they interact with indexing throughput guarantees.
   - Capturing evidence of live tool usage (OpenRouter-like gates) once remote embeddings are turned on.

## Immediate follow-ups
- Draft a migration doc that specifies the `embedding_nodes` schema, dual-write steps, and verification queries. Reference IoManager staging requirements.
- Finalize the trait + registry API (naming, module layout, shared wire layer) so provider implementations can start in parallel with schema work.
- Define telemetry artifact schemas and create placeholder writers so every upcoming test run produces evidence under `target/test-output/embedding/`.
- Prepare UX notes for `/embedding …` commands (parser grammar, executor flow, UI overlays) and how they coordinate with the planned `EmbeddingManager`.
- Inventory provider catalog + credential sources (env vars, config keys, cached pricing data) to unblock registry bootstrap.

This scratchpad should be extended as design decisions land and as we discover new risks while implementing the remote embedding feature.