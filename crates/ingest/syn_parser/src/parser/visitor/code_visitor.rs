use super::attribute_processing::extract_attributes;
use super::attribute_processing::extract_cfg_strings;
use super::attribute_processing::extract_docstring;
use super::state::VisitorState;
use super::type_processing::get_or_create_type;
use crate::parser::nodes::GraphId;
use crate::parser::nodes::ModuleNode;
use crate::parser::nodes::ValueKind;
use crate::parser::nodes::ValueNode;
use crate::parser::nodes::{
    EnumNode, FieldNode, FunctionNode, ImplNode, ImportKind, ImportNode, MacroKind, MacroNode,
    ProcMacroKind, StructNode, TraitNode, TypeAliasNode, TypeDefNode, UnionNode, VariantNode,
};
use crate::parser::relations::*;
use crate::parser::types::*;
use crate::parser::visitor::calculate_cfg_hash_bytes;
use crate::parser::ExtractSpan;

use crate::parser::nodes::ModuleDef;
use ploke_core::ItemKind; // Import TypeKind
use ploke_core::{NodeId, TypeId};

use colored::*; // Import colored
use log::trace; // Import trace macro
use quote::ToTokens;
use syn::spanned::Spanned;
use syn::TypePath;
use syn::{
    visit::{self, Visit},
    ItemEnum, ItemFn, ItemImpl, ItemStruct, ItemTrait, ReturnType, Type,
};

pub struct CodeVisitor<'a> {
    state: &'a mut VisitorState,
}

const LOG_TARGET_TRACE: &str = "visitor_trace"; // Define log target for trace logs

impl<'a> CodeVisitor<'a> {
    pub fn new(state: &'a mut VisitorState) -> Self {
        Self { state }
    }

    // Helper method to extract path segments from a use tree
    // Needs cfg_bytes passed down from visit_item_use
    fn process_use_tree(
        &mut self,
        tree: &syn::UseTree,
        base_path: &[String],
        cfg_bytes: Option<&[u8]>, // NEW: Accept CFG bytes
        vis_kind: &VisibilityKind,
    ) -> Vec<ImportNode> {
        let mut imports = Vec::new();

        match tree {
            syn::UseTree::Path(path) => {
                let mut new_base = base_path.to_vec();
                new_base.push(path.ident.to_string());

                imports.extend(self.process_use_tree(&path.tree, &new_base, cfg_bytes, vis_kind));
                // Pass cfg_bytes down
            }
            syn::UseTree::Name(name) => {
                let mut full_path = base_path.to_vec();
                let use_name = name.ident.to_string();
                let mut is_self_import = false;

                let span = name.extract_span_bytes();

                let checked_name = if use_name == "self" {
                    is_self_import = true;
                    full_path.last().unwrap().clone()
                } else {
                    full_path.push(use_name.clone());
                    use_name // This is the visible name in this case
                };
                // Pass the *visible name*, ItemKind::Import, and the statement's CFG bytes
                let import_id = self.add_contains_rel(
                    &checked_name,
                    ItemKind::Import,
                    cfg_bytes, // Pass down received cfg_bytes
                );

                imports.push(ImportNode {
                    id: import_id,
                    path: full_path,
                    kind: ImportKind::UseStatement(vis_kind.to_owned()),
                    visible_name: checked_name,
                    original_name: None,
                    is_glob: false,
                    span,
                    is_self_import,
                    cfgs: Vec::new(), // Individual use items don't have their own cfgs, they inherit
                });
            }
            syn::UseTree::Rename(rename) => {
                let mut full_path = base_path.to_vec();
                let original_name = rename.ident.to_string();
                let visible_name = rename.rename.to_string(); // The 'as' name

                let span = rename.extract_span_bytes();
                // Pass the *visible name*, ItemKind::Import, and the statement's CFG bytes
                let import_id = self.add_contains_rel(
                    &visible_name,
                    ItemKind::Import,
                    cfg_bytes, // Pass down received cfg_bytes
                );

                full_path.push(original_name.clone()); // Path still uses original name segment

                imports.push(ImportNode {
                    id: import_id,
                    path: full_path, // Path uses original name segment
                    kind: ImportKind::UseStatement(vis_kind.to_owned()),
                    visible_name,                       // The 'as' name
                    original_name: Some(original_name), // The original name before 'as'
                    is_glob: false,
                    span,
                    is_self_import: false,
                    cfgs: Vec::new(), // Individual use items don't have their own cfgs, they inherit
                });
            }
            syn::UseTree::Glob(glob) => {
                // Use a placeholder name like "<glob>" for the ID, pass ItemKind::Import
                let import_id = self.add_contains_rel(
                    "<glob>",
                    ItemKind::Import,
                    cfg_bytes, // Pass down received cfg_bytes
                );

                // Path stored should be the path *to* the glob, not including '*'
                let full_path = base_path.to_vec();

                imports.push(ImportNode {
                    id: import_id,
                    path: full_path, // Path to the glob
                    kind: ImportKind::UseStatement(vis_kind.to_owned()),
                    visible_name: "*".to_string(), // Visual representation
                    original_name: None,
                    is_glob: true,
                    span: glob.extract_span_bytes(), // Use glob span
                    is_self_import: false,
                    cfgs: Vec::new(), // Individual use items don't have their own cfgs, they inherit
                });
            }
            syn::UseTree::Group(group) => {
                for item in &group.items {
                    imports.extend(self.process_use_tree(item, base_path, cfg_bytes, vis_kind));
                    // Pass cfg_bytes down
                }
            }
        }

        imports
    }

    // Removed #[cfg(feature = "verbose_debug")]
    fn debug_mod_stack(&mut self) {
        if let Some(current_mod) = self.state.code_graph.modules.last() {
            let modules: Vec<(String, String)> = self // Changed to String tuples for display
                .state
                .code_graph
                .modules
                .iter() // Add .iter() here
                .map(|m| (m.id.to_string(), m.name.clone())) // Convert ID to string
                .collect();

            let items_str = current_mod
                .items()
                .map(|items| {
                    items
                        .iter()
                        .map(|id| id.to_string())
                        .collect::<Vec<_>>()
                        .join(", ")
                })
                .unwrap_or_else(|| "<None>".to_string());

            trace!(target: LOG_TARGET_TRACE, "{}", "--- Module Stack Debug ---".dimmed());
            trace!(target: LOG_TARGET_TRACE, "  Current Mod: {} ({})", current_mod.name.cyan(), current_mod.id.to_string().magenta());
            trace!(target: LOG_TARGET_TRACE, "  Items: [{}]", items_str.yellow());
            trace!(target: LOG_TARGET_TRACE, "  All Modules: {:?}", modules); // Keep this simple for now
            trace!(target: LOG_TARGET_TRACE, "{}", "--------------------------".dimmed());
        }
    }
    // Removed #[cfg(feature = "verbose_debug")]
    fn debug_mod_stack_push(&mut self, name: String, node_id: NodeId) {
        if let Some(current_mod) = self
            .state
            .code_graph
            .modules // Remove extra .code_graph.modules
            .iter()
            .find(|m| m.items().is_some_and(|items| items.contains(&node_id)))
        {
            let items_str = current_mod
                .items()
                .map(|items| {
                    items
                        .iter()
                        .map(|id| id.to_string())
                        .collect::<Vec<_>>()
                        .join(", ")
                })
                .unwrap_or_else(|| "<None>".to_string());

            trace!(target: LOG_TARGET_TRACE, "  [PUSH ITEM] Mod: {} -> Item: {} ({}) | Items now: [{}]",
                current_mod.name.cyan(),
                name.yellow(),
                node_id.to_string().magenta(),
                items_str.dimmed()
            );
        } else {
            // Log warning instead of panic
            log::warn!(target: LOG_TARGET_TRACE, "Could not find containing module for node with name {}, id {}", name, node_id);
        }
    }
    // Removed #[cfg(feature = "verbose_debug")]
    fn debug_new_id(&mut self, name: &str, node_id: NodeId) {
        if let Some(current_mod) = self.state.code_graph.modules.last() {
            trace!(target: LOG_TARGET_TRACE, "  [NEW ID] In Mod: {} -> Item: {} ({})",
                current_mod.name.cyan(),
                name.yellow(),
                node_id.to_string().magenta()
            );
        }
    }
    // Removed #[cfg(feature = "verbose_debug")]
    fn log_push(&self, stack_name: &str, stack: &[String]) {
        trace!(target: LOG_TARGET_TRACE, "  [PUSH STACK] {}: {:?} -> {:?}",
            stack_name.blue(),
            stack.last().unwrap_or(&"<empty>".to_string()).green(),
            stack
        );
    }

