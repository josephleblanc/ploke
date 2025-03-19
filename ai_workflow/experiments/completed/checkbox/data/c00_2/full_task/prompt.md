PLANNED_TASK:
1.  **Minimal `syn_parser` Rework:** Focus on adapting the existing `syn_parser` to output data directly compatible with CozoDB, *without* fundamentally altering its data flow or concurrency model. This means:
    *   **Type Alignment:** Change the types used within `syn_parser` to align with the CozoDB schema (e.g., using `Bytes` for content hashes, appropriate numeric types for embeddings).
    *   **Send + Sync:** Ensure all publicly exposed types are `Send + Sync`. This is a good practice regardless and will prepare the codebase for future concurrency improvements.
    *   **Remove `CodeGraph` (or significantly reduce its scope):** You're right to question the value of the `CodeGraph` as an intermediary. It adds complexity without necessarily providing significant benefits. We can likely stream data directly from the `syn` AST to CozoDB.


CREATE_PLAN:
```markdown
# Comprehensive Refactoring Plan: [Task Name]

## 1. Task Definition
**Task**: [Detailed description of the task]
**Purpose**: [Why this task is necessary]
**Success Criteria**: [How to determine when the task is complete]

## 2. Feature Flag Configuration
**Feature Name**: `refactor_[component_name]`

**Implementation Guide:**
```rust
// New implementation with feature flag
#[cfg(feature = "refactor_[component_name]")]
impl SomeStruct {
    pub fn new_method() { /* implementation */ }
}

// Maintain backward compatibility
#[cfg(not(feature = "refactor_[component_name]"))]
impl SomeStruct {
    pub fn old_method() { /* implementation */ }
}
```

## 3. Task Breakdown

### 3.1 Analysis & Preparation
- [ ] 3.1.1. Review current implementation
  - **Purpose**: [Why this step is necessary]
  - **Expected Outcome**: [What should be produced]
  - **Files to Examine**: [List of relevant files]
- [ ] 3.1.2. Create implementation plan
  - **Purpose**: [Why this step is necessary]

### 3.2 Core Implementation
- [ ] 3.2.1. [First implementation step]
  - **Files to Modify**: [List of files]
  - **Reasoning**: [Why this approach was chosen]
  - **Testing Approach**: [How to test this change]
- [ ] 3.2.2. [Second implementation step]
  - **Code Changes**:
    ```rust
    // Example of expected changes
    ```

### 3.3 Testing & Integration
- [ ] 3.3.1. Update existing tests
- [ ] 3.3.2. Add new tests for changed functionality
- [ ] 3.3.3. Test with and without feature flag enabled

### 3.4 Documentation & Knowledge Preservation
- [ ] 3.4.1. Update code documentation with reasoning
- [ ] 3.4.2. Document design decisions
- [ ] 3.4.3. Create commit message template capturing key changes

## 4. Rollback Strategy
[Instructions on how to revert changes if issues are found]

## 5. Progress Tracking
- [ ] Analysis Phase: 0/2 complete
- [ ] Implementation Phase: 0/2 complete
- [ ] Testing Phase: 0/3 complete
- [ ] Documentation Phase: 0/3 complete
```
