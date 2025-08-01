// TODO: Consider using a builder pattern for these nodes. The way we repeat code for the
// ImportNodeIds is just so painful.
// TODO: Start moving the logic for processing secondary/associated nodes into their own
// visit_item_* methods,
// e.g.
// fn visit_trait_item_fn(&mut self, i: &'ast syn::TraitItemFn)
// fn visit_trait_item_const(&mut self, i: &'ast syn::TraitItemConst)
// fn visit_impl_item_fn(&mut self, i: &'ast syn::ImplItemFn)
// fn visit_impl_item_const(&mut self, i: &'ast syn::ImplItemConst)

use super::attribute_processing::{extract_attributes, extract_cfg_strings, extract_docstring};
use super::state::VisitorState;
use super::type_processing::get_or_create_type;
use crate::parser::graph::GraphAccess;
use crate::parser::nodes::{FunctionNodeId, GeneratesAnyNodeId};
// NodeId wrapper types for individual node types
use crate::parser::nodes::{
    EnumNodeId, FieldNodeId, ImplNodeId, ImportNodeId, MethodNodeId, ModuleNodeId, StaticNodeId,
    StructNodeId, TraitNodeId, TypeAliasNodeId, UnionNodeId, VariantNodeId,
};
// Wrapper enums for catogories of individual node id wrapper types.
use crate::parser::nodes::{AnyNodeId, AssociatedItemNodeId, PrimaryNodeId, SecondaryNodeId};
// Nodes
use crate::parser::nodes::{
    ConstNode, EnumNode, FieldNode, FunctionNode, ImplNode, ImportNode, MacroNode, MethodNode,
    ModuleNode, StaticNode, StructNode, TraitNode, TypeAliasNode, TypeDefNode, UnionNode,
    VariantNode,
};
// Kinds of nodes
use crate::parser::nodes::{ImportKind, MacroKind, ModuleKind, ProcMacroKind};
// Imported Kinds from ploke-core
use ploke_core::ItemKind;

use crate::parser::relations::*;
use crate::parser::types::*;
use crate::parser::visitor::calculate_cfg_hash_bytes;
use crate::parser::ExtractSpan;

