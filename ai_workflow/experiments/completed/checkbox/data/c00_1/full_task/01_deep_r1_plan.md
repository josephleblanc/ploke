Let me create a structured implementation plan for adapting the `syn_parser` to work with CozoDB:

# Structured Implementation Plan: SynParser CozoDB Integration

## Task Overview
**Objective**: Adapt syn_parser to output CozoDB-compatible data streams and remove CodeGraph intermediate
**Impact Areas**: 
- `parser/graph.rs` (CodeGraph removal)
- `parser/visitor.rs` (AST traversal changes)
- All node definitions in `parser/nodes.rs`
- Serialization module
- CozoDB dependency management
**Estimated Complexity**: Medium

## Feature Flag Strategy
No feature flag needed - changes are sequential and foundational

## Subtasks

### Phase 1: Type System Alignment
- [ ] 1.1. Map Rust types to CozoDB equivalents:
  - [ ] String → String
  - [ ] Numeric IDs → Int
  - [ ] Content hashes → Bytes
  - [ ] Add `cozo` dependency to Cargo.toml
- [ ] 1.2. Add `#[derive(Send, Sync)]` to all public types in nodes.rs

### Phase 2: CodeGraph Elimination
- [ ] 2.1. Replace CodeGraph with direct Cozo writes:
  - [ ] Remove CodeGraph struct from graph.rs
  - [ ] Convert VisitorState to manage Cozo connections
  - [ ] Modify visit methods to write to CozoDB during traversal
- [ ] 2.2. Implement Cozo schema:
  - [ ] Create tables for functions/structs/relations
  - [ ] Define foreign key relationships

### Phase 3: Serialization Update
- [ ] 3.1. Remove RON serialization module
- [ ] 3.2. Add CozoDB export validation:
  - [ ] Verify type compatibility
  - [ ] Check relationship integrity

### Phase 4: Testing
- [ ] 4.1. Update integration tests:
  - [ ] Query CozoDB instead of checking CodeGraph
  - [ ] Add type validation tests
- [ ] 4.2. Add concurrency validation:
  - [ ] Verify Send/Sync properties
  - [ ] Test parallel parsing

## Dependencies
- Phase 2 depends on Phase 1 completion
- Testing phase requires full Cozo integration

## Implementation Notes

1. **Direct AST-to-Cozo Writing**:
- Maintain existing visitor pattern but insert into Cozo instead of collecting nodes
- Use Cozo's transactional API for atomic writes
- Example function insertion:
  ```rust
  fn visit_item_fn(&mut self, func: &ItemFn) {
      let fn_data = // ... parse function
      self.cozo.run("?[id, name] <- [[$id, $name]] -> :functions", 
          &[("id", fn_data.id.into()), ("name", fn_data.name.into())]
      );
  }
  ```

2. **Type Handling**:
- Keep existing IDs as Int in Cozo
- Store code snippets/contents as String
- Use Bytes for any checksums/hashes

3. **Concurrency Prep**:
- All public types already appear Send/Sync-safe
- Explicit derives ensure future-proofing
- Connection pooling will be needed later but out of current scope

Would you like me to provide detailed code examples for any specific part of this plan?
