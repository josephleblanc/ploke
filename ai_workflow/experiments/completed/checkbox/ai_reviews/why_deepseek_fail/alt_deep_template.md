
Here are three code-centric task templates following Rust module conventions while 
incorporating feature flags and progress tracking:

```rust
//! [TASK] Type Alignment: CozoDB Schema Conversion
mod cozo_type_alignment {
    #[cfg(feature = "cozo_type_align")]
    use bytes::Bytes;

    /* [ ] ANALYSIS PHASE
     * - Files: types.rs, nodes.rs, visitor.rs
     * - Safety: Y (Serialization format change)
     */
    
    //! Original: pub struct ContentHash(String);
    #[cfg_attr(feature = "cozo_type_align", derive(Serialize))]
    pub struct ContentHash(
        #[cfg(feature = "cozo_type_align")] Bytes,
        #[cfg(not(feature = "cozo_type_align"))] String
    );

    /* [ ] IMPLEMENTATION
     * Update TypeNode and FieldNode to use CozoDB-compatible:
     * - F64 vectors instead of Vec<f32>
     * - UUID types for identifiers
     */
    
    /* [ ] TESTING
     * #[test]
     * #[cfg(feature = "cozo_type_align")]
     * fn test_cozo_compatibility() {
     *     let hash = ContentHash(Bytes::from("test"));
     *     assert!(serde_json::to_value(hash).is_ok());
     * }
     */
}
```

```rust
//! [TASK] Concurrency Safety: Enforce Send + Sync
mod send_sync_validation {
    /* CONFIG */
    #[cfg(feature = "send_sync_audit")]
    use std::marker::{Send, Sync};

    /* [ ] ANALYSIS PHASE
     * - Files: graph.rs, nodes.rs
     * - Safety: Y (Trait impl changes)
     */
    
    /* [ ] IMPLEMENTATION
     * #[cfg(feature = "send_sync_audit")]
     * unsafe impl Send for CodeGraph {}
     * #[cfg(feature = "send_sync_audit")]
     * unsafe impl Sync for CodeGraph {}
     */

    /* [ ] TESTING
     * #[test]
     * #[cfg(feature = "send_sync_audit")]
     * fn test_thread_safety() {
     *     let graph = std::thread::spawn(|| CodeGraph::new()).join();
     *     assert!(graph.is_ok());
     * }
     */
    
    /* [ ] DOCUMENTATION
     * /// SAFETY: Manual Send/Sync impls require review of all contained types
     */
}
```

```rust
//! [TASK] Architecture: Remove CodeGraph Intermediate
mod codegraph_removal {
    /* CONFIG */
    #[cfg(feature = "direct_ast_stream")]
    use syn::File as AstRoot;

    /* [ ] ANALYSIS PHASE
     * - Files: graph.rs, visitor.rs, parser/mod.rs
     * - Safety: Y (Architectural change)
     */
    
    /* [ ] IMPLEMENTATION
     * #[cfg(feature = "direct_ast_stream")]
     * pub fn process_ast(ast: AstRoot) -> impl Iterator<Item=CozoEntity> {
     *     // Direct AST conversion logic
     * }
     * 
     * #[cfg(not(feature = "direct_ast_stream"))]
     * pub fn process_ast(_: AstRoot) -> CodeGraph {
     *     // Legacy code path
     * }
     */

    /* [ ] TESTING
     * #[test]
     * #[cfg(feature = "direct_ast_stream")]
     * fn test_ast_direct_processing() {
     *     let ast = syn::parse_file("fn main() {}").unwrap();
     *     assert!(process_ast(ast).count() > 0);
     * }
     */
    
    /* [ ] DOCUMENTATION
     * /// MIGRATION GUIDE: This feature removes intermediate graph representation
     */
}
```

**Key Design Choices:**
1. **Scoped Modules**: Each task encapsulated in its own module for isolation
2. **Dual-Path Code**: Shows old/new implementations via `cfg` attributes
3. **Safety Notations**: Explicit risk labeling in analysis comments
4. **Test Integration**: Feature-gated test cases verify functionality

**Uncertainties:**
1. Balance between template flexibility and prescriptiveness
2. Optimal granularity for code stubs vs. implementation detail
3. Feature flag naming convention consistency between templates

Would you like me to refine any particular aspect or demonstrate usage for a 
specific scenario from your codebase?
