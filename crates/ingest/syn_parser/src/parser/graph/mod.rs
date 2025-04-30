mod code_graph;
mod parsed_graph;

use std::collections::HashMap;

use crate::utils::logging::LOG_TARGET_GRAPH_FIND;

pub use code_graph::CodeGraph;
use colored::Colorize;
use log::{debug, trace};
pub use parsed_graph::ParsedCodeGraph;
use petgraph::graph::Node;

use crate::discovery::CrateContext;
use crate::error::SynParserError;
use crate::parser::nodes::*;
use crate::resolve::module_tree;
use crate::resolve::module_tree::ModuleTree;
use crate::utils::{LogStyle, LogStyleDebug};
use ploke_core::{NodeId, TypeId, TypeKind};
use serde::Deserialize;
use uuid::Uuid;

use crate::parser::visibility::VisibilityResult;
use crate::parser::{
    nodes::{
        ConstNode, FunctionNode, ImplNode, ImportNode, MacroNode, MethodNode, ModuleNode,
        StaticNode, TraitNode, TypeDefNode,
    }, // Updated node types
    relations::SyntacticRelation, // Use new relation enum
    types::TypeNode,
};

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
            for (i, module ) in self.modules().iter().filter(|m| m.id() == source.id()).enumerate() {
                debug!("{}: {} | {}", "Counting source:".log_header(), i, module.name());
            }


            for (i, module ) in self.modules().iter().filter(|m| m.id() == target.id()).enumerate() {
                debug!("{}: {} | {}", "Counting target:".log_header(), i, module.name());
            }

        }
        unique_rels.len() == rels.len()
    }

