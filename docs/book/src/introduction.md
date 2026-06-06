# Introduction

> Imported from the generated Hermes code wiki at `~/.hermes/wikis/ploke` (generated `2026-06-03T03:55:43Z`, source commit `97b1a101b9363b88b8e41f1447cb36d76a3eb8a2`). Verify implementation details against current source before making changes.


Ploke is a Rust 2024 workspace centered on a terminal AI assistant (`ploke`) that can index a local Rust codebase, store its structure in a Cozo-backed graph, retrieve code context with BM25 and dense embeddings, route prompts to LLM providers, and apply code-oriented tool calls through a safer I/O actor. The same workspace also contains typed protocol, passive-record, and tree-projection crates used by Prototype 1 evaluation and browser/egui inspection surfaces.

Original source snapshot: [`/home/brasides/code/ploke`](https://github.com/josephleblanc/ploke/tree/97b1a101b9363b88b8e41f1447cb36d76a3eb8a2)
Generated from commit: `97b1a101b9363b88b8e41f1447cb36d76a3eb8a2`

## Key Concepts

- **Workspace code graph** — `syn_parser` discovers Rust crates and builds `ParsedCodeGraph` plus `ModuleTree`; `ploke-transform` stores that shape as Cozo relations.
- **Retrieval substrate** — `ploke-db`, `ploke-embed`, and `ploke-rag` combine persisted graph rows, embedding sets, HNSW dense search, BM25 sparse search, and context assembly.
- **TUI runtime** — `ploke-tui` starts the user-facing app, owns `AppState`, runs command dispatch, and coordinates LLM/tool loops.
- **Provider routing** — `ploke-llm` models chat requests, routes, retries, response parsing, and provider calibration without depending on the TUI.
- **Safe file effects** — `ploke-io` serializes reads, scans, and writes through a dedicated actor with hash, range, and path-policy checks.
- **Eval/protocol records** — `ploke-protocol`, `ploke-records`, and `ploke-tree` separate typed procedure artifacts and passive read-side projections from runtime authority.

## Entry Points

- [`crates/ploke-tui/src/main.rs`](https://github.com/josephleblanc/ploke/blob/97b1a101b9363b88b8e41f1447cb36d76a3eb8a2/crates/ploke-tui/src/main.rs) — binary entry point for `ploke`; initializes tracing and calls `try_main`.
- [`crates/ploke-tui/src/lib.rs`](https://github.com/josephleblanc/ploke/blob/97b1a101b9363b88b8e41f1447cb36d76a3eb8a2/crates/ploke-tui/src/lib.rs) — subsystem startup: config, database, embedding runtime, I/O actor, BM25, RAG service, app state, channels, and TUI event loop.
- [`crates/ingest/syn_parser/src/lib.rs`](https://github.com/josephleblanc/ploke/blob/97b1a101b9363b88b8e41f1447cb36d76a3eb8a2/crates/ingest/syn_parser/src/lib.rs) — parsing entry points such as `parse_workspace_with_config`, `parse_workspace`, and `try_run_phases_and_resolve`.
- [`crates/ploke-rag/src/lib.rs`](https://github.com/josephleblanc/ploke/blob/97b1a101b9363b88b8e41f1447cb36d76a3eb8a2/crates/ploke-rag/src/lib.rs) — public RAG API: `RagService`, `RetrievalStrategy`, fusion functions, and context assembly types.
- [`crates/ploke-io/src/lib.rs`](https://github.com/josephleblanc/ploke/blob/97b1a101b9363b88b8e41f1447cb36d76a3eb8a2/crates/ploke-io/src/lib.rs) — public I/O actor API via `IoManagerHandle`.
- [`xtask/README.md`](https://github.com/josephleblanc/ploke/blob/97b1a101b9363b88b8e41f1447cb36d76a3eb8a2/xtask/README.md) — repository automation commands for fixtures, model catalogs, and backup DB validation.

## High-Level Architecture

Ploke has two main planes. The interactive plane starts in `ploke-tui`: commands mutate `AppState`, indexing fills the graph database, RAG retrieves context, `ploke-llm` performs model calls, and TUI tools use `ploke-io` for checked file effects. The evaluation/projection plane stores typed records and passive evidence, then lets `ploke-tree` and UI crates build read-only graphs without gaining mutation authority.

See the [Architecture Overview](./architecture/index.md) for diagrams and end-to-end flows.

## Module Map

| Module | Purpose |
|---|---|
| [`ploke-tui`](./crate-guide/ploke-tui.md) | Release-facing terminal application and orchestration runtime. |
| [`syn_parser`](./crate-guide/syn-parser.md) | Rust crate/workspace discovery, parsing, graph construction, and module resolution. |
| [`ploke-transform`](./crate-guide/ploke-transform.md) | Converts parsed Rust graphs into Cozo relations and schema-shaped rows. |
| [`ploke-embed`](./crate-guide/ploke-embed.md) | Embedding providers, active embedding-set runtime, and indexing task. |
| [`ploke-db`](./crate-guide/ploke-db.md) | Cozo database wrapper, schema setup, HNSW search, BM25 support, and namespace import/export. |
| [`ploke-rag`](./crate-guide/ploke-rag.md) | Sparse/dense/hybrid retrieval, fusion, reranking hooks, and token-budgeted context assembly. |
| [`ploke-llm`](./crate-guide/ploke-llm.md) | Provider routing, chat request/response types, retry policy, and model registry abstractions. |
| [`ploke-protocol`](./crate-guide/ploke-protocol.md) | Typed step/procedure abstractions and eval/adjudication artifacts. |
| [`ploke-io`](./crate-guide/ploke-io.md) | Actor-backed file reads, hash-verified snippets, scans, atomic writes, and path policy. |
| [`ploke-tree`](./crate-guide/ploke-tree.md) | Read-only run forest, history graph, playback, and artifact projections over passive records. |

## Diagrams

- [Architecture flowchart](./architecture/index.md#system-diagram)
- [Class/type diagram](./architecture/type-diagram.md)
- [Sequence diagrams](./architecture/sequence-diagrams.md)

## Getting Started

See [Quick Start](./quick-start.md).

## Scope Notes

This wiki intentionally documents a bounded core of 10 crates. Supporting crates such as `ploke-core`, `ploke-records`, `ploke-eval`, `ploke-egui`, and the proc-macro crates are referenced where they anchor the selected modules, but they do not each get a full deep-dive in this pass.
