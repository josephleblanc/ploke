Project structure (minimal)

- crates/
  - ploke-tui: Terminal UI, state manager, event bus, LLM integration
  - ploke-rag: Retrieval orchestration (BM25 service handle, hybrid search)
  - ploke-embed: IndexerTask and embedding pipeline (dense + BM25 actor)
  - ploke-db: Database access/helpers and Cozo integration
  - ploke-core: Core types/IDs and hashing
  - ingest/: Parsers and transforms (syn_parser, ploke-transform)
  - ploke-io: I/O manager for snippet retrieval
  - ploke-error: Error types and context helpers
- docs/
  - plans/bm25/overview.md: BM25 and hybrid retrieval plan and progress log

Notes
- EventBus splits realtime/background; UI consumes AppEvent to render.
- AppState holds shared handles (db, embedder, indexer_task, rag).
- RAG service is initialized in TUI and will service BM25/hybrid requests.

This file is intentionally concise to preserve context window.
