# Error Handling Implementation Guide

````markdown
# Comprehensive Refactoring Plan: Create ploke/error Crate

## 1. Task Definition
**Task**: Create a centralized error handling crate that unifies error reporting across:
- Parser operations (syn errors)
- Database operations (cozo errors)
- Code analysis workflows
  
**Purpose**:
1. Eliminate error type fragmentation between crates
2. Ensure compliance with `Send + Sync` requirements from CONVENTIONS.md
3. Streamline error reporting for async/parallel boundaries
  
**Success Criteria**:
1. All crates use error::Error as their primary error type
2. Full backtrace preservation across thread boundaries
3. Zero unwrap()/expect() in cross-crate error handling

## 2. Feature Flag Configuration
**Feature Name**: N/A (New crate, no backward compatibility needed)
**Justification**: Since this is a net-new foundational component, we'll implement it directly rather than behind a feature flag.

## 3. Task Breakdown

### 3.1 Analysis & Preparation
- [ ] 3.1.1 Audit existing error patterns
  - **Files to Examine**:
    - `crates/ingest/syn_parser/src/error.rs` (current placeholder)
    - `crates/ingest/syn_parser/src/parser/visitor/mod.rs` (syn::Error usage)
    - `crates/ingest/ploke_graph/src/schema.rs` (cozo::Error handling)
  - **Outcome**: List of error patterns to support
  
- [ ] 3.1.2 Design error type hierarchy
  - **Approach**:
    ```rust
    // Proposed core structure
    #[derive(Debug, thiserror::Error)]
    pub enum Error {
        #[error("Parser error: {0}")]
        Parser(#[from] syn::Error),
        
        #[error("Database error: {0}")]
        Database(#[from] cozo::Error),
        
        #[error("Validation error: {0}")]
        Validation(String)
    }
    
    // Implement MUST_HAVE traits from CONVENTIONS.md
    unsafe impl Send for Error {}
    unsafe impl Sync for Error {}
    ```

### 3.2 Core Implementation
- [ ] 3.2.1 Create error crate foundation
  - **Files**:
    - `crates/error/Cargo.toml`
    - `crates/error/src/lib.rs`
    - `crates/error/src/error.rs`
  - **Dependencies**:
    ```toml
    [dependencies]
    thiserror = "1.0"
    syn = { workspace = true }
    cozo = { workspace = true }
    ```

- [ ] 3.2.2 Integrate with syn_parser
  - **Files**:
    - `crates/ingest/syn_parser/Cargo.toml`
    - `crates/ingest/syn_parser/src/error.rs`
    - `crates/ingest/syn_parser/src/parser/visitor/mod.rs`
  - **Changes**:
    ```rust
    // Replace syn::Error with error::Error
    pub fn analyze_code(file_path: &Path) -> Result<CodeGraph, error::Error> {
        let file = syn::parse_file(...)?; // Automatic conversion
    }
    ```

- [ ] 3.2.3 Integrate with ploke_graph
  - **Files**:
    - `crates/ingest/ploke_graph/Cargo.toml`
    - `crates/ingest/ploke_graph/src/schema.rs`
    - `crates/ingest/ploke_graph/src/transform.rs`
  - **Conversion Example**:
    ```rust
    impl From<cozo::Error> for error::Error {
        fn from(e: cozo::Error) -> Self {
            Self::Database(e)
        }
    }
    ```

### 3.3 Testing & Integration
- [ ] 3.3.1 Add error compatibility tests
  - **Test Cases**:
    1. syn::Error → error::Error conversion preserves span info
    2. cozo::Error → error::Error maintains database error codes
    3. Thread-boundary safety verification

- [ ] 3.3.2 Update existing tests
  - **Files**:
    - All test files using `unwrap()`/`expect()`
    - `crates/ingest/syn_parser/tests/parser/*`

- [ ] 3.3.3 Benchmark error propagation
  - **Metrics**:
    - Zero-cost conversion overhead
    - Memory footprint across thread boundaries

### 3.4 Documentation & Knowledge
- [ ] 3.4.1 Add error taxonomy docs
  - **Location**: `crates/error/src/docs/error_types.md`
  - **Content**: Decision matrix for error variants

- [ ] 3.4.2 Create commit template
  ```txt
  Error Handling Commit Message:
  - Affected Crates: [list]
  - Error Types: [added/modified]
  - Propagation Path: [sync/async boundary details]
  ```

- [ ] 3.4.3 Update architecture docs
  - **File**: `PROPOSED_ARCH_V3.md`
  - **Section**: Add error crate to system architecture diagram

## 4. Rollback Strategy
If instability is detected:
1. Revert all crates' error type imports
2. Restore previous error handling in syn_parser/ploke_graph
3. Remove error crate from workspace members
4. Rollback Cargo.toml dependencies

## 5. Progress Tracking
- [ ] Analysis Phase: 0/2
- [ ] Implementation Phase: 0/3 
- [ ] Testing Phase: 0/3
- [ ] Documentation Phase: 0/3
````

## Implementation Notes

### Phase Prioritization
1. **Foundation First**: Create error crate before modifying existing code
2. **Vertical Integration**: Complete one crate's migration before moving to next
3. **Concurrency Validation**: Test thread boundaries early with flume channels

## Verification Checklist
```rust
// Example test case for cross-thread error handling
#[test]
fn test_send_sync_error_propagation() {
    let err = error::Error::Parser(syn::parse_file("").unwrap_err());
    
    // Verify Send capability
    thread::spawn(move || {
        let threaded_err = err;
        assert!(matches!(threaded_err, error::Error::Parser(_)));
    }).join().unwrap();
}
```

## Performance Considerations
- **Error Boxing**: Only box at async boundaries per CONVENTIONS.md
- **Backtrace**: Use `std::backtrace` behind feature flag for release-mode diagnostics
- **Coercion Costs**: Benchmark type conversions in schema.rs transforms
