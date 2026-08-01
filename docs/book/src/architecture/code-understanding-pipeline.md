# Code Understanding Pipeline

Ploke's code-aware behavior depends on a staged pipeline from Rust source to retrieved prompt context.

## 1. Parse

`syn_parser` discovers Cargo targets and parses Rust files into graph-shaped structures such as parsed code graphs and module trees.

## 2. Transform

`ploke-transform` converts parsed graph structures into database-ready relations. This keeps parsing semantics separate from storage shape.

## 3. Store

`ploke-db` owns Cozo schema setup and query helpers. It is the storage boundary for graph rows, relations, vector rows, and sparse-search metadata.

## 4. Embed and index

`ploke-embed` manages active embedding provider/model state and the background indexing task that fills vector relations.

BM25 support provides sparse lexical retrieval alongside dense search.

## 5. Retrieve and assemble context

`ploke-rag` chooses dense, sparse, or hybrid retrieval strategies, fuses results, applies token budgets, and assembles code context for prompts and tools.

## 6. Route to model and tools

`ploke-llm` handles provider-facing chat requests and responses. `ploke-tui` integrates those outcomes with UI messages and tool execution.
