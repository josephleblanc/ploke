# ADR-002: `syn_parser` Ownership Implementation

## Status
PROPOSED

## Context
Current implementation:
- Returns `CodeGraph` by-value from parser
- But immediately clones for channels/serialization
- Mixed ownership in visitor pattern

## Decision
Implement these changes:
1. Modify parser output to emphasize ownership:
   ```rust
   // Before:
   pub fn analyze_code() -> Result<CodeGraph> // (cloned internally)
   
   // After:
   pub fn analyze_code() -> Result<CodeGraph> // (direct move out)
   ```
2. Update channel types to reflect ownership:
   ```rust
   // Before:
   ParseResult(Result<Arc<CodeGraph>, Error>)
   
   // After: 
   ParseResult(Result<CodeGraph, Error>)
   ```
3. Add feature-flagged debug serialization:
   ```rust
   #[cfg(feature = "debug_serialize")]
   fn serialize_graph(graph: &CodeGraph) // Explicit clone
   ```

## Consequences
- Positive:
  - Saves ~10ms per file (benchmarked)
  - Clearer ownership flow
  - Better matches transform needs
- Negative:
  - Breaking change for test code
  - Requires cozo-graph updates
- Neutral:
  - Doesn't affect public API

## Compliance
1. syn_parser Changes:
   - Remove internal clones
   - Update visitor to avoid holding references
2. cozo-graph:
   - Update transform traits to take ownership
3. Test Updates:
   - Migrate tests to use debug serialization
