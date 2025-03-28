# ADR 001: Phased Visibility Resolution Approach

## Status
Accepted (2025-03-28)

## Context
To enable accurate code suggestions and refactoring, we need to determine:
1. What code is directly usable in a given scope
2. What requires imports
3. What is completely unreachable

This affects:
- Code suggestion accuracy
- Refactoring safety
- Cross-module analysis

## Decision
Implement in 3 phases:
1. **Module Path Tracking** (Completed)
   - Track item locations in module hierarchy
2. **Use Statement Handling** (Completed)  
   - Record all imports and aliases
3. **Final Resolution** (Current)
   - Combine data to determine actual visibility

## Implementation
```rust
pub enum VisibilityResult {
    Direct, // Immediately usable
    NeedsUse(Vec<String>), // Requires import
    OutOfScope { 
        reason: OutOfScopeReason,
        allowed_scopes: Option<Vec<String>>
    }
}
```

## Consequences
### Positive
- Enables context-aware suggestions
- Handles common visibility cases
- Clear upgrade path for future features

### Negative
- ~15% parsing overhead
- Doesn't handle workspaces yet
- Macro-generated items excluded from MVP

### Neutral
- Requires maintaining visibility state
- Serialization format extended
