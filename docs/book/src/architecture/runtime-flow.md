# Runtime Flow

This page summarizes the sequence diagrams imported from the generated code wiki. See [Sequence Diagrams](./sequence-diagrams.md) for full Mermaid diagrams and source-linked walkthroughs.

## Startup and subsystem wiring

The `ploke` binary initializes tracing and calls `try_main`, which builds database, embedding, I/O, RAG, event, and app-state services before handing control to the interactive TUI loop.

## Workspace indexing

Indexing flows through `StateCommand`, `syn_parser`, `ploke-transform`, `ploke-db`, `ploke-embed`, `ploke-io`, and BM25. The parser discovers and parses Rust workspace members, transform inserts graph facts, and the indexer fills sparse/dense retrieval structures.

## Chat turn with RAG and tools

A chat turn can retrieve code context through `RagService`, route a request through `ploke-llm`, validate tool calls in `ploke-tui`, and perform filesystem effects through `ploke-io`.

## Passive records to read-side graph

Prototype 1 run files are loaded as passive records and projected into read-only graph structures. This remains separate from runtime authority to advance eval state or apply changes.
