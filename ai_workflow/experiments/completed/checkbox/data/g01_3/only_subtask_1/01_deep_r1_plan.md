**Subtask ID: 1**
* Description: Create CozoDB type mapping system in `types.rs`
* Estimated Time: 3 hours
* Cfg Flag Required?: Yes - Changing type representations could break existing tests and serialization formats that rely on current type identifiers. Feature flag `feature_ai_task_type_alignment` needed.
* Dependencies: None
* Potential Issues: Handling Rust primitive type conversions (e.g., &[u8]→Bytes), nested generics resolution, potential loss of type fidelity during mapping

**Subtask ID: 2** 
* Description: Update `get_or_create_type` in `visitor.rs` to use Cozo type mapping
* Estimated Time: 2 hours
* Cfg Flag Required?: Yes - Directly modifies core type resolution logic used by multiple components. Feature flag prevents breaking existing analysis workflows.
* Dependencies: 1
* Potential Issues: Lifetime handling for reference types, maintaining backward-compatible type IDs

**Subtask ID: 3**
* Description: Modify serialization in `ron.rs` to handle Cozo types
* Estimated Time: 2.5 hours  
* Cfg Flag Required?: Yes - Serialization format changes could break downstream consumers. Flag allows maintaining dual output formats.
* Dependencies: 1,2
* Potential Issues: Versioning of serialized data, lossy conversions for Float32/Float64

**Subtask ID: 4**
* Description: Add validation for type conversions in `visitor/mod.rs`
* Estimated Time: 2 hours
* Cfg Flag Required?: No - Validation is additive and doesn't modify existing code paths
* Dependencies: 1
* Potential Issues: Handling ambiguous type mappings (e.g., usize vs Cozo Int)

**Subtask ID: 5**
* Description: Update test cases in `tests/mod.rs` and `parser_tests.rs`
* Estimated Time: 1.5 hours
* Cfg Flag Required?: Yes - New tests should be gated with the same feature flag to maintain existing CI/CD pipelines
* Dependencies: 3,4
* Potential Issues: Test flakiness during transitional period with dual type systems

**Implementation Order:** 1 → 2 → 4 → 3 → 5

**Critical Risk Mitigation:**
- Use `#[cfg(feature = "feature_ai_task_type_alignment")]` guards around:
  1. Modified type mapping logic
  2. Updated serialization paths  
  3. New validation checks
- Maintain original behavior as default until full validation completes
- Especially crucial for serialization format changes in Subtask 3 to prevent breaking RON consumers

**Key Challenge Areas:**
- Preserving type aliases during conversion (TypeAliasNode handling)
- Handling Rust enums that map to Cozo lists
- Vector type validation (fixed-length F32/F64 arrays)
- Maintaining bidirectional compatibility during transition period