    // Removed #[cfg(feature = "verbose_debug")]
    fn log_pop(&self, stack_name: &str, popped: Option<String>, stack: &[String]) {
        trace!(target: LOG_TARGET_TRACE, "  [POP STACK] {}: {:?} -> {:?}",
            stack_name.blue(),
            popped.unwrap_or("<empty>".to_string()).red(),
            stack
        );
    }
    /// Generates a new `NodeId::Synthetic` for the item being visited using the
    /// `VisitorState` helper, ensuring it's immediately linked to the current
    /// module via a `Contains` relation.
    /// Requires the item's name, kind, and calculated CFG bytes for UUID generation.
    fn add_contains_rel(
        &mut self,
        item_name: &str,
        item_kind: ItemKind,
        cfg_bytes: Option<&[u8]>, // NEW: Accept CFG bytes
    ) -> NodeId {
        // 1. Generate the Synthetic NodeId using the state helper, passing CFG bytes
        let node_id = self
            .state
            .generate_synthetic_node_id(item_name, item_kind, cfg_bytes); // Pass cfg_bytes

        // 2. Add the Contains relation using the new ID and GraphId wrapper
        // Find the parent module based on the *current path*, not just the last pushed module.
        let parent_module_id = self
            .state
            .code_graph
            .modules
            .iter()
            .find(|m| m.path == self.state.current_module_path) // Find module matching current path
            .map(|m| m.id);

        if let Some(parent_id) = parent_module_id {
            if let Some(_parent_mod) = self
                .state
                .code_graph
                .modules
                .iter_mut()
                .find(|m| m.id == parent_id)
            {
                if let Some(parent_mod) = self
                    .state
                    .code_graph
                    .modules
                    .iter_mut()
                    .find(|m| m.id == parent_id)
                {
                    // if let ModuleDef::Inline { items, .. } = &mut parent_mod.module_def {
                    //     items.push(node_id);
                    // }
                    // match for both Inline and FileBased, since either can contain the target
                    // items.
                    match &mut parent_mod.module_def {
                        ModuleDef::Inline { items, .. } => items.push(node_id),
                        ModuleDef::FileBased { items, .. } => items.push(node_id),
                        ModuleDef::Declaration { .. } => (),
                    }
                }
            }

            // Create the relation using GraphId wrappers
            self.state.code_graph.relations.push(Relation {
                source: GraphId::Node(parent_id), // Use the correctly found parent ID
                target: GraphId::Node(node_id),   // Wrap new item ID
                kind: RelationKind::Contains,
            });
            // Removed #[cfg(feature = "verbose_debug")] block
            // Logging is now handled by debug_mod_stack_push if needed
            // Keep the debug hook, assuming debug_mod_stack_push is updated
            // to handle the NodeId enum (e.g., using its Display impl).
            // Removed #[cfg(feature = "verbose_debug")]
            self.debug_mod_stack_push(item_name.to_owned(), node_id);
        } else {
            // This case should ideally not happen after the root module is created in analyze_file_phase2,
            // but log a warning just in case.
            // This block corresponds to parent_module_id being None
            log::warn!(
                target: LOG_TARGET_TRACE,
                "Could not find parent module for item '{}' ({:?}) using current_module_path {:?}. Contains relation not added.",
                item_name, item_kind, self.state.current_module_path
            );
            // Removed the stray 'else' block below
        } // <<< Add missing closing brace for the `if let Some(parent_id) = ...` block

        // 3. Return the newly generated NodeId enum
        node_id
    }

    // Helper to push scope and log (using trace!)
    fn push_scope(&mut self, name: &str, id: NodeId, cfgs: Vec<String>) {
        self.state.current_definition_scope.push(id);
        self.state
            .cfg_stack
            .push(self.state.current_scope_cfgs.clone());
        self.state.current_scope_cfgs = cfgs;
        trace!(target: LOG_TARGET_TRACE, ">>> Entering Scope: {} ({}) | CFGs: {:?}", name.cyan(), id.to_string().magenta(), self.state.current_scope_cfgs);
    }

    // Helper to pop scope and log (using trace!)
    fn pop_scope(&mut self, name: &str) {
        let popped_id = self.state.current_definition_scope.pop();
        let popped_cfgs = self.state.current_scope_cfgs.clone(); // Log before restoring
        self.state.current_scope_cfgs = self.state.cfg_stack.pop().unwrap_or_default();
        trace!(target: LOG_TARGET_TRACE, "<<< Exiting Scope: {} ({:?}) | Popped CFGs: {:?} | Restored CFGs: {:?}",
            name.cyan(),
            popped_id.map(|id| id.to_string()).unwrap_or("?".to_string()).magenta(),
            popped_cfgs,
            self.state.current_scope_cfgs
        );
    }
} // Added missing closing brace for impl CodeVisitor<'a>

#[allow(clippy::needless_lifetimes)]
impl<'a, 'ast> Visit<'ast> for CodeVisitor<'a> {
    // Visit function definitions
    fn visit_item_fn(&mut self, func: &'ast ItemFn) {
        // Check if this function is a procedural macro
        let is_proc_macro = func.attrs.iter().any(|attr| {
            attr.path().is_ident("proc_macro")
                || attr.path().is_ident("proc_macro_derive")
                || attr.path().is_ident("proc_macro_attribute")
        });

        // TODO: Validate Correctness:
        // This if block runs, then so does the following code.
        // Are we processing the proc_macro functions twice?
        // We don't want to deal with the more complex aspects of macros, but do want to note their
        // location, name, and other metadata that might be useful for the RAG to get easy wins.
        // Use if/else to prevent double processing
        if is_proc_macro {
            let macro_name = func.sig.ident.to_string();

            // --- CFG Handling for Proc Macro (Raw Strings) ---
            let scope_cfgs = self.state.current_scope_cfgs.clone();
            let item_cfgs = super::attribute_processing::extract_cfg_strings(&func.attrs);
            let provisional_effective_cfgs: Vec<String> = scope_cfgs
                .iter()
                .cloned()
                .chain(item_cfgs.iter().cloned())
                .collect();
            let cfg_bytes = calculate_cfg_hash_bytes(&provisional_effective_cfgs);
            // --- End CFG Handling ---

            // Pass ItemKind::Macro and cfg_bytes
            let macro_id =
                self.add_contains_rel(&macro_name, ItemKind::Macro, cfg_bytes.as_deref());

            // Removed #[cfg] block
            self.debug_new_id(&macro_name, macro_id); // Now uses trace!

            let span = func.extract_span_bytes();

            // Determine the kind of procedural macro
            let proc_macro_kind = if func
                .attrs
                .iter()
                .any(|attr| attr.path().is_ident("proc_macro_derive"))
            {
                ProcMacroKind::Derive
            } else if func
                .attrs
                .iter()
                .any(|attr| attr.path().is_ident("proc_macro_attribute"))
            {
                ProcMacroKind::Attribute
            } else {
                ProcMacroKind::Function
            };

            // Extract doc comments and other attributes
            let docstring = extract_docstring(&func.attrs);
            let attributes = extract_attributes(&func.attrs);

            // Extract function body as a string
            let body = Some(func.block.to_token_stream().to_string());

            // Create the macro node
            let macro_node = MacroNode {
                id: macro_id,
                name: macro_name,
                visibility: self.state.convert_visibility(&func.vis),
                kind: MacroKind::ProcedureMacro {
                    kind: proc_macro_kind,
                },
                attributes,
                docstring,
                body,
                span,
                tracking_hash: Some(self.state.generate_tracking_hash(&func.to_token_stream())),
                cfgs: item_cfgs, // Store proc macro's own cfgs
            };

            // Add the macro to the code graph
            self.state.code_graph.macros.push(macro_node);
            // Don't visit the body of the proc macro function itself with visit_item_fn
        } else {
            // --- Handle Regular Functions ---
            let fn_name = func.sig.ident.to_string();

            // --- CFG Handling (Raw Strings) ---
            let scope_cfgs = self.state.current_scope_cfgs.clone();
            let item_cfgs = super::attribute_processing::extract_cfg_strings(&func.attrs);
            let provisional_effective_cfgs: Vec<String> = scope_cfgs
                .iter()
                .cloned()
                .chain(item_cfgs.iter().cloned())
                .collect();
            let cfg_bytes = calculate_cfg_hash_bytes(&provisional_effective_cfgs);
            // --- End CFG Handling ---

            // Pass ItemKind::Function and cfg_bytes
            let fn_id = self.add_contains_rel(&fn_name, ItemKind::Function, cfg_bytes.as_deref());
            // Removed #[cfg] block
            self.debug_new_id(&fn_name, fn_id); // Now uses trace!

            let byte_range = func.span().byte_range();
            let span = (byte_range.start, byte_range.end);

            // Push the function's ID onto the scope stack BEFORE processing types/generics
            // Use helper function for logging
            self.push_scope(&fn_name, fn_id, provisional_effective_cfgs);

            // Process function parameters
            let mut parameters = Vec::new();
            for arg in &func.sig.inputs {
                if let Some(param) = self.state.process_fn_arg(arg) {
                    // Add relation between function and parameter

                    self.state.code_graph.relations.push(Relation {
                        source: GraphId::Node(fn_id),
                        target: GraphId::Type(param.type_id),
                        kind: RelationKind::FunctionParameter,
                    });
                    parameters.push(param);
                }
            }

            // Extract return type if it exists
            let return_type = match &func.sig.output {
                ReturnType::Default => None,
                ReturnType::Type(_, ty) => {
                    let type_id = get_or_create_type(self.state, ty);
                    // Add relation between function and return type
                    self.state.code_graph.relations.push(Relation {
                        source: GraphId::Node(fn_id),
                        target: GraphId::Type(type_id),
                        kind: RelationKind::FunctionReturn,
                    });
                    Some(type_id)
                }
            };

            // Process generic parameters
            let generic_params = self.state.process_generics(&func.sig.generics);

            // Pop the function's ID from the scope stack AFTER processing types/generics
            // Use helper function for logging
            self.pop_scope(&fn_name);

            // Extract doc comments and other attributes
            let docstring = extract_docstring(&func.attrs);
            let attributes = extract_attributes(&func.attrs);

            // Extract function body as a string
            let body = Some(func.block.to_token_stream().to_string());

            // Store function info
            self.state.code_graph.functions.push(FunctionNode {
                id: fn_id,
                name: fn_name,
                span,
                visibility: self.state.convert_visibility(&func.vis),
                parameters,
                return_type,
                generic_params,
                attributes,
                docstring,
                body,
                tracking_hash: Some(self.state.generate_tracking_hash(&func.to_token_stream())),
                cfgs: item_cfgs, // Store only the item's own cfgs
            });

            // Continue visiting the function body (fn_id is already off the stack)
            visit::visit_item_fn(self, func);
        } // End else block for regular functions
    }

