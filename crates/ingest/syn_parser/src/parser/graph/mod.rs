use crate::parser::nodes::AsAnyNodeId;
mod code_graph;
mod parsed_graph;

use std::collections::HashMap;

use crate::utils::logging::{LOG_TARGET_GRAPH_FIND, LOG_TARGET_NODE};

pub use code_graph::CodeGraph;
use colored::Colorize;
use itertools::Itertools;
use log::{debug, trace};
pub use parsed_graph::ParsedCodeGraph;
pub use parsed_graph::ParsedGraphError;

use crate::discovery::CrateContext;
use crate::error::SynParserError;
use crate::parser::nodes::*;
use crate::resolve::module_tree;
use crate::resolve::module_tree::ModuleTree;
use crate::utils::{LogStyle, LogStyleDebug};
use ploke_core::{ItemKind, TypeId, TypeKind};
use serde::Deserialize;
use uuid::Uuid;

use crate::parser::{
    nodes::{
        ConstNode, FunctionNode, ImplNode, ImportNode, MacroNode, MethodNode, ModuleNode,
        StaticNode, TraitNode, TypeDefNode,
    }, // Updated node types
    relations::SyntacticRelation, // Use new relation enum
    types::TypeNode,
};

use super::types::VisibilityKind;

pub trait GraphAccess {
    fn functions(&self) -> &[FunctionNode]; // Standalone functions
    fn defined_types(&self) -> &[TypeDefNode];
    fn type_graph(&self) -> &[TypeNode];
    fn impls(&self) -> &[ImplNode];
    fn traits(&self) -> &[TraitNode];
    fn relations(&self) -> &[SyntacticRelation]; // Updated type
    fn modules(&self) -> &[ModuleNode];
    fn consts(&self) -> &[ConstNode]; // Added
    fn statics(&self) -> &[StaticNode]; // Added
    // Removed values()
    fn macros(&self) -> &[MacroNode];
    fn use_statements(&self) -> &[ImportNode];

    fn functions_mut(&mut self) -> &mut Vec<FunctionNode>; // Standalone functions
    fn defined_types_mut(&mut self) -> &mut Vec<TypeDefNode>;
    fn type_graph_mut(&mut self) -> &mut Vec<TypeNode>;
    fn impls_mut(&mut self) -> &mut Vec<ImplNode>;
    fn traits_mut(&mut self) -> &mut Vec<TraitNode>;
    fn relations_mut(&mut self) -> &mut Vec<SyntacticRelation>; // Updated type
    fn modules_mut(&mut self) -> &mut Vec<ModuleNode>;
    fn consts_mut(&mut self) -> &mut Vec<ConstNode>; // Added
    fn statics_mut(&mut self) -> &mut Vec<StaticNode>; // Added
    // Removed values_mut()
    fn macros_mut(&mut self) -> &mut Vec<MacroNode>;
    fn use_statements_mut(&mut self) -> &mut Vec<ImportNode>;

    // ----- Secondary node methods -----
    fn find_methods_in_module(
        &self,
        module_id: ModuleNodeId,
    ) -> impl Iterator<Item = MethodNodeId> + '_ {
        // First get impls in module
        let impl_ids = self
            .relations()
            .iter()
            .filter_map(move |rel| rel.contains_target(module_id));

