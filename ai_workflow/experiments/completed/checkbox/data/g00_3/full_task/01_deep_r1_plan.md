```
1. **Subtask: Align Data Types with CozoDB Schema**
   - **Effort**: 4 hours
   - **Breaking Change**: Yes (feature_use_cozo_types)
   - **Commit Message**: "feat: Align core types with CozoDB schema requirements"
   - **Dependencies**: None
   - **Reasoning**:
     * Cross-reference CozoDB type documentation with existing type definitions
     * Replace String with Bytes for content hashes (e.g., `Vec<u8>` with base64 encoding)
     * Ensure vectors are properly typed for F32/F64 storage
     * Add nullability annotations through Option<> where appropriate
     * Protects future compatibility while allowing backward-compatibility through feature flags

2. **Subtask: Implement Send + Sync for Public API**
   - **Effort**: 2 hours
   - **Breaking Change**: No
   - **Commit Message**: "chore: Guarantee Send + Sync for all public types"
   - **Dependencies**: #1
   - **Reasoning**:
     * Add `+ Send + Sync` bounds to all public structs and trait definitions
     * Verify absence of Rc/RefCell/non-threadsafe internals
     * Enables safe usage in concurrent contexts without altering call patterns
     * Minimal risk as most structs already satisfy requirements

3. **Subtask: Streamline CodeGraph Structure**
   - **Effort**: 6 hours
   - **Breaking Change**: Yes (feature_code_graph_removal)
   - **Commit Message**: "refactor: Replace CodeGraph with direct CozoDB streaming"
   - **Dependencies**: #1, #2
   - **Reasoning**:
     * Remove aggregated CodeGraph struct from parser output
     * Convert visitor pattern to emit CozoDB-compatible tuples directly
     * Maintain flyweight pattern for type/node ID generation
     * Reduces memory overhead and middleware translation layer
     * Progressive migration path via feature flag

4. **Subtask: Implement CozoDB Serialization**
   - **Effort**: 4 hours
   - **Breaking Change**: Yes (feature_cozo_serialization)
   - **Commit Message**: "feat: Add native CozoDB tuple serialization"
   - **Dependencies**: #3
   - **Reasoning**:
     * Replace RON serialization with Cozo-compatible format
     * Create dedicated `CozoExport` trait with automatic tuple conversion
     * Preserve existing RON support under original feature flag
     * Leverage CozoDB's dynamic typing while maintaining Rust type safety

5. **Subtask: Update Validation & Add Migration Path**
   - **Effort**: 2 hours
   - **Breaking Change**: No
   - **Commit Message**: "test: Add CozoDB schema validation suite"
   - **Dependencies**: All previous tasks
   - **Reasoning**:
     * Create unit tests verifying output against CozoDB's type system
     * Document feature flag usage for gradual migration
     * Verify backward compatibility through integration tests
     * Ensures smooth transition without breaking existing integrations
```