    // Visit struct definitions
    fn visit_item_struct(&mut self, item_struct: &'ast ItemStruct) {
        let struct_name = item_struct.ident.to_string();

        // --- CFG Handling (Raw Strings) ---
        let scope_cfgs = self.state.current_scope_cfgs.clone();
        let item_cfgs = extract_cfg_strings(&item_struct.attrs);
        let provisional_effective_cfgs: Vec<String> = scope_cfgs
            .iter()
            .cloned()
            .chain(item_cfgs.iter().cloned())
            .collect();
        let cfg_bytes = calculate_cfg_hash_bytes(&provisional_effective_cfgs);
        // --- End CFG Handling ---

        // Pass ItemKind::Struct and cfg_bytes
        let struct_id = self.add_contains_rel(&struct_name, ItemKind::Struct, cfg_bytes.as_deref());

        // Removed #[cfg] block
        self.debug_new_id(&struct_name, struct_id); // Now uses trace!

        let byte_range = item_struct.span().byte_range();
        let span = (byte_range.start, byte_range.end);

        // Push the struct's ID onto the scope stack BEFORE processing fields/generics
        // Use helper function for logging
        self.push_scope(&struct_name, struct_id, provisional_effective_cfgs.clone()); // Clone cfgs for push

        // Process fields
        let mut fields = Vec::new();
        for field in &item_struct.fields {
            let field_name = field.ident.as_ref().map(|ident| ident.to_string());

            // --- CFG Handling for Field (Raw Strings) ---
            let field_scope_cfgs = self.state.current_scope_cfgs.clone(); // Inherited scope
            let field_item_cfgs = super::attribute_processing::extract_cfg_strings(&field.attrs);
            let field_provisional_effective_cfgs: Vec<String> = field_scope_cfgs
                .iter()
                .cloned()
                .chain(field_item_cfgs.iter().cloned())
                .collect();
            let field_cfg_bytes = calculate_cfg_hash_bytes(&field_provisional_effective_cfgs);
            // --- End CFG Handling ---

            // Pass ItemKind::Field and field_cfg_bytes
            let field_id = self.state.generate_synthetic_node_id(
                &field_name
                    .clone()
                    .unwrap_or_else(|| format!("unnamed_field_in_{}", struct_name)),
                ItemKind::Field,
                field_cfg_bytes.as_deref(), // Pass field's CFG bytes
            );

            // Removed #[cfg] block
            self.debug_new_id(
                // Now uses trace!
                &field
                    .ident
                    .as_ref()
                    .map(|ident| ident.to_string())
                    .unwrap_or("unnamed_struct_field".to_string()),
                field_id,
            );
            let type_id = get_or_create_type(self.state, &field.ty);

            // TODO: Remove Nodes from fields for new version
            // Support as full nodes for now
            let field_node = FieldNode {
                id: field_id,
                name: field_name,
                type_id,
                visibility: self.state.convert_visibility(&field.vis),
                attributes: extract_attributes(&field.attrs), // Non-CFG attributes
                cfgs: field_item_cfgs,                        // Store only the field's own cfgs
            };

            // Add relation between struct and field
            self.state.code_graph.relations.push(Relation {
                source: GraphId::Node(struct_id),
                target: GraphId::Node(field_id),
                kind: RelationKind::StructField,
            });

            fields.push(field_node);
        }

        // Process generic parameters (still within struct's scope)
        let generic_params = self.state.process_generics(&item_struct.generics);

        // Extract doc comments and other attributes
        let docstring = extract_docstring(&item_struct.attrs);
        let attributes = extract_attributes(&item_struct.attrs);

        // Store all structs regardless of visibility
        self.state
            .code_graph
            .defined_types
            .push(TypeDefNode::Struct(StructNode {
                id: struct_id,
                name: struct_name.clone(), // Clone here
                span,
                visibility: self.state.convert_visibility(&item_struct.vis),
                fields,
                generic_params,
                attributes,
                docstring,
                tracking_hash: Some(
                    self.state
                        .generate_tracking_hash(&item_struct.to_token_stream()),
                ),
                cfgs: item_cfgs.clone(), // Store struct's own cfgs
            }));

        // Continue visiting children (fields are handled above, visit generics/where clauses if needed)
        // Note: CFG scope is pushed/popped by push_scope/pop_scope helpers
        visit::visit_item_struct(self, item_struct);

        // Pop the struct's scope using the helper
        self.pop_scope(&struct_name);
    }

    // Visit type alias definitions
    fn visit_item_type(&mut self, item_type: &'ast syn::ItemType) {
        let type_alias_name = item_type.ident.to_string();

        // --- CFG Handling (Raw Strings) ---
        let scope_cfgs = self.state.current_scope_cfgs.clone();
        let item_cfgs = super::attribute_processing::extract_cfg_strings(&item_type.attrs);
        let provisional_effective_cfgs: Vec<String> = scope_cfgs
            .iter()
            .cloned()
            .chain(item_cfgs.iter().cloned())
            .collect();
        let cfg_bytes = calculate_cfg_hash_bytes(&provisional_effective_cfgs);
        // --- End CFG Handling ---

        // Pass ItemKind::TypeAlias and cfg_bytes
        let type_alias_id =
            self.add_contains_rel(&type_alias_name, ItemKind::TypeAlias, cfg_bytes.as_deref());

        // Removed #[cfg] block
        self.debug_new_id(&type_alias_name, type_alias_id); // Now uses trace!

        let span = item_type.extract_span_bytes();

        // Push the type alias's ID onto the scope stack BEFORE processing type/generics
        // Type aliases don't introduce a new CFG scope, so pass current scope cfgs
        self.push_scope(
            &type_alias_name,
            type_alias_id,
            self.state.current_scope_cfgs.clone(),
        );

        // Process the aliased type
        let type_id = get_or_create_type(self.state, &item_type.ty);

        // Process generic parameters
        let generic_params = self.state.process_generics(&item_type.generics);

        // Pop the type alias's ID from the scope stack AFTER processing type/generics
        // Use helper function for logging
        self.pop_scope(&type_alias_name);

        // Extract doc comments and other attributes
        let docstring = extract_docstring(&item_type.attrs);
        let attributes = extract_attributes(&item_type.attrs);

        self.state
            .code_graph
            .defined_types
            .push(TypeDefNode::TypeAlias(TypeAliasNode {
                id: type_alias_id,
                name: type_alias_name,
                span,
                visibility: self.state.convert_visibility(&item_type.vis),
                type_id,
                generic_params,
                attributes,
                docstring,
                tracking_hash: Some(
                    self.state
                        .generate_tracking_hash(&item_type.to_token_stream()),
                ),
                cfgs: item_cfgs, // Store type alias's own cfgs
            }));

        // Type aliases don't define a new CFG scope for children.
        // Continue visiting (type_alias_id is already off the definition stack)
        visit::visit_item_type(self, item_type);
    }