fn debug_relationships(visitor: &Self) {
    let unique_rels = visitor.relations().iter().fold(Vec::new(), |mut acc, r| {
        if !acc.contains(r) {
            acc.push(*r)
        }
        acc
    });
    let has_duplicate = unique_rels.len() == visitor.relations().len();
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
        visitor.relations().len().to_string().log_magenta_debug(),
        "Difference".log_step(),
        (visitor.relations().len() - unique_rels.len() ).to_string().log_magenta_debug(),
    );
    let rel_map: HashMap<SyntacticRelation, usize> = // Use updated type
        visitor
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

    // This loop seems incorrect - it logs relations *not* in the unique list,
    // which shouldn't happen if unique_rels was derived correctly.
    // Commenting out for now, can be revisited if needed.
    // for rel in visitor.relations() {
    //     if !unique_rels.contains(rel) {
    //         log::debug!(target: "temp",
    //             "{} | {}: {} -> {} | {:?}",
    //             "Unique!".log_header(), // This log message seems misleading
    //             rel.source_node_id().to_string().log_id(),
    //             rel.target_node_id().to_string().log_id(),
    //             rel,
    //         );
    //     }
    // }
}
    fn get_root_module_checked(&self) -> Result<&ModuleNode, SynParserError> {
        self.find_module_by_path(&["crate".to_string()])
            .ok_or(SynParserError::RootModuleNotFound)
    }

    /// Filters and allocates a new Vec for direct children of module id.
    fn get_child_modules(&self, module_id: NodeId) -> Vec<&ModuleNode> {
        self.relations()
            .iter()
            .filter_map(|rel| match rel {
                // Match only Contains originating from the target module
                SyntacticRelation::Contains { source, target }
                    if source.as_inner() == &module_id =>
                {
                    // Check if the target is a Module
                    match target {
                        PrimaryNodeId::Module(target_mod_id) => {
                            self.get_module(target_mod_id.into_inner())
                        }
                        _ => None, // Target is not a module
                    }
                }
                _ => None, // Not a Contains relation from the source module
            })
            .collect()
    }

    fn get_child_modules_inline(&self, module_id: NodeId) -> Vec<&ModuleNode> {
        self.get_child_modules(module_id)
            .into_iter()
            .filter(|m| matches!(m.module_def, ModuleKind::Inline { .. }))
            .collect()
    }

    fn get_child_modules_decl(&self, module_id: NodeId) -> Vec<&ModuleNode> {
        self.get_child_modules(module_id)
            .into_iter()
            .filter(|m| matches!(m.module_def, ModuleKind::Declaration { .. }))
            .collect()
    }

    /// Finds a module node by its full path.
    fn find_module_by_path(&self, path: &[String]) -> Option<&ModuleNode> {
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
            return Err(SynParserError::DuplicateNode(first.unwrap().id()));
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
    // AI: update the rest of the methods to use the typed ids AI!
    fn get_struct(&self, id: StructNodeId) -> Option<&StructNode> {
        self.defined_types().iter().find_map(|def| match def {
            TypeDefNode::Struct(s) if s.id == id => Some(s),
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
    fn get_struct_checked(&self, id: NodeId) -> Result<&StructNode, SynParserError> {
        let mut matches = self.defined_types().iter().filter_map(|def| match def {
            TypeDefNode::Struct(s) if s.id == id => Some(s),
            _ => None,
        });
        let first = matches.next();
        if matches.next().is_some() {
            return Err(SynParserError::DuplicateNode(id));
        }
        first.ok_or(SynParserError::NotFound(id))
    }

    /// Finds an enum node by its ID.
    ///
    /// Iterates through the defined types and returns a reference to the
    /// `EnumNode` if a matching `TypeDefNode::Enum` is found.
    fn get_enum(&self, id: NodeId) -> Option<&EnumNode> {
        self.defined_types().iter().find_map(|def| match def {
            TypeDefNode::Enum(e) if e.id == id => Some(e),
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
    fn get_enum_checked(&self, id: NodeId) -> Result<&EnumNode, SynParserError> {
        let mut matches = self.defined_types().iter().filter_map(|def| match def {
            TypeDefNode::Enum(e) if e.id == id => Some(e),
            _ => None,
        });
        let first = matches.next();
        if matches.next().is_some() {
            return Err(SynParserError::DuplicateNode(id));
        }
        first.ok_or(SynParserError::NotFound(id))
    }

    /// Finds a type alias node by its ID.
    ///
    /// Iterates through the defined types and returns a reference to the
    /// `TypeAliasNode` if a matching `TypeDefNode::TypeAlias` is found.
    fn get_type_alias(&self, id: NodeId) -> Option<&TypeAliasNode> {
        self.defined_types().iter().find_map(|def| match def {
            TypeDefNode::TypeAlias(t) if t.id == id => Some(t),
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
    fn get_type_alias_checked(&self, id: NodeId) -> Result<&TypeAliasNode, SynParserError> {
        let mut matches = self.defined_types().iter().filter_map(|def| match def {
            TypeDefNode::TypeAlias(t) if t.id == id => Some(t),
            _ => None,
        });
        let first = matches.next();
        if matches.next().is_some() {
            return Err(SynParserError::DuplicateNode(id));
        }
        first.ok_or(SynParserError::NotFound(id))
    }

    /// Finds a union node by its ID.
    ///
    /// Iterates through the defined types and returns a reference to the
    /// `UnionNode` if a matching `TypeDefNode::Union` is found.
    fn get_union(&self, id: NodeId) -> Option<&UnionNode> {
        self.defined_types().iter().find_map(|def| match def {
            TypeDefNode::Union(u) if u.id == id => Some(u),
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
    fn get_union_checked(&self, id: NodeId) -> Result<&UnionNode, SynParserError> {
        let mut matches = self.defined_types().iter().filter_map(|def| match def {
            TypeDefNode::Union(u) if u.id == id => Some(u),
            _ => None,
        });
        let first = matches.next();
        if matches.next().is_some() {
            return Err(SynParserError::DuplicateNode(id));
        }
        first.ok_or(SynParserError::NotFound(id))
    }

    /// Gets the full module path for an item by searching through all modules
    /// Returns ["crate"] if item not found in any module (should only happ for crate root items)
    fn debug_print_all_visible(&self) {
        // New implementation using NodeId
        let mut all_ids: Vec<(&str, NodeId)> = vec![];
        all_ids.extend(self.functions().iter().map(|n| (n.name(), n.id()))); // Standalone functions
        all_ids.extend(self.impls().iter().flat_map(|n| { // Methods within impls
            n.methods.iter().map(move |m| (m.name(), m.id()))
        }));
        all_ids.extend(self.traits().iter().flat_map(|n| { // Methods within traits (if stored directly)
             // Assuming TraitNode also has a 'methods' field similar to ImplNode
             // If not, adjust accordingly or remove this part if methods aren't stored on TraitNode
             n.methods.iter().map(move |m| (m.name(), m.id()))
        }));
        all_ids.extend(self.modules().iter().map(|n| (n.name(), n.id())));
        all_ids.extend(self.consts().iter().map(|n| (n.name(), n.id()))); // Use consts()
        all_ids.extend(self.statics().iter().map(|n| (n.name(), n.id()))); // Use statics()
        // Removed values()
        all_ids.extend(self.macros().iter().map(|n| (n.name(), n.id())));
        all_ids.extend(self.defined_types().iter().map(|def| match def {
            TypeDefNode::Struct(s) => (s.name(), s.id()),
            TypeDefNode::Enum(e) => (e.name(), e.id()),
            TypeDefNode::TypeAlias(a) => (a.name(), a.id()),
            TypeDefNode::Union(u) => (u.name(), u.id()),
        }));
        // Add other fields similarly...

        // NodeId enum derives Ord, so sorting should work
        all_ids.sort_by_key(|&(_, id)| id);
        for (name, id) in all_ids {
            println!("id: {:?}, name: {}", id, name); // Use Debug print for NodeId enum
        }
        // } // Removed corresponding closing brace if block was removed
    }

    fn get_item_module_path(&self, item_id: NodeId) -> Vec<String> {
        // Find the module that contains this item
        let containing_relation = self.relations().iter().find(|rel| match rel {
            SyntacticRelation::Contains { target, .. } => target.base_id() == item_id,
            _ => false,
        });

        if let Some(SyntacticRelation::Contains { source, .. }) = containing_relation {
            let mod_id = source.into_inner();
            // Get the module's path
            self.modules()
                .iter()
                .find(|m| m.id == mod_id) // Compare NodeId == NodeId
                .map(|m| m.path.clone())
                .unwrap_or_else(|| vec!["crate".to_string()]) // Should not happen if relation exists
        } else {
            // Item not in any module (crate root) or source wasn't a Node
            vec!["crate".to_string()]
        }
    }

    fn get_item_module(&self, item_id: NodeId) -> &ModuleNode {
        // Find the module that contains this item
        let containing_relation = self.relations().iter().find(|rel| match rel {
            SyntacticRelation::Contains { target, .. } => target.base_id() == item_id,
            _ => false,
        });

        if let Some(SyntacticRelation::Contains { source, .. }) = containing_relation {
            let mod_id = source.into_inner();
            // Get the module's path
            self.modules()
                .iter()
                .find(|m| m.id == mod_id)
                .unwrap_or_else(|| panic!("No containing module found"))
        } else {
            panic!("No containing module found");
        }
    }

    fn find_containing_mod_id(&self, node_id: NodeId) -> Option<NodeId> {
        self.relations().iter().find_map(|rel| match rel {
            SyntacticRelation::Contains { source, target } if target.base_id() == node_id => {
                Some(source.into_inner())
            }
            _ => None,
        })
    }

    fn find_node(&self, item_id: NodeId) -> Option<&dyn GraphNode> {
        // Check all node collections for matching ID

        self.functions()
            .iter()
            .find(|n| n.id == item_id)
            .map(|n| n as &dyn GraphNode)
            .or_else(|| -> Option<&dyn GraphNode> {
                self.defined_types().iter().find_map(|n| match n {
                    TypeDefNode::Struct(s) if s.id == item_id => Some(s as &dyn GraphNode),
                    TypeDefNode::Enum(e) if e.id == item_id => Some(e as &dyn GraphNode),
                    TypeDefNode::TypeAlias(t) if t.id == item_id => Some(t as &dyn GraphNode),
                    TypeDefNode::Union(u) if u.id == item_id => Some(u as &dyn GraphNode),
                    _ => None,
                })
            })
            .or_else(|| {
                self.traits()
                    .iter()
                    .find(|n| n.id == item_id)
                    .map(|n| n as &dyn GraphNode)
            })
            .or_else(|| {
                self.modules()
                    .iter()
                    .find(|n| n.id == item_id)
                    .map(|n| n as &dyn GraphNode)
            })
            .or_else(|| { // Search Consts
                self.consts()
                    .iter()
                    .find(|n| n.id == item_id)
                    .map(|n| n as &dyn GraphNode)
            })
            .or_else(|| { // Search Statics
                self.statics()
                    .iter()
                    .find(|n| n.id == item_id)
                    .map(|n| n as &dyn GraphNode)
            })
            .or_else(|| {
                self.macros()
                    .iter()
                    .find(|n| n.id == item_id)
                    .map(|n| n as &dyn GraphNode)
            })
            .or_else(|| { // Search Methods within Impls
                self.impls().iter().find_map(|i| {
                    i.methods
                        .iter()
                        .find(|m| m.id == item_id)
                        .map(|m| m as &dyn GraphNode) // Cast MethodNode
                })
            })
             .or_else(|| { // Search Methods within Traits (assuming similar structure)
                self.traits().iter().find_map(|t| {
                    t.methods // Assuming TraitNode has 'methods' Vec<MethodNode>
                        .iter()
                        .find(|m| m.id == item_id)
                        .map(|m| m as &dyn GraphNode) // Cast MethodNode
                })
            })
           // --- Add ImportNode search ---
            .or_else(|| {
                self.use_statements()
                    .iter()
                    .find(|n| n.id == item_id)
                    .map(|n| n as &dyn GraphNode)
            })
    }

    /// Finds a node by its ID, returning a `Result` with a reference to the node
    /// as a `dyn GraphNode`, or a `SynParserError::NotFound` if the node is not found.
    fn find_node_checked(&self, item_id: NodeId) -> Result<&dyn GraphNode, SynParserError> {
        self.find_node(item_id)
            .ok_or(SynParserError::NotFound(item_id))
    }

    /// Finds a node by its ID across all collections, returning an error if not found or if duplicates exist.
    ///
    /// Iterates through all node collections (`functions`, `defined_types`, `traits`, `modules`, etc.),
    /// collects all matching nodes, and returns:
    /// - `Ok(&dyn GraphNode)` if exactly one match is found.
    /// - `Err(SynParserError::NotFound)` if no matches are found.
    /// - `Err(SynParserError::DuplicateNode)` if more than one match is found.
    fn find_node_unique(&self, item_id: NodeId) -> Result<&dyn GraphNode, SynParserError> {
        // Chain iterators over all node collections, filter by ID, and map to &dyn GraphNode
        let mut matches_iter = self
            .functions()
            .iter()
            .filter(move |n| n.id == item_id)
            .map(|n| n as &dyn GraphNode)
            .inspect(|n| {
        trace!(target: LOG_TARGET_GRAPH_FIND, "    Search graph for: {} ({}): {} | {}", 
            item_id.to_string().log_id(),
            n.name().log_name(),
            n.kind().log_spring_green_debug(),
            n.visibility().log_vis_debug(),
        );
            })
            .chain(self.defined_types().iter().filter_map(move |n| match n {
                TypeDefNode::Struct(s) if s.id == item_id => Some(s as &dyn GraphNode),
                TypeDefNode::Enum(e) if e.id == item_id => Some(e as &dyn GraphNode),
                TypeDefNode::TypeAlias(t) if t.id == item_id => Some(t as &dyn GraphNode),
                TypeDefNode::Union(u) if u.id == item_id => Some(u as &dyn GraphNode),
                _ => None,
            }))
            .chain(
                self.traits()
                    .iter()
                    .filter(move |n| n.id == item_id)
                    .map(|n| n as &dyn GraphNode),
            )
            .chain(
                self.modules()
                    .iter()
                    .filter(move |n| n.id == item_id)
                    .map(|n| n as &dyn GraphNode),
            )
            .chain( // Search Consts
                self.consts()
                    .iter()
                    .filter(move |n| n.id == item_id)
                    .map(|n| n as &dyn GraphNode),
            )
            .chain( // Search Statics
                self.statics()
                    .iter()
                    .filter(move |n| n.id == item_id)
                    .map(|n| n as &dyn GraphNode),
            )
            .chain(
                self.macros()
                    .iter()
                    .filter(move |n| n.id == item_id)
                    .map(|n| n as &dyn GraphNode),
            )
             .chain(self.impls().iter().flat_map(move |i| { // Search Methods in Impls
                i.methods
                    .iter()
                    .filter(move |m| m.id == item_id)
                    .map(|m| m as &dyn GraphNode)
            }))
             .chain(self.traits().iter().flat_map(move |t| { // Search Methods in Traits
                t.methods // Assuming TraitNode has 'methods' Vec<MethodNode>
                    .iter()
                    .filter(move |m| m.id == item_id)
                    .map(|m| m as &dyn GraphNode)
            }))
           // --- Add ImportNode search ---
            .chain(
                self.use_statements()
                    .iter()
                    .filter(move |n| n.id == item_id)
                    .map(|n| n as &dyn GraphNode),
            );

        // Check for uniqueness using the iterator
        let first = matches_iter.next();
        let second = matches_iter.next();


        match (first, second) {
            (Some(node), None) => Ok(node), // Exactly one match found
            (None, _) => Err(SynParserError::NotFound(item_id)), // No matches found
            (Some(_), Some(_)) => Err(SynParserError::DuplicateNode(item_id)), // More than one match found
        }
    }


    fn get_nodes_by_ids(&self, ids: &[NodeId]) -> Vec<&dyn GraphNode> {
        ids.iter().filter_map(|id| self.find_node(*id)).collect()
    }

    fn get_children(&self, node_id: NodeId) -> Vec<NodeId> {
        self.relations()
            .iter()
            .filter_map(|rel| match rel {
                SyntacticRelation::Contains { source, target } if source.as_inner() == &node_id => {
                    Some(target.base_id())
                }
                _ => None,
            })
            .collect()
    }

    fn module_contains_node(&self, module_id: NodeId, item_id: NodeId) -> bool {
        // Check if module directly contains the item
        self.modules()
            .iter()
            .find(|m| m.id == module_id)
            .map(|module| module.items().is_some_and(|m| m.contains(&item_id)));

        // Check if module contains the item through nested modules
        self.relations().iter().any(|rel| match rel {
            SyntacticRelation::Contains { source, target } => {
                source.as_inner() == &module_id && target.base_id() == item_id
            }
            _ => false,
        })
    }

    // TODO: Improve this. It is old code and needs to be refactored to be more idiomatic and
    // checked for correctness.
    #[allow(dead_code, reason = "Useful in upcoming uuid changes for Phase 3")]
    fn check_use_statements(&self, item_id: NodeId, context_module: &[String]) -> VisibilityResult {
        let context_module_id = match self.find_module_by_path(context_module) {
            Some(m) => m.id,
            None => {
                panic!("Trying to access another workspace.")
            }
        };

        // Get all ModuleImports relations for this context module
        let import_relations = self.relations().iter().filter_map(|rel| match rel {
            SyntacticRelation::ModuleImports { source, target }
                if source.as_inner() == &context_module_id =>
            {
                Some(target.into_inner()) // Get the ImportNodeId's base NodeId
            }
            _ => None,
        });

        // Iterate over the NodeIds of the ImportNodes
        for import_node_id in import_relations {
            // We need the ImportNode itself to check its path/kind, not just the ID here.
            // Let's adjust the logic below to work with the module's imports list directly.

            // Check if this is a glob import - This logic needs rethinking.
            // A glob import (`use some::path::*;`) doesn't have a single target ID.
            // We need to check the `ImportNode`'s kind/path.

            // Direct import match - This also needs the ImportNode's path info.
            // if import_node_id == item_id { // This comparison is likely incorrect
            //     return VisibilityResult::Direct;
            // }
        }

        let item = match self.find_node(item_id) {
            Some(item) => item,
            None => {
                panic!("Node not in graph");
            }
        };

        // Get current module's use statements
        let current_module = self.modules().iter().find(|m| m.path == context_module);

        if let Some(module) = current_module {
            for use_stmt in &module.imports {
                // Check if use statement brings the item into scope
                if use_stmt.source_path.ends_with(&[item.name().to_string()]) {
                    return VisibilityResult::NeedsUse(use_stmt.source_path.clone());
                }
            }
        }

        // Default to private if no matching use statement found
        VisibilityResult::OutOfScope {
            allowed_scopes: None,
        }
    }

    // --- FunctionNode Getters ---

    /// Finds a function node by its ID.
    fn get_function(&self, id: NodeId) -> Option<&FunctionNode> {
        self.functions().iter().find(|f| f.id == id)
    }

    /// Finds a function node by its ID, returning an error if not found or if duplicates exist.
    fn get_function_checked(&self, id: NodeId) -> Result<&FunctionNode, SynParserError> {
        let mut matches = self.functions().iter().filter(|f| f.id == id);
        let first = matches.next();
        if matches.next().is_some() {
            return Err(SynParserError::DuplicateNode(id));
        }
        first.ok_or(SynParserError::NotFound(id))
    }

    // --- ImplNode Getters ---

    /// Finds an impl node by its ID.
    fn get_impl(&self, id: NodeId) -> Option<&ImplNode> {
        self.impls().iter().find(|i| i.id == id)
    }

    /// Finds an impl node by its ID, returning an error if not found or if duplicates exist.
    fn get_impl_checked(&self, id: NodeId) -> Result<&ImplNode, SynParserError> {
        let mut matches = self.impls().iter().filter(|i| i.id == id);
        let first = matches.next();
        if matches.next().is_some() {
            return Err(SynParserError::DuplicateNode(id));
        }
        first.ok_or(SynParserError::NotFound(id))
    }

    // --- TraitNode Getters ---

    /// Finds a trait node by its ID, searching both public and private traits.
    fn get_trait(&self, id: NodeId) -> Option<&TraitNode> {
        self.traits().iter().find(|t| t.id == id)
    }

    /// Finds a trait node by its ID, searching both public and private traits,
    /// returning an error if not found or if duplicates exist across both lists.
    fn get_trait_checked(&self, id: NodeId) -> Result<&TraitNode, SynParserError> {
        let mut matches = self.traits().iter().filter(|t| t.id == id);
        let first = matches.next();
        if matches.next().is_some() {
            return Err(SynParserError::DuplicateNode(id));
        }
        first.ok_or(SynParserError::NotFound(id))
    }

    // --- ModuleNode Getters ---

    /// Finds a module node by its ID.
    fn get_module(&self, id: NodeId) -> Option<&ModuleNode> {
        self.modules().iter().find(|m| m.id == id)
    }

    /// Finds a module node by its ID, returning an error if not found or if duplicates exist.
    fn get_module_checked(&self, id: NodeId) -> Result<&ModuleNode, SynParserError> {
        let mut matches = self.modules().iter().filter(|m| m.id == id);
        let first = matches.next();
        if matches.next().is_some() {
            return Err(SynParserError::DuplicateNode(id));
        }
        first.ok_or(SynParserError::NotFound(id))
    }


    // --- ConstNode Getters ---

    /// Finds a const node by its ID.
    fn get_const(&self, id: NodeId) -> Option<&ConstNode> {
        self.consts().iter().find(|c| c.id == id)
    }

    /// Finds a const node by its ID, returning an error if not found or if duplicates exist.
    fn get_const_checked(&self, id: NodeId) -> Result<&ConstNode, SynParserError> {
        let mut matches = self.consts().iter().filter(|c| c.id == id);
        let first = matches.next();
        if matches.next().is_some() {
            return Err(SynParserError::DuplicateNode(id));
        }
        first.ok_or(SynParserError::NotFound(id))
    }

    // --- StaticNode Getters ---

    /// Finds a static node by its ID.
    fn get_static(&self, id: NodeId) -> Option<&StaticNode> {
        self.statics().iter().find(|s| s.id == id)
    }

    /// Finds a static node by its ID, returning an error if not found or if duplicates exist.
    fn get_static_checked(&self, id: NodeId) -> Result<&StaticNode, SynParserError> {
        let mut matches = self.statics().iter().filter(|s| s.id == id);
        let first = matches.next();
        if matches.next().is_some() {
            return Err(SynParserError::DuplicateNode(id));
        }
        first.ok_or(SynParserError::NotFound(id))
    }


    // --- MacroNode Getters ---

    /// Finds a macro node by its ID.
    fn get_macro(&self, id: NodeId) -> Option<&MacroNode> {
        self.macros().iter().find(|m| m.id == id)
    }

    /// Finds a macro node by its ID, returning an error if not found or if duplicates exist.
    fn get_macro_checked(&self, id: NodeId) -> Result<&MacroNode, SynParserError> {
        let mut matches = self.macros().iter().filter(|m| m.id == id);
        let first = matches.next();
        if matches.next().is_some() {
            return Err(SynParserError::DuplicateNode(id));
        }
        first.ok_or(SynParserError::NotFound(id))
    }

    // --- ImportNode Getters ---

    /// Finds an import node by its ID (searches `use_statements`).
    fn get_import(&self, id: NodeId) -> Option<&ImportNode> {
        self.use_statements().iter().find(|u| u.id == id)
    }

    /// Finds an import node by its ID (searches `use_statements`),
    /// returning an error if not found or if duplicates exist.
    fn get_import_checked(&self, id: NodeId) -> Result<&ImportNode, SynParserError> {
        let mut matches = self.use_statements().iter().filter(|u| u.id == id);
        let first = matches.next();
        if matches.next().is_some() {
            return Err(SynParserError::DuplicateNode(id));
        }
        first.ok_or(SynParserError::NotFound(id))
    }
}
