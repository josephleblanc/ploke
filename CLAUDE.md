# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

Ploke is building a production-ready agentic system that can autonomously implement user goals via code edits, while preserving user control, observability, and correctness. The system features a powerful terminal UI (TUI) for LLM interaction, backed by a comprehensive Retrieval-Augmented Generation (RAG) system with a code graph built by parsing Rust crates.

### Current High-Level Focus: Agentic System Development

The project is actively developing an agentic workflow with these principles:
- **Autonomy with control**: Human-in-the-loop by default with progressive autonomy
- **Observability**: Every action auditable with persisted logs, diffs, and tool calls
- **Validity**: Automated checks (diff preview, compile, test, lint) before changes
- **Composability**: Modular tools, agents, and workflows; LLMs can call tools and other LLMs
- **Safety-first editing**: Staged edits with verified file hashes, atomic application

### Current Low-Level Focus: Model Picker in Terminal User Interface

After recently finishing internal SDK for OpenRouter API, we are expanding our UI
- Updating our model picker overlay with expanded functionality
- See TODO list at the end of this document.
- Use the `xtask` helper (`cargo xtask verify-fixtures`, see `/xtask`) before running suites so fixture requirements are enforced consistently across machines.

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

## Performance Optimization Methodology

When optimizing hot paths or addressing allocation issues, follow this systematic methodology to avoid regressions and ensure meaningful improvements.

### Systematic Performance Analysis Process

1. **Map the Complete Data Flow**
   - Trace values from initial deserialization through final consumption
   - Document every type conversion: `Type A → method() → Type B → method() → Type C`
   - Identify where data crosses async boundaries or thread boundaries
   - Note every allocation, clone, and ownership transfer

2. **Question All Design Constraints**
   - Don't accept existing APIs as immutable when optimizing
   - Ask "Why does this structure need `String` when we have an enum?"
   - Consider whether intermediate representations are necessary
   - Evaluate if event/message structures can be made more efficient

3. **Follow Ownership Patterns End-to-End**
   - Track every `.clone()`, especially in loops or hot paths
   - Identify unnecessary moves vs borrows
   - Look for places where data is parsed then immediately re-serialized
   - Check for values that are moved then immediately cloned elsewhere

4. **Holistic vs Incremental Optimization**
   - Start with end-to-end data flow analysis before making changes
   - Consider architectural changes alongside micro-optimizations
   - Don't optimize individual allocations without understanding the larger context
   - Look for systemic patterns (e.g., "we're converting enums to strings everywhere")

### Performance Anti-Patterns to Avoid

- **Surface-Level Analysis**: Only looking at immediate allocations without tracing data flow
- **API Constraint Acceptance**: Treating existing structures as unchangeable during optimization
- **Clone Blindness**: Missing obvious `.clone()` calls while focusing on other allocations
- **Boundary Ignorance**: Not understanding where data crosses async/thread boundaries
- **Type Conversion Chains**: Allowing `A → B → C → D` conversions when `A → D` might be possible

### Safe Design Constraint Evolution

**Risk Assessment Framework** - Before considering structural changes, evaluate:

1. **Blast Radius Analysis**
   ```bash
   # Find all usages of the structure
   rg "StructName" --type rust
   rg "\.field_name" --type rust -A 2 -B 2  # Field access patterns
   ```
   - Map every consumer of the data structure
   - Identify which are hot paths vs cold paths
   - Note any serialization boundaries (JSON, database, network)

2. **Change Compatibility Assessment**
   - **Backward Compatible**: Adding optional fields, widening types
   - **Forward Compatible**: Changing internal representation while preserving interface
   - **Breaking Changes**: Removing fields, narrowing types, changing semantics

**Safe Change Strategies:**

1. **Gradual Migration Pattern**
   ```rust
   // Phase 1: Add new field alongside old
   pub struct Event {
       pub name: String,           // Legacy
       pub name_typed: Option<ToolName>,  // New
   }
   
   // Phase 2: Populate both during transition
   // Phase 3: Update consumers to use new field
   // Phase 4: Remove legacy field
   ```

2. **Feature Flag Protection**
   ```rust
   #[cfg(feature = "optimized_events")]
   pub struct Event { pub name: ToolName }  // New optimized version
   
   #[cfg(not(feature = "optimized_events"))]
   pub struct Event { pub name: String }    // Existing version
   ```

**Decision Framework** - Make structural changes when:
- Performance improvement is measurable (>10% in hot paths)
- Change eliminates entire classes of bugs (e.g., invalid state representations)
- You have comprehensive test coverage for all consumers
- Migration path is clear and reversible

**Avoid structural changes when:**
- Improvement is theoretical without measurement
- Change affects serialization boundaries without compatibility plan
- Consumer code is spread across many modules without clear ownership
- No rollback strategy exists

### Optimization Validation

- **Before/After Measurement**: Always benchmark before implementing changes
- **Correctness First**: Ensure optimizations don't break async boundaries or event semantics
- **Incremental Changes**: Make one optimization at a time to isolate impact
- **Test Integration**: Verify that optimizations work across the entire pipeline, not just in isolation

### Code Review Questions for Performance

When reviewing performance-sensitive code, ask:
- "Can we eliminate this type conversion entirely?"
- "Why are we cloning this value instead of moving it?"
- "Does this data structure match how we actually use the data?"
- "Are we parsing data just to re-serialize it later?"
- "Could we use static lifetime strings or enums instead of allocated strings?"
- **Impact Analysis**: All consumers identified and migration planned?
- **Backwards Compatibility**: Can old code continue working during transition?
- **Serialization Safety**: JSON/DB schemas remain compatible?
- **Performance Evidence**: Benchmarks show actual improvement?
- **Rollback Plan**: Can change be reverted safely if issues arise?

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