    // Visit union definitions
    fn visit_item_union(&mut self, item_union: &'ast syn::ItemUnion) {
        let union_name = item_union.ident.to_string();

        // --- CFG Handling (Raw Strings) ---
        let scope_cfgs = self.state.current_scope_cfgs.clone();
        let item_cfgs = super::attribute_processing::extract_cfg_strings(&item_union.attrs);
        let provisional_effective_cfgs: Vec<String> = scope_cfgs
            .iter()
            .cloned()
            .chain(item_cfgs.iter().cloned())
            .collect();
        let cfg_bytes = calculate_cfg_hash_bytes(&provisional_effective_cfgs);
        // --- End CFG Handling ---

        // Pass ItemKind::Union and cfg_bytes
        let union_id = self.add_contains_rel(&union_name, ItemKind::Union, cfg_bytes.as_deref());

        // Removed #[cfg] block
        self.debug_new_id(&union_name, union_id); // Now uses trace!

        let span = item_union.extract_span_bytes();

        // Push the union's ID onto the scope stack BEFORE processing fields/generics
        // Use helper function for logging
        self.push_scope(&union_name, union_id, provisional_effective_cfgs.clone()); // Clone cfgs for push

        // Process fields
        let mut fields = Vec::new();
        for field in &item_union.fields.named {
            let field_name = field.ident.as_ref().map(|ident| ident.to_string());

            // --- CFG Handling for Field (Raw Strings) ---
            let field_scope_cfgs = self.state.current_scope_cfgs.clone(); // Inherited scope
            let field_item_cfgs = super::attribute_processing::extract_cfg_strings(&field.attrs);
            let field_provisional_effective_cfgs: Vec<String> = field_scope_cfgs
                .iter()
                .cloned()
                .chain(field_item_cfgs.iter().cloned())
                .collect();
            let field_cfg_bytes = calculate_cfg_hash_bytes(&field_provisional_effective_cfgs);
            // --- End CFG Handling ---

            // Pass ItemKind::Field and field_cfg_bytes
            let field_id = self.state.generate_synthetic_node_id(
                &field_name
                    .clone()
                    .unwrap_or_else(|| format!("unnamed_field_in_{}", union_name)),
                ItemKind::Field,
                field_cfg_bytes.as_deref(), // Pass field's CFG bytes
            );
            // Removed #[cfg] block
            self.debug_new_id(
                // Now uses trace!
                &field_name
                    .clone()
                    .unwrap_or_else(|| format!("Unnamed field of {}", union_name.clone())),
                field_id,
            );
            let type_id = get_or_create_type(self.state, &field.ty);

            let field_node = FieldNode {
                id: field_id,
                name: field_name,
                type_id,
                visibility: self.state.convert_visibility(&field.vis),
                attributes: extract_attributes(&field.attrs),
                cfgs: field_item_cfgs, // Store field's own cfgs
            };

            // Add relation between union and field
            self.state.code_graph.relations.push(Relation {
                source: GraphId::Node(union_id),
                target: GraphId::Node(field_id),
                kind: RelationKind::StructField, // Reuse StructField relation for union fields
            });

            fields.push(field_node);
        }

        // Process generic parameters
        let generic_params = self.state.process_generics(&item_union.generics);

        // Pop the union's ID from the scope stack AFTER processing fields/generics
        // Note: This pop happens *before* visiting children, which might be incorrect
        // if generics/where clauses need the union scope. Let's move the pop after visit.
        // self.state.current_definition_scope.pop(); // Moved below

        // Extract doc comments and other attributes
        let docstring = extract_docstring(&item_union.attrs);
        let attributes = extract_attributes(&item_union.attrs);

        self.state
            .code_graph
            .defined_types
            .push(TypeDefNode::Union(UnionNode {
                id: union_id,
                name: union_name.clone(), // Clone here
                visibility: self.state.convert_visibility(&item_union.vis),
                fields,
                generic_params,
                attributes,
                docstring,
                span,
                tracking_hash: Some(
                    self.state
                        .generate_tracking_hash(&item_union.to_token_stream()),
                ),
                cfgs: item_cfgs, // Store union's own cfgs
            }));

        // Continue visiting children (fields handled above, visit generics/where clauses if needed)
        // Note: CFG scope is pushed/popped by push_scope/pop_scope helpers
        visit::visit_item_union(self, item_union);

        // Pop the union's scope using the helper *after* visiting children
        self.pop_scope(&union_name);
    }

    // Visit enum definitions
    fn visit_item_enum(&mut self, item_enum: &'ast ItemEnum) {
        let enum_name = item_enum.ident.to_string();

        // --- CFG Handling (Raw Strings) ---
        let scope_cfgs = self.state.current_scope_cfgs.clone();
        let item_cfgs = super::attribute_processing::extract_cfg_strings(&item_enum.attrs);
        let provisional_effective_cfgs: Vec<String> = scope_cfgs
            .iter()
            .cloned()
            .chain(item_cfgs.iter().cloned())
            .collect();
        let cfg_bytes = calculate_cfg_hash_bytes(&provisional_effective_cfgs);
        // --- End CFG Handling ---

        // Pass ItemKind::Enum and cfg_bytes
        let enum_id = self.add_contains_rel(&enum_name, ItemKind::Enum, cfg_bytes.as_deref());

        // Removed #[cfg] block
        self.debug_new_id(&enum_name, enum_id); // Now uses trace!

        let span = item_enum.extract_span_bytes();

        // Process variants
        let mut variants = Vec::new();
        for variant in &item_enum.variants {
            let variant_name = variant.ident.to_string();

            // --- CFG Handling for Variant (Raw Strings) ---
            let variant_scope_cfgs = self.state.current_scope_cfgs.clone(); // Inherited scope
            let variant_item_cfgs =
                super::attribute_processing::extract_cfg_strings(&variant.attrs);
            let variant_provisional_effective_cfgs: Vec<String> = variant_scope_cfgs
                .iter()
                .cloned()
                .chain(variant_item_cfgs.iter().cloned())
                .collect();
            let variant_cfg_bytes = calculate_cfg_hash_bytes(&variant_provisional_effective_cfgs);
            // --- End CFG Handling ---

            // Pass ItemKind::Variant and variant_cfg_bytes
            let variant_id = self.state.generate_synthetic_node_id(
                &variant_name,
                ItemKind::Variant,
                variant_cfg_bytes.as_deref(), // Pass variant's CFG bytes
            );

            // Push the variant's ID onto the scope stack BEFORE processing its fields
            // Variants don't introduce a new CFG scope, pass current (enum's) scope cfgs
            self.push_scope(
                &variant_name,
                variant_id,
                self.state.current_scope_cfgs.clone(),
            );

            // Process fields of the variant
            let mut fields = Vec::new();
            match &variant.fields {
                syn::Fields::Named(fields_named) => {
                    for field in &fields_named.named {
                        let field_name = field.ident.as_ref().map(|ident| ident.to_string());

                        // --- CFG Handling for Variant Field (Raw Strings) ---
                        let field_scope_cfgs = self.state.current_scope_cfgs.clone();
                        let field_item_cfgs =
                            super::attribute_processing::extract_cfg_strings(&field.attrs);
                        let field_provisional_effective_cfgs: Vec<String> = field_scope_cfgs
                            .iter()
                            .cloned()
                            .chain(field_item_cfgs.iter().cloned())
                            .collect();
                        let field_cfg_bytes =
                            calculate_cfg_hash_bytes(&field_provisional_effective_cfgs);
                        // --- End CFG Handling ---

                        // Pass ItemKind::Field and field_cfg_bytes
                        let field_id = self.state.generate_synthetic_node_id(
                            &field_name
                                .clone()
                                .unwrap_or_else(|| format!("unnamed_field_in_{}", variant_name)),
                            ItemKind::Field,
                            field_cfg_bytes.as_deref(), // Pass field's CFG bytes
                        );
                        // Removed #[cfg] block
                        self.debug_new_id(
                            // Now uses trace!
                            &field_name
                                .clone()
                                .unwrap_or("unnamed_enum_field".to_string()),
                            field_id,
                        );
                        let type_id = get_or_create_type(self.state, &field.ty);

                        let field_node = FieldNode {
                            id: field_id,
                            name: field_name,
                            type_id,
                            visibility: self.state.convert_visibility(&field.vis),
                            attributes: extract_attributes(&field.attrs), // Non-CFG attributes
                            cfgs: field_item_cfgs,                        // Store field's own cfgs
                        };
                        // Add relation between variant and named field
                        self.state.code_graph.relations.push(Relation {
                            source: GraphId::Node(variant_id),
                            target: GraphId::Node(field_id),
                            kind: RelationKind::VariantField,
                        });

                        fields.push(field_node);
                    }
                }
                syn::Fields::Unnamed(fields_unnamed) => {
                    for (index, field) in fields_unnamed.unnamed.iter().enumerate() {
                        // Use index for unnamed fields
                        let field_name_placeholder =
                            format!("unnamed_field_{}_in_{}", index, variant_name);

                        // --- CFG Handling for Variant Field (Raw Strings) ---
                        let field_scope_cfgs = self.state.current_scope_cfgs.clone();
                        let field_item_cfgs =
                            super::attribute_processing::extract_cfg_strings(&field.attrs);
                        let field_provisional_effective_cfgs: Vec<String> = field_scope_cfgs
                            .iter()
                            .cloned()
                            .chain(field_item_cfgs.iter().cloned())
                            .collect();
                        let field_cfg_bytes =
                            calculate_cfg_hash_bytes(&field_provisional_effective_cfgs);
                        // --- End CFG Handling ---

                        // Pass ItemKind::Field and field_cfg_bytes
                        let field_id = self.state.generate_synthetic_node_id(
                            &field_name_placeholder, // Use placeholder for ID generation
                            ItemKind::Field,
                            field_cfg_bytes.as_deref(),
                        );
                        let type_id = get_or_create_type(self.state, &field.ty);
                        // Removed #[cfg] block
                        self.debug_new_id("unnamed_enum_field", field_id); // Now uses trace!

                        let field_node = FieldNode {
                            id: field_id,
                            name: None, // Tuple fields don't have names
                            type_id,
                            visibility: self.state.convert_visibility(&field.vis),
                            attributes: extract_attributes(&field.attrs), // Non-CFG attributes
                            cfgs: field_item_cfgs,                        // Store field's own cfgs
                        };
                        // Add relation between variant and unnamed field
                        self.state.code_graph.relations.push(Relation {
                            source: GraphId::Node(variant_id),
                            target: GraphId::Node(field_id),
                            kind: RelationKind::VariantField,
                        });

                        fields.push(field_node);
                    }
                }
                syn::Fields::Unit => {
                    // Unit variants don't have fields
                }
            }

            // Pop the variant's ID from the scope stack AFTER processing its fields
            // Use helper function for logging
            self.pop_scope(&variant_name);

            // Extract discriminant if any
            let discriminant = variant
                .discriminant
                .as_ref()
                .map(|(_, expr)| expr.to_token_stream().to_string());

            let variant_node = VariantNode {
                id: variant_id,
                name: variant_name,
                fields,
                discriminant,
                attributes: extract_attributes(&variant.attrs), // Non-CFG attributes
                cfgs: variant_item_cfgs,                        // Store variant's own cfgs
            };

            // Add relation between enum and variant
            self.state.code_graph.relations.push(Relation {
                source: GraphId::Node(enum_id),
                target: GraphId::Node(variant_id),
                kind: RelationKind::EnumVariant,
            });

            variants.push(variant_node);
        }

        // Push the enum's ID onto the scope stack BEFORE processing its generics
        // Use helper function for logging
        self.push_scope(&enum_name, enum_id, provisional_effective_cfgs.clone()); // Clone cfgs for push

        // Process generic parameters
        let generic_params = self.state.process_generics(&item_enum.generics);

        // Pop the enum's ID from the scope stack AFTER processing its generics
        // Note: This pop happens *before* visiting children, which might be incorrect
        // if generics/where clauses need the enum scope. Let's move the pop after visit.
        // self.state.current_definition_scope.pop(); // Moved below

        // Extract doc comments and other attributes
        let docstring = extract_docstring(&item_enum.attrs);
        let attributes = extract_attributes(&item_enum.attrs);

        self.state
            .code_graph
            .defined_types
            .push(TypeDefNode::Enum(EnumNode {
                id: enum_id,
                name: enum_name.clone(), // Clone here
                span,
                visibility: self.state.convert_visibility(&item_enum.vis),
                variants,
                generic_params,
                attributes,
                docstring,
                tracking_hash: Some(
                    self.state
                        .generate_tracking_hash(&item_enum.to_token_stream()),
                ),
                cfgs: item_cfgs.clone(), // Store enum's own cfgs
            }));

        // Continue visiting children (variants/fields handled above, visit generics/where)
        // Note: CFG scope is pushed/popped by push_scope/pop_scope helpers
        visit::visit_item_enum(self, item_enum);

        // Pop the enum's scope using the helper *after* visiting children
        self.pop_scope(&enum_name);
    }