        // Then find methods in each impl
        impl_ids.flat_map(move |impl_id| {
            self.relations().iter().filter_map(move |rel| match rel {
                SyntacticRelation::ImplAssociatedItem { source, target } if *source == impl_id => {
                    (*target).try_into().ok()
                }
                _ => None,
            })
        })
    }

    fn validate_unique_rels(&self) -> bool {
        let rels: &[SyntacticRelation] = &self.relations(); // Use updated type
        let mut dups = Vec::new();
        let unique_rels = rels.iter().fold(Vec::new(), |mut acc, rel| {
            if !acc.contains(rel) {
                acc.push(*rel);
            } else {
                dups.push(*rel);
            }
            acc
        });
        for dup in &dups {

            debug!("{:#?}", dup);
            let target = self.find_node_unique(dup.target()).unwrap();
            let source = self.find_node_unique(dup.source()).unwrap();
            target.log_node_debug();
            source.log_node_debug();
            if let Some(m_target) = target.as_module() {
                debug!("{:#?}", m_target);
                for (i, node ) in self.modules().iter().filter(|m| m.path == m_target.path ).enumerate() {
                    debug!("{}: {} | {}", "Find by Path:".log_header(), i, node.name());
                }
            }
            if let Some(module_source_node) = source.as_module() {
                debug!("{:#?}", module_source_node);
            }
            for (i, module ) in self.modules().iter().filter(|m| m.any_id() == source.any_id()).enumerate() {
                debug!("{}: {} | {}", "Counting source:".log_header(), i, module.name());
            }


            for (i, module ) in self.modules().iter().filter(|m| m.id.as_any() == target.any_id()).enumerate() {
                debug!("{}: {} | {}", "Counting target:".log_header(), i, module.name());
            }

        }
        unique_rels.len() == rels.len()
    }

    fn debug_relationships(&self) {
        let unique_rels = self.relations().iter().fold(Vec::new(), |mut acc, r| {
            if !acc.contains(r) {
                acc.push(*r)
            }
            acc
        });
        let has_duplicate = unique_rels.len() == self.relations().len();
        log::debug!(target: "temp",
            "{} {} {}: {} | {}: {} | {}: {}",
            "Relations are unique?".log_header(),
            if has_duplicate {
                "Yes!".log_spring_green().bold()
            } else {
                "NOOOO".log_error()
            },
            "Unique".log_step(),
            unique_rels.len().to_string().log_magenta_debug(),
            "Total".log_step(),
            self.relations().len().to_string().log_magenta_debug(),
            "Difference".log_step(),
            (self.relations().len() - unique_rels.len() ).to_string().log_magenta_debug(),
        );
        let rel_map: HashMap<SyntacticRelation, usize> = // Use updated type
            self
                .relations()
                .iter()
                .copied()
                .fold(HashMap::new(), |mut hmap, r| {
                    match hmap.entry(r) {
                        std::collections::hash_map::Entry::Occupied(mut occupied_entry) => {
                            let existing_count = occupied_entry.get();
                            occupied_entry.insert(existing_count + 1);
                        }
                        std::collections::hash_map::Entry::Vacant(vacant_entry) => {
                            vacant_entry.insert(1);
                        }
                    };
                    hmap
                });
        for (rel, count) in rel_map {
            if count > 1 {
                // Use helper methods to get base NodeIds
                log::debug!(target: "temp",
                    "{} | {}: {} | {} -> {} | {:?}", // Log the full relation variant
                    "Duplicate!".log_header(),
                    "Count".log_step(),
                    count.to_string().log_error(),
                    rel.source().to_string().log_id(),
                    rel.target().to_string().log_id(),
                    rel, // Log the specific variant
                );
            }
        }
    }


    #[cfg(feature = "type_bearing_ids")]
    fn get_child_modules(&self, module_id: ModuleNodeId) -> impl Iterator< Item = &ModuleNode >  {
        use itertools::Itertools;

        let mut ids = self.ids_contained_by(module_id);
        self.modules().iter().filter(move |m| ids.contains(&m.id.to_pid())).into_iter()
    }
    #[cfg(feature = "type_bearing_ids")]
    fn ids_contained_by(&self, module_id: ModuleNodeId) -> impl Iterator<Item = PrimaryNodeId>  {
        self.relations()
            .iter()
            .filter_map(move |rel| rel.contains_target(module_id))
    }

    fn get_child_modules_inline(&self, module_id: ModuleNodeId) -> impl Iterator<Item = &ModuleNode> {

        self.get_child_modules(module_id)
            .filter(|m| matches!(m.module_def, ModuleKind::Inline { .. }))
    }

    fn get_child_modules_decl(&self, module_id: ModuleNodeId) -> impl Iterator<Item = &ModuleNode> {
        self.get_child_modules(module_id)
            .filter(|m| matches!(m.module_def, ModuleKind::Declaration { .. }))
    }

    /// Finds a module node by its full path.
    fn find_module_by_path_unchecked(&self, path: &[String]) -> Option<&ModuleNode> {
        self.modules().iter().find(|m| m.path == path)
    }
    /// Finds a module node by its full path, returning an error if not found or if duplicates exist.
    ///
    /// Iterates through the modules, collects all matching `ModuleNode`s based on the path,
    /// and returns:
    /// - `Ok(&ModuleNode)` if exactly one match is found.
    /// - `Err(SynParserError::ModulePathNotFound)` if no matches are found.
    /// - `Err(SynParserError::DuplicateModulePath)` if more than one match is found.
    fn find_module_by_path_checked(&self, path: &[String]) -> Result<&ModuleNode, SynParserError> {
        let mut matches = self.modules().iter().filter(|m| m.path == path);
        let first = matches.next();
        if matches.next().is_some() {
            // Convert path slice to Vec<String> for the error variant
            return Err(SynParserError::DuplicateModulePath(path.to_vec()));
        }
        first.ok_or_else(|| SynParserError::ModulePathNotFound(path.to_vec()))
    }

    /// Finds a module node by its definition path (e.g., ["crate", "module", "submodule"]),
    /// Finds a module node *definition* (FileBased or Inline) by its definition path,
    /// returning an error if not found or if duplicates exist.
    /// Excludes ModuleKind::Declaration nodes.
    fn find_module_by_defn_path_checked(
        &self,
        defn_path: &[String],
    ) -> Result<&ModuleNode, SynParserError> {
        debug!(target: LOG_TARGET_GRAPH_FIND, "{} {}", "Searching for defn_path:".cyan(), defn_path.join("::").yellow());
        let matching_nodes: Vec<&ModuleNode> = self // Find ALL nodes matching path first
            .modules()
            .iter()
            .filter(|m| m.defn_path() == defn_path)
            .collect();

        debug!(target: LOG_TARGET_GRAPH_FIND, "Found {} nodes matching path:", matching_nodes.len().to_string().green());
        for node in &matching_nodes {
            let def_type = if node.is_declaration() {
                "Decl".red()
            } else {
                "Def".green()
            };
            // Moved comment outside the format string literal
            debug!(target: LOG_TARGET_GRAPH_FIND,
                "  - {}: {} | {}: {} | Path: {} | Def: {}",
                "ID".bold(), node.id.to_string().magenta(),
                "Name".bold(), node.name.yellow(),
                node.path.join("::").blue(),
                def_type // Simplified Def output for brevity
            );
        }

        // Now filter the collected nodes
        let mut non_decl_matches = matching_nodes
            .into_iter() // Iterate over the collected matches
            .filter(|m| !m.is_declaration()); // Apply the filter

        let first = non_decl_matches.next();
        let second = non_decl_matches.next(); // Check if there's a second match *after* filtering

        if second.is_some() {
            // If second exists, there was a duplicate *after* filtering
            debug!(target: LOG_TARGET_GRAPH_FIND, "{}", "Found duplicate non-declaration nodes!".red().bold());
            // Collect all non-declaration matches again for error reporting (slightly inefficient but clear)
            let all_matches: Vec<_> = self
                .modules()
                .iter()
                .filter(|m| m.defn_path() == defn_path && !m.is_declaration())
                .collect();
            // Log only the IDs of duplicates for brevity
            let duplicate_ids: Vec<String> = all_matches.iter().map(|m| m.id.to_string()).collect();
            debug!(target: LOG_TARGET_GRAPH_FIND,
                "Duplicate non-declaration modules found for path {}: [{}]",
                defn_path.join("::").yellow(), duplicate_ids.join(", ").magenta()
            );
            return Err(SynParserError::DuplicateModulePath(defn_path.to_vec()));
        }

        match first {
            Some(node) => {
                debug!(target: LOG_TARGET_GRAPH_FIND, 
                    "{} {}", 
                    "Found unique non-declaration node:".log_header(), 
                    node.id.to_string().magenta());
                node.log_node_debug();
            }
            None => {
                debug!(target: LOG_TARGET_GRAPH_FIND, "{}", "No non-declaration node found!".yellow())
            }
        }

        first.ok_or_else(|| SynParserError::ModulePathNotFound(defn_path.to_vec()))
    }

    fn get_item_module_path(&self, item_id: PrimaryNodeId) -> Vec<String> {
        // Find the module that contains this item
        let containing_relation = self
            .relations()
            .iter()
            .find(|rel| rel.source_contains(item_id).is_some());

        if let Some(SyntacticRelation::Contains { source, .. }) = containing_relation {
            self.modules()
                .iter()
                .filter(|m| m.is_file_based() || m.is_inline())
                .find(|m| m.id == *source) // Compare NodeId == NodeId
                .map(|m| m.path.clone())
                // NOTE: Might have a problem here regarding the root directory, which should be
                // the only directory without a parent module. However, we don't want to just
                // assume that the only module without a parent directory is the root. See if this
                // lead to errors, revisit later.
                .expect("Invalid state: Primary Node not contained by Module") // Should not happen if relation exists
        } else {
            // Item not in any module (crate root) or source wasn't a Node
            vec!["crate".to_string()];
            // WARNING: Intentional panic! temporary for testing. DO NOT REMOVE UNTIL THIS CAUSES
            // RUNTIME ERROR.
            panic!(
                "Adding a panic here since this should never happen. We'll likely change this to an
            `Err` and have this function return a Result, but I want to see if we encounter any
            occurrances of this, and if passing an error is the right way to handle it."
            );
        }
    }
    /// Finds a module node by its file path relative to the crate root,
    /// returning an error if not found or if duplicates exist.
    ///
    /// Iterates through the modules, collects all matching `ModuleNode`s based on the file path,
    /// and returns:
    /// - `Ok(&ModuleNode)` if exactly one match is found.
    /// - `Err(SynParserError::NotFound)` if no matches are found (using a generic NotFound for file paths).
    /// - `Err(SynParserError::DuplicateNode)` if more than one match is found (using DuplicateNode as path isn't the primary ID).
    fn find_module_by_file_path_checked(
        &self,
        relative_file_path: &std::path::Path,
    ) -> Result<&ModuleNode, SynParserError> {
        let mut matches = self.modules().iter().filter(|m| {
            m.file_path()
                .map(|fp| fp.ends_with(relative_file_path))
                .unwrap_or(false)
        });
        let first = matches.next();
        if let Some(_second) = matches.next() {
            // If duplicates found, return DuplicateNode error using the ID of the first match
            return Err(SynParserError::DuplicateNode(first.unwrap().any_id()));
        }
        // If only one or zero found, proceed.
        first.ok_or_else(|| {
            SynParserError::InternalState(format!(
                "ModuleNode with file path ending in '{}' not found.",
                relative_file_path.display()
            ))
        })
    }


    fn resolve_type(&self, type_id: TypeId) -> Option<&TypeNode> {
        self.type_graph().iter().find(|t| t.id == type_id)
    }

    fn get_type_kind(&self, type_id: TypeId) -> Option<&TypeKind> {
        self.resolve_type(type_id).map(|t| &t.kind)
    }

    /// Finds a struct node by its ID.
    ///
    /// Iterates through the defined types and returns a reference to the
    /// `StructNode` if a matching `TypeDefNode::Struct` is found.
    fn get_struct_unchecked(&self, id: StructNodeId) -> Option<&StructNode> {
        self.defined_types().iter().find_map(|def| match def {
            TypeDefNode::Struct(s) if s.id == id => Some(s), // Compare StructNodeId == StructNodeId
            _ => None,
        })
    }

    /// Finds a struct node by its ID, returning an error if not found or if duplicates exist.
    ///
    /// Iterates through the defined types, collects all matching `StructNode`s,
    /// and returns:
    /// - `Ok(&StructNode)` if exactly one match is found.
    /// - `Err(SynParserError::NotFound)` if no matches are found.
    /// - `Err(SynParserError::DuplicateNode)` if more than one match is found.
    fn get_struct_checked(&self, id: StructNodeId) -> Result<&StructNode, SynParserError> {
        let mut matches = self.defined_types().iter().filter_map(|def| match def {
            TypeDefNode::Struct(s) if s.id == id => Some(s), // Compare StructNodeId == StructNodeId
            _ => None,
        });
        let first = matches.next();
        if matches.next().is_some() {
            return Err(SynParserError::DuplicateNode(id.as_any()));
        }
        first.ok_or(SynParserError::NotFound(id.as_any()))
    }

    /// Finds an enum node by its ID.
    ///
    /// Iterates through the defined types and returns a reference to the
    /// `EnumNode` if a matching `TypeDefNode::Enum` is found.
    fn get_enum_unchecked(&self, id: EnumNodeId) -> Option<&EnumNode> {
        self.defined_types().iter().find_map(|def| match def {
            TypeDefNode::Enum(e) if e.id == id => Some(e), // Compare EnumNodeId == EnumNodeId
            _ => None,
        })
    }

    /// Finds an enum node by its ID, returning an error if not found or if duplicates exist.
    ///
    /// Iterates through the defined types, collects all matching `EnumNode`s,
    /// and returns:
    /// - `Ok(&EnumNode)` if exactly one match is found.
    /// - `Err(SynParserError::NotFound)` if no matches are found.
    /// - `Err(SynParserError::DuplicateNode)` if more than one match is found.
    fn get_enum_checked(&self, id: EnumNodeId) -> Result<&EnumNode, SynParserError> {
        let mut matches = self.defined_types().iter().filter_map(|def| match def {
            TypeDefNode::Enum(e) if e.id == id => Some(e), // Compare EnumNodeId == EnumNodeId
            _ => None,
        });
        let first = matches.next();
        if matches.next().is_some() {
            return Err(SynParserError::DuplicateNode(id.as_any()));
        }
        first.ok_or(SynParserError::NotFound(id.as_any()))
    }

    /// Finds a type alias node by its ID.
    ///
    /// Iterates through the defined types and returns a reference to the
    /// `TypeAliasNode` if a matching `TypeDefNode::TypeAlias` is found.
    fn get_type_alias_unchecked(&self, id: TypeAliasNodeId) -> Option<&TypeAliasNode> {
        self.defined_types().iter().find_map(|def| match def {
            TypeDefNode::TypeAlias(t) if t.id == id => Some(t), // Compare TypeAliasNodeId == TypeAliasNodeId
            _ => None,
        })
    }

    /// Finds a type alias node by its ID, returning an error if not found or if duplicates exist.
    ///
    /// Iterates through the defined types, collects all matching `TypeAliasNode`s,
    /// and returns:
    /// - `Ok(&TypeAliasNode)` if exactly one match is found.
    /// - `Err(SynParserError::NotFound)` if no matches are found.
    /// - `Err(SynParserError::DuplicateNode)` if more than one match is found.
    fn get_type_alias_checked(&self, id: TypeAliasNodeId) -> Result<&TypeAliasNode, SynParserError> {
        let mut matches = self.defined_types().iter().filter_map(|def| match def {
            TypeDefNode::TypeAlias(t) if t.id == id => Some(t), // Compare TypeAliasNodeId == TypeAliasNodeId
            _ => None,
        });
        let first = matches.next();
        if matches.next().is_some() {
            return Err(SynParserError::DuplicateNode(id.as_any()));
        }
        first.ok_or(SynParserError::NotFound(id.as_any()))
    }

    /// Finds a union node by its ID.
    ///
    /// Iterates through the defined types and returns a reference to the
    /// `UnionNode` if a matching `TypeDefNode::Union` is found.
    fn get_union(&self, id: UnionNodeId) -> Option<&UnionNode> {
        self.defined_types().iter().find_map(|def| match def {
            TypeDefNode::Union(u) if u.id == id => Some(u), // Compare UnionNodeId == UnionNodeId
            _ => None,
        })
    }

    /// Finds a union node by its ID, returning an error if not found or if duplicates exist.
    ///
    /// Iterates through the defined types, collects all matching `UnionNode`s,
    /// and returns:
    /// - `Ok(&UnionNode)` if exactly one match is found.
    /// - `Err(SynParserError::NotFound)` if no matches are found.
    /// - `Err(SynParserError::DuplicateNode)` if more than one match is found.
    fn get_union_checked(&self, id: UnionNodeId) -> Result<&UnionNode, SynParserError> {
        let mut matches = self.defined_types().iter().filter_map(|def| match def {
            TypeDefNode::Union(u) if u.id == id => Some(u), // Compare UnionNodeId == UnionNodeId
            _ => None,
        });
        let first = matches.next();
        if matches.next().is_some() {
            return Err(SynParserError::DuplicateNode(id.as_any()));
        }
        first.ok_or(SynParserError::NotFound(id.as_any()))
    }

    /// Gets the full module path for an item by searching through all modules
    /// Returns ["crate"] if item not found in any module (should only happ for crate root items)
    fn debug_print_all_visible(&self) {
        // New implementation using NodeId
        let mut all_ids: Vec<(&str, AnyNodeId)> = vec![];
        all_ids.extend(self.functions().iter().map(|n| (n.name(), n.id.as_any()))); // Standalone functions
        all_ids.extend(self.impls().iter().flat_map(|n| { // Methods within impls
            n.methods.iter().map(move |m| (m.name(), m.id.as_any()))
        }));
        all_ids.extend(self.traits().iter().flat_map(|n| { // Methods within traits (if stored directly)
             // Assuming TraitNode also has a 'methods' field similar to ImplNode
             // If not, adjust accordingly or remove this part if methods aren't stored on TraitNode
             n.methods.iter().map(move |m| (m.name(), m.id.as_any()))
        }));
        all_ids.extend(self.modules().iter().map(|n| (n.name(), n.id.as_any())));
        all_ids.extend(self.consts().iter().map(|n| (n.name(), n.id.as_any()))); // Use consts()
        all_ids.extend(self.statics().iter().map(|n| (n.name(), n.id.as_any()))); // Use statics()
        // Removed values()
        all_ids.extend(self.macros().iter().map(|n| (n.name(), n.id.as_any())));
        all_ids.extend(self.defined_types().iter().map(|def| match def {
            TypeDefNode::Struct(s) => (s.name(), s.id.as_any()),
            TypeDefNode::Enum(e) => (e.name(), e.id.as_any()),
            TypeDefNode::TypeAlias(a) => (a.name(), a.id.as_any()),
            TypeDefNode::Union(u) => (u.name(), u.id.as_any()),
        }));
        // Add other fields similarly...

        // NodeId enum derives Ord, so sorting should work
        all_ids.sort_by_key(|&(_, id)| id);
        for (name, id) in all_ids {
            println!("id: {:?}, name: {}", id, name); // Use Debug print for NodeId enum
        }
        // } // Removed corresponding closing brace if block was removed
    }


    fn find_containing_mod(&self, item_id: PrimaryNodeId) -> &ModuleNode {
        // Find the module that contains this item
        let module_id_maybe: Option< ModuleNodeId > = self.relations().iter()
            .find_map(|rel| rel.source_contains(item_id));

        if let Some(module_id) = module_id_maybe {
            // Get the module's path
            self.modules()
                .iter()
                .find(|m| m.id == module_id)
                .unwrap_or_else(|| panic!("No containing module found"))
        } else {
            panic!("No containing module found");
        }
    }

    // A simple wrapper around source_contains. Might remove later.
    fn find_containing_mod_id(&self, any_id: PrimaryNodeId) -> Option<ModuleNodeId> {
        self.relations().iter().find_map(|r| r.source_contains(any_id))
    }

    // Not sure how I want to deal with this. Leaving it for now. It is only being used in a couple
    // places and they all have `todo!()` on them now.
    fn find_any_node(&self, item_id: AnyNodeId) -> Option<&dyn GraphNode> {
        // Check all node collections for matching ID
        self.functions()
            .iter()
            .find(|n| n.id.as_any() == item_id)
            .map(|n| n as &dyn GraphNode)
            .or_else(|| -> Option<&dyn GraphNode> {
                self.defined_types().iter().find_map(|n| match n {
                    TypeDefNode::Struct(s) if s.id.as_any() == item_id => Some(s as &dyn GraphNode),
                    TypeDefNode::Enum(e) if e.id.as_any() == item_id => Some(e as &dyn GraphNode),
                    TypeDefNode::TypeAlias(t) if t.id.as_any() == item_id => Some(t as &dyn GraphNode),
                    TypeDefNode::Union(u) if u.id.as_any() == item_id => Some(u as &dyn GraphNode),
                    _ => None,
                })
            })
            .or_else(|| {
                self.traits()
                    .iter()
                    .find(|n| n.id.as_any() == item_id)
                    .map(|n| n as &dyn GraphNode)
            })
            .or_else(|| {
                self.modules()
                    .iter()
                    .find(|n| n.id.as_any() == item_id)
                    .map(|n| n as &dyn GraphNode)
            })
            .or_else(|| { // Search Consts
                self.consts()
                    .iter()
                    .find(|n| n.id.as_any() == item_id)
                    .map(|n| n as &dyn GraphNode)
            })
            .or_else(|| { // Search Statics
                self.statics()
                    .iter()
                    .find(|n| n.id.as_any() == item_id)
                    .map(|n| n as &dyn GraphNode)
            })
            .or_else(|| {
                self.macros()
                    .iter()
                    .find(|n| n.id.as_any() == item_id)
                    .map(|n| n as &dyn GraphNode)
            })
            .or_else(|| { // Search Methods within Impls
                self.impls().iter().find_map(|i| {
                    i.methods
                        .iter()
                        .find(|m| m.id.as_any() == item_id)
                        .map(|m| m as &dyn GraphNode) // Cast MethodNode
                })
            })
             .or_else(|| { // Search Methods within Traits (assuming similar structure)
                self.traits().iter().find_map(|t| {
                    t.methods // Assuming TraitNode has 'methods' Vec<MethodNode>
                        .iter()
                        .find(|m| m.id.as_any() == item_id)
                        .map(|m| m as &dyn GraphNode) // Cast MethodNode
                })
            })
           // --- Add ImportNode search ---
            .or_else(|| {
                self.use_statements()
                    .iter()
                    .find(|n| n.id.as_any() == item_id)
                    .map(|n| n as &dyn GraphNode)
            })
    }

    /// Finds a node by its ID, returning a `Result` with a reference to the node
    /// as a `dyn GraphNode`, or a `SynParserError::NotFound` if the node is not found.
    fn find_any_node_checked(&self, item_id: AnyNodeId) -> Result<&dyn GraphNode, SynParserError> {
        self.find_any_node(item_id)
            .ok_or(SynParserError::NotFound(item_id))
    }

    /// Finds a node by its ID across all collections, returning an error if not found or if duplicates exist.
    ///
    /// Iterates through all node collections (`functions`, `defined_types`, `traits`, `modules`, etc.),
    /// collects all matching nodes, and returns:
    /// - `Ok(&dyn GraphNode)` if exactly one match is found.
    /// - `Err(SynParserError::NotFound)` if no matches are found.
    /// - `Err(SynParserError::DuplicateNode)` if more than one match is found.
    fn find_node_unique(&self, item_id: AnyNodeId) -> Result<&dyn GraphNode, SynParserError> {
        // Chain iterators over all node collections, filter by ID, and map to &dyn GraphNode
        let mut matches_iter = self
            .functions()
            .iter()
            .filter(move |n| n.id.as_any() == item_id)
            .map(|n| n as &dyn GraphNode)
            .inspect(|n| {
        trace!(target: LOG_TARGET_GRAPH_FIND, "    Search graph for: {} ({}): {} | {}", 
            item_id.as_any().to_string().log_id(),
            n.name().log_name(),
            n.kind().log_spring_green_debug(),
            n.visibility().log_vis_debug(),
        );
            })
            .chain(self.defined_types().iter().filter_map(move |n| match n {
                TypeDefNode::Struct(s) if s.id.as_any() == item_id => Some(s as &dyn GraphNode),
                TypeDefNode::Enum(e) if e.id.as_any() == item_id => Some(e as &dyn GraphNode),
                TypeDefNode::TypeAlias(t) if t.id.as_any() == item_id => Some(t as &dyn GraphNode),
                TypeDefNode::Union(u) if u.id.as_any() == item_id => Some(u as &dyn GraphNode),
                _ => None,
            }))
            .chain(
                self.traits()
                    .iter()
                    .filter(move |n| n.id.as_any() == item_id)
                    .map(|n| n as &dyn GraphNode),
            )
            .chain(
                self.modules()
                    .iter()
                    .filter(move |n| n.id.as_any() == item_id)
                    .map(|n| n as &dyn GraphNode),
            )
            .chain( // Search Consts
                self.consts()
                    .iter()
                    .filter(move |n| n.id.as_any() == item_id)
                    .map(|n| n as &dyn GraphNode),
            )
            .chain( // Search Statics
                self.statics()
                    .iter()
                    .filter(move |n| n.id.as_any() == item_id)
                    .map(|n| n as &dyn GraphNode),
            )
            .chain(
                self.macros()
                    .iter()
                    .filter(move |n| n.id.as_any() == item_id)
                    .map(|n| n as &dyn GraphNode),
            )
             .chain(self.impls().iter().flat_map(move |i| { // Search Methods in Impls
                i.methods
                    .iter()
                    .filter(move |m| m.id.as_any() == item_id)
                    .map(|m| m as &dyn GraphNode)
            }))
             .chain(self.traits().iter().flat_map(move |t| { // Search Methods in Traits
                t.methods // Assuming TraitNode has 'methods' Vec<MethodNode>
                    .iter()
                    .filter(move |m| m.id.as_any() == item_id)
                    .map(|m| m as &dyn GraphNode)
            }))
           // --- Add ImportNode search ---
            .chain(
                self.use_statements()
                    .iter()
                    .filter(move |n| n.id.as_any() == item_id)
                    .map(|n| n as &dyn GraphNode),
            );

        // Check for uniqueness using the iterator
        let first = matches_iter.next();
        let second = matches_iter.next();


        match (first, second) {
            (Some(node), None) => Ok(node), // Exactly one match found
            (None, _) => Err(SynParserError::NotFound(item_id.as_any())), // No matches found
            (Some(_), Some(_)) => Err(SynParserError::DuplicateNode(item_id.as_any())), // More than one match found
        }
    }


    fn get_children_ids_iter<T: PrimaryNodeIdTrait>(&self, module_id: ModuleNodeId) -> impl Iterator<Item = T>  {
        self.relations()
            .iter()
            .copied()
            .filter_map(move |rel| rel.contains_target(module_id))
    }

    fn module_contains_node(&self, module_id: ModuleNodeId, item_id: PrimaryNodeId) -> bool {
        // Check if module directly contains the item
        self.ids_contained_by(module_id).contains(&item_id)
    }


    // --- FunctionNode Getters ---

    /// Finds a function node by its ID.
    fn get_function(&self, id: FunctionNodeId) -> Option<&FunctionNode> {
        self.functions().iter().find(|f| f.id == id) // Compare FunctionNodeId == FunctionNodeId
    }

    /// Finds a function node by its ID, returning an error if not found or if duplicates exist.
    fn get_function_checked(&self, id: FunctionNodeId) -> Result<&FunctionNode, SynParserError> {
        let mut matches = self.functions().iter().filter(|f| f.id == id); // Compare FunctionNodeId == FunctionNodeId
        let first = matches.next();
        if matches.next().is_some() {
            return Err(SynParserError::DuplicateNode(id.as_any()));
        }
        first.ok_or(SynParserError::NotFound(id.as_any()))
    }


    // --- ImplNode Getters ---

    /// Finds an impl node by its ID.
    fn get_impl(&self, id: ImplNodeId) -> Option<&ImplNode> {
        self.impls().iter().find(|i| i.id == id) // Compare ImplNodeId == ImplNodeId
    }

    /// Finds an impl node by its ID, returning an error if not found or if duplicates exist.
    fn get_impl_checked(&self, id: ImplNodeId) -> Result<&ImplNode, SynParserError> {
        let mut matches = self.impls().iter().filter(|i| i.id == id); // Compare ImplNodeId == ImplNodeId
        let first = matches.next();
        if matches.next().is_some() {
            return Err(SynParserError::DuplicateNode(id.as_any()));
        }
        first.ok_or(SynParserError::NotFound(id.as_any()))
    }

    // --- TraitNode Getters ---

    /// Finds a trait node by its ID, searching both public and private traits.
    fn get_trait(&self, id: TraitNodeId) -> Option<&TraitNode> {
        self.traits().iter().find(|t| t.id == id) // Compare TraitNodeId == TraitNodeId
    }

    /// Finds a trait node by its ID, searching both public and private traits,
    /// returning an error if not found or if duplicates exist across both lists.
    fn get_trait_checked(&self, id: TraitNodeId) -> Result<&TraitNode, SynParserError> {
        let mut matches = self.traits().iter().filter(|t| t.id == id); // Compare TraitNodeId == TraitNodeId
        let first = matches.next();
        if matches.next().is_some() {
            return Err(SynParserError::DuplicateNode(id.as_any()));
        }
        first.ok_or(SynParserError::NotFound(id.as_any()))
    }

    // --- ModuleNode Getters ---

    /// Finds a module node by its ID.
    fn get_module(&self, id: ModuleNodeId) -> Option<&ModuleNode> {
        self.modules().iter().find(|m| m.id == id) // Compare ModuleNodeId == ModuleNodeId
    }

    /// Finds a module node by its ID, returning an error if not found or if duplicates exist.
    fn get_module_checked(&self, id: ModuleNodeId) -> Result<&ModuleNode, SynParserError> {
        let mut matches = self.modules().iter().filter(|m| m.id == id); // Compare ModuleNodeId == ModuleNodeId
        let first = matches.next();
        if matches.next().is_some() {
            return Err(SynParserError::DuplicateNode(id.as_any()));
        }
        first.ok_or(SynParserError::NotFound(id.as_any()))
    }


    // --- ConstNode Getters ---

    /// Finds a const node by its ID.
    fn get_const(&self, id: ConstNodeId) -> Option<&ConstNode> {
        self.consts().iter().find(|c| c.id == id) // Compare ConstNodeId == ConstNodeId
    }

    /// Finds a const node by its ID, returning an error if not found or if duplicates exist.
    fn get_const_checked(&self, id: ConstNodeId) -> Result<&ConstNode, SynParserError> {
        let mut matches = self.consts().iter().filter(|c| c.id == id); // Compare ConstNodeId == ConstNodeId
        let first = matches.next();
        if matches.next().is_some() {
            return Err(SynParserError::DuplicateNode(id.as_any()));
        }
        first.ok_or(SynParserError::NotFound(id.as_any()))
    }

    // --- StaticNode Getters ---

    /// Finds a static node by its ID.
    fn get_static(&self, id: StaticNodeId) -> Option<&StaticNode> {
        self.statics().iter().find(|s| s.id == id) // Compare StaticNodeId == StaticNodeId
    }

    /// Finds a static node by its ID, returning an error if not found or if duplicates exist.
    fn get_static_checked(&self, id: StaticNodeId) -> Result<&StaticNode, SynParserError> {
        let mut matches = self.statics().iter().filter(|s| s.id == id); // Compare StaticNodeId == StaticNodeId
        let first = matches.next();
        if matches.next().is_some() {
            return Err(SynParserError::DuplicateNode(id.as_any()));
        }
        first.ok_or(SynParserError::NotFound(id.as_any()))
    }


    // --- MacroNode Getters ---

    /// Finds a macro node by its ID.
    fn get_macro(&self, id: MacroNodeId) -> Option<&MacroNode> {
        self.macros().iter().find(|m| m.id == id) // Compare MacroNodeId == MacroNodeId
    }

    /// Finds a macro node by its ID, returning an error if not found or if duplicates exist.
    fn get_macro_checked(&self, id: MacroNodeId) -> Result<&MacroNode, SynParserError> {
        let mut matches = self.macros().iter().filter(|m| m.id == id); // Compare MacroNodeId == MacroNodeId
        let first = matches.next();
        if matches.next().is_some() {
            return Err(SynParserError::DuplicateNode(id.as_any()));
        }
        first.ok_or(SynParserError::NotFound(id.as_any()))
    }

    // --- ImportNode Getters ---

    /// Finds an import node by its ID (searches `use_statements`).
    fn get_import(&self, id: ImportNodeId) -> Option<&ImportNode> {
        self.use_statements().iter().find(|u| u.id == id) // Compare ImportNodeId == ImportNodeId
    }

    /// Finds an import node by its ID (searches `use_statements`),
    /// returning an error if not found or if duplicates exist.
    fn get_import_checked(&self, id: ImportNodeId) -> Result<&ImportNode, SynParserError> {
        let mut matches = self.use_statements().iter().filter(|u| u.id == id); // Compare ImportNodeId == ImportNodeId
        let first = matches.next();
        if matches.next().is_some() {
            return Err(SynParserError::DuplicateNode(id.as_any()));
        }
        first.ok_or(SynParserError::NotFound(id.as_any()))
    }

}



