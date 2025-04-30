use super::attribute_processing::extract_attributes;
use super::attribute_processing::extract_cfg_strings;
use super::attribute_processing::extract_docstring;
use super::state::VisitorState;
use super::type_processing::get_or_create_type;
use crate::parser::graph::GraphAccess;
use crate::parser::nodes::AssociatedItemId;
use crate::parser::nodes::ConstNode;
use crate::parser::nodes::MacroNodeId;
use crate::parser::nodes::MethodNode;
use crate::parser::nodes::ModuleNode;
use crate::parser::nodes::ModuleNodeId; // Keep
use crate::parser::nodes::PrimaryNodeId; // Keep
use crate::parser::nodes::StaticNode; // Keep
use crate::parser::nodes::StructNodeId;
use crate::parser::nodes::StructNodeInfo;
// Keep
// Remove StructNodeInfo import, it's generated implicitly now
use crate::parser::nodes::{
    ConstNodeInfo, // Add generated info struct imports
    EnumNode,
    EnumNodeInfo,
    FieldNode,
    FieldNodeInfo,
    FunctionNode,
    FunctionNodeInfo,
    ImplNode,
    ImplNodeInfo,
    ImportKind,
    ImportNode,
    ImportNodeInfo,
    MacroKind,
    MacroNode,
    MacroNodeInfo,
    MethodNodeInfo, // Add generated info struct imports
    ModuleNodeInfo, // Add generated info struct imports
    ProcMacroKind,
    StaticNodeInfo, // Add generated info struct imports
    StructNode,
    TraitNode,
    TraitNodeInfo, // Add generated info struct imports
    TypeAliasNode,
    TypeAliasNodeInfo, // Add generated info struct imports
    TypeDefNode,
    UnionNode,
    UnionNodeInfo, // Add generated info struct imports
    VariantNode,
    VariantNodeInfo, // Add generated info struct imports
};
use crate::parser::relations::*;
use crate::parser::types::*;
use crate::parser::visitor::calculate_cfg_hash_bytes;
use crate::parser::ExtractSpan;

use crate::error::CodeVisitorError; // Import the new error type
use crate::parser::nodes::ModuleKind;
use ploke_core::ItemKind;
use ploke_core::{NodeId, TypeId};

use colored::*;
use log::{error, trace}; // Import error macro
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
const LOG_TARGET_STACK_TRACE: &str = "stack_trace";

impl<'a> CodeVisitor<'a> {
    pub fn new(state: &'a mut VisitorState) -> Self {
        Self { state }
    }

    pub(crate) fn validate_unique_rels(&self) -> bool {
        self.state.code_graph.validate_unique_rels()
    }

    // Update return type to use SyntacticRelation
    pub(crate) fn relations(&self) -> &[SyntacticRelation] {
        self.state.code_graph.relations()
    }