    // Visit impl blocks
    fn visit_item_impl(&mut self, item_impl: &'ast ItemImpl) {
        let impl_name = name_impl(item_impl); // Use helper to generate a name for the impl block

        // --- CFG Handling (Raw Strings) ---
        let scope_cfgs = self.state.current_scope_cfgs.clone();
        let item_cfgs = super::attribute_processing::extract_cfg_strings(&item_impl.attrs);
        let provisional_effective_cfgs: Vec<String> = scope_cfgs
            .iter()
            .cloned()
            .chain(item_cfgs.iter().cloned())
            .collect();
        let cfg_bytes = calculate_cfg_hash_bytes(&provisional_effective_cfgs);
        // --- End CFG Handling ---

        // Pass ItemKind::Impl and cfg_bytes
        let impl_id = self.add_contains_rel(&impl_name, ItemKind::Impl, cfg_bytes.as_deref());

        // Removed #[cfg] block
        self.debug_new_id(&impl_name, impl_id); // Log with the generated name, now uses trace!

        // Process self type,
        // // Case 1: Simple struct
        // impl MyStruct {}
        // // item_impl.self_ty = Type::Path for "MyStruct"
        //
        // // Case 2: Generic struct
        // impl<T> MyStruct<T> {}
        // // item_impl.self_ty = Type::Path for "MyStruct<T>"
        //
        // // Case 3: Trait impl
        // impl MyTrait for MyStruct {}
        // // item_impl.self_ty = Type::Path for "MyStruct"
        // // item_impl.trait_ = Some for "MyTrait"

        // Pushing parent node id to stack BEFORE generating self type.
        // Use helper function for logging
        self.push_scope(&impl_name, impl_id, provisional_effective_cfgs.clone()); // Clone cfgs for push
        let self_type_id = get_or_create_type(self.state, &item_impl.self_ty);

        // Process trait type if it's a trait impl
        let trait_type_id = item_impl.trait_.as_ref().map(|(_, path, _)| {
            let ty = Type::Path(TypePath {
                qself: None,
                path: path.clone(),
            });

            // Return TraitId
            get_or_create_type(self.state, &ty)
        });

        // Process methods
        let mut methods = Vec::new();
        for item in &item_impl.items {
            // NOTE: There are NO other match arms or if-let chains here
            //       to handle syn::ImplItem::Const or syn::ImplItem::Type
            if let syn::ImplItem::Fn(method) = item {
                let method_name = method.sig.ident.to_string();

                // --- CFG Handling for Method (Raw Strings) ---
                let method_scope_cfgs = self.state.current_scope_cfgs.clone(); // Inherited scope
                let method_item_cfgs =
                    super::attribute_processing::extract_cfg_strings(&method.attrs);
                let method_provisional_effective_cfgs: Vec<String> = method_scope_cfgs
                    .iter()
                    .cloned()
                    .chain(method_item_cfgs.iter().cloned())
                    .collect();
                let method_cfg_bytes = calculate_cfg_hash_bytes(&method_provisional_effective_cfgs);
                // --- End CFG Handling ---

                // Pass ItemKind::Function and method_cfg_bytes
                let method_node_id = self.add_contains_rel(
                    &method_name,
                    ItemKind::Function,
                    method_cfg_bytes.as_deref(),
                );

                // Removed #[cfg] block
                self.debug_new_id(&method_name, method_node_id); // Now uses trace!

                // Push the method's ID onto the scope stack BEFORE processing its types/generics
                // Methods don't introduce a new CFG scope, pass current (impl's) scope cfgs
                self.push_scope(
                    &method_name,
                    method_node_id,
                    self.state.current_scope_cfgs.clone(),
                );

                // Process method parameters
                let mut parameters = Vec::new();
                for arg in &method.sig.inputs {
                    if let Some(param) = self.state.process_fn_arg(arg) {
                        // Add relation between method and parameter
                        self.state.code_graph.relations.push(Relation {
                            source: GraphId::Node(method_node_id),
                            target: GraphId::Type(param.type_id),
                            kind: RelationKind::FunctionParameter,
                        });
                        parameters.push(param);
                    }
                }

                // Extract return type if it exists
                let return_type = match &method.sig.output {
                    ReturnType::Default => None,
                    ReturnType::Type(_, ty) => {
                        let type_id = get_or_create_type(self.state, ty);
                        // Add relation between method and return type
                        self.state.code_graph.relations.push(Relation {
                            source: GraphId::Node(method_node_id),
                            target: GraphId::Type(type_id),
                            kind: RelationKind::FunctionReturn,
                        });
                        Some(type_id)
                    }
                };
                self.state.code_graph.relations.push(Relation {
                    source: GraphId::Type(self_type_id), // The struct/enum type
                    target: GraphId::Node(method_node_id),
                    kind: RelationKind::Method,
                });
                // Process generic parameters for methods
                let generic_params = self.state.process_generics(&method.sig.generics);

                // Pop the method's ID from the scope stack AFTER processing its types/generics
                // Use helper function for logging
                self.pop_scope(&method_name);

                // Extract doc comments and other attributes for methods
                let docstring = extract_docstring(&method.attrs);
                let attributes = extract_attributes(&method.attrs);

                // Extract method body as a string
                let body = Some(method.block.to_token_stream().to_string());

                // Store method info
                let method_node = FunctionNode {
                    id: method_node_id,
                    name: method_name.clone(),
                    span: method.extract_span_bytes(),
                    visibility: self.state.convert_visibility(&method.vis),
                    parameters,
                    return_type,
                    generic_params,
                    attributes,
                    docstring,
                    body,
                    tracking_hash: Some(
                        self.state.generate_tracking_hash(&method.to_token_stream()), // Use method tokens
                    ),
                    cfgs: method_item_cfgs, // Store method's own cfgs
                };

                methods.push(method_node);
            }
        }

        // Process generic parameters for impl block
        let generic_params = self.state.process_generics(&item_impl.generics);

        // Store impl info
        let impl_node = ImplNode {
            id: impl_id,
            span: item_impl.extract_span_bytes(),
            self_type: self_type_id,
            trait_type: trait_type_id,
            methods,
            generic_params,
            cfgs: item_cfgs, // Store impl's own cfgs
        };
        self.state.code_graph.impls.push(impl_node);

        // Add relation: ImplementsFor or ImplementsTrait
        let relation_kind = if trait_type_id.is_some() {
            RelationKind::ImplementsTrait
        } else {
            RelationKind::ImplementsFor
        };
        self.state.code_graph.relations.push(Relation {
            source: GraphId::Node(impl_id),
            target: GraphId::Type(self_type_id),
            kind: relation_kind,
        });
        if let Some(trait_type_id) = trait_type_id {
            self.state.code_graph.relations.push(Relation {
                source: GraphId::Node(impl_id),
                target: GraphId::Type(trait_type_id),
                kind: RelationKind::ImplementsTrait,
            });
        }

        // Continue visiting children (methods handled above, visit generics/where)
        // Note: CFG scope is pushed/popped by push_scope/pop_scope helpers
        visit::visit_item_impl(self, item_impl);

        // Pop the impl's scope using the helper *after* visiting children
        self.pop_scope(&impl_name);
    }

