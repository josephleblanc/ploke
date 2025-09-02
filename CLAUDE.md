# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

Ploke is building a production-ready agentic system that can autonomously implement user goals via code edits, while preserving user control, observability, and correctness. The system features a powerful terminal UI (TUI) for LLM interaction, backed by a comprehensive Retrieval-Augmented Generation (RAG) system with a code graph built by parsing Rust crates.

### Current Focus: Agentic System Development

The project is actively developing an agentic workflow with these principles:
- **Autonomy with control**: Human-in-the-loop by default with progressive autonomy
- **Observability**: Every action auditable with persisted logs, diffs, and tool calls
- **Validity**: Automated checks (diff preview, compile, test, lint) before changes
- **Composability**: Modular tools, agents, and workflows; LLMs can call tools and other LLMs
- **Safety-first editing**: Staged edits with verified file hashes, atomic application

## Build Commands

```bash
# Build the entire workspace
cargo build

# Build in release mode (recommended for performance)
cargo build --release

# Run the main application
cargo run

# Run with release optimizations
cargo run --release

# Check code without building
cargo check

# Format code
cargo fmt

# Lint with Clippy
cargo clippy

# Check for gratuitous collect patterns (custom linting)
./scripts/no_gratuitous_collect.sh
```

## Test Commands

```bash
# Run all tests in the workspace
cargo test

# Run tests with output
cargo test -- --nocapture

# Run tests for specific crate
cargo test -p ploke-tui
cargo test -p syn_parser
cargo test -p ploke-db
cargo test -p ploke-transform
cargo test -p ploke-embed
cargo test -p ploke-rag
cargo test -p ploke-io

# Run specific test
cargo test test_name

# Run tests with features
cargo test --features "live_api_tests"
cargo test -p ploke-io --features watcher
```

## Development Setup

### Environment Variables
```bash
# Required for OpenRouter API integration
export OPENROUTER_API_KEY="your_key_here"

# Or use .env file (already present in repo)
```

### Running the Application
```bash
# Start the TUI application
cargo run

# In the TUI, use vim-like bindings (press 'i' for insert mode)
# Index a crate for RAG:
/index start /absolute/path/to/target/crate

# Quick test with fixtures:
/index start /path/to/ploke/tests/fixture_crates/fixture_tracking_hash
/index start /path/to/ploke/tests/fixture_crates/fixture_nodes
```

### Useful Development Scripts
```bash
# Generate project context overview
./scripts/gen_project_context.sh

# Full project overview (uncapped)
MODE=full ./scripts/gen_project_context.sh

# Custom output file
./scripts/gen_project_context.sh my_context.txt
```

## Architecture

### Current Development Priorities

**Active Workstreams (see `crates/ploke-tui/docs/feature/agent-system/agentic_system_plan.md`):**

1. **OpenRouter API & Tool Calling System**
   - Trait-based tool system with `request_more_context` using vector similarity + BM25
   - Strong typing on all request/response schemas (no stringly-typed plumbing)
   - Live API testing framework with multiple providers
   - Tool call telemetry and persistence

2. **Safe Editing Pipeline**
   - Human-in-the-loop approval flow with diff previews
   - Git integration for versioning and rollback
   - Atomic file operations with hash verification via IoManager

3. **Testing & Validation**
   - E2E testing with TEST_APP behind `#[cfg(feature = "test_harness")]`
   - Live gates discipline for API tests
   - Evidence-based readiness tracking

### Workspace Structure
The project is organized as a Cargo workspace with specialized crates:

**Core Components:**
- `ploke-tui` - Terminal UI, agentic system, and main application entry point (Tokio-based)
- `ploke-core` - Core data types (NodeId, TrackingHash, TypeId)
- `ploke-db` - CozoDB database for hybrid vector-graph storage
- `ploke-rag` - RAG orchestration with BM25 and hybrid search

**Ingestion Pipeline:**
- `syn_parser` - AST parsing using syn with visitor pattern (Rayon-based)
- `ploke-transform` - AST to database transformation (34 schemas)
- `ploke-embed` - Vector embeddings for code semantic search

**Supporting Infrastructure:**
- `ploke-io` - I/O handling with safety-first file operations, snippet retrieval
- `ploke-error` - Cross-crate error types
- `ploke-ty-mcp` - MCP protocol integration
- `common` - Shared utilities
- `test-utils` - Testing utilities

### Concurrency Model
The system uses a dual-runtime architecture:
- **Rayon Domain**: CPU-intensive parsing and analysis
- **Tokio Domain**: Async I/O, networking, and UI
- **Communication**: Flume channels between domains

### Key Design Patterns
1. **Type-safe NodeIds**: Strongly typed IDs with PhantomData for compile-time safety
2. **Visitor Pattern**: syn::Visitor implementation for AST traversal
3. **Hybrid Database**: CozoDB supporting both vector and graph queries
4. **Deterministic Hashing**: TrackingHash for consistent node identification