### System Data Flow Reports (Generated: 2025-09-06)
For understanding existing UX and state changes, reference these comprehensive analyses:
- **App Startup**: `crates/ploke-tui/docs/reports/app_startup_dataflow_analysis.md` - 7-phase initialization from binary launch to ready state
- **Configuration Loading**: `crates/ploke-tui/docs/reports/user_config_loading_detailed.md` - TOML/env loading, API key resolution, OpenRouter integration
- **User Query Processing**: `crates/ploke-tui/docs/reports/user_query_api_dataflow_analysis.md` - Complete flow from input to LLM response including RAG context assembly
- **Model Commands**: `crates/ploke-tui/docs/reports/model_commands_dataflow_analysis.md` - Command parsing, registry validation, OpenRouter search integration
- **Event Flow Summary**: `crates/ploke-tui/docs/reports/event_flow_summaries.md` - Consolidated flow diagrams and event priority architecture

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

## General guidelines

- Use ArcStr over Arc<str> or String across threads, defined in ploke_core::ArcStr
  - located in `crates/ploke-core/src/arc_str.rs` from workspace root
  - re-exported as `ploke_core::ArcStr`
- Do not prefer ArcStr over &'static str for static strings
- Testing in `ploke-tui`
Use more crate-local tests instead of having a folder with tests outside of src, since it
forces us to make our data structures public.
Instead, we can run our integration tests here. Since we are a user-facing application, we are
more concerned with running tests that ensure our application works and is correct than
providing public-visibility functions to other applications.
In short, `ploke-tui` is a binary, first, not a lib.
- If you have an "Error editing file", check your spelling on "openai-codx", it should be "openai-codex"
- If you have an "Error reading file", check your spelling on "openai-codx", it should be "openai-codex"

## TODO

1. [x] Generate summary and full data flow for existing UX and state changes *(Completed 2025-09-06)*
- [x] Created comprehensive reports in `crates/ploke-tui/docs/reports/`
- [x] **App startup and initialization** - See `app_startup_dataflow_analysis.md`
  - [x] User config loading (TOML/env priority, defaults merging, API key resolution)
  - [x] BM25 service initialization and database setup
  - [x] System state summary (embedding processors, RAG service, model registry)
- [x] **Initial user query + API dataflow** - See `user_query_api_dataflow_analysis.md` 
  - [x] Complete flow from keyboard input to LLM response
  - [x] RAG context assembly (vector + BM25 search)
  - [x] Message lifecycle and UI updates
- [x] **Model commands dataflow** - See `model_commands_dataflow_analysis.md`
  - [x] Command parsing and validation (model list, info, use, refresh, load, save)
  - [x] Model search UI + OpenRouter API integration
  - [x] Provider selection and registry updates
- [x] **Event flow summaries** - See `event_flow_summaries.md`
  - [x] Priority architecture (realtime vs background events)
  - [x] Error handling and recovery patterns
- [x] **Provider commands** - See `model_commands_dataflow_analysis.md` (sections 3.1 & 4)
  - [x] Provider strictness policies (openrouter-only|allow-custom|allow-any)
  - [x] Provider tools-only filtering
  - [x] Provider select/pin for specific OpenRouter endpoints

2. Implement UI/UX improvements
- [ ] Improve model selection
  - [ ] Cache OpenRouter endpoints `Endpoints` for each model after user search
    - [ ] add `EndpointsCache`, keyed by `model_id`, e.g. `moonshotai/kimi-k2-0905`
    - [ ] update model picker to use cache when searching, before querying OpenRouter
- [ ] Expand model picker window functionality
  - [ ] Add scrolling functionality to model picker window
  - [ ] Add a way to select from among the provider list in window UI
  - [ ] Descend down into the list of endpoints with `l` or `RightArrow`
  - [ ] Ascend back up to list of models with `h` or `LeftArrow`
  - [ ] Select a model with `s` from model list (implemented), and select an
  endpoint from the list of endpoints with `s`
    - [ ] Selection should leave model picker window for model selection
    - [ ] Selection should not leave model picker window for endpoint selection
      - [ ] Instead, add a new data structure `SelectedEndpoints`, map `model_id` to `Endpoint`
      - `SelectedEndpoints` will be used during request to OpenRouter using
      `CompReq` formation in `session.rs` or `llm/mod.rs`
      - [ ] Selecting an endpoint will 
        - set `use_fallback` (forgetting exact name) to false by default
        - change `active_model` to endpoint's model name
    - [ ] Add a new command to remove a model from the `SelectedEndpoint` with `r`
- [ ] Revisit `ModelRegistry`
- [ ] Add Improve feedback for tool items in the conversation
  - [ ] new trait item: summary, with short summary of tool results
  - [ ] implement for each tool in `ploke-tui/src/tools/`
    - [ ] code edit
    - [ ] get file metadata
    - [ ] request code context
  - [ ] update tool results to include both a summary and the full results
  - [ ] when tool results are received in `session.rs`, change the added tool
  message to show summary
  - [ ] Full results are sent to a new struct that wraps a map,
  `ToolResultsCache`, mapping the summary message id to their `String` of json
  output
    - [ ] also included is any metadata about the tool call (e.g. `stats` from `AssembledContext` for `request_code_context`)
    - [ ] change `context`