    // Visit trait definitions
    fn visit_item_trait(&mut self, item_trait: &'ast ItemTrait) {
        let trait_name = item_trait.ident.to_string();

        // --- CFG Handling (Raw Strings) ---
        let scope_cfgs = self.state.current_scope_cfgs.clone();
        let item_cfgs = super::attribute_processing::extract_cfg_strings(&item_trait.attrs);
        let provisional_effective_cfgs: Vec<String> = scope_cfgs
            .iter()
            .cloned()
            .chain(item_cfgs.iter().cloned())
            .collect();
        let cfg_bytes = calculate_cfg_hash_bytes(&provisional_effective_cfgs);
        // --- End CFG Handling ---

        // Pass ItemKind::Trait and cfg_bytes
        let trait_id = self.add_contains_rel(&trait_name, ItemKind::Trait, cfg_bytes.as_deref());

        // Removed #[cfg] block
        self.debug_new_id(&trait_name, trait_id); // Now uses trace!

        // Process methods
        let mut methods = Vec::new();
        for item in &item_trait.items {
            if let syn::TraitItem::Fn(method) = item {
                let method_name = method.sig.ident.to_string();

                // --- CFG Handling for Trait Method (Raw Strings) ---
                let method_scope_cfgs = self.state.current_scope_cfgs.clone(); // Inherited scope
                let method_item_cfgs =
                    super::attribute_processing::extract_cfg_strings(&method.attrs);
                let method_provisional_effective_cfgs: Vec<String> = method_scope_cfgs
                    .iter()
                    .cloned()
                    .chain(method_item_cfgs.iter().cloned())
                    .collect();
                let method_cfg_bytes = calculate_cfg_hash_bytes(&method_provisional_effective_cfgs);
                // --- End CFG Handling ---

                // Pass ItemKind::Function and method_cfg_bytes
                // Note: This ID is for the *definition* within the trait.
                let method_node_id = self.state.generate_synthetic_node_id(
                    &method_name,
                    ItemKind::Function,
                    method_cfg_bytes.as_deref(),
                );

                // Removed #[cfg] block
                self.debug_new_id(&method_name, method_node_id); // Now uses trace!

                // Push the method's ID onto the scope stack BEFORE processing its types/generics
                // Methods don't introduce a new CFG scope, pass current (trait's) scope cfgs
                self.push_scope(
                    &method_name,
                    method_node_id,
                    self.state.current_scope_cfgs.clone(),
                );

                // Process method parameters
                let mut parameters = Vec::new();
                for arg in &method.sig.inputs {
                    if let Some(param) = self.state.process_fn_arg(arg) {
                        // Add relation between method and parameter
                        self.state.code_graph.relations.push(Relation {
                            source: GraphId::Node(method_node_id),
                            target: GraphId::Type(param.type_id),
                            kind: RelationKind::FunctionParameter,
                        });
                        parameters.push(param);
                    }
                }

                // Extract return type if it exists
                let return_type = match &method.sig.output {
                    ReturnType::Default => None,
                    ReturnType::Type(_, ty) => {
                        let type_id = get_or_create_type(self.state, ty);
                        // Add relation between method and return type
                        self.state.code_graph.relations.push(Relation {
                            source: GraphId::Node(method_node_id),
                            target: GraphId::Type(type_id),
                            kind: RelationKind::FunctionReturn,
                        });
                        Some(type_id)
                    }
                };

                // Process generic parameters for methods
                let generic_params = self.state.process_generics(&method.sig.generics);

                // Pop the method's ID from the scope stack AFTER processing its types/generics
                // Use helper function for logging
                self.pop_scope(&method_name);

                // Extract doc comments and other attributes for methods
                let docstring = extract_docstring(&method.attrs);
                let attributes = extract_attributes(&method.attrs);

                // Extract method body if available (trait methods may have default implementations)
                let body = method
                    .default
                    .as_ref()
                    .map(|block| block.to_token_stream().to_string());

                // Store method info
                let method_node = FunctionNode {
                    id: method_node_id,
                    name: method_name,
                    span: method.extract_span_bytes(),
                    visibility: self.state.convert_visibility(&item_trait.vis),
                    parameters,
                    return_type,
                    generic_params,
                    attributes,
                    docstring,
                    body,
                    tracking_hash: Some(
                        self.state
                            .generate_tracking_hash(&method.clone().to_token_stream()),
                    ),
                    cfgs: method_item_cfgs, // Store method's own cfgs
                };
                methods.push(method_node);
            }
        }

        // Push the trait's ID onto the scope stack BEFORE processing its generics/supertraits
        // Use helper function for logging
        self.push_scope(&trait_name, trait_id, provisional_effective_cfgs.clone()); // Clone cfgs for push

        // Process generic parameters
        let generic_params = self.state.process_generics(&item_trait.generics);

        // Process super traits
        let super_traits: Vec<TypeId> = item_trait
            .supertraits
            .iter()
            .filter_map(|bound| {
                // Use filter_map to handle non-trait bounds if necessary
                match bound {
                    syn::TypeParamBound::Trait(trait_bound) => {
                        // Construct a Type::Path from the TraitBound's path
                        // This correctly represents the supertrait type itself.
                        let ty = syn::Type::Path(syn::TypePath {
                            qself: None, // Supertraits typically don't have qself
                            path: trait_bound.path.clone(),
                        });
                        Some(get_or_create_type(self.state, &ty))
                    }
                    syn::TypeParamBound::Lifetime(_) => {
                        // We don't store lifetime bounds as supertrait TypeIds currently
                        None
                    }
                    // Handle other TypeParamBound variants like Verbatim if needed in the future
                    _ => {
                        // Log or handle unexpected bound types if necessary
                        None
                    }
                }
            })
            .collect();

        // Pop the trait's ID from the scope stack AFTER processing its generics/supertraits
        // Note: This pop happens *before* visiting children, which might be incorrect
        // if generics/where clauses need the trait scope. Let's move the pop after visit.
        // self.state.current_definition_scope.pop(); // Moved below

        // Extract doc comments and other attributes
        let docstring = extract_docstring(&item_trait.attrs);
        let attributes = extract_attributes(&item_trait.attrs);

        // Store trait info
        let trait_node = TraitNode {
            id: trait_id,
            name: trait_name.clone(),
            span: item_trait.extract_span_bytes(),
            visibility: self.state.convert_visibility(&item_trait.vis),
            methods,
            generic_params,
            super_traits: super_traits.clone(),
            attributes,
            docstring,
            tracking_hash: Some(
                self.state
                    .generate_tracking_hash(&item_trait.to_token_stream()),
            ),
            cfgs: item_cfgs.clone(), // Store trait's own cfgs
        };
        self.state.code_graph.traits.push(trait_node);
        // }

        // Add relation for super traits
        for super_trait_id in &super_traits {
            self.state.code_graph.relations.push(Relation {
                source: GraphId::Node(trait_id),
                target: GraphId::Type(*super_trait_id),
                kind: RelationKind::Inherits,
            });
        }

        // Continue visiting children (methods handled above, visit generics/where/supertraits)
        // Note: CFG scope is pushed/popped by push_scope/pop_scope helpers
        visit::visit_item_trait(self, item_trait);

        // Pop the trait's scope using the helper *after* visiting children
        self.pop_scope(&trait_name);
    }

