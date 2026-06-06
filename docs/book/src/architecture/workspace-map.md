# Workspace Map

The root `Cargo.toml` declares a Rust workspace. The release-facing default member is:

- `crates/ploke-tui` — the terminal application and primary binary.

The major subsystem groups are:

## Ingest

- `crates/ingest/syn_parser` — Rust workspace discovery and parsing.
- `crates/ingest/ploke-transform` — parsed graph to database relations.
- `crates/ingest/ploke-embed` — embedding providers and indexing runtime.
- `crates/ingest/ploke-mbe` — macro-by-example support work.

## Runtime services

- `crates/ploke-db` — Cozo database wrapper, schemas, vector and sparse search support.
- `crates/ploke-rag` — retrieval strategies, fusion, and context assembly.
- `crates/ploke-llm` — provider routing and chat request/response handling.
- `crates/ploke-io` — actor-backed file I/O and edit boundaries.

## Application and integration

- `crates/ploke-tui` — UI, command dispatch, runtime orchestration, tools.
- `crates/ploke-ty-mcp` — MCP/type integration surface.
- `xtask` — repository automation and fixture management.

## Protocol, records, and projections

- `crates/ploke-protocol` — typed procedure abstractions and artifacts.
- `crates/ploke-records` — passive persisted record schemas.
- `crates/ploke-tree` — read-only run forest, history graph, and artifact projections.
- `crates/ploke-eval` — internal evaluation/prototype tooling.
- `crates/ploke-egui`, `crates/ploke-tree-browser`, `crates/ploke-tree-egui` — browser/egui inspection surfaces.

## Foundation crates

- `crates/ploke-core`
- `crates/ploke-error`
- `crates/common`
- `crates/test-utils`
- procedural macro crates under `proc_macros/`
