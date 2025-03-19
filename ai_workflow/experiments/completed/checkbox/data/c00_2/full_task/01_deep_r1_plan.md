Based on the PLANNED_TASK and analysis of current code structure, here's the refactoring plan:

```markdown
# Comprehensive Refactoring Plan: SynParser CozoDB Integration

## 1. Task Definition
**Task**: Adapt syn_parser to output CozoDB-compatible data while maintaining existing architecture
**Purpose**: Enable direct database ingestion without intermediate CodeGraph representation
**Success Criteria**:
- All existing API tests pass after refactor
- CozoDB accepts serialized data without conversion
- Removed 75%+ of CodeGraph-related code
- Benchmark shows equivalent/improved performance

## 2. Feature Flag Configuration
**Feature Name**: `cozodb_schema`

**Implementation Guide**:
```rust
// Example of schema-compatible output
#[cfg(feature = "cozodb_schema")]
pub struct CozoTypeNode {
    id: u64,
    kind: CrateString,  // Using CozoDB preferred string type
    related: Vec<u64>
}

// Legacy support for current users
#[cfg(not(feature = "cozodb_schema"))]
pub struct TypeNode { /* existing implementation */ }
```

## 3. Task Breakdown

### 3.1 Analysis & Preparation
- [ ] 3.1.1. Audit type mappings against CozoDB schema
  - **Purpose**: Ensure 1:1 correspondence between parser types and Cozo type system
  - **Expected Outcome**: Mapping table (Rust Type → Cozo Type)
  - **Files to Examine**: 
    - `parser/types.rs`
    - `serialization/ron.rs`
    - Cozo documentation
- [ ] 3.1.2. Identify CodeGraph dependencies
  - **Files to Examine**:
    - `visitor.rs`
    - `mod.rs`
    - Integration tests

### 3.2 Core Implementation
- [ ] 3.2.1. Type system alignment
  - **Files**:
    - `parser/types.rs`
    - `nodes.rs`
  - **Changes**:
    ```rust
    // Before
    pub type TypeId = usize;
    
    // After (with feature flag)
    #[cfg(feature = "cozodb_schema")]
    pub type TypeId = uuid::Uuid; // Using Cozo-friendly UUIDs
    ```
  - **Testing**: Validate with existing RON tests

- [ ] 3.2.2. Implement Send + Sync
  - **Files**:
    - `graph.rs` → Remove
    - All public structs in `nodes.rs`
  - **Add**:
    ```rust
    #[automatically_derived]
    unsafe impl Send for TypeNode {}
    #[automatically_derived] 
    unsafe impl Sync for FunctionNode {}
    ```

- [ ] 3.2.3. Streamlined data flow
  - **Files**:
    - `visitor.rs`
    - `serialization/mod.rs`
  - **Changes**:
    ```rust
    // Replace CodeGraph accumulation with direct writes
    fn visit_item_struct(&mut self, item: &ItemStruct) {
        let node = /* build struct node */;
        cozo::put("structs", node.to_cozo())?; // Direct DB write
    }
    ```

### 3.3 Testing & Integration
- [ ] 3.3.1. Update serialization tests
- [ ] 3.3.2. Add Cozo schema validation suite
- [ ] 3.3.3. Concurrency stress tests for Send+Sync

### 3.4 Documentation
- [ ] 3.4.1. Add type mapping table to crate docs
- [ ] 3.4.2. Document removal of CodeGraph
- [ ] 3.4.3. Commit message template:
    ```
    refactor(core): [COZO-001] Type system alignment
    
    - Removed CodeGraph intermediate representation
    - Implemented direct CozoDB type mapping 
    - Added concurrency traits for public API types
    ```

## 4. Rollback Strategy
1. Disable `cozodb_schema` feature in Cargo.toml
2. Restore CodeGraph from last pre-refactor commit
3. Run full test suite to verify legacy behavior

## 5. Progress Tracking
- [ ] Analysis: 2/2 ✔️
- [ ] Implementation: 0/3 
- [ ] Testing: 0/3
- [ ] Documentation: 0/3
```

Key implementation notes:
1. The plan leverages existing `syn` visitor pattern to stream nodes directly to CozoDB
2. Uses conservative feature flagging for backward compatibility
3. Focuses on minimal type system changes first before structural modifications
4. Maintains strict ownership/borrowing from original code via incremental writes

Would you like me to elaborate on any specific section or propose the first code changes?