    fn visit_item_mod(&mut self, module: &'ast syn::ItemMod) {
        let module_name = module.ident.to_string();

        // --- CFG Handling (Raw Strings) ---
        let scope_cfgs = self.state.current_scope_cfgs.clone();
        let item_cfgs = super::attribute_processing::extract_cfg_strings(&module.attrs);
        let provisional_effective_cfgs: Vec<String> = scope_cfgs
            .iter()
            .cloned()
            .chain(item_cfgs.iter().cloned())
            .collect();
        let cfg_bytes = calculate_cfg_hash_bytes(&provisional_effective_cfgs);
        // --- End CFG Handling ---

        // Pass ItemKind::Module and cfg_bytes
        let module_id = self.add_contains_rel(&module_name, ItemKind::Module, cfg_bytes.as_deref());

        // Removed #[cfg] block
        self.debug_mod_stack(); // Now uses trace!

        let span = module.extract_span_bytes();

        // Removed #[cfg] block
        self.debug_new_id(&module_name, module_id); // Now uses trace!

        // Save current path before entering module
        let parent_path = self.state.current_module_path.clone();

        // Update path for nested module visitation
        self.state.current_module_path.push(module_name.clone());

        // Process module contents

        // Create module node with proper path tracking
        // Create module node with proper hierarchy tracking
        let module_def = match &module.content {
            Some(_) => ModuleDef::Inline {
                items: Vec::new(),
                span,
                // Inline modules don't store their own CFG here; it's part of the scope.
            },
            None => ModuleDef::Declaration {
                declaration_span: span,
                resolved_definition: None, // Resolved during phase 3 resolution
                                           // cfgs removed from here, belongs on ModuleNode
            },
        };
        let module_node = ModuleNode {
            id: module_id,
            name: module_name.clone(),
            path: self.state.current_module_path.clone(),
            visibility: self.state.convert_visibility(&module.vis),
            attributes: extract_attributes(&module.attrs),
            docstring: extract_docstring(&module.attrs),
            imports: Vec::new(),
            exports: Vec::new(),
            span, // Assign the extracted span
            tracking_hash: Some(self.state.generate_tracking_hash(&module.to_token_stream())),
            module_def,
            cfgs: item_cfgs, // Store module's own cfgs
        };

        // Restore parent path after processing module
        self.state.current_module_path = parent_path;

        self.state.current_module.push(module_node.name.clone());
        // Removed #[cfg] block
        self.log_push("current module", &self.state.current_module); // Now uses trace!

        self.state
            .current_module_path
            .push(module_node.name.clone());
        // Removed #[cfg] block
        self.log_push("current_module_path", &self.state.current_module_path); // Now uses trace!

        self.state.code_graph.modules.push(module_node);

        // Push the module's scope using the helper *before* visiting children
        self.push_scope(&module_name, module_id, provisional_effective_cfgs);

        // Continue visiting children.
        visit::visit_item_mod(self, module);

        // Pop the module's scope using the helper *after* visiting children
        self.pop_scope(&module_name);

        let popped_mod = self.state.current_module.pop();
        // Removed #[cfg] block
        self.log_pop("current_module", popped_mod, &self.state.current_module); // Now uses trace!

        let popped_path = self.state.current_module_path.pop();
        // Removed #[cfg] block
        self.log_pop(
            // Now uses trace!
            "current_module_path",
            popped_path,
            &self.state.current_module_path,
        );
    }

    /// Visits `use` statements during AST traversal.
    ///
    /// # Current Limitations
    /// - Does not handle macro-generated `use` statements (MVP exclusion)
    /// - `pub use` re-exports are treated as regular imports
    ///
    /// # Flow
    /// 1. Captures raw path segments and spans
    /// 2. Normalizes `self`/`super` prefixes
    /// 3. Stores statements in `VisitorState` for later resolution
    fn visit_item_use(&mut self, use_item: &'ast syn::ItemUse) {
        // --- CFG Handling (Raw Strings) ---
        let scope_cfgs = self.state.current_scope_cfgs.clone();
        let item_cfgs = super::attribute_processing::extract_cfg_strings(&use_item.attrs);
        let provisional_effective_cfgs: Vec<String> = scope_cfgs
            .iter()
            .cloned()
            .chain(item_cfgs.iter().cloned())
            .collect();
        let cfg_bytes = calculate_cfg_hash_bytes(&provisional_effective_cfgs);
        // --- End CFG Handling ---

        // Process the use tree first
        let base_path = if use_item.leading_colon.is_some() {
            vec!["".to_string()] // Absolute path
        } else {
            Vec::new() // Relative path
        };

        let vis_kind = self.state.convert_visibility(&use_item.vis);
        // Pass cfg_bytes down to process_use_tree
        let imports =
            self.process_use_tree(&use_item.tree, &base_path, cfg_bytes.as_deref(), &vis_kind);

        // Get a mutable reference to the graph only once
        let graph = &mut self.state.code_graph;
        let current_module_path = &self.state.current_module_path;

        // Add all imports to the current module
        if let Some(module) = graph
            .modules
            .iter_mut()
            .find(|m| &m.path == current_module_path)
        {
            let module_id = module.id;

            for import in imports {
                // Add module import relation
                graph.relations.push(Relation {
                    source: GraphId::Node(module_id),
                    target: GraphId::Node(import.id),
                    kind: RelationKind::ModuleImports,
                });

                graph.use_statements.push(import.clone());
                // Add to module's imports list
                module.imports.push(import);
            }
        }
        visit::visit_item_use(self, use_item);
    }
    // Continue visiting

    /// Visit extern crate item, e.g. `extern crate serde;`
    /// Note that these may be renamed, e.g. `extern crate serde as MySerde;`
    /// This visit method will:
    ///     - Create an `ImportNode` for the extern crate and add it to current_module.imports
    ///     - Adds a new `Contains` relation with current module (through `add_contains_rel`)
    ///     - `NodeId::Synthetic` created through `add_contains_rel` using the *visible name*.
    fn visit_item_extern_crate(&mut self, extern_crate: &'ast syn::ItemExternCrate) {
        // --- CFG Handling (Raw Strings) ---
        let scope_cfgs = self.state.current_scope_cfgs.clone();
        let item_cfgs = super::attribute_processing::extract_cfg_strings(&extern_crate.attrs);
        let provisional_effective_cfgs: Vec<String> = scope_cfgs
            .iter()
            .cloned()
            .chain(item_cfgs.iter().cloned())
            .collect();
        let cfg_bytes = calculate_cfg_hash_bytes(&provisional_effective_cfgs);
        // --- End CFG Handling ---

        // Determine the visible name (alias or original name)
        let visible_name = extern_crate
            .rename
            .as_ref()
            .map(|(_, id)| id.to_string()) // Use the rename identifier if present
            .unwrap_or_else(|| extern_crate.ident.to_string()); // Otherwise, use the original identifier

        // Pass the *visible name*, ItemKind::ExternCrate, and cfg_bytes
        let import_id = self.add_contains_rel(
            &visible_name,
            ItemKind::ExternCrate,
            cfg_bytes.as_deref(), // Pass CFG bytes
        );

        // Removed #[cfg] block
        self.debug_new_id(&visible_name, import_id); // Log with visible name, now uses trace!

        let crate_name = extern_crate.ident.to_string(); // Keep original name for path etc.

        let span = extern_crate.extract_span_bytes();

        let import_node = ImportNode {
            id: import_id,
            span,
            path: vec![crate_name.clone()], // Path is just crate name
            kind: ImportKind::ExternCrate,  // <<< Correct kind
            // Name used by `use` statements
            visible_name: extern_crate
                .rename
                .as_ref()
                .map(|(_, id)| id.to_string())
                .unwrap_or_else(|| crate_name.clone()),
            // Original name, only Some if item is renamed, otherwise None
            original_name: extern_crate.rename.as_ref().map(|_| crate_name.clone()),
            is_glob: false,
            is_self_import: false,
            cfgs: item_cfgs, // Store extern crate's own cfgs
        };
        let module_id = if let Some(module) = self
            .state
            .code_graph
            .modules
            .iter_mut()
            .find(|m| m.items().is_some_and(|items| items.contains(&import_id)))
        {
            module.imports.push(import_node.clone());
            module.id
        } else {
            panic!(
                "Could not find containing module for import_node: {:#?}",
                import_node
            );
        };
        self.state.code_graph.use_statements.push(import_node);
        self.state.code_graph.relations.push(Relation {
            source: GraphId::Node(module_id),
            target: GraphId::Node(import_id),
            kind: RelationKind::ModuleImports,
        });

        let type_id = {
            // 1. Construct a representative syn::Type for the external crate.
            //    Using just the crate name as the path is simplest.
            let syn_type_path = syn::parse_str::<syn::TypePath>(&crate_name).unwrap_or_else(|_| {
                // Fallback if crate_name isn't a valid path segment (highly unlikely)
                eprintln!(
                    "Warning: Could not parse extern crate name '{}' as a TypePath.",
                    crate_name
                );
                syn::parse_str::<syn::TypePath>("__invalid_extern_crate_name__").unwrap()
            });
            let syn_type = syn::Type::Path(syn_type_path);

            // 2. Use the standard function to get/create the TypeId and register the TypeNode.
            //    This handles caching and ensures the TypeNode is added to type_graph.
            //    The synthetic ID will be based on hashing "crate_name".
            get_or_create_type(self.state, &syn_type)
        };

        // --- Relation Creation ---
        // TODO: Remove this relation
        // I did not really understand that an `extern crate some_crate` statement actually worked
        // on the entire crate namespace itself, and cannot be specific enough to import a type
        // directly. Therefore, the `TypeId` generation doesn't make sense here. `TypeId` is
        // reserved for actual types.
        self.state.code_graph.relations.push(Relation {
            source: GraphId::Node(import_id),
            target: GraphId::Type(type_id), // type_id is now guaranteed to be registered
            kind: RelationKind::Uses,
        });

        // Continue visiting
        visit::visit_item_extern_crate(self, extern_crate);
    }

