# Ingest and Indexing Crates

## `syn_parser`

Parses Rust crates and workspaces. It is responsible for discovering files, parsing syntax, building module structure, and producing graph-shaped semantic records.

## `ploke-transform`

Transforms parser output into Cozo relation rows. This crate is the seam between Rust syntax/semantic structures and database schema.

## `ploke-embed`

Manages embedding providers, embedding-set identity, and background indexing. It writes vectors for code nodes so dense retrieval can participate in RAG.

## `ploke-mbe`

Supports macro-by-example work in the ingest layer. Keep macro-specific behavior separated from the main parser pipeline where practical.
