# ADR-001: Ownership Transfer Pipeline

## Status
PROPOSED

## Context
Current architecture (`PROPOSED_ARCH_V3.md`) shows:
1. `CodeGraph` cloning between pipeline stages
2. RON serialization as primary persistence
3. Ambiguous ownership boundaries

This causes:
- Unnecessary memory pressure
- Architectural drift from database-centric vision
- Implicit cloning costs in hot paths

## Decision
Transition to linear ownership flow:
```
Parser → Transform → DB
```
With these principles:
1. `CodeGraph` moves by-value between stages
2. Serialization becomes debug-only feature
3. All transforms consume their inputs

## Consequences
- Positive:
  - Eliminates 2-3 clone operations per file
  - Enforces cleaner pipeline boundaries
  - Better aligns with CozoDB integration
- Negative:
  - Breaks existing debug serialization tests
  - Requires careful error handling design
- Neutral:
  - Doesn't preclude future zero-copy optimizations

## Compliance
1. PROPOSED_ARCH_V3.md Changes:
   - Update data flow diagrams
   - Remove RON as primary storage
   - Document ownership boundaries
2. IDIOMATIC_RUST.md:
   - Add ownership transfer guidelines
3. CONVENTIONS.md:
   - New rule: "Prefer move semantics in pipelines"
