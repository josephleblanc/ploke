# SCOPE_DEFINITION
Objective: Build Rust RAG system for code tasks
Core Components:
- AST parser (syn-based) in crate `syn_parser`
- Hybrid graph/vector DB (cozo) unimplemented
- Local LLM Integration (7B-13B models) unimplementd
Constraints:
- Must rust on consumer GPUs (8GB VRAM)
- Preserve rust type relationships in graph
- All components written in rust
Non-Goals:
- Support non-Rust languages
- Cloud dependencies