/// Core trait for all graph nodes
pub trait GraphNode {
    fn any_id(&self) -> AnyNodeId;
    fn visibility(&self) -> &VisibilityKind;
    fn name(&self) -> &str;
    fn cfgs(&self) -> &[String];

    // --- Default implementations for downcasting ---
    fn as_function(&self) -> Option<&FunctionNode> {
        // Standalone function
        None
    }
    fn as_method(&self) -> Option<&MethodNode> {
        // Associated function/method
        None
    }
    fn as_struct(&self) -> Option<&StructNode> {
        None
    }
    fn as_enum(&self) -> Option<&EnumNode> {
        None
    }
    fn as_union(&self) -> Option<&UnionNode> {
        None
    }
    fn as_type_alias(&self) -> Option<&TypeAliasNode> {
        None
    }
    fn as_trait(&self) -> Option<&TraitNode> {
        None
    }
    fn as_impl(&self) -> Option<&ImplNode> {
        None
    }
    fn as_module(&self) -> Option<&ModuleNode> {
        None
    }
    fn as_const(&self) -> Option<&ConstNode> {
        // Added
        None
    }
    fn as_static(&self) -> Option<&StaticNode> {
        // Added
        None
    }
    fn as_macro(&self) -> Option<&MacroNode> {
        None
    }
    fn as_import(&self) -> Option<&ImportNode> {
        None
    }
    fn kind_matches(&self, kind: ItemKind) -> bool {
        match kind {
            ItemKind::Function => self.as_function().is_some(), // Matches standalone functions
            ItemKind::Method => self.as_method().is_some(), // Matches associated functions/methods
            ItemKind::Struct => self.as_struct().is_some(),
            ItemKind::Enum => self.as_enum().is_some(),
            ItemKind::Union => self.as_union().is_some(),
            ItemKind::TypeAlias => self.as_type_alias().is_some(),
            ItemKind::Trait => self.as_trait().is_some(),
            ItemKind::Impl => self.as_impl().is_some(),
            ItemKind::Module => self.as_module().is_some(),
            ItemKind::Const => self.as_const().is_some(), // Updated
            ItemKind::Static => self.as_static().is_some(), // Updated
            ItemKind::Macro => self.as_macro().is_some(),
            ItemKind::Import => self.as_import().is_some(),
            ItemKind::ExternCrate => {
                // kind of a hack job. needs cleaner solution
                if let Some(import_node) = self.as_import() {
                    // Use if let for safety
                    import_node.is_extern_crate()
                } else {
                    false
                }
            }

            // ItemKind::Field | ItemKind::Variant | ItemKind::GenericParam
            // are not directly represented as top-level GraphNode types this way.
            _ => false,
        }
    }