    // Visit constant items
    fn visit_item_const(&mut self, item_const: &'ast syn::ItemConst) {
        let const_name = item_const.ident.to_string();

        // --- CFG Handling (Raw Strings) ---
        let scope_cfgs = self.state.current_scope_cfgs.clone();
        let item_cfgs = super::attribute_processing::extract_cfg_strings(&item_const.attrs);
        let provisional_effective_cfgs: Vec<String> = scope_cfgs
            .iter()
            .cloned()
            .chain(item_cfgs.iter().cloned())
            .collect();
        let cfg_bytes = calculate_cfg_hash_bytes(&provisional_effective_cfgs);
        // --- End CFG Handling ---

        // Pass ItemKind::Const and cfg_bytes
        let const_id = self.add_contains_rel(&const_name, ItemKind::Const, cfg_bytes.as_deref());

        // Removed #[cfg] block
        self.debug_new_id(&const_name, const_id); // Now uses trace!

        let span = item_const.extract_span_bytes();

        // Process the type
        // NOTE: I'm not sure this approach really makes sense for the "type" of the const here. I
        // mean I suppose you could consider it in the "scope" of the const definition, but it is
        // fundementally different from the way a, e.g., generic is "scoped", or `Self`. For now
        // this gives us unique IDs, but this will probably have to change at some point for
        // incremental parsing. For now its... fine, I suppose.
        // push "parent" scope first
        self.state.current_definition_scope.push(const_id);
        let type_id = get_or_create_type(self.state, &item_const.ty);
        // pop "parent" scope
        self.state.current_definition_scope.pop();

        // Extract the value expression as a string
        let value = Some(item_const.expr.to_token_stream().to_string());

        // Extract doc comments and other attributes
        let docstring = extract_docstring(&item_const.attrs);
        let attributes = extract_attributes(&item_const.attrs);

        // Create the constant node
        let const_node = ValueNode {
            id: const_id,
            name: const_name,
            visibility: self.state.convert_visibility(&item_const.vis),
            type_id,
            kind: ValueKind::Constant,
            value,
            attributes,
            docstring,
            span,
            tracking_hash: Some(
                self.state
                    .generate_tracking_hash(&item_const.to_token_stream()),
            ),
            cfgs: item_cfgs, // Store const's own cfgs
        };

        // Add the constant to the code graph
        self.state.code_graph.values.push(const_node);

        // Add relation between constant and its type
        self.state.code_graph.relations.push(Relation {
            source: GraphId::Node(const_id),
            target: GraphId::Type(type_id),
            kind: RelationKind::ValueType,
        });

        // add this state management if recursing into the children of the const node, which
        // should... only happen if we are parding `syn::Expr`?
        // self.state.current_definition_scope.push(const_id);
        // Continue visiting
        visit::visit_item_const(self, item_const);
        // pop parent id onto stack, appropriate state management
        // self.state.current_definition_scope.pop();
    }

    // Visit static items
    fn visit_item_static(&mut self, item_static: &'ast syn::ItemStatic) {
        let static_name = item_static.ident.to_string();

        // --- CFG Handling (Raw Strings) ---
        let scope_cfgs = self.state.current_scope_cfgs.clone();
        let item_cfgs = super::attribute_processing::extract_cfg_strings(&item_static.attrs);
        let provisional_effective_cfgs: Vec<String> = scope_cfgs
            .iter()
            .cloned()
            .chain(item_cfgs.iter().cloned())
            .collect();
        let cfg_bytes = calculate_cfg_hash_bytes(&provisional_effective_cfgs);
        // --- End CFG Handling ---

        // Pass ItemKind::Static and cfg_bytes
        let static_id = self.add_contains_rel(&static_name, ItemKind::Static, cfg_bytes.as_deref());

        // Removed #[cfg] block
        self.debug_new_id(&static_name, static_id); // Now uses trace!

        let span = item_static.extract_span_bytes();

        // Process the type
        self.state.current_definition_scope.push(static_id);
        let type_id = get_or_create_type(self.state, &item_static.ty);
        self.state.current_definition_scope.pop();

        // Extract the value expression as a string
        let value = Some(item_static.expr.to_token_stream().to_string());

        // Extract doc comments and other attributes
        let docstring = extract_docstring(&item_static.attrs);
        let attributes = extract_attributes(&item_static.attrs);

        // Create the static node
        let static_node = ValueNode {
            id: static_id,
            name: static_name,
            visibility: self.state.convert_visibility(&item_static.vis),
            type_id,
            kind: ValueKind::Static {
                is_mutable: matches!(item_static.mutability, syn::StaticMutability::Mut(_)),
            },
            value,
            attributes,
            docstring,
            span,
            tracking_hash: Some(
                self.state
                    .generate_tracking_hash(&item_static.to_token_stream()),
            ),
            cfgs: item_cfgs, // Store static's own cfgs
        };

        // Add the static to the code graph
        self.state.code_graph.values.push(static_node);

        // Add relation between static and its type
        self.state.code_graph.relations.push(Relation {
            source: GraphId::Node(static_id),
            target: GraphId::Type(type_id),
            kind: RelationKind::ValueType,
        });

        // Continue visiting
        // add this state management if recursing into the children of the const node, which
        // should... only happen if we are parding `syn::Expr`?
        // push parent id onto stack for type processing
        // self.state.current_definition_scope.push(static_id);
        visit::visit_item_static(self, item_static);
        // pop parent id onto stack, appropriate state management
        // self.state.current_definition_scope.pop();
    }

    // Visit macro definitions (macro_rules!)
    fn visit_item_macro(&mut self, item_macro: &'ast syn::ItemMacro) {
        let is_exported = item_macro
            .attrs
            .iter()
            .any(|attr| attr.path().is_ident("macro_export"));

        // Determine visibility based on #[macro_export]
        let visibility = if is_exported {
            VisibilityKind::Public
        } else {
            // Macros defined without #[macro_export] follow module scoping rules.
            // Represent this as Inherited, meaning visibility depends on the containing module.
            VisibilityKind::Inherited
        };

        // Original suggestion had a check here to potentially skip non-exported macros.
        // Let's remove that skip for now and process all encountered macro_rules! definitions,
        // assigning appropriate visibility. We can filter later if needed.
        // if !is_exported && visibility == VisibilityKind::Inherited {
        //     return; // Reconsider this later if we only want exported/explicitly pub macros
        // }

        let macro_name = item_macro
            .ident
            .as_ref()
            .map(|ident| ident.to_string())
            .unwrap_or_else(|| "unnamed_macro".to_string());

        // --- CFG Handling (Raw Strings) ---
        let scope_cfgs = self.state.current_scope_cfgs.clone();
        let item_cfgs = super::attribute_processing::extract_cfg_strings(&item_macro.attrs);
        let provisional_effective_cfgs: Vec<String> = scope_cfgs
            .iter()
            .cloned()
            .chain(item_cfgs.iter().cloned())
            .collect();
        let cfg_bytes = calculate_cfg_hash_bytes(&provisional_effective_cfgs);
        // --- End CFG Handling ---

        // Pass ItemKind::Macro and cfg_bytes
        let macro_id = self.add_contains_rel(&macro_name, ItemKind::Macro, cfg_bytes.as_deref());

        // Removed #[cfg] block
        self.debug_new_id(&macro_name, macro_id); // Now uses trace!

        let span = item_macro.extract_span_bytes();

        let body = Some(item_macro.mac.tokens.to_string());
        let docstring = extract_docstring(&item_macro.attrs);
        let attributes = extract_attributes(&item_macro.attrs); // Includes #[macro_export] if present

        let macro_node = MacroNode {
            id: macro_id,
            name: macro_name,
            visibility, // Use the correctly determined visibility
            kind: MacroKind::DeclarativeMacro,
            // rules field removed
            attributes,
            docstring,
            body,
            span,
            tracking_hash: Some(
                self.state
                    .generate_tracking_hash(&item_macro.to_token_stream()),
            ),
            cfgs: item_cfgs, // Store macro's own cfgs
        };

        self.state.code_graph.macros.push(macro_node);

        // Do NOT recurse into the macro body with visit::visit_item_macro
    }
}

/// Helper function to name item_impol in visit_item_impl
fn name_impl(item_impl: &ItemImpl) -> String {
    // Naming:
    // `impl MyStruct { ... }`
    // `self_type_str`: `"MyStruct"`
    // `trait_str`: `None`
    //   `impl_name`:** `"impl MyStruct"`
    //
    // `impl<T: Display> MyStruct<T> { ... }`
    // `self_type_str`: `"MyStruct < T >"` (Note: `to_token_stream` includes spaces around `< >`)
    // `trait_str`: `None`
    //   `impl_name`:** `"impl MyStruct < T >"`
    //
    // `impl MyTrait for MyStruct { ... }`
    // `self_type_str`: `"MyStruct"`
    // `trait_str`: `Some("MyTrait")`
    // `impl_name`: `"impl MyTrait for MyStruct"`
    let self_type_str = item_impl.self_ty.to_token_stream().to_string();
    let trait_str = item_impl
        .trait_
        .as_ref()
        .map(|(_, path, _)| path.to_token_stream().to_string());

    match trait_str {
        Some(t) => format!("impl {} for {}", t, self_type_str),
        None => format!("impl {}", self_type_str),
    }
}
