# Known issues from last audit of PROPOSED_FILE_ARCH1.md

### **MVP Readiness Checklist**

1. **Core Path Implemented** ✅  
   (syn_parser → ingest → Cozo → query)

2. **Critical Debt Addressed** ⚠️  
   Requires AST differencing stubs

3. **Concurrency Safe** ⚠️  
   Needs runtime unification audit

4. **Testing Baseline** ✅  
   Robust test structure present

---

### **Critical Issues Requiring Immediate Attention**

1. **Type System Migration Phasing**
   - **Problem**: syn_parser's ID refactor (STRATEGY.md) creates a hard dependency for all other components
   - **Risk**: Blocked MVP progress until Phase 3 completes
   - **Solution**:
     ```rust
     // Temporary adapter during migration (core/src/types.rs)
     #[cfg(feature = "legacy_ids")]
     pub type NodeId = usize; // Only enabled in tests
     ```
   - **Action**: Implement phased rollout gates using feature flags

2. **Concurrency Model Inconsistency**
   - **Violation**: Mixing Tokio/Channel types (IDIOMATIC_RUST C-SEND-SYNC)
   - **Example**:
     ```rust
     // Currently proposed:
     rayon::scope(|s| { s.spawn(|| process_file()) }); // Blocking
     // In async context violates C-OVERLOAD (predictable exec)
     ```
   - **Fix**: Unify under Tokio runtime with `spawn_blocking`
     ```rust
     tokio::task::spawn_blocking(|| process_file())
     ```

3. **Embedding Dimension Hardcoding**
   - **Problem**: 384D vectors assume specific model (violates C-GENERIC)
   - **Risk**: Breaks future model upgrades
   - **Solution**:
     ```rust
     // core/src/embeddings.rs
     pub struct Embedding<const D: usize>(pub [f32; D]);
     ```

---

### **Idiomatic Rust Compliance Gaps**

1. **Error Handling (CONVENTIONS.md)**
   - **Issue**: Cross-crate errors not `#[non_exhaustive]`
   - **Risk**: Downstream can't match variants
   - **Fix**:
     ```rust
     #[non_exhaustive]
     pub enum RageError { ... } // Forces catch-all arm
     ```

2. **Trait Implementation (C-COMMON-TRAITS)**
   - **Missing**: `Serialize`/`Deserialize` for CozoNode
   - **Example**:
     ```rust
     #[derive(Serialize, Deserialize)] // Required for RON tests
     pub struct CozoNode { ... }
     ```

3. **Zero-Copy Violation**
   - **Problem**: Current AST parser clones string values
   - **Violation**: CONVENTIONS.md strict ownership
   - **Example Fix**:
     ```rust
     pub struct FunctionNode<'a> {
         docs: &'a str, // Borrow from parsed source
         params: Vec<&'a Type>,
     }
     ```

---

### **Technical Debt Assessment**

| Debt Category | Severity | Mitigation Path |
|--------------|----------|-----------------|
| AST Differencing | Critical | Implement before MVP scaling |
| Embedding Versioning | High | Add to ingest pipeline phase 1 |
| CUDA Integration | Medium | Post-MVP with feature flags |
| Macro Expansion Tracking | Low | CozoDB schema annotation |

---

### **CozoDB Alignment Validation**

1. **Schema Compatibility**
   - **Risk**: Unvalidated HNSW indexing syntax
   - **Verification Needed**: Confirm with Cozo 0.7.x
     ```cozo
     ::hnsw create ... // Real-world example query
     ```

2. **Temporal Query Support**
   - **UUIDv7 Usage**: Matches Cozo's native timestamp handling
   - **Recommended**: Add test for time-ordered queries
     ```rust
     #[test]
     fn test_temporal_query() { /* Uses UUIDv7 ordering */ }
     ```

---
