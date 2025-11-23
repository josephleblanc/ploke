# Multi-Embedding Progress Report

## Scope of work (this session)
- Refined the multi-embedding implementation to be vector-only (no metadata dual-write) and validated `ploke-db` tests with and without `multi_embedding` feature.
- Sanitized vector relation naming so Cozo scripts accept model IDs containing slashes (`emb_<model>_<dims>`).
- Updated pending/unembedded logic to consider runtime vector relations; added a concrete test to prove runtime vectors drop pending counts and are filtered from unembedded batches.
- Adjusted HNSW/index creation call sites to accept `EmbeddingModelId`; fixed TUI/RAG/test harness call sites to pass a model and normalized cfg flags.
- Repaired multi-embedding tests to avoid mixed-dimension batches and to use the per-dimension embedding model for vector row lookups.
- Seeded schema expectations in legacy fixture tests so legacy imports do not require multi-embedding relations.
- Verified `cargo test -p ploke-db` and `cargo test -p ploke-db --features multi_embedding` now pass (warnings remain only from intentional `todo!()` in observability tests).

## Intended functionality of the new multi-embedding feature
- Store embeddings in per-dimension vector relations (one relation per embedding model/dimension), avoiding the legacy single `embedding` column and avoiding metadata dual-writes.
- Support multiple embedding sets (provider/model/dimension) in parallel; batch updates are directed to the correct relation via `VectorDimensionSpec` lookup.
- HNSW indexes are created per vector relation so searches run against the correct embedding set and return nodes joined back to primary relations.
- Pending/embedded computation uses runtime vector relations: nodes with vectors are filtered out of “pending” lists; counts subtract embedded IDs gathered across all vector relations.
- Relation naming is sanitized and stable for Cozo/HNSW (`emb_<safe_model>_<dims>`), preventing script parse failures from raw model IDs.
- Legacy path remains available (single `embedding` column, legacy fixtures) and can coexist; multi-embedding can be toggled via feature/runtime gate.

## Current gaps / follow-ups
- Warnings from `todo!()` placeholders in `tests/observability_tests.rs` remain.
- Relation naming is sanitized but still derived from model strings; consider hashing/registry if user-provided IDs are untrusted or very long.
- Runtime selection of embedding model/dimension is still driven by static `sample_vector_dimension_specs`; dynamic registration isn’t implemented.
- TUI/RAG config still defaults to the first spec; a single “current embedding model” setting should flow from user config → embedder → DB/HNSW helpers.
- Old metadata dual-write tests are removed/updated; if metadata tables are still needed for reporting, new helpers would be required.

## Quick validation evidence
- `cargo test -p ploke-db` ✅
- `cargo test -p ploke-db --features multi_embedding` ✅ (doc tests with `clear_hnsw_idx` example ignored by design; observability tests emit warnings from `todo!()`).
