Let's create a focused plan for the minimal `syn_parser` rework:

# Task Breakdown: CozoDB Type Alignment & CodeGraph Reduction

## Main Task Description:
Adapt `syn_parser` to emit CozoDB-compatible data with minimal architectural changes while ensuring thread safety.

## Feature Flag: `feature_cozo_types`
Applied to breaking type changes and new Cozo-specific serialization code.

---

## Subtasks:

### 1. Type System Alignment
- [ ] Change ID types to Cozo-compatible Ints
  - Replace `NodeId`/`TypeId` from `usize` to `u64` (matches Cozo's 64-bit Number)
  - Update ID generation in `VisitorState`
- [ ] Add byte-oriented fields
  - Add `content_hash: Vec<u8>` to relevant nodes
  - Convert string-based hashes to BLAKE3 in visitor
- [ ] Mark numeric fields explicitly
  - Use `i64`/`f64` instead of generic numeric types
  - Add `#[repr(C)]` for FFI compatibility

### 2. Thread Safety Enforcement
- [ ] Derive `Send + Sync` for all public types
  ```rust
  #[derive(Debug, Clone, Send, Sync)]
  pub struct FunctionNode { /* ... */ }
  ```
- [ ] Audit collection types
  - Replace `Vec` with `Arc<[]>` for shared immutable data
  - Verify all contained types are thread-safe

### 3. CodeGraph Streamlining
- [ ] Break monolithic structure into tables
  ```rust
  #[cfg_attr(feature = "feature_cozo_types", derive(Serialize))]
  pub struct CodeTables {
      pub functions: Vec<FunctionRow>,
      pub types: Vec<TypeRow>,
      pub edges: Vec<RelationRow>,
  }
  ```
- [ ] Add direct serialization to Cozo script
  - Replace RON output with Cozo's JSON import format
  - Generate `:put` operations during serialization

---

## Testing Strategy:
1. **Type Validation**
   - Add roundtrip tests for ID type conversions
   - Verify numeric precision with large values (>2^53)

2. **Concurrency Checks**
   - Create cross-thread passing tests for all public types
   ```rust
   #[test]
   fn test_send_sync() {
       fn assert_send_sync<T: Send + Sync>() {}
       assert_send_sync::<FunctionNode>();
   }
   ```

3. **Cozo Compatibility**
   - Add integration test that:
     1. Parses sample Rust code
     2. Generates Cozo script
     3. Loads into in-memory Cozo DB
     4. Performs basic graph queries

Key considerations: Maintain dual output capability (RON + Cozo) behind feature flags during transition. Focus on type compatibility first before optimizing data flow.