    // Helper method to extract path segments from a use tree
    // Needs cfg_bytes passed down from visit_item_use
    /// Recursively processes a `syn::UseTree` to extract `ImportNode`s.
    ///
    /// Returns a `Result` containing a `Vec` of `ImportNode`s or a `CodeVisitorError`
    /// if registration fails for any item within the tree.
    fn process_use_tree(
        &mut self,
        tree: &syn::UseTree,
        // Accept base_path by value (Vec<String>)
        mut base_path: Vec<String>,
        cfg_bytes: Option<&[u8]>,
        vis_kind: &VisibilityKind,
    ) -> Result<Vec<ImportNode>, CodeVisitorError> {
        // Return Result
        let mut imports = Vec::new();
        // AI: Let's rethink how we are using this function. I don't think it is going to do what
        // we want, which is, as described in the doc comment, to process a `syn::UseTree`. It
        // looks like you earlier tried to make it iterative but forgot to add the loop, and now it
        // is kind of caught between the two approaches. Let's go ahead and make it fully
        // rescursive, since that suits the tree-like structure and potentially branching paths of
        // a glob import well. Refactor this funciton so it will just be recursive without the
        // half-done recursive structure (e.g. the `let mut imports` above). AI!

        match tree {
            syn::UseTree::Path(path) => {
                // No need to clone base_path, just push and pass ownership
                base_path.push(path.ident.to_string());
                // Use `?` to propagate errors from recursive calls
                imports.extend(self.process_use_tree(&path.tree, base_path, cfg_bytes, vis_kind)?);
            }
            syn::UseTree::Name(name) => {
                let mut full_path = base_path.to_vec();
                let use_name = name.ident.to_string();
                let mut is_self_import = false;

                let span = name.extract_span_bytes();

                let checked_name = if use_name == "self" {
                    is_self_import = true;
                    full_path.last().cloned().unwrap_or_default() // Handle empty path case
                } else {
                    full_path.push(use_name.clone());
                    use_name // This is the visible name in this case
                };

                // Register the new node ID (but don't get parent ID, handled later)
                let registration_result = self.register_new_node_id(
                    &checked_name,
                    ItemKind::Import,
                    cfg_bytes, // Pass down received cfg_bytes
                );
                // Log the error before returning
                error!(target: LOG_TARGET_TRACE, "{}", err);
                return Err(err);

                let (import_base_id, _) = registration_result.unwrap();

                let import_info = ImportNodeInfo {
                    id: import_base_id,
                    source_path: full_path,
                    kind: ImportKind::UseStatement(vis_kind.to_owned()),
                    visible_name: checked_name,
                    original_name: None,
                    is_glob: false,
                    span,
                    is_self_import,
                    cfgs: Vec::new(), // CFGs are handled at the ItemUse level
                };
                // Return a Vec containing just this single import node.
                Ok(vec![ImportNode::new(import_info)])
            }
            syn::UseTree::Rename(rename) => {
                // Base case: Renaming an imported item.
                let mut full_path_for_id = base_path.clone(); // Clone for ID registration if needed
                let original_name = rename.ident.to_string();
                let visible_name = rename.rename.to_string(); // The 'as' name

                let span = rename.extract_span_bytes();

                // Register the new node ID
                let registration_result = self.register_new_node_id(
                    &visible_name,
                    ItemKind::Import,
                    cfg_bytes, // Pass down received cfg_bytes
                );
                // Check if registration failed
                if registration_result.is_none() {
                    let err = CodeVisitorError::RegistrationFailed {
                        item_name: visible_name.clone(),
                        item_kind: ItemKind::Import,
                    };
                    error!(target: LOG_TARGET_TRACE, "{}", err);
                    return Err(err);
                }
                let (import_base_id, _) = registration_result.unwrap();

                // The source path uses the original name.
                let mut source_path = base_path; // Take ownership
                source_path.push(original_name.clone());

                let import_info = ImportNodeInfo {
                    id: import_base_id,
                    source_path: full_path,
                    kind: ImportKind::UseStatement(vis_kind.to_owned()),
                    visible_name,
                    original_name: Some(original_name), // The original name before 'as'
                    is_glob: false,
                    span,
                    is_self_import: false,
                    cfgs: Vec::new(), // CFGs are handled at the ItemUse level
                };
                // Return a Vec containing just this single import node.
                Ok(vec![ImportNode::new(import_info)])
            }
            syn::UseTree::Glob(glob) => {
                // Base case: A glob import.
                let registration_result = self.register_new_node_id(
                    "<glob>", // Use placeholder name
                    ItemKind::Import,
                    cfg_bytes, // Pass down received cfg_bytes
                );
                // Check if registration failed
                if registration_result.is_none() {
                    let err = CodeVisitorError::RegistrationFailed {
                        item_name: "<glob>".to_string(),
                        item_kind: ItemKind::Import,
                    };
                    error!(target: LOG_TARGET_TRACE, "{}", err);
                    return Err(err);
                }
                let (import_base_id, _) = registration_result.unwrap();

                // Use the original base_path
                let full_path = base_path; // Take ownership

                let import_info = ImportNodeInfo {
                    id: import_base_id,
                    source_path: full_path,
                    kind: ImportKind::UseStatement(vis_kind.to_owned()),
                    visible_name: "*".to_string(),
                    original_name: None,
                    is_glob: true,
                    span: glob.extract_span_bytes(),
                    is_self_import: false,
                    cfgs: Vec::new(), // CFGs are handled at the ItemUse level
                };
                // Return a Vec containing just this single import node.
                Ok(vec![ImportNode::new(import_info)])
            }
            syn::UseTree::Group(group) => {
                // Recursive case: A group import like `use std::{fs, io};`
                let mut group_imports = Vec::new();
                for item in &group.items {
                    // Recursively process each item within the group.
                    // Crucially, clone `base_path` for each recursive call,
                    // as they represent different branches from the same base.
                    match self.process_use_tree(item, base_path.clone(), cfg_bytes, vis_kind) {
                        Ok(item_imports) => group_imports.extend(item_imports),
                        Err(e) => return Err(e), // Propagate the first error encountered.
                    }
                }
                // Return the aggregated imports from all items in the group.
                Ok(group_imports)
            }
        }
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

            trace!(target: LOG_TARGET_STACK_TRACE, "  [PUSH ITEM] Mod: {} -> Item: {} ({}) | Items now: [{}]",
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
        trace!(target: LOG_TARGET_STACK_TRACE, "  [PUSH STACK] {}: {:?} -> {:?}",
            stack_name.blue(),
            stack.last().unwrap_or(&"<empty>".to_string()).green(),
            stack
        );
    }

    // Removed #[cfg(feature = "verbose_debug")]
    fn log_pop(&self, stack_name: &str, popped: Option<String>, stack: &[String]) {
        trace!(target: LOG_TARGET_STACK_TRACE, "  [POP STACK] {}: {:?} -> {:?}",
            stack_name.blue(),
            popped.unwrap_or("<empty>".to_string()).red(),
            stack
        );
    }
    /// Generates a new `NodeId::Synthetic` for the item being visited using the
    /// `VisitorState` helper and adds the new ID to the current module's item list.
    /// Returns the generated base `NodeId`. The caller is responsible for creating
    /// the appropriate `SyntacticRelation` variant.
    /// Requires the item's name, kind, and calculated CFG bytes for UUID generation.
    fn register_new_node_id(
        &mut self,
        item_name: &str,
        item_kind: ItemKind,
        cfg_bytes: Option<&[u8]>, // NEW: Accept CFG bytes
    ) -> Option<(NodeId, ModuleNodeId)> {
        // Return base ID and parent ModuleNodeId
        // 1. Generate the Synthetic NodeId using the state helper, passing CFG bytes
        let node_id = self
            .state
            .generate_synthetic_node_id(item_name, item_kind, cfg_bytes); // Pass cfg_bytes

        // 2. Find the parent module based on the *current path* and add the item ID.
        let parent_module_id = self
            .state
            .code_graph
            .modules
            .iter_mut() // Need mutable access to add to items list
            .find(|m| m.path == self.state.current_module_path) // Find module matching current path
            .map(|parent_mod| {
                // Add item ID to the module's definition
                match &mut parent_mod.module_def {
                    ModuleKind::Inline { items, .. } => items.push(node_id),
                    ModuleKind::FileBased { items, .. } => items.push(node_id),
                    ModuleKind::Declaration { .. } => {
                        // Cannot add items to a declaration, log warning
                        log::warn!(
                            target: LOG_TARGET_TRACE,
                            "Attempted to add item '{}' ({:?}) to a module declaration node '{}' ({}). Item not added to list.",
                            item_name, item_kind, parent_mod.name, parent_mod.id
                        );
                    }
                }
                self.debug_mod_stack_push(item_name.to_owned(), node_id);
                parent_mod.module_id() // Return the typed ModuleNodeId
            });

        if parent_module_id.is_none() {
            // This case should ideally not happen after the root module is created.
            log::warn!(
                target: LOG_TARGET_TRACE,
                "Could not find parent module for item '{}' ({:?}) using current_module_path {:?}. Item not added to module list.",
                item_name, item_kind, self.state.current_module_path
            );
            None
        } else {
            Some((node_id, parent_module_id.unwrap())) // Return base ID and parent typed ID
        }
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
}

#[allow(clippy::needless_lifetimes)]
impl<'a, 'ast> Visit<'ast> for CodeVisitor<'a> {
    // Visit function definitions
    fn visit_item_fn(&mut self, func: &'ast ItemFn) {
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
            let scope_cfgs = self.state.current_scope_cfgs.clone();
            let item_cfgs = extract_cfg_strings(&func.attrs);
            let provisional_effective_cfgs: Vec<String> = scope_cfgs
                .iter()
                .cloned()
                .chain(item_cfgs.iter().cloned())
                .collect();
            let cfg_bytes = calculate_cfg_hash_bytes(&provisional_effective_cfgs);
            let registration_result =
            self.register_new_node_id(&macro_name, ItemKind::Macro,
                cfg_bytes.as_deref()).unwrap_or_else(|| todo!("Implement proper error handling. We can't return a result inside the visitor implementation but we can log the error."));
            let (macro_base_id, parent_mod_id) = registration_result;
            self.debug_new_id(&macro_name, macro_base_id);
            let span = func.extract_span_bytes();
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
            let docstring = extract_docstring(&func.attrs);
            let attributes = extract_attributes(&func.attrs);
            let body = Some(func.block.to_token_stream().to_string());

            let macro_info = MacroNodeInfo {
                id: macro_base_id,
                name: macro_name.clone(),
                visibility: self.state.convert_visibility(&func.vis),
                kind: MacroKind::ProcedureMacro {
                    kind: proc_macro_kind,
                },
                attributes,
                docstring,
                body,
                span,
                tracking_hash: Some(self.state.generate_tracking_hash(&func.to_token_stream())),
                cfgs: item_cfgs,
            };
            let macro_node = MacroNode::new(macro_info);
            let typed_macro_id = macro_node.macro_id();
            self.state.code_graph.macros.push(macro_node);
            let relation = SyntacticRelation::Contains {
                source: parent_mod_id,
                target: PrimaryNodeId::from(typed_macro_id), // Use category enum
            };
            self.state.code_graph.relations.push(relation);
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

            // Register the new node ID and get parent module ID
            let registration_result =
                self.register_new_node_id(&fn_name, ItemKind::Function, cfg_bytes.as_deref());
            if registration_result.is_none() {
                return;
            } // Skip if parent module not found
            let (fn_base_id, parent_mod_id) = registration_result.unwrap();

            self.debug_new_id(&fn_name, fn_base_id); // Now uses trace!

            let byte_range = func.span().byte_range();
            let span = (byte_range.start, byte_range.end);

            // Push the function's base ID onto the scope stack BEFORE processing types/generics
            // Use helper function for logging
            self.push_scope(&fn_name, fn_base_id, provisional_effective_cfgs);

            // Process function parameters
            let mut parameters = Vec::new();
            for arg in &func.sig.inputs {
                if let Some(param) = self.state.process_fn_arg(arg) {
                    // RelationKind::FunctionParameter removed. TypeId is stored in ParamData.
                    parameters.push(param);
                }
            }

            // Extract return type if it exists
            let return_type = match &func.sig.output {
                ReturnType::Default => None,
                ReturnType::Type(_, ty) => {
                    let type_id = get_or_create_type(self.state, ty);
                    // RelationKind::FunctionReturn removed. TypeId is stored in FunctionNode.return_type.
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

            // Create info struct and then the node
            let function_info = FunctionNodeInfo {
                id: fn_base_id,
                name: fn_name.clone(),
                span,
                visibility: self.state.convert_visibility(&func.vis),
                parameters,
                return_type,
                generic_params,
                attributes,
                docstring,
                body,
                tracking_hash: Some(self.state.generate_tracking_hash(&func.to_token_stream())),
                cfgs: item_cfgs,
            };
            let function_node = FunctionNode::new(function_info);
            let typed_fn_id = function_node.function_id();
            self.state.code_graph.functions.push(function_node);

            // Create and add the Contains relation
            let relation = SyntacticRelation::Contains {
                source: parent_mod_id,
                target: PrimaryNodeId::from(typed_fn_id), // Use category enum
            };
            self.state.code_graph.relations.push(relation);

            // Continue visiting the function body (fn_base_id is already off the stack)
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

        // Register the new node ID and get parent module ID
        let registration_result =
            self.register_new_node_id(&struct_name, ItemKind::Struct, cfg_bytes.as_deref());
        if registration_result.is_none() {
            return;
        } // Skip if parent module not found
        let (struct_base_id, parent_mod_id) = registration_result.unwrap();

        self.debug_new_id(&struct_name, struct_base_id); // Now uses trace!

        let byte_range = item_struct.span().byte_range();
        let span = (byte_range.start, byte_range.end);

        // Push the struct's base ID onto the scope stack BEFORE processing fields/generics
        // Use helper function for logging
        self.push_scope(
            &struct_name,
            struct_base_id,
            provisional_effective_cfgs.clone(),
        ); // Clone cfgs for push

        // Process fields
        let mut fields = Vec::new();
        for (i, field) in item_struct.fields.iter().enumerate() {
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

            // Generate base ID for the field
            // Note: Fields are contained within the struct, not directly in the module,
            // so we don't use register_new_node_id here.
            let field_base_id = self.state.generate_synthetic_node_id(
                &field_name
                    .clone()
                    .unwrap_or_else(|| format!("unnamed_field{}_in_{}", i, struct_name)),
                ItemKind::Field,
                field_cfg_bytes.as_deref(), // Pass field's CFG bytes
            );

            // Removed #[cfg] block
            self.debug_new_id(
                &field_name
                    .clone()
                    .unwrap_or("unnamed_struct_field".to_string()),
                field_base_id,
            );
            let type_id = get_or_create_type(self.state, &field.ty);

            let field_info = FieldNodeInfo {
                id: field_base_id,
                name: field_name,
                type_id,
                visibility: self.state.convert_visibility(&field.vis),
                attributes: extract_attributes(&field.attrs),
                cfgs: field_item_cfgs,
            };
            let field_node = FieldNode::new(field_info);
            fields.push(field_node);

            // Add relation between struct and field (defer until struct node is created)
            // We need the typed struct ID first.
        }

        // Process generic parameters (still within struct's scope)
        let generic_params = self.state.process_generics(&item_struct.generics);

        // Extract doc comments and other attributes
        let docstring = extract_docstring(&item_struct.attrs);
        let attributes = extract_attributes(&item_struct.attrs);

        // Create the struct node
        let struct_node_info = StructNodeInfo {
            id: struct_base_id, // Use base ID
            name: struct_name.clone(),
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
        };
        let struct_node = StructNode::new(struct_node_info);
        let typed_struct_id = struct_node.struct_id();

        // Now add the StructField relations using the fields from the created struct_node
        for field_node in &struct_node.fields {
            let relation = SyntacticRelation::StructField {
                source: typed_struct_id,
                target: field_node.field_id(),
            };
            self.state.code_graph.relations.push(relation);
        }

        // Add the struct node to the graph
        self.state
            .code_graph
            .defined_types
            .push(TypeDefNode::Struct(struct_node));

        // Add the Contains relation for the struct itself
        let contains_relation = SyntacticRelation::Contains {
            source: parent_mod_id,
            target: PrimaryNodeId::from(typed_struct_id), // Use category enum
        };
        self.state.code_graph.relations.push(contains_relation);

        // Continue visiting children (fields are handled above, visit generics/where clauses if needed)
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

        // Register the new node ID and get parent module ID
        let registration_result =
            self.register_new_node_id(&type_alias_name, ItemKind::TypeAlias, cfg_bytes.as_deref());
        if registration_result.is_none() {
            return;
        } // Skip if parent module not found
        let (type_alias_base_id, parent_mod_id) = registration_result.unwrap();

        self.debug_new_id(&type_alias_name, type_alias_base_id); // Now uses trace!

        let span = item_type.extract_span_bytes();

        // Push the type alias's base ID onto the scope stack BEFORE processing type/generics
        // Type aliases don't introduce a new CFG scope, so pass current scope cfgs
        self.push_scope(
            &type_alias_name,
            type_alias_base_id,
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

        // Create info struct and then the node
        let type_alias_info = TypeAliasNodeInfo {
            id: type_alias_base_id,
            name: type_alias_name.clone(),
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
            cfgs: item_cfgs,
        };
        let type_alias_node = TypeAliasNode::new(type_alias_info);
        let typed_alias_id = type_alias_node.type_alias_id();

        // Add the node to the graph
        self.state
            .code_graph
            .defined_types
            .push(TypeDefNode::TypeAlias(type_alias_node));

        // Add the Contains relation
        let contains_relation = SyntacticRelation::Contains {
            source: parent_mod_id,
            target: PrimaryNodeId::from(typed_alias_id), // Use category enum
        };
        self.state.code_graph.relations.push(contains_relation);

        // Type aliases don't define a new CFG scope for children.
        // Continue visiting (type_alias_base_id is already off the definition stack)
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

        // Register the new node ID and get parent module ID
        let registration_result =
            self.register_new_node_id(&union_name, ItemKind::Union, cfg_bytes.as_deref());
        if registration_result.is_none() {
            return;
        } // Skip if parent module not found
        let (union_base_id, parent_mod_id) = registration_result.unwrap();

        self.debug_new_id(&union_name, union_base_id); // Now uses trace!

        let span = item_union.extract_span_bytes();

        // Push the union's base ID onto the scope stack BEFORE processing fields/generics
        // Use helper function for logging
        self.push_scope(
            &union_name,
            union_base_id,
            provisional_effective_cfgs.clone(),
        ); // Clone cfgs for push

        // Process fields
        let mut fields = Vec::new();
        for (i, field) in item_union.fields.named.iter().enumerate() {
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

            // Generate base ID for the field
            let field_base_id = self.state.generate_synthetic_node_id(
                &field_name
                    .clone()
                    .unwrap_or_else(|| format!("unnamed_field{}_in_{}", i, union_name)),
                ItemKind::Field,
                field_cfg_bytes.as_deref(), // Pass field's CFG bytes
            );
            self.debug_new_id(
                &field_name
                    .clone()
                    .unwrap_or_else(|| format!("unnamed_field{}_in_{}", i, union_name)),
                field_base_id,
            );
            let type_id = get_or_create_type(self.state, &field.ty);

            let field_info = FieldNodeInfo {
                id: field_base_id,
                name: field_name,
                type_id,
                visibility: self.state.convert_visibility(&field.vis),
                attributes: extract_attributes(&field.attrs),
                cfgs: field_item_cfgs,
            };
            let field_node = FieldNode::new(field_info);
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

        // Create info struct and then the node
        let union_info = UnionNodeInfo {
            id: union_base_id,
            name: union_name.clone(),
            span, // Add span here
            visibility: self.state.convert_visibility(&item_union.vis),
            fields, // Pass the collected FieldNode Vec
            generic_params,
            attributes,
            docstring,
            tracking_hash: Some(
                self.state
                    .generate_tracking_hash(&item_union.to_token_stream()),
            ),
            cfgs: item_cfgs,
        };
        let union_node = UnionNode::new(union_info);
        let typed_union_id = union_node.union_id();

        // Now add the UnionField relations using fields from the created union_node
        for field_node in &union_node.fields {
            let relation = SyntacticRelation::UnionField {
                source: typed_union_id,
                target: field_node.field_id(),
            };
            self.state.code_graph.relations.push(relation);
        }

        // Add the union node to the graph
        self.state
            .code_graph
            .defined_types
            .push(TypeDefNode::Union(union_node));

        // Add the Contains relation for the union itself
        let contains_relation = SyntacticRelation::Contains {
            source: parent_mod_id,
            target: PrimaryNodeId::from(typed_union_id), // Use category enum
        };
        self.state.code_graph.relations.push(contains_relation);

        // Continue visiting children (fields handled above, visit generics/where clauses if needed)
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

        // Register the new node ID and get parent module ID
        let registration_result =
            self.register_new_node_id(&enum_name, ItemKind::Enum, cfg_bytes.as_deref());
        if registration_result.is_none() {
            return;
        } // Skip if parent module not found
        let (enum_base_id, parent_mod_id) = registration_result.unwrap();

        self.debug_new_id(&enum_name, enum_base_id); // Now uses trace!

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

            // Generate base ID for the variant
            let variant_base_id = self.state.generate_synthetic_node_id(
                &variant_name,
                ItemKind::Variant,
                variant_cfg_bytes.as_deref(), // Pass variant's CFG bytes
            );

            // Push the variant's base ID onto the scope stack BEFORE processing its fields
            // Variants don't introduce a new CFG scope, pass current (enum's) scope cfgs
            self.push_scope(
                &variant_name,
                variant_base_id,
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

                        // Generate base ID for the field
                        let field_base_id = self.state.generate_synthetic_node_id(
                            &field_name
                                .clone()
                                .unwrap_or_else(|| format!("unnamed_field_in_{}", variant_name)),
                            ItemKind::Field,
                            field_cfg_bytes.as_deref(), // Pass field's CFG bytes
                        );
                        self.debug_new_id(
                            &field_name
                                .clone()
                                .unwrap_or("unnamed_enum_field".to_string()),
                            field_base_id,
                        );
                        let type_id = get_or_create_type(self.state, &field.ty);

                        let field_info = FieldNodeInfo {
                            id: field_base_id,
                            name: field_name,
                            type_id,
                            visibility: self.state.convert_visibility(&field.vis),
                            attributes: extract_attributes(&field.attrs),
                            cfgs: field_item_cfgs,
                        };
                        let field_node = FieldNode::new(field_info);
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

                        // Generate base ID for the field
                        let field_base_id = self.state.generate_synthetic_node_id(
                            &field_name_placeholder, // Use placeholder for ID generation
                            ItemKind::Field,
                            field_cfg_bytes.as_deref(),
                        );
                        let type_id = get_or_create_type(self.state, &field.ty);
                        self.debug_new_id("unnamed_enum_field", field_base_id);

                        let field_info = FieldNodeInfo {
                            id: field_base_id,
                            name: None,
                            type_id,
                            visibility: self.state.convert_visibility(&field.vis),
                            attributes: extract_attributes(&field.attrs),
                            cfgs: field_item_cfgs,
                        };
                        let field_node = FieldNode::new(field_info);
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

            // Create info struct and then the node
            let variant_info = VariantNodeInfo {
                id: variant_base_id,
                name: variant_name,
                fields: fields.clone(), // Clone the collected FieldNode Vec
                discriminant,
                attributes: extract_attributes(&variant.attrs),
                cfgs: variant_item_cfgs,
            };
            let variant_node = VariantNode::new(variant_info);
            let typed_variant_id = variant_node.variant_id();
            variants.push(variant_node);

            // Add EnumVariant relation (defer until enum node is created)

            // Add VariantField relations now that we have the typed variant ID
            for field_node in fields {
                // Iterate over original fields Vec
                let relation = SyntacticRelation::VariantField {
                    source: typed_variant_id,
                    target: field_node.field_id(), // Use typed field ID
                };
                self.state.code_graph.relations.push(relation);
            }
        }

        // Push the enum's base ID onto the scope stack BEFORE processing its generics
        // Use helper function for logging
        self.push_scope(&enum_name, enum_base_id, provisional_effective_cfgs.clone()); // Clone cfgs for push

        // Process generic parameters
        let generic_params = self.state.process_generics(&item_enum.generics);

        // Pop the enum's ID from the scope stack AFTER processing its generics
        // Note: This pop happens *before* visiting children, which might be incorrect
        // if generics/where clauses need the enum scope. Let's move the pop after visit.
        // self.state.current_definition_scope.pop(); // Moved below

        // Extract doc comments and other attributes
        let docstring = extract_docstring(&item_enum.attrs);
        let attributes = extract_attributes(&item_enum.attrs);

        // Create info struct and then the node
        let enum_info = EnumNodeInfo {
            id: enum_base_id,
            name: enum_name.clone(),
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
            cfgs: item_cfgs.clone(),
        };
        let enum_node = EnumNode::new(enum_info);
        let typed_enum_id = enum_node.enum_id();

        // Now add the EnumVariant relations using variants from the created enum_node
        for variant_node in &enum_node.variants {
            let relation = SyntacticRelation::EnumVariant {
                source: typed_enum_id,
                target: variant_node.variant_id(),
            };
            self.state.code_graph.relations.push(relation);
        }

        // Add the enum node to the graph
        self.state
            .code_graph
            .defined_types
            .push(TypeDefNode::Enum(enum_node));

        // Add the Contains relation for the enum itself
        let contains_relation = SyntacticRelation::Contains {
            source: parent_mod_id,
            target: PrimaryNodeId::from(typed_enum_id), // Use category enum
        };
        self.state.code_graph.relations.push(contains_relation);

        // Continue visiting children (variants/fields handled above, visit generics/where)
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

        // Register the new node ID and get parent module ID
        let registration_result =
            self.register_new_node_id(&impl_name, ItemKind::Impl, cfg_bytes.as_deref());
        if registration_result.is_none() {
            return;
        } // Skip if parent module not found
        let (impl_base_id, parent_mod_id) = registration_result.unwrap();

        self.debug_new_id(&impl_name, impl_base_id); // Log with the generated name, now uses trace!

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

        // Pushing parent node base id to stack BEFORE generating self type.
        // Use helper function for logging
        self.push_scope(&impl_name, impl_base_id, provisional_effective_cfgs.clone()); // Clone cfgs for push
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

                // Generate base ID for the method
                // Methods are contained within the impl, not directly in the module.
                let method_base_id = self.state.generate_synthetic_node_id(
                    &method_name,
                    ItemKind::Method, // Use Method kind
                    method_cfg_bytes.as_deref(),
                );

                self.debug_new_id(&method_name, method_base_id); // Now uses trace!

                // Push the method's base ID onto the scope stack BEFORE processing its types/generics
                // Methods don't introduce a new CFG scope, pass current (impl's) scope cfgs
                self.push_scope(
                    &method_name,
                    method_base_id,
                    self.state.current_scope_cfgs.clone(),
                );

                // Process method parameters
                let mut parameters = Vec::new();
                for arg in &method.sig.inputs {
                    if let Some(param) = self.state.process_fn_arg(arg) {
                        // RelationKind::FunctionParameter removed. TypeId stored in ParamData.
                        parameters.push(param);
                    }
                }

                // Extract return type if it exists
                let return_type = match &method.sig.output {
                    ReturnType::Default => None,
                    ReturnType::Type(_, ty) => {
                        let type_id = get_or_create_type(self.state, ty);
                        // RelationKind::FunctionReturn removed. TypeId stored in FunctionNode.return_type.
                        Some(type_id)
                    }
                };
                // RelationKind::Method removed. Replaced by AssociatedItem below.

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

                // Create info struct and then the node
                let method_info = MethodNodeInfo {
                    id: method_base_id,
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
                        self.state.generate_tracking_hash(&method.to_token_stream()),
                    ),
                    cfgs: method_item_cfgs,
                };
                let method_node = MethodNode::new(method_info);
                methods.push(method_node);
            }
            // TODO: Handle syn::ImplItem::Const and syn::ImplItem::Type here
            // 1. Generate base ID for ConstNode/TypeAliasNode
            // 2. Create the ConstNode/TypeAliasNode
            // 3. Store the node (e.g., in separate Vecs or a shared collection)
            // 4. Add the node's typed ID to a list of associated items for this impl
        }

        // Placeholder for other associated items (consts, types)
        let associated_consts: Vec<ConstNode> = Vec::new(); // TODO: Populate this
        let associated_types: Vec<TypeAliasNode> = Vec::new(); // TODO: Populate this

        // Process generic parameters for impl block
        let generic_params = self.state.process_generics(&item_impl.generics);

        // Create info struct and then the node
        let impl_info = ImplNodeInfo {
            id: impl_base_id,
            span: item_impl.extract_span_bytes(),
            self_type: self_type_id,
            trait_type: trait_type_id,
            methods, // Pass the collected MethodNode Vec
            generic_params,
            cfgs: item_cfgs,
        };
        let impl_node = ImplNode::new(impl_info);
        let typed_impl_id = impl_node.impl_id();

        // Now add the ImplAssociatedItem relations using methods from the created impl_node
        for method_node in &impl_node.methods {
            let relation = SyntacticRelation::ImplAssociatedItem {
                source: typed_impl_id,
                target: AssociatedItemId::from(method_node.method_id()),
            };
            self.state.code_graph.relations.push(relation);
        }
        for const_node in &associated_consts {
            // TODO: Populate associated_consts
            let relation = SyntacticRelation::ImplAssociatedItem {
                source: typed_impl_id,
                target: AssociatedItemId::from(const_node.const_id()),
            };
            self.state.code_graph.relations.push(relation);
        }
        for type_node in &associated_types {
            // TODO: Populate associated_types
            let relation = SyntacticRelation::ImplAssociatedItem {
                source: typed_impl_id,
                target: AssociatedItemId::from(type_node.type_alias_id()),
            };
            self.state.code_graph.relations.push(relation);
        }

        // Add the impl node to the graph
        self.state.code_graph.impls.push(impl_node);

        // Add the Contains relation for the impl itself
        let contains_relation = SyntacticRelation::Contains {
            source: parent_mod_id,
            target: PrimaryNodeId::from(typed_impl_id), // Use category enum
        };
        self.state.code_graph.relations.push(contains_relation);

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

        // Register the new node ID and get parent module ID
        let registration_result =
            self.register_new_node_id(&trait_name, ItemKind::Trait, cfg_bytes.as_deref());
        if registration_result.is_none() {
            return;
        } // Skip if parent module not found
        let (trait_base_id, parent_mod_id) = registration_result.unwrap();

        self.debug_new_id(&trait_name, trait_base_id); // Now uses trace!

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

                // Generate base ID for the method definition within the trait
                let method_base_id = self.state.generate_synthetic_node_id(
                    &method_name,
                    ItemKind::Method, // Use Method kind
                    method_cfg_bytes.as_deref(),
                );

                self.debug_new_id(&method_name, method_base_id); // Now uses trace!

                // Push the method's base ID onto the scope stack BEFORE processing its types/generics
                // Methods don't introduce a new CFG scope, pass current (trait's) scope cfgs
                self.push_scope(
                    &method_name,
                    method_base_id,
                    self.state.current_scope_cfgs.clone(),
                );

                // Process method parameters
                let mut parameters = Vec::new();
                for arg in &method.sig.inputs {
                    if let Some(param) = self.state.process_fn_arg(arg) {
                        // RelationKind::FunctionParameter removed. TypeId stored in ParamData.
                        parameters.push(param);
                    }
                }

                // Extract return type if it exists
                let return_type = match &method.sig.output {
                    ReturnType::Default => None,
                    ReturnType::Type(_, ty) => {
                        let type_id = get_or_create_type(self.state, ty);
                        // RelationKind::FunctionReturn removed. TypeId stored in FunctionNode.return_type.
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

                // Create info struct and then the node
                let method_info = MethodNodeInfo {
                    id: method_base_id,
                    name: method_name,
                    span: method.extract_span_bytes(),
                    visibility: self.state.convert_visibility(&item_trait.vis), // Trait items inherit trait visibility
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
                    cfgs: method_item_cfgs,
                };
                let method_node = MethodNode::new(method_info);
                methods.push(method_node);
            }
            // TODO: Handle syn::TraitItem::Const and syn::TraitItem::Type here
            // 1. Generate base ID for ConstNode/TypeAliasNode
            // 2. Create the ConstNode/TypeAliasNode
            // 3. Store the node
            // 4. Add the node's typed ID to a list of associated items for this trait
        }

        // Placeholder for other associated items (consts, types)
        let associated_consts: Vec<ConstNode> = Vec::new(); // TODO: Populate this
        let associated_types: Vec<TypeAliasNode> = Vec::new(); // TODO: Populate this

        // Push the trait's base ID onto the scope stack BEFORE processing its generics/supertraits
        // Use helper function for logging
        self.push_scope(
            &trait_name,
            trait_base_id,
            provisional_effective_cfgs.clone(),
        ); // Clone cfgs for push

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

        // Create info struct and then the node
        let trait_info = TraitNodeInfo {
            id: trait_base_id,
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
            cfgs: item_cfgs.clone(),
        };
        let trait_node = TraitNode::new(trait_info);
        let typed_trait_id = trait_node.trait_id();

        // Now add the TraitAssociatedItem relations using methods from the created trait_node
        for method_node in &trait_node.methods {
            let relation = SyntacticRelation::TraitAssociatedItem {
                source: typed_trait_id,
                target: AssociatedItemId::from(method_node.method_id()),
            };
            self.state.code_graph.relations.push(relation);
        }
        for const_node in &associated_consts {
            // TODO: Populate associated_consts
            let relation = SyntacticRelation::TraitAssociatedItem {
                source: typed_trait_id,
                target: AssociatedItemId::from(const_node.const_id()),
            };
            self.state.code_graph.relations.push(relation);
        }
        for type_node in &associated_types {
            // TODO: Populate associated_types
            let relation = SyntacticRelation::TraitAssociatedItem {
                source: typed_trait_id,
                target: AssociatedItemId::from(type_node.type_alias_id()),
            };
            self.state.code_graph.relations.push(relation);
        }

        // Add the trait node to the graph
        self.state.code_graph.traits.push(trait_node);

        // Add the Contains relation for the trait itself
        let contains_relation = SyntacticRelation::Contains {
            source: parent_mod_id,
            target: PrimaryNodeId::from(typed_trait_id), // Use category enum
        };
        self.state.code_graph.relations.push(contains_relation);

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

        // Register the new node ID and get parent module ID
        // Note: This assumes the parent module was already registered.
        let registration_result =
            self.register_new_node_id(&module_name, ItemKind::Module, cfg_bytes.as_deref());
        if registration_result.is_none() {
            return;
        } // Skip if parent module not found
        let (module_base_id, parent_mod_id) = registration_result.unwrap();

        self.debug_mod_stack(); // Now uses trace!

        let span = module.extract_span_bytes();

        self.debug_new_id(&module_name, module_base_id); // Now uses trace!

        // Save current path before entering module
        let parent_path = self.state.current_module_path.clone();

        // Update path for nested module visitation
        self.state.current_module_path.push(module_name.clone());

        // Process module contents

        // Create module node with proper path tracking
        // Create module node with proper hierarchy tracking
        let module_def = match &module.content {
            Some(_) => ModuleKind::Inline {
                items: Vec::new(),
                span,
                // Inline modules don't store their own CFG here; it's part of the scope.
            },
            None => ModuleKind::Declaration {
                declaration_span: span,
                resolved_definition: None, // Resolved during phase 3 resolution
                                           // cfgs removed from here, belongs on ModuleNode
            },
        };

        // Create info struct and then the node
        let module_info = ModuleNodeInfo {
            id: module_base_id,
            name: module_name.clone(),
            path: self.state.current_module_path.clone(),
            visibility: self.state.convert_visibility(&module.vis),
            attributes: extract_attributes(&module.attrs),
            docstring: extract_docstring(&module.attrs),
            imports: Vec::new(),
            exports: Vec::new(),
            span,
            tracking_hash: Some(self.state.generate_tracking_hash(&module.to_token_stream())),
            module_def,
            cfgs: item_cfgs,
        };
        let module_node = ModuleNode::new(module_info);

        // Restore parent path after processing module
        self.state.current_module_path = parent_path;

        let typed_module_id = module_node.module_id();
        self.state.current_module.push(module_node.name.clone());
        self.log_push("current module", &self.state.current_module);

        self.state
            .current_module_path
            .push(module_node.name.clone());
        self.log_push("current_module_path", &self.state.current_module_path); // Now uses trace!

        // Add the node to the graph
        self.state.code_graph.modules.push(module_node);

        // Add the Contains relation for the module itself
        let contains_relation = SyntacticRelation::Contains {
            source: parent_mod_id,
            target: PrimaryNodeId::from(typed_module_id), // Use category enum
        };
        self.state.code_graph.relations.push(contains_relation);

        // Push the module's scope using the helper *before* visiting children
        self.push_scope(&module_name, module_base_id, provisional_effective_cfgs);

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

        // Call the modified function and handle the Result
        let imports_result =
            self.process_use_tree(&use_item.tree, base_path, cfg_bytes.as_deref(), &vis_kind);

        // Get a mutable reference to the graph only once
        let graph = &mut self.state.code_graph;
        let current_module_path = &self.state.current_module_path;

        // Add all imports to the current module
        if let Some(module) = graph
            .modules
            .iter_mut()
            .find(|m| &m.path == current_module_path)
        {
            let parent_mod_id = module.module_id();

            // Process imports only if process_use_tree succeeded
            match imports_result {
                Ok(imports) => {
                    for import_node in imports {
                        let typed_import_id = import_node.import_id();

                        // Add Contains relation (Import is a PrimaryNode)
                        let contains_relation = SyntacticRelation::Contains {
                            source: parent_mod_id,
                            target: PrimaryNodeId::from(typed_import_id),
                        };
                        graph.relations.push(contains_relation);

                        // Add ModuleImports relation
                        let module_import_relation = SyntacticRelation::ModuleImports {
                            source: parent_mod_id,
                            target: typed_import_id,
                        };
                        graph.relations.push(module_import_relation);

                        // Add the node itself
                        graph.use_statements.push(import_node.clone());
                        // Add to module's imports list
                        module.imports.push(import_node);
                    }
                }
                Err(err) => {
                    // Log the error from process_use_tree, but don't stop parsing
                    error!(target: LOG_TARGET_TRACE, "Error processing use tree: {}", err);
                }
            }
        } else {
            log::warn!(target: LOG_TARGET_TRACE, "Could not find parent module for use statement at path {:?}. Imports not added.", current_module_path);
        }
        // Continue visiting
        visit::visit_item_use(self, use_item);
    }

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

        // Register the new node ID and get parent module ID
        let registration_result = self.register_new_node_id(
            &visible_name,
            ItemKind::ExternCrate, // Use correct kind
            cfg_bytes.as_deref(),  // Pass CFG bytes
        );
        // If registration fails (e.g., no parent module found), skip this item
        if registration_result.is_none() {
            return;
        }
        let (import_base_id, parent_mod_id) = registration_result.unwrap();

        self.debug_new_id(&visible_name, import_base_id); // Log with visible name, now uses trace!

        let crate_name = extern_crate.ident.to_string();

        let span = extern_crate.extract_span_bytes();

        let import_info = ImportNodeInfo {
            id: import_base_id,
            span,
            source_path: vec![crate_name.clone()],
            kind: ImportKind::ExternCrate,
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
            cfgs: item_cfgs,
        };
        let import_node = ImportNode::new(import_info);
        let typed_import_id = import_node.import_id();

        // Add the node to the graph and module list
        if let Some(module) = self
            .state
            .code_graph
            .modules
            .iter_mut()
            .find(|m| m.id == parent_mod_id)
        // Find by parent ID
        {
            module.imports.push(import_node.clone());
        }
        self.state.code_graph.use_statements.push(import_node);

        // Add Contains relation (Import is a PrimaryNode)
        let contains_relation = SyntacticRelation::Contains {
            source: parent_mod_id,
            target: PrimaryNodeId::from(typed_import_id),
        };
        self.state.code_graph.relations.push(contains_relation);

        // Add ModuleImports relation
        let module_import_relation = SyntacticRelation::ModuleImports {
            source: parent_mod_id,
            target: typed_import_id,
        };
        self.state.code_graph.relations.push(module_import_relation);
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

        // RelationKind::Uses removed. The TypeId generated here was not meaningful.

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

        // Register the new node ID and get parent module ID
        let registration_result =
            self.register_new_node_id(&const_name, ItemKind::Const, cfg_bytes.as_deref());
        if registration_result.is_none() {
            return;
        } // Skip if parent module not found
        let (const_base_id, parent_mod_id) = registration_result.unwrap();

        self.debug_new_id(&const_name, const_base_id); // Now uses trace!

        let span = item_const.extract_span_bytes();

        // Process the type
        // Process the type (no need to push/pop scope for this)
        let type_id = get_or_create_type(self.state, &item_const.ty);

        // Extract the value expression as a string
        let value = Some(item_const.expr.to_token_stream().to_string());

        // Extract doc comments and other attributes
        let docstring = extract_docstring(&item_const.attrs);
        let attributes = extract_attributes(&item_const.attrs);

        // Create info struct and then the node
        let const_info = ConstNodeInfo {
            id: const_base_id,
            name: const_name,
            span, // Add span here
            visibility: self.state.convert_visibility(&item_const.vis),
            type_id,
            value,
            attributes,
            docstring,
            tracking_hash: Some(
                self.state
                    .generate_tracking_hash(&item_const.to_token_stream()),
            ),
            cfgs: item_cfgs,
        };
        let const_node = ConstNode::new(const_info);
        let typed_const_id = const_node.const_id();

        // Add the constant node to the graph
        self.state.code_graph.consts.push(const_node);

        // Add the Contains relation
        let contains_relation = SyntacticRelation::Contains {
            source: parent_mod_id,
            target: PrimaryNodeId::from(typed_const_id), // Use category enum
        };
        self.state.code_graph.relations.push(contains_relation);

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

        // Register the new node ID and get parent module ID
        let registration_result =
            self.register_new_node_id(&static_name, ItemKind::Static, cfg_bytes.as_deref());
        if registration_result.is_none() {
            return;
        } // Skip if parent module not found
        let (static_base_id, parent_mod_id) = registration_result.unwrap();

        self.debug_new_id(&static_name, static_base_id); // Now uses trace!

        let span = item_static.extract_span_bytes();

        // Process the type (no need to push/pop scope)
        let type_id = get_or_create_type(self.state, &item_static.ty);

        // Extract the value expression as a string
        let value = Some(item_static.expr.to_token_stream().to_string());

        // Extract doc comments and other attributes
        let docstring = extract_docstring(&item_static.attrs);
        let attributes = extract_attributes(&item_static.attrs);

        // Create info struct and then the node
        let static_info = StaticNodeInfo {
            id: static_base_id,
            name: static_name,
            span, // Add span here
            visibility: self.state.convert_visibility(&item_static.vis),
            type_id,
            is_mutable: matches!(item_static.mutability, syn::StaticMutability::Mut(_)),
            value,
            attributes,
            docstring,
            tracking_hash: Some(
                self.state
                    .generate_tracking_hash(&item_static.to_token_stream()),
            ),
            cfgs: item_cfgs,
        };
        let static_node = StaticNode::new(static_info);
        let typed_static_id = static_node.static_id();

        // Add the static node to the graph
        self.state.code_graph.statics.push(static_node);

        // Add the Contains relation
        let contains_relation = SyntacticRelation::Contains {
            source: parent_mod_id,
            target: PrimaryNodeId::from(typed_static_id), // Use category enum
        };
        self.state.code_graph.relations.push(contains_relation);

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

        // Register the new node ID and get parent module ID
        let registration_result =
            self.register_new_node_id(&macro_name, ItemKind::Macro, cfg_bytes.as_deref());
        if registration_result.is_none() {
            return;
        } // Skip if parent module not found
        let (macro_base_id, parent_mod_id) = registration_result.unwrap();

        self.debug_new_id(&macro_name, macro_base_id); // Now uses trace!

        let span = item_macro.extract_span_bytes();

        let body = Some(item_macro.mac.tokens.to_string());
        let docstring = extract_docstring(&item_macro.attrs);
        let attributes = extract_attributes(&item_macro.attrs);

        // Create info struct and then the node
        let macro_info = MacroNodeInfo {
            id: macro_base_id,
            name: macro_name,
            span, // Add span here
            visibility,
            kind: MacroKind::DeclarativeMacro,
            // rules field removed
            attributes,
            docstring,
            body,
            tracking_hash: Some(
                self.state
                    .generate_tracking_hash(&item_macro.to_token_stream()),
            ),
            cfgs: item_cfgs,
        };
        let macro_node = MacroNode::new(macro_info);
        let typed_macro_id = macro_node.macro_id();

        // Add the macro node to the graph
        self.state.code_graph.macros.push(macro_node);

        // Add the Contains relation
        let contains_relation = SyntacticRelation::Contains {
            source: parent_mod_id,
            target: PrimaryNodeId::from(typed_macro_id), // Use category enum
        };
        self.state.code_graph.relations.push(contains_relation);

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
