# Syn Parser Error Handling Policy

## Core Principles

1. **Don't Panic**: Library code should never panic on bad input
2. **Be Explicit**: All possible error states should be represented in the return type
3. **Preserve Context**: Errors should include enough information to diagnose issues
4. **Graceful Degradation**: Where possible, continue processing after non-fatal errors

## Error Handling Strategy

### Current Implementation (MVP Phase)
```rust
// Simple wrapper around syn::Error
pub fn analyze_code(file_path: &Path) -> Result<CodeGraph, syn::Error> {
    let file = syn::parse_file(&std::fs::read_to_string(file_path)?)?;
    // ... processing
    Ok(graph)
}
```

### Future Evolution
```rust
#[derive(Debug, thiserror::Error)]
pub enum ParserError {
    #[error("Syntax error: {0}")]
    Syntax(#[from] syn::Error),
    
    #[error("Invalid use statement at {span:?}: {reason}")]
    Semantic {
        span: (usize, usize),
        reason: String  
    },
    
    #[error("Completed with {error_count} errors")]
    Partial {
        error_count: usize,
        graph: CodeGraph
    }
}
```

## Guidelines for Contributors

1. When adding new parsing logic:
   - Prefer returning `Result` over panicking
   - Include source location information in errors
   - Document possible error conditions

2. For error recovery:
   - Fatal errors (syntax): return `Err` immediately
   - Non-fatal (semantic): consider `Partial` variant
   - Never silently ignore errors

3. Testing requirements:
   - All error paths should be tested
   - Include malformed input tests
   - Verify error messages are actionable

## Migration Path

1. First, wrap all parsing in `Result`
2. Then, introduce our own error type
3. Finally, add partial processing capability

[See also: General Error Handling Guidelines](./error_handling_best_practices.md)