use crate::error::CodeVisitorError; // Import the new error type
use crate::utils::logging::LogErrorConversion as _;
use crate::utils::LogStyleDebug;
use itertools::Itertools;
use ploke_core::TypeId;

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
        // Fully recursive approach: each match arm returns the result directly.

        match tree {
            syn::UseTree::Path(path) => {
                // Push the current segment onto the path and recurse.
                base_path.push(path.ident.to_string());
                // The result of the recursive call is the result for this path segment.
                self.process_use_tree(&path.tree, base_path, cfg_bytes, vis_kind)
            }
            syn::UseTree::Name(name) => {
                // Base case: A specific item is being imported.
                let mut full_path = base_path; // Take ownership
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
                // Check if registration failed
                if registration_result.is_none() {
                    let err = CodeVisitorError::RegistrationFailed {
                        item_name: checked_name.clone(),
                        item_kind: ItemKind::Import,
                    };
                    // Log the error before returning
                    error!(target: LOG_TARGET_TRACE, "{}", err);
                    return Err(err);
                }
                // If registration succeeded, unwrap and proceed
                let (import_any_id, _) = registration_result.unwrap();
                let import_typed_id: ImportNodeId = import_any_id.try_into().map_err(|e| {
                    // Use the logging trait method
                    self.state
                        .log_import_id_conversion_error(&checked_name, &full_path, e);
                    // Return the specific CodeVisitorError variant
                    CodeVisitorError::IdConversionFailed {
                        item_name: checked_name.clone(),
                        item_kind: ItemKind::Import,
                        expected_type: "ImportNodeId",
                        source_error: e,
                    }
                })?; // Use ? to propagate the error

                let import_node = ImportNode {
                    id: import_typed_id,
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
                Ok(vec![import_node])
            }
            syn::UseTree::Rename(rename) => {
                // Base case: Renaming an imported item.
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
                let (import_any_id, _) = registration_result.unwrap();
                let import_node_id: ImportNodeId = import_any_id.try_into().map_err(|e| {
                    // Use the logging trait method
                    self.state
                        .log_import_id_conversion_error(&visible_name, &base_path, e); // Use base_path for context here
                                                                                       // Return the specific CodeVisitorError variant
                    CodeVisitorError::IdConversionFailed {
                        item_name: visible_name.clone(),
                        item_kind: ItemKind::Import,
                        expected_type: "ImportNodeId",
                        source_error: e,
                    }
                })?; // Use ? to propagate the error

                // The source path uses the original name.
                let mut source_path = base_path; // Take ownership
                source_path.push(original_name.clone());

                let import_node = ImportNode {
                    id: import_node_id,
                    source_path,
                    kind: ImportKind::UseStatement(vis_kind.to_owned()),
                    visible_name,
                    original_name: Some(original_name), // The original name before 'as'
                    is_glob: false,
                    span,
                    is_self_import: false,
                    cfgs: Vec::new(), // CFGs are handled at the ItemUse level
                };
                // Return a Vec containing just this single import node.
                Ok(vec![import_node])
            }
            syn::UseTree::Glob(glob) => {
                // Base case: A glob import.
                // NOTE: Previously the '*" glob was being replaced with the str, "<glob>", for ID
                // generation, but this doesn't really make any sense, especially if the original
                // name is not differentiated. Instead, we will just use the "*" as the name, and
                // then if we run into issues later, we can use a replacement and store the "*" as
                // the `original_name`.
                // WARN: Using a simple "*" for the name, as we did previously, leads to id
                // collisions among globs in the same module path. Instead we need to use the
                // entire path of the glob import.
                let mut full_path_string = base_path.join("::");
                full_path_string.push_str("::*");
                let registration_result = self.register_new_node_id(
                    &full_path_string, // Use the literal "*" glob symbol directly.
                    ItemKind::Import,
                    cfg_bytes, // Pass down received cfg_bytes
                );
                // Check if registration failed
                if registration_result.is_none() {
                    let mut glob_name_err_msg = base_path.clone();
                    glob_name_err_msg.push("*".to_string());
                    let err = CodeVisitorError::RegistrationFailed {
                        item_name: glob_name_err_msg.join("::"),
                        item_kind: ItemKind::Import,
                    };
                    error!(target: LOG_TARGET_TRACE, "{}", err);
                    return Err(err);
                }
                let (import_any_id, _) = registration_result.unwrap();
                // Convert AnyNodeId to the specific ImportNodeId
                let import_typed_id: ImportNodeId = import_any_id.try_into().map_err(|e| {
                    // Use the logging trait method
                    self.state
                        .log_import_id_conversion_error("<glob>", &base_path, e); // Use placeholder and base_path
                                                                                  // Return the specific CodeVisitorError variant
                    CodeVisitorError::IdConversionFailed {
                        item_name: "<glob>".to_string(),
                        item_kind: ItemKind::Import,
                        expected_type: "ImportNodeId",
                        source_error: e,
                    }
                })?; // Use ? to propagate the error

                // Use the original base_path
                let full_path = base_path; // Take ownership

                // Construct ImportNode directly, removing ImportNode
                let import_node = ImportNode {
                    id: import_typed_id, // Use the typed ID
                    source_path: full_path,
                    kind: ImportKind::UseStatement(vis_kind.to_owned()),
                    visible_name: full_path_string, // Glob imports use "*" as the visible name placeholder
                    original_name: None,
                    is_glob: true,
                    span: glob.extract_span_bytes(),
                    is_self_import: false,
                    cfgs: Vec::new(), // CFGs are handled at the ItemUse level
                };

                // Return a Vec containing just this single import node.
                Ok(vec![import_node]) // Return the directly constructed node
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

    fn debug_mod_stack(&mut self) {
        if let Some(current_mod) = self.state.code_graph.modules.last() {
            let modules: Vec<(String, String)> = self // Changed to String tuples for display
                .state
                .code_graph
                .modules
                .iter() // Add .iter() here
                .map(|m| (m.id.to_string(), m.name.clone())) // Convert ID to string
                .collect();

            // Adjusted to only show first/last 3 items. This field gets a bit big and might not be
            // very helful for debugging (long lists of Uuids clutter everything), but maybe the
            // first/last 3 will be helpful.
            // Might want to switch it back to old version depending on usefulness
            let items_str = current_mod
                .items()
                .map(|items| {
                    items
                        .iter()
                        // new lines for condensed items
                        .take(3)
                        .chain(current_mod.items().into_iter().rev().take(3).flatten())
                        // end new condensed items changes
                        .map(|id| id.log_id_debug())
                        .join(", ")
                })
                .unwrap_or_else(|| "<None>".to_string());

            trace!(target: LOG_TARGET_TRACE, "{}", "--- Module Stack Debug ---".dimmed());
            trace!(target: LOG_TARGET_TRACE, "  Current Mod: {} ({})", current_mod.name.cyan(), current_mod.id.to_string().magenta());
            trace!(target: LOG_TARGET_TRACE, "  Items: [{}]", items_str);
            trace!(target: LOG_TARGET_TRACE, "  All Modules: {:?}", modules); // Keep this simple for now
            trace!(target: LOG_TARGET_TRACE, "{}", "--------------------------".dimmed());
        }
    }

    #[allow(dead_code, reason = "Useful for debugging")]
    fn debug_mod_stack_push(&mut self, name: String, pr_id: PrimaryNodeId) {
        if let Some(current_mod) = self
            .state
            .code_graph
            .modules // Remove extra .code_graph.modules
            .iter()
            .find(|m| m.items().is_some_and(|items| items.contains(&pr_id)))
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
                pr_id.to_string().magenta(),
                items_str.dimmed()
            );
        } else {
            // Log warning instead of panic
            log::warn!(target: LOG_TARGET_TRACE, "Could not find containing module for node with name {}, id {}", name, pr_id);
        }
    }
    // Removed #[cfg(feature = "verbose_debug")]
    fn debug_new_id(&mut self, name: &str, node_id: AnyNodeId) {
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
        trace!(target: LOG_TARGET_STACK_TRACE, "  [PUSH STACK] {}: {} -> {:?}",
            stack_name.blue(),
            stack.last().unwrap_or(&"<empty>".to_string()).green(),
            stack
        );
    }

    // Removed #[cfg(feature = "verbose_debug")]
    fn log_pop(&self, stack_name: &str, popped: Option<String>, stack: &[String]) {
        trace!(target: LOG_TARGET_STACK_TRACE, "  [POP STACK] {}: {} -> {:?}",
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
    ) -> Option<(AnyNodeId, ModuleNodeId)> {
        // TODO: Now that we have typed ids and can match on the kind of the id, we may be able to
        // locate all relation creation here. The tight coupling of id generation to relation
        // creation was set aside in favor of creating more node ids previously due to our desire
        // to keep the `Contains` relation only between the module and it's contained items. The
        // problem with generating the relations here was that we needed to use a `Contains`
        // relation for `ModuleNode`->any primary node (including other modules), and needed to
        // generate ids for, e.g. a struct FieldNode which does not have a `Contains` relation but
        // rather a `Struct`

        // Return base ID and parent ModuleNodeId
        // 1. Generate the Synthetic NodeId using the state helper, passing CFG bytes
        let node_id = self
            .state
            .generate_synthetic_node_id(item_name, item_kind, cfg_bytes); // Pass cfg_bytes
                                                                          // 2. Find the parent module based on the *current path* and add the item ID.
        let parent_module_opt = self
            .state
            .code_graph
            .modules
            .iter_mut() // Need mutable access to add to items list
            .find(|m| m.path == self.state.current_module_path); // Find module matching current path
        let parent_mod_id_opt = match parent_module_opt {
            Some(parent_mod) => {
                // Try to convert the generated AnyNodeId to PrimaryNodeId
                let primary_node_id_opt: Option<PrimaryNodeId> = node_id.try_into().ok();

                match &mut parent_mod.module_def {
                    ModuleKind::Inline { items, .. } => {
                        // Only add if it's a PrimaryNodeId
                        if let Some(primary_id) = primary_node_id_opt {
                            items.push(primary_id);
                        }
                        Some(parent_mod.id) // Always return the parent ID
                    }
                    ModuleKind::FileBased { items, .. } => {
                        // Only add if it's a PrimaryNodeId
                        if let Some(primary_id) = primary_node_id_opt {
                            items.push(primary_id);
                        }
                        Some(parent_mod.id) // Always return the parent ID
                    }
                    ModuleKind::Declaration { .. } => {
                        // Cannot add items to a declaration, log warning
                        log::warn!(
                            target: LOG_TARGET_TRACE,
                            "Attempted to add item '{}' ({:?}) to a module declaration node '{}' ({}). Item not added to list.",
                            item_name, item_kind, parent_mod.name, parent_mod.id
                        );
                        // Still return the parent ID, even though item wasn't added to list
                        Some(parent_mod.id)
                    }
                }
            }
            None => {
                log::warn!(
                target: LOG_TARGET_TRACE,
                "Could not find parent module for item '{}' ({:?}) using current_module_path {:?}. Item not added to module list.",
                item_name, item_kind, self.state.current_module_path
                );
                None
            }
        };
        parent_mod_id_opt.map(|parent_mod_id| (node_id, parent_mod_id))
    }

    /// Helper to push primary scope and log (using trace!)
    /// 'primary scope' here means a primary node, which may be defined directly within a module
    fn push_primary_scope(&mut self, name: &str, id: PrimaryNodeId, cfgs: &[String]) {
        self.state.current_primary_defn_scope.push(id);
        self.state
            .cfg_stack
            .push(self.state.current_scope_cfgs.clone()); // current_scope_cfgs shared among
                                                          // primary, secondary, associated, etc.
        self.state.current_scope_cfgs = cfgs.to_vec();
        trace!(target: LOG_TARGET_TRACE, ">>> Entering Primary Scope: {} ({}) | CFGs: {:?}", name.cyan(), id.to_string().magenta(), self.state.current_scope_cfgs);
    }

    /// Helper to push secondary scope and log (using trace!)
    /// 'secondary scope' here means a secondary node, which cannot be defined directly within a
    /// module, but may be defined within a primary node, such as a struct field, emum variant, etc
    ///
    /// This is useful for including the hashed NodeId of, e.g. a VariantNode, in the hash
    /// generation of the variant's field's NodeIds.
    fn push_secondary_scope(&mut self, name: &str, id: SecondaryNodeId, cfgs: &[String]) {
        self.state.current_secondary_defn_scope.push(id);
        self.state
            .cfg_stack
            .push(self.state.current_scope_cfgs.clone()); // current_scope_cfgs shared among
                                                          // primary, secondary, associated, etc.
        self.state.current_scope_cfgs = cfgs.to_vec();
        trace!(target: LOG_TARGET_TRACE, ">>> Entering Secondary Scope: {} ({}) | CFGs: {:?}", name.cyan(), id.to_string().magenta(), self.state.current_scope_cfgs);
    }

    /// Helper function to push associated scope and log (using trace!)
    /// 'associated scope' here means an item that may be defined directly within an impl block,
    /// such as a method, associated function, associated const, etc.
    ///
    /// This is useful for including the hashed NodeId of, e.g. an associated constant, in the hash
    /// generation of the associated constant's items (such as the generic parameters)
    fn push_assoc_scope(&mut self, name: &str, id: AssociatedItemNodeId, cfgs: &[String]) {
        self.state.current_assoc_defn_scope.push(id);
        self.state
            .cfg_stack
            .push(self.state.current_scope_cfgs.clone()); // current_scope_cfgs shared among
                                                          // primary, secondary, associated, etc.
        self.state.current_scope_cfgs = cfgs.to_vec();
        trace!(target: LOG_TARGET_TRACE, ">>> Entering Scope: {} ({}) | CFGs: {:?}", name.cyan(), id.to_string().magenta(), self.state.current_scope_cfgs);
    }

    // Helper to pop scope and log (using trace!)
    fn pop_primary_scope(&mut self, name: &str) {
        let popped_id = self.state.current_primary_defn_scope.pop();
        let popped_cfgs = self.state.current_scope_cfgs.clone(); // Log before restoring
        self.state.current_scope_cfgs = self.state.cfg_stack.pop().unwrap_or_default(); // current_scope_cfgs shared among
                                                                                        // primary, secondary, associated, etc.
        trace!(target: LOG_TARGET_TRACE, "<<< Exiting Primary Scope: {} ({}) | Popped CFGs: {:?} | Restored CFGs: {:?}",
            name.cyan(),
            popped_id.map(|id| id.to_string()).unwrap_or("?".to_string()).magenta(),
            popped_cfgs,
            self.state.current_scope_cfgs
        );
    }
    fn pop_secondary_scope(&mut self, name: &str) {
        let popped_id = self.state.current_secondary_defn_scope.pop();
        let popped_cfgs = self.state.current_scope_cfgs.clone(); // Log before restoring
        self.state.current_scope_cfgs = self.state.cfg_stack.pop().unwrap_or_default(); // current_scope_cfgs shared among
                                                                                        // primary, secondary, associated, etc.
        trace!(target: LOG_TARGET_TRACE, "<<< Exiting Secondary Scope: {} ({}) | Popped CFGs: {:?} | Restored CFGs: {:?}",
            name.cyan(),
            popped_id.map(|id| id.to_string()).unwrap_or("?".to_string()).magenta(),
            popped_cfgs,
            self.state.current_scope_cfgs
        );
    }
    #[allow(
        dead_code,
        reason = "useful later for when we implement associated item parsing"
    )]
    fn pop_assoc_scope(&mut self, name: &str) {
        let popped_id = self.state.current_assoc_defn_scope.pop();
        let popped_cfgs = self.state.current_scope_cfgs.clone(); // Log before restoring
        self.state.current_scope_cfgs = self.state.cfg_stack.pop().unwrap_or_default(); // current_scope_cfgs shared among
                                                                                        // primary, secondary, associated, etc.
        trace!(target: LOG_TARGET_TRACE, "<<< Exiting Assoc Scope: {} ({:?}) | Popped CFGs: {:?} | Restored CFGs: {:?}",
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

        #[cfg(feature = "cfg_eval")]
        {
            use crate::parser::visitor::attribute_processing::should_include_item;
            let active_cfg = &self.state.active_cfg;

            if !should_include_item(&func.attrs, active_cfg) {
                return; // Skip this item due to cfg
            }
        }
        let is_proc_macro = func.attrs.iter().any(|attr| {
            attr.path().is_ident("proc_macro")
                || attr.path().is_ident("proc_macro_derive")
                || attr.path().is_ident("proc_macro_attribute")
        });

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
            let registration_result = self.register_new_node_id(
                &macro_name, ItemKind::Macro, cfg_bytes.as_deref()
            ).unwrap_or_else(|| todo!("Implement proper error handling. We can't return a result inside the visitor implementation but we can log the error."));
            let (macro_any_id, parent_mod_id) = registration_result;
            self.debug_new_id(&macro_name, macro_any_id);
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

            let macro_node = MacroNode {
                id: macro_any_id.try_into().unwrap(), // temporary unwrap for testing refactor
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
            let (fn_any_id, parent_mod_id) = registration_result.unwrap();

            self.debug_new_id(&fn_name, fn_any_id); // Now uses trace!

            let byte_range = func.span().byte_range();
            let span = (byte_range.start, byte_range.end);

            let fn_typed_id: FunctionNodeId = fn_any_id.try_into()
                .expect("Invalid State: FunctionNodeId could not be obtained from AnyNodeId generated by register_new_node_id");
            // Push the function's base ID onto the scope stack BEFORE processing types/generics
            // Use helper function for logging
            self.push_primary_scope(&fn_name, fn_typed_id.into(), &provisional_effective_cfgs);

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
            self.pop_primary_scope(&fn_name);

            // Extract doc comments and other attributes
            let docstring = extract_docstring(&func.attrs);
            let attributes = extract_attributes(&func.attrs);

            // Extract function body as a string
            let body = Some(func.block.to_token_stream().to_string());

            // Create info struct and then the node
            let function_node = FunctionNode {
                id: fn_typed_id,
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
            self.state.code_graph.functions.push(function_node);

            // Create and add the Contains relation
            let relation = SyntacticRelation::Contains {
                source: parent_mod_id,
                target: PrimaryNodeId::from(fn_typed_id), // Use category enum
            };
            self.state.code_graph.relations.push(relation);

            // NOTE: We are already visiting all the items we are processing within this
            // visit_item_fn, but this is where we would put the `visit_item_fn` to call the method
            // again and continue visiting, like this:
            // self.push_primary_scope(&fn_name);
            // visit::visit_item_fn(self, func);
            // self.pop_primary_scope(&fn_name);
        } // End else block for regular functions
    }

    #[cfg(not(feature = "cfg_eval"))]
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
        // Visit struct definitions

        // Register the new node ID and get parent module ID
        let registration_result =
            self.register_new_node_id(&struct_name, ItemKind::Struct, cfg_bytes.as_deref());
        if registration_result.is_none() {
            return;
        } // Skip if parent module not found
        let (struct_any_id, parent_mod_id) = registration_result.unwrap();

        self.debug_new_id(&struct_name, struct_any_id); // Now uses trace!

        let byte_range = item_struct.span().byte_range();
        let span = (byte_range.start, byte_range.end);

        // Push the struct's base ID onto the scope stack BEFORE processing fields/generics
        // Use helper function for logging
        let struct_typed_id: StructNodeId = struct_any_id.try_into().unwrap();
        self.push_primary_scope(
            &struct_name,
            struct_typed_id.into(),
            &provisional_effective_cfgs,
        ); // Clone cfgs for push

        // Process fields
        let mut fields = Vec::new();
        for (field, i) in item_struct.fields.iter().zip(u8::MIN..u8::MAX) {
            let mut field_name = field.ident.as_ref().map(|ident| ident.to_string());
            let field_ref = field_name.get_or_insert_default();
            field_ref.extend("unnamed_field".chars().chain(struct_name.as_str().chars()));
            field_ref.push(i.into());

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
            let field_any_id = self.state.generate_synthetic_node_id(
                &field_ref.clone(),
                // .unwrap_or_else(|| format!("unnamed_field{}_in_{}", i, struct_name)),
                ItemKind::Field,
                field_cfg_bytes.as_deref(), // Pass field's CFG bytes
            );

            // Removed #[cfg] block
            self.debug_new_id(&field_ref.clone(), field_any_id);
            let type_id = get_or_create_type(self.state, &field.ty);

            let field_node_id: FieldNodeId = field_any_id.try_into().unwrap();
            let field_node = FieldNode {
                id: field_node_id,
                name: field_name,
                type_id,
                visibility: self.state.convert_visibility(&field.vis),
                attributes: extract_attributes(&field.attrs),
                cfgs: field_item_cfgs,
            };
            fields.push(field_node);

            // Add relation between struct and field (defer until struct node is created)
            // We need the typed struct ID first.
        }

        // Process generic parameters (still within struct's scope)
        let generic_params = self.state.process_generics(&item_struct.generics);

        // Extract doc comments and other attributes
        let docstring = extract_docstring(&item_struct.attrs);
        let attributes = extract_attributes(&item_struct.attrs);

        let struct_node_id: StructNodeId = struct_any_id.try_into().unwrap();
        // Create the struct node
        let struct_node = StructNode {
            id: struct_node_id, // Use base ID
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

        // Now add the StructField relations using the fields from the created struct_node
        for field_node in &struct_node.fields {
            let relation = SyntacticRelation::StructField {
                source: struct_node_id,
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
            target: PrimaryNodeId::from(struct_node_id), // Use category enum
        };
        self.state.code_graph.relations.push(contains_relation);

        // Continue visiting children (fields are handled above, visit generics/where clauses if needed)
        visit::visit_item_struct(self, item_struct);

        // Pop the struct's scope using the helper
        self.pop_primary_scope(&struct_name);
    }

    // Visit struct definitions
    #[cfg(feature = "cfg_eval")]
    fn visit_item_struct(&mut self, item_struct: &'ast ItemStruct) {
        let struct_name = item_struct.ident.to_string();

        // --- CFG Handling (Expression Evaluation) ---
        use crate::parser::visitor::attribute_processing::should_include_item;
        let active_cfg = &self.state.active_cfg;

        if !should_include_item(&item_struct.attrs, active_cfg) {
            return; // Skip this item due to cfg
        }

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
        let (struct_any_id, parent_mod_id) = registration_result.unwrap();

        self.debug_new_id(&struct_name, struct_any_id); // Now uses trace!

        let byte_range = item_struct.span().byte_range();
        let span = (byte_range.start, byte_range.end);

        // Push the struct's base ID onto the scope stack BEFORE processing fields/generics
        // Use helper function for logging
        let struct_typed_id: StructNodeId = struct_any_id.try_into().unwrap();
        self.push_primary_scope(
            &struct_name,
            struct_typed_id.into(),
            &provisional_effective_cfgs,
        ); // Clone cfgs for push

        // Process fields
        let mut fields = Vec::new();
        for (field, i) in item_struct.fields.iter().zip(u8::MIN..u8::MAX) {
            let mut field_name = field.ident.as_ref().map(|ident| ident.to_string());
            let field_ref = field_name.get_or_insert_default();
            field_ref.extend("unnamed_field".chars().chain(struct_name.as_str().chars()));
            field_ref.push(i.into());

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
            let field_any_id = self.state.generate_synthetic_node_id(
                &field_ref.clone(),
                // .unwrap_or_else(|| format!("unnamed_field{}_in_{}", i, struct_name)),
                ItemKind::Field,
                field_cfg_bytes.as_deref(), // Pass field's CFG bytes
            );

            // Removed #[cfg] block
            self.debug_new_id(&field_ref.clone(), field_any_id);
            let type_id = get_or_create_type(self.state, &field.ty);

            let field_node_id: FieldNodeId = field_any_id.try_into().unwrap();
            let field_node = FieldNode {
                id: field_node_id,
                name: field_name,
                type_id,
                visibility: self.state.convert_visibility(&field.vis),
                attributes: extract_attributes(&field.attrs),
                cfgs: field_item_cfgs,
            };
            fields.push(field_node);

            // Add relation between struct and field (defer until struct node is created)
            // We need the typed struct ID first.
        }

        // Process generic parameters (still within struct's scope)
        let generic_params = self.state.process_generics(&item_struct.generics);

        // Extract doc comments and other attributes
        let docstring = extract_docstring(&item_struct.attrs);
        let attributes = extract_attributes(&item_struct.attrs);

        let struct_node_id: StructNodeId = struct_any_id.try_into().unwrap();
        // Create the struct node
        let struct_node = StructNode {
            id: struct_node_id, // Use base ID
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

        // Now add the StructField relations using the fields from the created struct_node
        for field_node in &struct_node.fields {
            let relation = SyntacticRelation::StructField {
                source: struct_node_id,
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
            target: PrimaryNodeId::from(struct_node_id), // Use category enum
        };
        self.state.code_graph.relations.push(contains_relation);

        // Continue visiting children (fields are handled above, visit generics/where clauses if needed)
        visit::visit_item_struct(self, item_struct);

        // Pop the struct's scope using the helper
        self.pop_primary_scope(&struct_name);
    }

    // Visit type alias definitions
    fn visit_item_type(&mut self, item_type: &'ast syn::ItemType) {
        #[cfg(feature = "cfg_eval")]
        {
            use crate::parser::visitor::attribute_processing::should_include_item;
            let active_cfg = &self.state.active_cfg;

            if !should_include_item(&item_type.attrs, active_cfg) {
                return; // Skip this item due to cfg
            }
        }
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
        let (type_alias_any_id, parent_mod_id) = registration_result.unwrap();

        self.debug_new_id(&type_alias_name, type_alias_any_id); // Now uses trace!

        let span = item_type.extract_span_bytes();

        // Push the type alias's base ID onto the scope stack BEFORE processing type/generics
        // Type aliases don't introduce a new CFG scope, so pass current scope cfgs
        let type_alias_node_id: TypeAliasNodeId = type_alias_any_id.try_into().unwrap();
        self.push_primary_scope(
            &type_alias_name,
            type_alias_node_id.into(),
            &self.state.current_scope_cfgs.clone(),
        );

        // Process the aliased type
        let type_id = get_or_create_type(self.state, &item_type.ty);

        // Process generic parameters
        let generic_params = self.state.process_generics(&item_type.generics);

        // Pop the type alias's ID from the scope stack AFTER processing type/generics
        // Use helper function for logging
        self.pop_primary_scope(&type_alias_name);

        // Extract doc comments and other attributes
        let docstring = extract_docstring(&item_type.attrs);
        let attributes = extract_attributes(&item_type.attrs);

        // Create info struct and then the node
        let type_alias_node = TypeAliasNode {
            id: type_alias_node_id,
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

        // Add the node to the graph
        self.state
            .code_graph
            .defined_types
            .push(TypeDefNode::TypeAlias(type_alias_node));

        // Add the Contains relation
        let contains_relation = SyntacticRelation::Contains {
            source: parent_mod_id,
            target: PrimaryNodeId::from(type_alias_node_id), // Use category enum
        };
        self.state.code_graph.relations.push(contains_relation);

        // Type aliases don't define a new CFG scope for children.
        // Continue visiting (type_alias_any_id is already off the definition stack)
        visit::visit_item_type(self, item_type);
    }

    // Visit union definitions
    fn visit_item_union(&mut self, item_union: &'ast syn::ItemUnion) {
        #[cfg(feature = "cfg_eval")]
        {
            use crate::parser::visitor::attribute_processing::should_include_item;
            let active_cfg = &self.state.active_cfg;

            if !should_include_item(&item_union.attrs, active_cfg) {
                return; // Skip this item due to cfg
            }
        }
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
        let (union_any_id, parent_mod_id) = registration_result.unwrap();

        self.debug_new_id(&union_name, union_any_id); // Now uses trace!

        let span = item_union.extract_span_bytes();

        // Push the union's base ID onto the scope stack BEFORE processing fields/generics
        // Use helper function for logging
        let union_typed_id: UnionNodeId = union_any_id.try_into().unwrap();
        self.push_primary_scope(
            &union_name,
            union_typed_id.into(),
            &provisional_effective_cfgs,
        ); // Clone cfgs for push

        // Process fields
        let mut fields = Vec::new();
        for (field, i) in item_union.fields.named.iter().zip(u8::MIN..u8::MAX) {
            let mut field_name = field.ident.as_ref().map(|ident| ident.to_string());
            let field_ref = field_name.get_or_insert_default();
            field_ref.extend("_field_".chars().chain(union_name.as_str().chars()));
            field_ref.push(i.into());

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
            let field_any_id = self.state.generate_synthetic_node_id(
                &field_name
                    .clone()
                    .unwrap_or_else(|| format!("_unnamed_field_{}_in_{}", i, union_name)),
                ItemKind::Field,
                field_cfg_bytes.as_deref(), // Pass field's CFG bytes
            );
            self.debug_new_id(
                &field_name
                    .clone()
                    .unwrap_or_else(|| format!("_unnamed_field_{}_in_{}", i, union_name)),
                field_any_id,
            );
            let type_id = get_or_create_type(self.state, &field.ty);
            let field_node_id: FieldNodeId = field_any_id.try_into().unwrap();

            let field_node = FieldNode {
                id: field_node_id,
                name: field_name,
                type_id,
                visibility: self.state.convert_visibility(&field.vis),
                attributes: extract_attributes(&field.attrs),
                cfgs: field_item_cfgs,
            };
            fields.push(field_node);
        }

        // Process generic parameters
        let generic_params = self.state.process_generics(&item_union.generics);

        // Pop the union's ID from the scope stack AFTER processing fields/generics
        // Note: This pop happens *before* visiting children, which might be incorrect
        // if generics/where clauses need the union scope. Let's move the pop after visit.
        // self.state.current_primary_defn_scope.pop(); // Moved below

        // Extract doc comments and other attributes
        let docstring = extract_docstring(&item_union.attrs);
        let attributes = extract_attributes(&item_union.attrs);

        // Create info struct and then the node
        let union_node_id: UnionNodeId = union_any_id.try_into().unwrap();
        let union_node = UnionNode {
            id: union_node_id,
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
        // Now add the UnionField relations using fields from the created union_node
        for field_node in &union_node.fields {
            let relation = SyntacticRelation::UnionField {
                source: union_node_id,
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
            target: PrimaryNodeId::from(union_node_id), // Use category enum
        };
        self.state.code_graph.relations.push(contains_relation);

        // Continue visiting children (fields handled above, visit generics/where clauses if needed)
        visit::visit_item_union(self, item_union);

        // Pop the union's scope using the helper *after* visiting children
        self.pop_primary_scope(&union_name);
    }

    // Visit enum definitions
    fn visit_item_enum(&mut self, item_enum: &'ast ItemEnum) {
        #[cfg(feature = "cfg_eval")]
        {
            use crate::parser::visitor::attribute_processing::should_include_item;
            let active_cfg = &self.state.active_cfg;

            if !should_include_item(&item_enum.attrs, active_cfg) {
                return; // Skip this item due to cfg
            }
        }
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
        let (enum_any_id, parent_mod_id) = registration_result.unwrap();

        self.debug_new_id(&enum_name, enum_any_id); // Now uses trace!

        let span = item_enum.extract_span_bytes();

        let enum_node_id: EnumNodeId = enum_any_id.try_into().unwrap();
        // Push the enum's base ID onto the scope stack BEFORE processing its generics
        // Use helper function for logging
        self.push_primary_scope(&enum_name, enum_node_id.into(), &provisional_effective_cfgs); // Clone cfgs for push

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
            let variant_any_id = self.state.generate_synthetic_node_id(
                &variant_name,
                ItemKind::Variant,
                variant_cfg_bytes.as_deref(), // Pass variant's CFG bytes
            );

            let variant_node_id: VariantNodeId = variant_any_id.try_into().unwrap();
            // Push the variant's base ID onto the scope stack BEFORE processing its fields
            // Variants don't introduce a new CFG scope, pass current (enum's) scope cfgs
            self.push_secondary_scope(
                &variant_name,
                // TODO: Implement SecondaryNodeId and update CodeVisitor to have another field
                // with the secondary node scope. Update create a `push_secondary_scope` and
                // `pop_secondary_scope` to manage this state. Any nodes created within the scope
                // of the variant should receive the node_id as part of their node id generation.
                SecondaryNodeId::from(variant_node_id),
                &self.state.current_scope_cfgs.clone(),
            );

            // Process fields of the variant
            let mut fields = Vec::new();
            match &variant.fields {
                syn::Fields::Named(fields_named) => {
                    for (i, field) in fields_named.named.iter().enumerate() {
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
                        let field_any_id = self.state.generate_synthetic_node_id(
                            &field_name.clone().unwrap_or_else(|| {
                                format!("unnamed_field{}_in_{}", i, variant_name)
                            }),
                            ItemKind::Field,
                            field_cfg_bytes.as_deref(), // Pass field's CFG bytes
                        );
                        self.debug_new_id(
                            &field_name.clone().unwrap_or_else(|| {
                                format!("unnamed_field{}_in_{}", i, variant_name)
                            }),
                            field_any_id,
                        );
                        let type_id = get_or_create_type(self.state, &field.ty);

                        let field_node_id: FieldNodeId = field_any_id.try_into().unwrap();
                        let field_node = FieldNode {
                            id: field_node_id,
                            name: field_name,
                            type_id,
                            visibility: self.state.convert_visibility(&field.vis),
                            attributes: extract_attributes(&field.attrs),
                            cfgs: field_item_cfgs,
                        };
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
                        let field_any_id = self.state.generate_synthetic_node_id(
                            &field_name_placeholder, // Use placeholder for ID generation
                            ItemKind::Field,
                            field_cfg_bytes.as_deref(),
                        );
                        let type_id = get_or_create_type(self.state, &field.ty);
                        self.debug_new_id(&field_name_placeholder, field_any_id);

                        let field_node_id: FieldNodeId = field_any_id.try_into().unwrap();
                        let field_node = FieldNode {
                            id: field_node_id,
                            name: None,
                            type_id,
                            visibility: self.state.convert_visibility(&field.vis),
                            attributes: extract_attributes(&field.attrs),
                            cfgs: field_item_cfgs,
                        };
                        fields.push(field_node);
                    }
                }
                syn::Fields::Unit => {
                    // Unit variants don't have fields
                }
            }

            // Pop the variant's ID from the scope stack AFTER processing its fields
            // Use helper function for logging
            self.pop_secondary_scope(&variant_name);

            // Extract discriminant if any
            let discriminant = variant
                .discriminant
                .as_ref()
                .map(|(_, expr)| expr.to_token_stream().to_string());

            let variant_node_id: VariantNodeId = variant_any_id.try_into().unwrap();
            // Create info struct and then the node
            let variant_node = VariantNode {
                id: variant_node_id,
                name: variant_name,
                fields: fields.clone(), // Clone the collected FieldNode Vec
                discriminant,
                attributes: extract_attributes(&variant.attrs),
                cfgs: variant_item_cfgs,
            };
            variants.push(variant_node);

            // Add EnumVariant relation (defer until enum node is created)

            // Add VariantField relations now that we have the typed variant ID
            for field_node in fields {
                // Iterate over original fields Vec
                let relation = SyntacticRelation::VariantField {
                    source: variant_node_id,
                    target: field_node.field_id(), // Use typed field ID
                };
                self.state.code_graph.relations.push(relation);
            }
        }

        // Process generic parameters
        let generic_params = self.state.process_generics(&item_enum.generics);

        // Pop the enum's ID from the scope stack AFTER processing its generics
        // Note: This pop happens *before* visiting children, which might be incorrect
        // if generics/where clauses need the enum scope. Let's move the pop after visit.
        // self.state.current_primary_defn_scope.pop(); // Moved below

        // Extract doc comments and other attributes
        let docstring = extract_docstring(&item_enum.attrs);
        let attributes = extract_attributes(&item_enum.attrs);

        // Create info struct and then the node
        let enum_node = EnumNode {
            id: enum_node_id,
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

        // Now add the EnumVariant relations using variants from the created enum_node
        for variant_node in &enum_node.variants {
            let relation = SyntacticRelation::EnumVariant {
                source: enum_node_id,
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
            target: PrimaryNodeId::from(enum_node_id), // Use category enum
        };
        self.state.code_graph.relations.push(contains_relation);

        // Continue visiting children (variants/fields handled above, visit generics/where)
        visit::visit_item_enum(self, item_enum);

        // Pop the enum's scope using the helper *after* visiting children
        self.pop_primary_scope(&enum_name);
    }

    // Visit impl blocks
    fn visit_item_impl(&mut self, item_impl: &'ast ItemImpl) {
        let impl_name = name_impl(item_impl); // Use helper to generate a name for the impl block
        #[cfg(feature = "cfg_eval")]
        {
            use crate::parser::visitor::attribute_processing::should_include_item;
            let active_cfg = &self.state.active_cfg;

            if !should_include_item(&item_impl.attrs, active_cfg) {
                return; // Skip this item due to cfg
            }
        }

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
        let (impl_any_id, parent_mod_id) = registration_result.unwrap();

        self.debug_new_id(&impl_name, impl_any_id); // Log with the generated name, now uses trace!

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
        let impl_node_id: ImplNodeId = impl_any_id.try_into().unwrap();
        self.push_primary_scope(&impl_name, impl_node_id.into(), &provisional_effective_cfgs); // Clone cfgs for push
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
            // for (item, i) in item_impl.items.iter().zip(u8::MIN..u8::MAX) {
            //     // NOTE: There are NO other match arms or if-let chains here
            //     //       to handle syn::ImplItem::Const or syn::ImplItem::Type
            if let syn::ImplItem::Fn(method) = item {
                let method_name = method.sig.ident.to_string();
                // NOTE: We may not actually want to change this to the above enumerated loop,
                // since we shouldn't ever have a situation in which the same impl block has the
                // same name repeat for each method.
                // let method_name = method.sig.ident.to_string();
                // let mut method_name: String = method
                //     .sig
                //     .ident
                //     .to_string()
                //     .chars()
                //     .chain("unnamed_method".chars())
                //     .chain(impl_name.as_str().chars())
                //     .collect();
                // method_name.push(i.into());

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
                let method_any_id = self.state.generate_synthetic_node_id(
                    &method_name,
                    ItemKind::Method, // Use Method kind
                    method_cfg_bytes.as_deref(),
                );

                self.debug_new_id(&method_name, method_any_id); // Now uses trace!

                // Convert method ID and push scope
                let method_typed_id: MethodNodeId = method_any_id.try_into().unwrap();
                self.push_assoc_scope(
                    &method_name,
                    AssociatedItemNodeId::from(method_typed_id), // Use AssociatedItemNodeId for scope
                    &self.state.current_scope_cfgs.clone(),
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
                self.pop_secondary_scope(&method_name);

                // Extract doc comments and other attributes for methods
                let docstring = extract_docstring(&method.attrs);
                let attributes = extract_attributes(&method.attrs);

                // Extract method body as a string
                let body = Some(method.block.to_token_stream().to_string());

                // Create info struct and then the node
                let method_node_id = method_any_id.try_into().unwrap();
                let method_node = MethodNode {
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
                        self.state.generate_tracking_hash(&method.to_token_stream()),
                    ),
                    cfgs: method_item_cfgs,
                };
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
        let impl_node = ImplNode {
            id: impl_node_id,
            span: item_impl.extract_span_bytes(),
            self_type: self_type_id,
            trait_type: trait_type_id,
            methods, // Pass the collected MethodNode Vec
            generic_params,
            cfgs: item_cfgs,
        };
        let typed_impl_id = impl_node.impl_id();
        trace!(target:"duplicate_impl", "{:-^40}
            ---- impl_node_id: {} --
            current_file_path: {}
            current_module_path: {:?}
            current_mod: {:?}\nimpl_node: {:#?}\n{:-^40}", "", impl_node_id, self.state.current_file_path.display(), self.state.current_module, self.state.current_module_path, impl_node, "");

        // Now add the ImplAssociatedItem relations using methods from the created impl_node
        for method_node in &impl_node.methods {
            let relation = SyntacticRelation::ImplAssociatedItem {
                source: typed_impl_id,
                target: AssociatedItemNodeId::from(method_node.method_id()),
            };
            self.state.code_graph.relations.push(relation);
        }
        for const_node in &associated_consts {
            // TODO: Populate associated_consts
            let relation = SyntacticRelation::ImplAssociatedItem {
                source: typed_impl_id,
                target: AssociatedItemNodeId::from(const_node.const_id()),
            };
            self.state.code_graph.relations.push(relation);
        }
        for type_node in &associated_types {
            // TODO: Populate associated_types
            let relation = SyntacticRelation::ImplAssociatedItem {
                source: typed_impl_id,
                target: AssociatedItemNodeId::from(type_node.type_alias_id()),
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
        // Note: CFG scope is pushed/popped by push_*_scope/pop_*_scope helpers
        visit::visit_item_impl(self, item_impl);

        // Pop the impl's scope using the helper *after* visiting children
        self.pop_primary_scope(&impl_name);
    }

    // Visit trait definitions
    fn visit_item_trait(&mut self, item_trait: &'ast ItemTrait) {
        #[cfg(feature = "cfg_eval")]
        {
            use crate::parser::visitor::attribute_processing::should_include_item;
            let active_cfg = &self.state.active_cfg;

            if !should_include_item(&item_trait.attrs, active_cfg) {
                return; // Skip this item due to cfg
            }
        }
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
        let (trait_any_id, parent_mod_id) = registration_result.unwrap();

        self.debug_new_id(&trait_name, trait_any_id); // Now uses trace!

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
                let method_any_id = self.state.generate_synthetic_node_id(
                    &method_name,
                    ItemKind::Method, // Use Method kind
                    method_cfg_bytes.as_deref(),
                );

                self.debug_new_id(&method_name, method_any_id); // Now uses trace!

                let method_node_id: MethodNodeId = method_any_id.try_into().unwrap();
                // Push the method's base ID onto the scope stack BEFORE processing its types/generics
                // Methods don't introduce a new CFG scope, pass current (trait's) scope cfgs
                self.push_assoc_scope(
                    // TODO: Update the `CodeVisitor` to have another field for associated node id
                    // for scope management. See comment on `&variant_name,` for more info on how
                    // to update.
                    &method_name,
                    AssociatedItemNodeId::from(method_node_id),
                    &self.state.current_scope_cfgs.clone(),
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
                self.pop_secondary_scope(&method_name);

                // Extract doc comments and other attributes for methods
                let docstring = extract_docstring(&method.attrs);
                let attributes = extract_attributes(&method.attrs);

                // Extract method body if available (trait methods may have default implementations)
                let body = method
                    .default
                    .as_ref()
                    .map(|block| block.to_token_stream().to_string());

                let method_node_id = method_any_id.try_into().unwrap();
                // Construct MethodNode directly
                let method_node = MethodNode {
                    id: method_node_id, // Use typed ID (assuming method_typed_id is defined earlier)
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

        // Convert trait ID and push scope
        let trait_typed_id: TraitNodeId = trait_any_id.try_into().unwrap();
        self.push_primary_scope(
            &trait_name,
            PrimaryNodeId::from(trait_typed_id), // Use PrimaryNodeId for scope
            &provisional_effective_cfgs,
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
        // self.state.current_primary_defn_scope.pop(); // Moved below

        // Extract doc comments and other attributes
        let docstring = extract_docstring(&item_trait.attrs);
        let attributes = extract_attributes(&item_trait.attrs);

        // Construct TraitNode directly
        let trait_node = TraitNode {
            id: trait_typed_id, // Use typed ID
            name: trait_name.clone(),
            span: item_trait.extract_span_bytes(),
            visibility: self.state.convert_visibility(&item_trait.vis),
            methods, // Use collected methods
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

        // Now add the TraitAssociatedItem relations using methods from the created trait_node
        for method_node in &trait_node.methods {
            let relation = SyntacticRelation::TraitAssociatedItem {
                source: trait_typed_id, // Use typed trait ID
                target: AssociatedItemNodeId::from(method_node.method_id()),
            };
            self.state.code_graph.relations.push(relation);
        }
        for const_node in &associated_consts {
            // TODO: Populate associated_consts
            let relation = SyntacticRelation::TraitAssociatedItem {
                source: trait_typed_id, // Use typed trait ID
                target: AssociatedItemNodeId::from(const_node.const_id()),
            };
            self.state.code_graph.relations.push(relation);
        }
        for type_node in &associated_types {
            // TODO: Populate associated_types
            let relation = SyntacticRelation::TraitAssociatedItem {
                source: trait_typed_id, // Use typed trait ID
                target: AssociatedItemNodeId::from(type_node.type_alias_id()),
            };
            self.state.code_graph.relations.push(relation);
        }

        // Add the trait node to the graph
        self.state.code_graph.traits.push(trait_node);

        // Add the Contains relation for the trait itself
        let contains_relation = SyntacticRelation::Contains {
            source: parent_mod_id,
            target: PrimaryNodeId::from(trait_typed_id), // Use typed trait ID
        };
        self.state.code_graph.relations.push(contains_relation);

        // Continue visiting children (methods handled above, visit generics/where/supertraits)
        // Note: CFG scope is pushed/popped by push_*_scope/pop_*_scope helpers
        visit::visit_item_trait(self, item_trait);

        // Pop the trait's scope using the helper *after* visiting children
        self.pop_primary_scope(&trait_name);
    }

    fn visit_item_mod(&mut self, module: &'ast syn::ItemMod) {

        #[cfg(feature = "cfg_eval")]
        {
            use crate::parser::visitor::attribute_processing::should_include_item;
            let active_cfg = &self.state.active_cfg;

            if !should_include_item(&module.attrs, active_cfg) {
                return; // Skip this item due to cfg
            }
        }
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
        let (module_any_id, parent_mod_id) = registration_result.unwrap();

        self.debug_mod_stack(); // Now uses trace!

        let span = module.extract_span_bytes();

        self.debug_new_id(&module_name, module_any_id); // Now uses trace!

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

        // Convert module ID
        let module_typed_id: ModuleNodeId = module_any_id.try_into().unwrap();

        // Construct ModuleNode directly
        let module_node = ModuleNode {
            id: module_typed_id, // Use typed ID
            name: module_name.clone(),
            path: self.state.current_module_path.clone(), // Path before restoring parent
            visibility: self.state.convert_visibility(&module.vis),
            attributes: extract_attributes(&module.attrs),
            docstring: extract_docstring(&module.attrs),
            imports: Vec::new(), // Imports added later in visit_item_use/extern_crate
            exports: Vec::new(), // Exports handled during resolution phase
            span,
            tracking_hash: Some(self.state.generate_tracking_hash(&module.to_token_stream())),
            module_def,
            cfgs: item_cfgs,
        };

        // Restore parent path *after* creating the node with its path
        self.state.current_module_path = parent_path;

        // Log stack changes
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
            target: PrimaryNodeId::from(module_typed_id), // Use typed module ID
        };
        self.state.code_graph.relations.push(contains_relation);

        // Push the module's scope using the helper *before* visiting children
        self.push_primary_scope(
            &module_name,
            PrimaryNodeId::from(module_typed_id), // Use PrimaryNodeId for scope
            &provisional_effective_cfgs,
        );

        // Continue visiting children.
        visit::visit_item_mod(self, module);

        // Pop the module's scope using the helper *after* visiting children
        self.pop_primary_scope(&module_name);

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
        let is_in_module_scope = self.state.current_primary_defn_scope.last().is_some_and(|tyid| tyid.kind() == ItemKind::Module );
        log::trace!(target: "some_target",
            "
is_in_module_scope: {}
current_primary_defn_scope is module: {:?}", 
            is_in_module_scope,
            self.state.current_primary_defn_scope.last());
        if !is_in_module_scope {
            log::trace!(target: "some_target", "
use statement primary scope: {:?}
use statement ident: {:?}
", self.state.current_primary_defn_scope, use_item.tree.to_token_stream().to_string());
            log::trace!(target: "some_target", "use statement : {:?}", self.state.current_primary_defn_scope);
            return;
        }
        let item_clone = use_item.clone().to_token_stream().to_string();
        log::trace!("use statement: {}", item_clone);
        #[cfg(feature = "cfg_eval")]
        {
            use crate::parser::visitor::attribute_processing::should_include_item;
            let active_cfg = &self.state.active_cfg;

            if !should_include_item(&use_item.attrs, active_cfg) {
                return; // Skip this item due to cfg
            }
        }
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

        log::trace!(target: LOG_TARGET_TRACE, "{:?}", imports_result);
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
                        trace!(target: "some_target", "\n\tmodule name: {}\t\nimport: {:?}",module.name, import_node);
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
        #[cfg(feature = "cfg_eval")]
        {
            use crate::parser::visitor::attribute_processing::should_include_item;
            let active_cfg = &self.state.active_cfg;

            if !should_include_item(&extern_crate.attrs, active_cfg) {
                return; // Skip this item due to cfg
            }
        }
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
        let (import_any_id, parent_mod_id) = registration_result.unwrap();

        self.debug_new_id(&visible_name, import_any_id); // Log with visible name, now uses trace!

        let crate_name = extern_crate.ident.to_string();
        let span = extern_crate.extract_span_bytes();

        // Convert import ID
        let typed_import_id: ImportNodeId = import_any_id.try_into().unwrap();

        // Construct ImportNode directly
        let import_node = ImportNode {
            id: typed_import_id, // Use typed ID
            span,
            source_path: vec![crate_name.clone()],
            kind: ImportKind::ExternCrate,
            visible_name: extern_crate
                .rename
                .as_ref()
                .map(|(_, id)| id.to_string())
                .unwrap_or_else(|| crate_name.clone()),
            original_name: extern_crate.rename.as_ref().map(|_| crate_name.clone()),
            is_glob: false,
            is_self_import: false,
            cfgs: item_cfgs,
        };

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
        // TODO: Figure out what the heck this is all about
        let _type_id = {
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
        #[cfg(feature = "cfg_eval")]
        {
            use crate::parser::visitor::attribute_processing::should_include_item;
            let active_cfg = &self.state.active_cfg;

            if !should_include_item(&item_const.attrs, active_cfg) {
                return; // Skip this item due to cfg
            }
        }
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
        let (const_any_id, parent_mod_id) = registration_result.unwrap();

        self.debug_new_id(&const_name, const_any_id); // Now uses trace!

        let span = item_const.extract_span_bytes();

        // Process the type
        let type_id = get_or_create_type(self.state, &item_const.ty);

        // Extract the value expression as a string
        let value = Some(item_const.expr.to_token_stream().to_string());

        // Extract doc comments and other attributes
        let docstring = extract_docstring(&item_const.attrs);
        let attributes = extract_attributes(&item_const.attrs);

        // Convert const ID
        let typed_const_id: crate::parser::nodes::ConstNodeId = const_any_id.try_into().unwrap();

        // Construct ConstNode directly
        let const_node = ConstNode {
            id: typed_const_id, // Use typed ID
            name: const_name,
            span,
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

        // Add the constant node to the graph
        self.state.code_graph.consts.push(const_node);

        // Add the Contains relation
        let contains_relation = SyntacticRelation::Contains {
            source: parent_mod_id,
            target: PrimaryNodeId::from(typed_const_id), // Use typed const ID
        };
        self.state.code_graph.relations.push(contains_relation);

        // add this state management if recursing into the children of the const node, which
        // should... only happen if we are parding `syn::Expr`?
        // self.state.current_primary_defn_scope.push(const_id);
        // Continue visiting
        visit::visit_item_const(self, item_const);
        // pop parent id onto stack, appropriate state management
        // self.state.current_primary_defn_scope.pop();
    }

    // Visit static items
    fn visit_item_static(&mut self, item_static: &'ast syn::ItemStatic) {
        #[cfg(feature = "cfg_eval")]
        {
            use crate::parser::visitor::attribute_processing::should_include_item;
            let active_cfg = &self.state.active_cfg;

            if !should_include_item(&item_static.attrs, active_cfg) {
                return; // Skip this item due to cfg
            }
        }
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
        let (static_any_id, parent_mod_id) = registration_result.unwrap();

        self.debug_new_id(&static_name, static_any_id); // Now uses trace!

        let span = item_static.extract_span_bytes();

        // Process the type (no need to push/pop scope)
        let type_id = get_or_create_type(self.state, &item_static.ty);

        // Extract the value expression as a string
        let value = Some(item_static.expr.to_token_stream().to_string());

        // Extract doc comments and other attributes
        let docstring = extract_docstring(&item_static.attrs);
        let attributes = extract_attributes(&item_static.attrs);

        // Convert static ID
        let typed_static_id: StaticNodeId = static_any_id.try_into().unwrap();

        // Construct StaticNode directly
        let static_node = StaticNode {
            id: typed_static_id, // Use typed ID
            name: static_name,
            span,
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

        // Add the static node to the graph
        self.state.code_graph.statics.push(static_node);

        // Add the Contains relation
        let contains_relation = SyntacticRelation::Contains {
            source: parent_mod_id,
            target: PrimaryNodeId::from(typed_static_id), // Use typed static ID
        };
        self.state.code_graph.relations.push(contains_relation);

        // Continue visiting
        // add this state management if recursing into the children of the const node, which
        // should... only happen if we are parding `syn::Expr`?
        // push parent id onto stack for type processing
        // self.state.current_primary_defn_scope.push(static_id);
        visit::visit_item_static(self, item_static);
        // pop parent id onto stack, appropriate state management
        // self.state.current_primary_defn_scope.pop();
    }

    // Visit macro definitions (macro_rules!)
    fn visit_item_macro(&mut self, item_macro: &'ast syn::ItemMacro) {
        if item_macro.ident.as_ref().is_none() {
            return;
        }
        #[cfg(feature = "cfg_eval")]
        {
            use crate::parser::visitor::attribute_processing::should_include_item;
            let active_cfg = &self.state.active_cfg;

            if !should_include_item(&item_macro.attrs, active_cfg) {
                return; // Skip this item due to cfg
            }
        }
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
        let (macro_any_id, parent_mod_id) = registration_result.unwrap();

        self.debug_new_id(&macro_name, macro_any_id); // Now uses trace!

        let span = item_macro.extract_span_bytes();

        let body = Some(item_macro.mac.tokens.to_string());
        let docstring = extract_docstring(&item_macro.attrs);
        let attributes = extract_attributes(&item_macro.attrs);

        // Convert macro ID
        let typed_macro_id: crate::parser::nodes::MacroNodeId = macro_any_id.try_into().unwrap();

        // Construct MacroNode directly
        let macro_node = MacroNode {
            id: typed_macro_id, // Use typed ID
            name: macro_name,
            span,
            visibility,
            kind: MacroKind::DeclarativeMacro,
            attributes,
            docstring,
            body,
            tracking_hash: Some(
                self.state
                    .generate_tracking_hash(&item_macro.to_token_stream()),
            ),
            cfgs: item_cfgs,
        };

        // Add the macro node to the graph
        self.state.code_graph.macros.push(macro_node);

        // Add the Contains relation
        let contains_relation = SyntacticRelation::Contains {
            source: parent_mod_id,
            target: PrimaryNodeId::from(typed_macro_id), // Use typed macro ID
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
    //     let self_type_str = item_impl.self_ty.to_token_stream().to_string();
    //     let trait_str = item_impl
    //         .trait_
    //         .as_ref()
    //         .map(|(_, path, _)| path.to_token_stream().to_string());
    //     let impl_generics_str = item_impl.generics.to_token_stream().to_string();
    //
    //     match trait_str {
    //         Some(t) => format!("impl {} for {}", t, self_type_str),
    //         None => format!("impl {}", self_type_str),
    //     }
    // }
    let self_type_str = type_to_string(&item_impl.self_ty);

    // Get the impl's own generics (e.g., <T: Debug> in impl<T: Debug> MyType<T>)
    let impl_generics_str = format_generics_for_name(&item_impl.generics);

    let mut name_parts = vec!["impl".to_string()];

    if !impl_generics_str.is_empty() {
        name_parts.push(impl_generics_str);
    }

    if let Some((_, trait_path, _)) = &item_impl.trait_ {
        let trait_str = trait_path.to_token_stream().to_string();
        let normalized_trait_str = trait_str
            .split_whitespace()
            .collect::<Vec<&str>>()
            .join(" ");
        name_parts.push(normalized_trait_str);
        name_parts.push("for".to_string());
    }

    name_parts.push(self_type_str);

    name_parts.join(" ")
}
// Helper to format generics (params and where clause) into a canonical string
fn format_generics_for_name(generics: &syn::Generics) -> String {
    let mut parts = Vec::new();

    if !generics.params.is_empty() {
        let params_str = generics
            .params
            .iter()
            .map(|p| {
                let s = p.to_token_stream().to_string();
                s.split_whitespace().collect::<Vec<&str>>().join(" ")
            })
            .collect::<Vec<_>>()
            .join(", ");
        parts.push(format!("<{}>", params_str));
    }

    if let Some(where_clause) = &generics.where_clause {
        let s = where_clause.to_token_stream().to_string();
        let where_str = s.split_whitespace().collect::<Vec<&str>>().join(" ");
        parts.push(where_str);
    }
    parts.join(" ")
}
// Helper to get a simplified string for a type, trying to resolve "Self" if possible
// This is a conceptual helper; actual resolution of "Self" is complex and
// might not be fully possible at this stage without more context.
// For now, we'll rely on what syn gives us for self_ty.
fn type_to_string(ty: &Type) -> String {
    // Normalize whitespace and remove extra spaces from token stream
    let s = ty.to_token_stream().to_string();
    s.split_whitespace().collect::<Vec<&str>>().join(" ")
}
