# Workspace Structure

The project is organized as a Cargo workspace with specialized crates:

## Core Components
- `ploke-tui` - Terminal UI, agentic system, main application entry point (Tokio-based)
- `ploke-core` - Core data types (NodeId, TrackingHash, TypeId, ArcStr)
- `ploke-db` - CozoDB database for hybrid vector-graph storage
- `ploke-rag` - RAG orchestration with BM25 and hybrid search

## Ingestion Pipeline
- `syn_parser` - AST parsing using syn with visitor pattern (Rayon-based)
- `ploke-transform` - AST to database transformation (34 schemas)
- `ploke-embed` - Vector embeddings for code semantic search

## Supporting Infrastructure
- `ploke-io` - I/O handling with safety-first file operations, snippet retrieval
- `ploke-error` - Cross-crate error types
- `ploke-ty-mcp` - MCP protocol integration
- `common` - Shared utilities
- `test-utils` - Testing utilities

## Key Design Patterns
1. **Type-safe NodeIds**: Strongly typed IDs with PhantomData for compile-time safety
2. **Visitor Pattern**: syn::Visitor implementation for AST traversal
3. **Hybrid Database**: CozoDB supporting both vector and graph queries
4. **Deterministic Hashing**: TrackingHash for consistent node identification
5. **Dual-Runtime Architecture**: Rayon domain for CPU work, Tokio domain for async I/O

## Test Structure
- Unit tests for core functions
- Integration tests for visitor context and AST processing
- End-to-end tests for full parsing pipeline
- Test fixtures in `tests/fixture_crates/` for realistic scenarios