    fn kind(&self) -> ItemKind {
        // Check for Method first as it might overlap with Function if not careful
        if self.as_method().is_some() {
            ItemKind::Method // Method is more specific
        } else if self.as_function().is_some() {
            ItemKind::Function // Standalone function
        } else if self.as_struct().is_some() {
            ItemKind::Struct
        } else if self.as_enum().is_some() {
            ItemKind::Enum
        } else if self.as_union().is_some() {
            ItemKind::Union
        } else if self.as_type_alias().is_some() {
            ItemKind::TypeAlias
        } else if self.as_trait().is_some() {
            ItemKind::Trait
        } else if self.as_impl().is_some() {
            ItemKind::Impl
        } else if self.as_module().is_some() {
            ItemKind::Module
        } else if self.as_macro().is_some() {
            ItemKind::Macro
        } else if self.as_import().is_some() {
            // Check for extern crate specifically within import
            if self.kind_matches(ItemKind::ExternCrate) {
                ItemKind::ExternCrate
            } else {
                ItemKind::Import
            }
        } else if self.as_static().is_some() {
            // Updated check order
            ItemKind::Static
        } else if self.as_const().is_some() {
            // Updated check order
            ItemKind::Const
        } else {
            // This panic indicates a GraphNode implementation is missing a corresponding
            // 'as_xxx' method or the kind() logic here is incomplete.
            panic!(
                "Unknown GraphNode kind encountered. Name: {}, ID: {}",
                self.name(),
                self.any_id()
            )
        }

        // ItemKind::Field | ItemKind::Variant | ItemKind::GenericParam | ItemKind::ExternCrate
        // are not directly represented as top-level GraphNode types this way.
    }

    fn log_node_debug(&self) {
        debug!(target: LOG_TARGET_NODE,
            "{} {: <12} {: <20} | {: <12} | {: <15}",
            "NodeInfo".log_header(),
            self.name().log_name(),
            self.any_id().to_string().log_id(),
            self.kind().log_vis_debug(),
            self.visibility().log_name_debug(),
        );
    }

    fn log_node_error(&self) {
        log::error!(target: LOG_TARGET_NODE,
            "{} {} {: <12} {: <20} | {: <12} | {: <15}",
            "ERROR".log_error(),
            "NodeInfo".log_header(),
            self.name().log_name(),
            self.any_id().to_string().log_id(),
            self.kind().log_vis_debug(),
            self.visibility().log_name_debug(),
        );
    }

    // Add others like VariantNode, FieldNode if they implement GraphNode directly
}
