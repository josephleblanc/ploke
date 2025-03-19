Let me create the implementation plan for the "Type Alignment" task in your requirements:

# Structured Implementation Plan: Type Alignment

## Task Overview
**Objective**: Align internal type representations with CozoDB's type system while maintaining existing data flow
**Impact Areas**: 
- parser/nodes.rs
- parser/types.rs
- serialization/ron.rs
- visitor.rs
**Estimated Complexity**: Medium

## Feature Flag Strategy
Not required for type changes as they can be atomically implemented

## Subtasks

### Phase 1: Analysis
- [ ] 1.1. Map CozoDB types to Rust representations
  - String => String
  - Bytes => Vec<u8>
  - UUID => uuid::Uuid
  - Number => enum { Int(i64), Float(f64) }
  - Etc.
- [ ] 1.2. Audit current type usage in CodeGraph and nodes

### Phase 2: Implementation
- [ ] 2.1. Overhaul type representations
  - [ ] 2.1.1. Replace String content hashes with Vec<u8>
  - [ ] 2.1.2. Add CozoNumber enum to types.rs
  - [ ] 2.1.3. Update TypeKind variants to match Cozo type system
- [ ] 2.2. Modify serialization 
  - [ ] 2.2.1. Update RON serializer for new types
  - [ ] 2.2.2. Add custom serialization for Cozo-specific formats
- [ ] 2.3. Ensure Send + Sync
  - [ ] 2.3.1. Add derive/impl Send + Sync for all public types
  - [ ] 2.3.2. Validate thread safety

### Phase 3: Testing & Validation
- [ ] 3.1. Add round-trip tests for type conversions
- [ ] 3.2. Validate against Cozo schema compatibility
- [ ] 3.3. Benchmark memory impact

## Dependencies
- Subtask 2.2 depends on 2.1 completion
- Subtask 3.1 requires 2.2 to test serialization

## Implementation Notes
1. **Number Handling**:
```rust
// Old: Simple f64 usage
pub value: f64,

// New: Cozo-compatible number
pub value: CozoNumber,

// In types.rs
pub enum CozoNumber {
    Int(i64),
    Float(f64)
}
```

2. **Byte Storage**:
```rust
// Old
pub content_hash: String,

// New
pub content_hash: Vec<u8>, // Stored as Blob in Cozo
```

3. **Type Graph Alignment**:
- Modify TypeKind variants to match Cozo's column types
- Preserve type relationship tracking during conversion

4. **Serialization**:
- Add custom serialization logic for Cozo-specific types
- Maintain RON compatibility for existing tests while phasing out

Would you like me to elaborate on any specific part of this plan?
