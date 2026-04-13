# Architecture Decision: xtask Commands Feature

**Date:** 2026-03-25  
**Milestone:** M.2.2 - Design Consolidation  
**Status:** DECISION MADE

## Decision

**Selected Architecture: Proposal 3 (Executor Pattern with Resources)**

With modifications from Proposals 1 and 2.

## Consensus

All 5 review agents unanimously recommend Proposal 3.

## Key Reasons for Selection

### 1. Only Proposal Meeting All Requirements
- **Usage Tracking**: Required by Section B (rolling suggestions every 50 runs) - only Proposal 3 has `UsageTracker`
- **Test Infrastructure**: Required by Section E (TDD approach) - only Proposal 3 has `TestableCommand` + `CommandTestHarness`
- **Resource Management**: Critical for database and embedding runtime lifecycle - only Proposal 3 has `CommandContext` with `OnceLock`
- **Sync/Async Hybrid**: Required for mixing parsing (sync) and embedding (async) - only Proposal 3 has `MaybeAsync` pattern

### 2. Superior Design Attributes
- **Modularity**: Trait-based with dependency injection
- **Extensibility**: Registry pattern with auto-discovery
- **Maintainability**: Centralized resource management
- **Testability**: Dedicated test harness with fixture support

## Modifications from Other Proposals

### From Proposal 1 (Layered Modular)
- Adopt per-crate module organization: `commands/parse/`, `commands/db/`, etc.

### From Proposal 2 (Clap-based)
- Adopt `OutputFormatter` trait with Human/JSON/Table/Compact implementations
- Adopt `ErrorCode` enum for structured error identification
- Adopt `RecoveryHint` struct for enriched error context
- Use clap derive macros for CLI argument parsing

## Architecture Overview

### Core Components

```
xtask/src/
├── main.rs                    # CLI entry point with clap
├── lib.rs                     # Public exports
├── error.rs                   # XtaskError with recovery hints
├── context.rs                 # CommandContext with lazy resources
├── executor.rs                # CommandExecutor with registry
├── usage.rs                   # UsageTracker for statistics
├── test_harness.rs            # CommandTestHarness
├── formatter/                 # Output formatting
│   ├── mod.rs                 # OutputFormatter trait
│   ├── human.rs               # Human-readable
│   ├── json.rs                # JSON output
│   ├── table.rs               # TSV/aligned
│   └── compact.rs             # Single-line
└── commands/                  # Per-crate modules
    ├── mod.rs                 # Common command traits
    ├── parse.rs               # A.1 syn_parser commands
    ├── transform.rs           # A.2 ploke_transform commands
    ├── ingest.rs              # A.3 ploke_embed commands
    ├── db.rs                  # A.4 ploke_db commands
    ├── pipeline.rs            # Cross-crate pipelines
    ├── tui.rs                 # A.5 headless TUI
    └── tool.rs                # A.6 tool execution
```

### Key Traits

```rust
/// Core command trait
trait Command: Send + Sync + 'static {
    type Output: Serialize;
    type Error: Into<XtaskError>;
    
    fn name(&self) -> &'static str;
    fn requires_async(&self) -> bool;
    fn execute(&self, ctx: &CommandContext) -> Result<Self::Output, Self::Error>;
}

/// Testable command extension
trait TestableCommand: Command {
    fn with_fixture(&self, fixture: &str) -> Self;
    fn expected_invariants(&self) -> Vec<Box<dyn Fn(&Self::Output) -> bool>>;
}

/// Output formatting
trait OutputFormatter {
    fn format<T: Serialize>(&self, output: &T) -> String;
}
```

### Error Handling

```rust
enum XtaskError {
    Parse(SynParserError),
    Transform(TransformError),
    Db(DbError),
    Embed(String),
    InvalidInput { field: String, reason: String },
    // ...
}

struct RecoveryHint {
    pub code: ErrorCode,
    pub message: String,
    pub suggestion: String,
}
```

## Implementation Phases

### Phase 1: Foundation (M.3)
- Core traits (`Command`, `TestableCommand`)
- `CommandContext` with database initialization
- `CommandExecutor` with registry
- `XtaskError` with recovery hints
- 2-3 simple sync commands as proof of concept

### Phase 2: Command Suite (M.4)
- Implement all A.1-A.4 commands
- Add output formatters
- Usage tracking
- Integration tests

### Phase 3: Advanced Features (M.5)
- Headless TUI commands (A.5)
- Tool execution commands (A.6)
- Pipeline commands
- Documentation generation

## Success Criteria

1. **All commands provide useful feedback** (Section C invariant 1)
2. **All commands have help entries** (Section C invariant 3)
3. **Error handling uses `ploke_error::Error`** (Section D)
4. **Usage statistics persisted** (Section B.4)
5. **Tests written before implementation** (Section E)

## Risk Mitigation

| Risk | Mitigation |
|------|------------|
| Higher complexity | Start with simpler Command trait, add features incrementally |
| Over-engineering | Defer EventBus, doc gen to Phase 3 |
| Proc macro dependency | Implement manually first, add derive macros later |
| Learning curve | Comprehensive documentation and examples |

## Next Steps

1. Dispatch task adherence agent to review M.2 completion
2. Begin M.3.1: Implement Types + Plan next steps
3. Create engineering agents for foundation implementation

---

**Decision Approved By:** 5/5 Review Agents  
**Recommended By:** Task Adherence Agent (pending)  
**Proceed To:** M.3 - Implement Architecture Foundation
