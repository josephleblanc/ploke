**Core Problem:**

During the parallel processing of individual source files (Phase 2), the parser identifies syntactic constructs that signify relationships between code elements (like imports, type usages, module declarations). However, the definitive, stable identifier (`NodeId::Resolved` or `TypeId::Resolved`) for the *target* of the relationship often cannot be determined using only the information within that single file. The target might be defined in another file or require global context (like the fully resolved module tree) established later. We need a mechanism to capture the *intent* and necessary *context* of these potential relationships during Phase 2, so they can be accurately formed during the global resolution phase (Phase 3).

---

**Solution: `PendingRelation` Enum (Deferred Resolution Task Queue)**

*   **Concept:** Define an explicit enum where each variant represents a specific kind of unresolved link discovered during Phase 2. Each variant stores the necessary context (source ID, unresolved path/name, spans, attributes associated with the link itself) required for Phase 3 to find the target and create the final `Relation`.
*   **Mechanism:** Phase 2 workers populate a list of these `PendingRelation` objects alongside the nodes they discover. Phase 3 processes this list, resolves the targets using the complete set of discovered nodes, and generates the final `Relation` structs.
*   **Minimal Implementation:**

```rust
// In parser/relations.rs (or similar)

use crate::parser::nodes::{Attribute, ImportNode, NodeId, VisibilityKind};
use crate::parser::types::TypeId; // Assuming TypeId is defined

// The final, resolved relation structure
#[derive(Debug, Clone)]
pub struct Relation {
    pub source: GraphId, // Using GraphId wrapper
    pub target: GraphId,
    pub kind: RelationKind,
}

// Enum identifying the source/target type for GraphId
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum GraphId {
    Node(NodeId),
    Type(TypeId),
}

// Enum for different kinds of final relations
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RelationKind {
    Contains,
    DefinesModule, // Link from declaration to definition
    Imports,
    UsesType, // Generic usage, specific kind below
    FunctionParameter,
    FunctionReturn,
    StructField,
    // ... other kinds
}

// Enum capturing unresolved relation intents from Phase 2
#[derive(Debug, Clone)]
pub enum PendingRelation {
    // A 'mod foo;' declaration was found
    ResolveModuleDecl {
        parent_module_id: NodeId, // Module containing the 'mod foo;'
        declared_name: String,    // "foo"
        visibility: VisibilityKind,
        span: (usize, usize), // Span of 'mod foo;'
        attributes: Vec<Attribute>, // Attributes on 'mod foo;' like #[cfg] or #[path]
    },
    // A 'use some::path::Item;' was found
    ResolveImport {
        importing_module_id: NodeId,
        import_node: ImportNode, // Contains path, alias, kind, span etc.
    },
    // A type was used (e.g., in a function signature, field)
    ResolveTypeUsage {
        using_node_id: NodeId,     // ID of the function, struct, etc. using the type
        context_module_id: NodeId, // ID of the module where the usage occurs
        unresolved_path: Vec<String>, // The path string used, e.g., ["crate", "Foo"] or ["Bar"]
        usage_context: TypeUsageContext, // Parameter, Return, Field, etc.
        span: (usize, usize),      // Span of the type usage in the source
    },
    // Add other variants as needed, e.g., ResolveTraitBound, ResolveSuperTrait
}

// Context for type usage resolution
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TypeUsageContext {
    FunctionParameter,
    FunctionReturn,
    StructField,
    TypeAliasDefinition,
    ImplTrait,
    ImplFor,
    // ...
}

// In parser/visitor/state.rs
struct VisitorState {
    // ... nodes_found: Vec<NodeTypeEnum>, ...
    resolved_relations: Vec<Relation>, // For intra-file links like Contains
    pending_relations: Vec<PendingRelation>,
    // ... other state ...
}

// In parser/visitor/code_visitor.rs (Example Usage)
impl<'a> CodeVisitor<'a> {
    fn visit_item_mod(&mut self, node: &syn::ItemMod) {
        // ... logic to handle module definition or declaration ...
        if node.content.is_none() { // It's a declaration: 'mod foo;'
            let parent_id = self.state.current_module_id(); // Get current module ID
            let pending = PendingRelation::ResolveModuleDecl {
                parent_module_id: parent_id,
                declared_name: node.ident.to_string(),
                visibility: self.state.convert_visibility(&node.vis),
                span: self.state.get_span(&node.span()), // Get span helper
                attributes: extract_attributes(&node.attrs), // Extract attributes from 'mod foo;'
            };
            self.state.pending_relations.push(pending);
        } else {
            // Handle inline module definition: create ModuleNode, add Contains relation
            // ...
        }
    }

    // NOTE: Notional only, there is no `visit_signature` in `code_visitor.rs`
    fn visit_signature(&mut self, sig: &syn::Signature) {
        // ... get function_node_id ...
        // For parameters:
        for input in &sig.inputs {
             if let syn::FnArg::Typed(pat_type) = input {
                 let ty = &pat_type.ty;
                 let path_opt = get_path_from_type(ty); // Helper to extract path segments
                 if let Some(unresolved_path) = path_opt {
                     let pending = PendingRelation::ResolveTypeUsage {
                         using_node_id: function_node_id,
                         context_module_id: self.state.current_module_id(),
                         unresolved_path,
                         usage_context: TypeUsageContext::FunctionParameter,
                         span: self.state.get_span(&ty.span()),
                     };
                     self.state.pending_relations.push(pending);
                 }
             }
        }
        // Similar logic for return type sig.output
        // ...
    }
}
```

*   **Pros:** Explicit, type-safe representation of pending work. Keeps final `Relation` clean. Easy to process in Phase 3 by matching on the enum. Stores context specific to the *type* of resolution needed.
*   **Cons:** Requires defining and maintaining the `PendingRelation` enum.

