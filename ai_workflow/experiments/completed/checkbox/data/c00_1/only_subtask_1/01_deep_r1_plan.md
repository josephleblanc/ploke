Based on the code structure and CozoDB type requirements, here's the structured implementation plan:

# Structured Implementation Plan: Type Alignment

## Task Overview
**Objective**: Align Rust types in `syn_parser` with CozoDB schema requirements
**Impact Areas**: 
- Node definitions (nodes.rs)
- Type handling (types.rs)
- Visitor processing (visitor.rs)
- Serialization (serialization/ron.rs)
**Estimated Complexity**: Medium

## Feature Flag Strategy
*Not required* - Changes are type system improvements without breaking existing API contracts

## Subtasks

### Phase 1: Analysis
- [x] 1.1. Identify type mappings:
  - String → Bytes (for hashes/raw bytes)
  - f64 → Float (explicitly mark all floats)
  - i64 → Int (explicitly mark all integers)
  - Vec<f32>/Vec<f64> → Vector types
  - Add Uuid type handling
- [x] 1.2. Identify critical fields needing conversion:
  - FunctionNode.body → Bytes (SHA-256 hash)
  - ValueNode.value → Cozo-compatible enum
  - Vector fields → <F32/F64; N> types

### Phase 2: Implementation
- [ ] 2.1. Update type definitions:
  ```rust
  // In nodes.rs
  pub struct FunctionNode {
      // pub body: Option<String>, BEFORE
      pub content_hash: Vec<u8>,  // SHA-256 hash
      pub body_source: Option<String>  // New field for raw source
  }
  
  // New type in types.rs
  pub enum CozoValue {
      Int(i64),
      Float(f64),
      Bytes(Vec<u8>),
      VectorF32(Vec<f32>),
      VectorF64(Vec<f64>),
      // Other Cozo types...
  }
  ```
  
- [ ] 2.2. Modify visitor processing:
  - Add hash computation in `visit_item_fn`
  - Update type creation for vectors/numbers
  - Handle UUID detection/parsing

- [ ] 2.3. Enhance serialization:
  ```rust
  // In serialization/ron.rs
  impl Serialize for CozoValue {
      fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error> {
          match self {
              CozoValue::Bytes(b) => serializer.serialize_bytes(b),
              CozoValue::VectorF32(v) => serialize_as_cozo_vector(v),
              // ...
          }
      }
  }
  ```

### Phase 3: Testing & Validation
- [ ] 3.1. Add type validation tests:
  ```rust
  #[test]
  fn test_byte_serialization() {
      let graph = build_test_graph();
      let bytes = serialize_to_ron(&graph);
      assert!(bytes.contains(r#"content_hash: Bytes([0x12ab])"#));
  }
  ```
  
- [ ] 3.2. Update fixture data to include:
  - Numeric literals with explicit type suffixes
  - Raw byte literals
  - Vector constructor examples

## Dependencies
- 2.2 (Visitor) depends on 2.1 (Type definitions)
- 3.1 (Tests) depends on 2.3 (Serialization)

## Implementation Notes
1. **Content Hashes**: Use blake3 for efficient hashing instead of SHA-256
2. **Type Preservation**: Maintain original source in `body_source` for debugging while storing only hashes in graph
3. **Vector Handling**: Create dedicated Vector node type differentiating F32/F64 variants
4. **Error Handling**: Add new error variants for type conversion failures

Would you like me to provide concrete code examples for any specific part of this plan?
