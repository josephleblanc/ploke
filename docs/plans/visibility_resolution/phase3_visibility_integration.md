# Comprehensive Refactoring Plan: Visibility Resolution Integration

## 1. Task Definition
**Task**: Implement visibility resolution system combining module paths and use statements  
**Purpose**: Enable accurate determination of item visibility in any given scope  
**Success Criteria**:
- System correctly classifies items into three visibility states:
  1. Directly visible in current scope
  2. Potentially visible through use statements
  3. Not visible due to privacy restrictions
- Resolution works across module boundaries
- All test cases pass with <5% false positives/negatives

## 2. Feature Flag Configuration
**Feature Name**: `visibility_resolution`

**Implementation Guide**:
```rust
#[cfg(feature = "visibility_resolution")]
impl CodeGraph {
    pub fn resolve_visibility(
        &self,
        item_id: NodeId,
        context_module: &[String]
    ) -> VisibilityResult {
        // Implementation using module_paths and use_statements
    }
}

#[cfg(not(feature = "visibility_resolution"))]
impl CodeGraph {
    pub fn resolve_visibility(&self, _: NodeId, _: &[String]) -> VisibilityResult {
        VisibilityResult::Unknown
    }
}
```

## 3. Task Breakdown

### 3.1 Analysis & Preparation
- [ ] 3.1.1. Design visibility resolution algorithm
  - **Purpose**: Create decision tree combining:
    - Item's explicit visibility
    - Current module path
    - Relevant use statements
    - Crate boundaries
  - **Expected Outcome**: Documented algorithm with:
    - Flowchart of resolution process
    - Edge case handling strategy
    - Examples of complex scenarios
  - **Files to Examine**:
    - Current visibility handling in `state.rs`
    - Module path tracking implementation
    - Use statement collection

### 3.2 Core Implementation
- [ ] 3.2.1. Implement VisibilityResult type
  - **Files to Modify**:
    - `nodes.rs` (new type definition)
    - `graph.rs` (serialization)
  - **Reasoning**: Need rich return type that captures:
    - Final visibility state
    - Required use statement (if applicable)
    - Alternative paths to item
  - **Code Changes**:
    ```rust
    #[derive(Debug, Serialize, Deserialize)]
    pub enum VisibilityResult {
        DirectlyVisible,
        RequiresUse(Vec<String>), // Path needed for visibility
        Restricted,
        Unknown
    }
    ```

- [ ] 3.2.2. Develop resolution engine
  - **Files to Modify**: `graph.rs`
  - **Implementation Details**:
    - Take current module path as input
    - Check item's explicit visibility first
    - For restricted visibility, verify module hierarchy
    - Scan use statements for potential imports
    - Handle special cases (e.g. pub(in path))
  - **Testing Approach**:
    - Unit tests for each resolution branch
    - Integration tests with real module structures

### 3.3 Testing & Integration
- [ ] 3.3.1. Create comprehensive test suite
  - **Test Cases**:
    - Direct visibility in same module
    - Parent module access to pub(super) items
    - Cross-crate public items
    - Private items in dependencies
    - Complex use statement scenarios
  - **Files**:
    - `tests/visibility/resolution_tests.rs`
    - New fixtures in `tests/fixtures/visibility/`

- [ ] 3.3.2. Performance benchmarking
  - **Purpose**: Ensure resolution doesn't significantly impact parsing
  - **Metrics**:
    - Memory usage with resolution enabled
    - Parsing time increase
    - Graph serialization size

- [ ] 3.3.3. Integration with existing systems
  - **Verification Points**:
    - Works with current serialization format
    - Doesn't break existing visibility queries
    - Compatible with database schema

### 3.4 Documentation & Knowledge Preservation
- [ ] 3.4.1. Document resolution rules
  - **Content**:
    - Detailed precedence rules
    - Diagram of resolution process
    - Examples of edge cases
  - **Location**: `docs/visibility_resolution.md`

- [ ] 3.4.2. Annotate source code
  - **Key Areas**:
    - Complex branching logic
    - Non-obvious decisions
    - Limitations/TODO items

- [ ] 3.4.3. Create commit message template:
    ```
    feat(visibility): Implement scope-aware resolution

    Key Changes:
    - Added VisibilityResult enum with serialization
    - Implemented multi-stage resolution algorithm
    - Verified handling of:
      * Nested module hierarchies
      * Use statement effects
      * Crate boundaries

    Performance Impact:
    - Parsing time increased by <15%
    - Memory usage grew by ~8%
    ```

## 4. Rollback Strategy
1. Disable feature flag to fall back to basic visibility
2. Remove VisibilityResult type
3. Maintain legacy visibility fields
4. Preserve all tests under #[cfg] blocks

## 5. Progress Tracking
- [ ] Analysis Phase: 0/1 complete
- [ ] Implementation Phase: 0/2 complete
- [ ] Testing Phase: 0/3 complete
- [ ] Documentation Phase: 0/3 complete
