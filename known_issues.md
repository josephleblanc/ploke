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

## Franz Review v1 - google endpoint Gemma3-27B

**1. Type System Design – A Disaster Waiting to Happen:**

*   **Content Addressing with Blake3:** 
  - blake3 vs MD5.
  - performance?
  - tradeoffs?
    - collision resistance?
    - speed?
    - sufficient vs overkill?
*   **Temporal Versioning with UUIDv7:** UUIDs for *every* version? Are you
kidding me? That's going to bloat the database to an unmanageable size. We need
a proper versioning scheme, not just throwing random identifiers at the
problem. And UUIDv7? Seriously? It's barely standardized.
*   **CozoDB Schema:** The schema itself is… questionable. `vec: <F32; 384>`?
Hardcoding the embedding dimension *in the schema*? What happens when we switch
embedding models? We have to migrate the entire database? This is inflexible
and short-sighted. And why are we storing the hash *and* the type versions?
Redundancy!
  - Possible solution: Compile time generics? What are those called again?
*   **DashMap for Caching:** DashMap? For *everything*? That's going to
introduce a massive amount of contention. We need a more sophisticated caching
strategy, potentially leveraging a read-copy-update (RCU) approach.
  - *Double checking*: DashMap is a concurrent map, but if there's high
  contention, it might not be the best choice. The reviewer suggests RCU
  (Read-Copy-Update), which is a good point for high-read/low-write
  scenarios. So that's a valid concern. 
DashMap is mentioned as a lock-free structure, but DashMap uses        
   sharding and locks internally. It's thread-safe but not lock-free. The 
   distinction is important for performance but maybe not correctness.    
   However, the review's suggestion to use lock-free alternatives where   
   appropriate needs clarification. Which structures? Maybe crossbeam or  
   other crates?

**2. Hybrid Vector/Graph Architecture – A Confused Mess:**
*   **Graph Storage Schema:** Again, hardcoding the embedding dimension. And
the `relations` field? A list of `(target, kind)` pairs? That's going to be
incredibly inefficient for graph traversal. We need a proper adjacency list or
matrix representation.
  * *Double Checking*: Hybrid Vector/Graph Architecture: The reviewer is right
  about the relations field being inefficient for graph traversal.
  Using an adjacency list or a proper graph structure in the database would be
  better. The test vectors only checking dimension is indeed insufficient; more
  comprehensive testing is needed.

**3. Crate Structure Rationale – Over-Engineered and Unnecessary:**
*   `ide/`? A separate crate for an IDE watcher? That's premature optimization.
It should be part of the `interface` crate, if it's even necessary at all.
  * *Double Checking:* Crate structure: Having many small crates can lead to
  dependency management issues and increased compilation times. The reviewer's
  point about crate bloat is valid, especially if some crates don't have clear
  responsibilities.
