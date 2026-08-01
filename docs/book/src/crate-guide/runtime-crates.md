# Runtime Crates

## `ploke-tui`

Release-facing terminal application. It owns UI state, command handling, runtime startup, model and embedding picker integration, tool dispatch, and chat-loop integration.

## `ploke-io`

Actor-backed file effects. Use it for checked reads, snippet lookup, path policy, hash verification, range-aware edits, and serialized writes.

## `ploke-db`

Cozo database boundary. Owns schema initialization, graph import/query helpers, vector rows, HNSW support, BM25 metadata, and database snapshot behavior.

## `ploke-rag`

Retrieval and context assembly. It composes sparse and dense retrieval, fusion, reranking hooks, and token-budgeted context packs.

## `ploke-llm`

Provider routing and chat protocol types. It should remain usable outside the TUI where possible.