## Engineering Principles (from AGENTS.md)

### Core Philosophy
Our approach prioritizes **extensible, maintainable, and highly performant code** through:
- **Upfront systems design** - Invest time in architecture to avoid technical debt
- **Strong type-safety** - Make invalid states unrepresentable at compile time
- **Performance by design** - Choose efficient patterns from the start, not as an afterthought

### Non-Negotiable Standards
- **Strong typing everywhere**: No stringly typed plumbing
  - All OpenRouter-touching code must use strongly typed structs/enums with `Serialize`/`Deserialize`
  - Numeric fields as numeric types (e.g., `u32` for tokens, `f64` for costs)
  - Make invalid states unrepresentable with enums and tagged unions
  - Treat ad-hoc JSON maps and loosely typed values as errors at boundaries
- **Safety-first editing**: Stage edits with verified file hashes; apply atomically via IoManager
- **Evidence-based changes**: Run targeted and full test suites; update design/reflection docs for trade-offs
- **Live gates discipline**: When live gates are ON, tests must exercise the live path and verify key properties

### Performance & Advanced Rust Patterns
- **Static dispatch over dynamic dispatch** - Compile-time polymorphism for zero-cost abstractions
- **Macros for boilerplate reduction** - Especially for test generation and trait implementations
- **PhantomData for type-state patterns** - Compile-time state validation without runtime cost
- **GATs (Generic Associated Types)** - Enable zero-copy deserialization patterns
- **Efficient memory patterns**:
  - Prefer iterators over collecting (enforced by `no_gratuitous_collect.sh`)
  - Use `Arc`/`Rc` judiciously, prefer borrowing
  - Stack allocation over heap where possible
  - Consider `SmallVec` for small, variable-sized collections

### Code Style Guidelines
- Follow standard Rust idioms and conventions
- Use `Result<T, E>` for error handling, avoid `unwrap()` in production code
- Design APIs to make misuse difficult or impossible
- Keep public APIs minimal and well-documented
- Use feature flags for optional functionality
- Maintain comprehensive test coverage, especially for parser components
- Validate early at boundaries, transform to strongly-typed internal representations

## Testing Strategy

The project uses a tiered testing approach:

1. **Unit Tests**: Core function testing (ID generation, hashing)
2. **Integration Tests**: Visitor context gathering and AST processing
3. **End-to-End Tests**: Full parsing pipeline with fixtures
4. **Structural Tests**: Graph relationships and node connections

Test fixtures are located in `tests/fixture_crates/` for realistic parsing scenarios.

## Common Development Tasks

### Adding New Node Types
1. Define the node type in `ploke-core`
2. Add visitor implementation in `syn_parser`
3. Create transformation schema in `ploke-transform`
4. Add corresponding tests with fixtures

### Debugging Parser Issues
1. Use test fixtures in `tests/fixture_crates/`
2. Check known limitations in `docs/plans/uuid_refactor/`
3. Run specific parser tests: `cargo test -p syn_parser`

### Working with the Database
1. Cozo queries are in Datalog syntax
2. Use `/query` command in TUI for testing queries
3. Database schemas defined in `ploke-transform/src/schema/`

## Important Documentation

### Key Planning Documents
- **Agentic System Roadmap**: `crates/ploke-tui/docs/feature/agent-system/agentic_system_plan.md`
- **Engineering Principles**: `AGENTS.md`
- **Architecture Overview**: `PROPOSED_ARCH_V3.md`
- **Implementation Logs**: `crates/ploke-tui/docs/agent-system/impl-log/`
- **Decisions Required**: `crates/ploke-tui/docs/decisions_required.md`

### Development Workflow
- Plans and logs live in `crates/ploke-tui/docs/plans/agentic-system-plan/` and `crates/ploke-tui/docs/reports/`
- Implementation logs track decisions with evidence and cross-references
- Request human input when blockers are encountered or tests need cfg gating

### Current Limitations
- Parser has known limitations in `docs/plans/uuid_refactor/02c_phase2_known_limitations.md`
- No callers/callees or full type resolution yet
- Vector embeddings use sentence-transformers by default (GPU support planned)
- MCP integration is in prototype phase

## Agentic System Milestones (Summary)

**M0: Baseline hardening** - Telemetry, persistence, event deduplication  
**M1: Safe editing pipeline** - Human-in-the-loop approval with diff previews  
**M2: Context/navigation tools** - Enhanced LLM tooling for code exploration  
**M3: Automated validation** - Compile, test, lint gates before changes  
**M4: Single-agent loop** - Plan → act → observe → reflect workflow  
**M5: Multi-path search** - Parallel solution exploration and scoring  
**M6: Knowledge graph** - Conversation persistence and retrieval  
**M7: User personalization** - Profile-based generation tuning  
**M8: Multi-agent orchestration** - Role-based collaboration  
**M9: Reliability & ops** - Checkpointing, metrics, resilience  
**M10: Packaging & docs** - Templates and onboarding