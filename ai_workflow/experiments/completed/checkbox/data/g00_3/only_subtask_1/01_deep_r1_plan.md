```
PLAN
1. **Analyze CozoDB type system requirements**
   - Effort: 1h
   - Breaking Change: No
   - Commit: docs(cozo-types): Document type mapping requirements
   - Dependencies: None
   - Reasoning: Establish clear mapping between Rust types and Cozo requirements (from cozodb_docs_types.txt) for UUIDs, Bytes, Vectors (F32/F64), Json, Validity

2. **Extend TypeNode with Cozo-specific types**
   - Effort: 2h
   - Breaking Change: Yes (changing type_graph structure)
   - Flag: `feature_cozo_type_alignment`
   - Commit: feat(types): Add CozoDB type variants to TypeKind
   - Dependencies: #1
   - Reasoning: Update TypeKind enum in types.rs with variants:
     ```rust
     pub enum TypeKind {
         // Existing variants...
         Bytes,
         Uuid,
         VectorF32(usize),
         VectorF64(usize),
         Json,
         Validity,
     ```
     Modify TypeNode relationships to support nested type requirements

3. **Enhance type detection in visitor**
   - Effort: 3h
   - Breaking Change: Yes
   - Flag: `feature_cozo_type_alignment`
   - Commit: feat(parser): Detect Cozo-compatible types during analysis
   - Dependencies: #2
   - Reasoning: Update get_or_create_type and process_type in visitor.rs to:
     - Recognize Vec<u8> as Bytes
     - Identify uuid::Uuid types
     - Handle fixed-length float arrays as Vectors
     - Detect serde_json::Value as Json type

4. **Update serialization formats**
   - Effort: 2h
   - Breaking Change: Yes (output format changes)
   - Flag: `feature_cozo_type_alignment`
   - Commit: feat(serialization): Align RON output with Cozo types
   - Dependencies: #3
   - Reasoning: Modify ron.rs serialization to:
     - Store Bytes as base64 strings
     - Represent Vectors with type+length metadata
     - Preserve UUID formatting

5. **Add type validation layer**
   - Effort: 2h
   - Breaking Change: No
   - Commit: feat(validation): Add Cozo type compatibility checks
   - Dependencies: #3
   - Reasoning: Implement C-VALIDATE checks from IDIOMATIC_RUST.md:
     - Verify type constraints before serialization
     - Ensure proper numeric type handling per Cozo requirements

6. **Update test cases**
   - Effort: 2h
   - Breaking Change: No
   - Commit: test(cozo-types): Add type alignment integration tests
   - Dependencies: #4
   - Reasoning: Expand tests/mod.rs with:
     - Roundtrip tests for new type variants
     - Edge case validation (e.g., mixed Int/Float comparisons)
     - Invalid type mapping detection

TOTAL ESTIMATED EFFORT: 12 hours

RISK MITIGATION:
- Use feature flags to stage rollout
- Maintain backward compatibility during transition
- Add type versioning to serialization output
- Update documentation examples per C-EXAMPLE
```
