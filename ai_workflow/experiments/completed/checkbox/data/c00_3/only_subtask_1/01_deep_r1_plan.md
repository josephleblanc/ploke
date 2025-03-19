```
# Task Breakdown: Type Alignment with CozoDB Schema

## Main Task Description:
Refactor type representations in syn_parser to match CozoDB's type system requirements:
- Replace String-based hashes with `Bytes`
- Implement fixed-length F32/F64 Vectors for embeddings
- Add explicit Int/Float types with proper nullability
- Ensure correct type mapping for serialization

## Feature Flag: `cozo_type_alignment`
```rust
#[cfg(feature = "cozo_type_alignment")]
struct ValueNode {
    content_hash: Vec<u8>, // Was String
}

#[cfg(not(feature = "cozo_type_alignment"))]
struct ValueNode {
    content_hash: String, // Legacy
}
```

## Subtasks:
- [ ] 1. Update type representations in nodes.rs
  - [ ] 1.1 Add `Bytes` variant to ValueNode
  - [ ] 1.2 Implement Vector(FloatKind, usize) in TypeKind
  - [ ] 1.3 Separate Int/Float numeric types
  
- [ ] 2. Refactor type processing in visitor.rs
  - [ ] 2.1 Modify get_or_create_type to detect embeddings
  - [ ] 2.2 Update process_type to handle fixed vectors
  - [ ] 2.3 Add nullability checks for optional fields

- [ ] 3. Update serialization formats
  - [ ] 3.1 Add Bytes serialization to RON/JSON modules
  - [ ] 3.2 Implement fixed-vector encoding
  - [ ] 3.3 Handle type variants in serialization

## Testing Strategy:
1. Add type mapping unit tests in parser_tests.rs
2. Create roundtrip tests for all Cozo types
3. Validate against sample Cozo schemas
4. Test both feature-flag enabled/disabled states

Implements CONVENTIONS.md rules:
- Maintains Result<> error handling
- Preserves zero-copy parsing
- Adds documentation tests for new types
```
