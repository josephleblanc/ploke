# Glossary

## AppState

The central TUI application state owned by `ploke-tui` and updated through command/event boundaries.

## BM25

Sparse lexical retrieval used alongside dense vector search.

## Code graph

The parsed representation of Rust crates, modules, items, relations, spans, and metadata that Ploke stores and queries.

## Cozo

The embedded graph/relational database used by Ploke for code graph and retrieval state.

## Embedding set

The active provider/model/dimension identity for vector rows. Keeping this identity explicit prevents mixing incompatible vectors.

## RAG

Retrieval-augmented generation: search and context assembly before or during an LLM turn.

## StateCommand

A command boundary used to route state mutations through explicit messages rather than arbitrary shared mutation.

## Passive record

A typed persisted record used as evidence for replay or projection. Passive records should not grant runtime authority.
