# ADR-021: Always Index Test Items in Code Graph

## Status
ACCEPTED

## Context
When developers ask the LLM to help write or evaluate tests, the RAG system needs
to have complete context about test items (`#[test]` functions, `#[cfg(test)]`
modules, etc.). Currently, our cfg evaluation skips test items by default, which
means the LLM lacks crucial context when users need help with testing.

## Decision
Treat `#[cfg(test)]` as always true during indexing phase. Always include test
items in the code graph while still evaluating other cfg attributes normally.

## Consequences
- **Positive:**
  - LLM always has complete test context for code generation and evaluation
  - No user-facing configuration needed
  - Zero additional arguments to propagate through call chains
  - Simple implementation - one change in cfg builder

- **Negative:**
  - Slightly larger code graph (test items are typically <5% of codebase)
  - May include test utilities in general queries unless explicitly filtered

- **Neutral:**
  - Production vs test filtering can be done at query time rather than parse time

## Compliance
- PROPOSED_ARCH_V3.md: Supports always-available context for LLM operations
- IDIOMATIC_RUST.md: Uses Rust's cfg system effectively
- CONVENTIONS.md: Follows simple, predictable defaults
