use crate::error::SynParserError;
use crate::parser::relations::GraphId;
use ploke_core::{NodeId, TypeId, TypeKind};

use super::module_tree::ModuleTree;
use super::nodes::{
    EnumNode, GraphNode, ImportNode, ModuleDef, ModuleNodeId, StructNode, TypeAliasNode, UnionNode,
};
use super::relations::RelationKind;
use crate::parser::visibility::VisibilityResult;
use crate::parser::{
    nodes::{FunctionNode, ImplNode, MacroNode, ModuleNode, TraitNode, TypeDefNode, ValueNode},
    relations::Relation,
    types::TypeNode,
};

use serde::{Deserialize, Serialize};

// Main structure representing the entire code graph
// Derive Send and Sync automatically since all component types implement them
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct CodeGraph {
    // Functions defined in the code
    pub functions: Vec<FunctionNode>,
    // Types (structs, enums) defined in the code
    pub defined_types: Vec<TypeDefNode>,
    // All observed types, including nested and generic types
    pub type_graph: Vec<TypeNode>,
    // Implementation blocks
    pub impls: Vec<ImplNode>,
    // Public traits defined in the code
    pub traits: Vec<TraitNode>,
    // Private traits defined in the code
    pub private_traits: Vec<TraitNode>,
    // Relations between nodes
    pub relations: Vec<Relation>,
    // Modules defined in the code
    pub modules: Vec<ModuleNode>,
    // Constants and static variables
    pub values: Vec<ValueNode>,
    // Macros defined in the code
    pub macros: Vec<MacroNode>,
    pub use_statements: Vec<ImportNode>,
}

impl CodeGraph {
    // Here is the overall breakdown on what we are doing:
    // So we do a parsing using `syn` in another file, traversing the tree and processing the
    // code items into these `ModuleNode`, `FunctionNode`, `ImportNode`, etc,
    // We basically get the processed nodes by doing the following:
    //  - Phase 1: File discovery phase (implemented)
    //      - traverse directories without parsing files
    //      - get list of `.rs` files for later parsing
    //      - process file names into best guess at module path
    //  - Phase 2: Parallel parse (implemented)
    //      - parse all files in parallel to form partial code graphs.
    //          - no shared state, but initialized with file path
    //      - file-level modules get the best guess made in Phase 1 as module path
    //          - file-level modules initialized and added to state before parsing file, all direct
    //          children of file-level module are added to `ModuleNode.items` of file-level module.
    //      - generated unique Uuids for all defined items e.g. `FunctionNode`
    //          - Note: this is complex, using deterministic hashing to enable later incremental parsing,
    //          but we are more concerned with the module tree and path resolution here. Just know
    //          that we have validated all nodes to have unique IDs.
    //      - type definitions for struct, union, enum, type alias stored in nested `TypeDefNode`
    //      - all primary defined items are children of `ModuleNode`, child `NodeId` added to
    //        `ModuleNode.items`, including:
    //          - `FunctionNode`, `TraitNode`, `StructNode` etc.
    //          - `StructField`, `GenericParam`, etc
    //      - special treatment of `ImportNode`, stored as both `use_statements` in `CodeGraph` and
    //      as `ModuleNode.imports`. `ModuleNode.exports` not currently used in processing, could
    //      be good for tracking the `pub use` statements of re-exports.
    //      - `#cfg` attributes naively stored as string with spaces between,
    //          e.g. ( feature = \"feature\" )``
    //          may be processed later using `cfg_expr`.
    //      - attributes processed and stored in `attributes` field on all applicable items, e.g.
    //          `#[path = ".."]`
    //  - Phase 3: Merge and Resolution (unimplemented)
    //      - Create module tree
    //          - This may be complex, because what we need to do is be able to map every defined
    //          type, no matter the path, to two different paths:
    //              1. Canonical path, e.g.
    //                  - project_name::mod_a::mod_b::DefinedItem
    //                  - project_name::mod_a::mod_b::DefinedItem::example_method
    //              2. Shortest public path, e.g. if mod_a re-exports DefinedItem,
    //                  - project_name::mod_a::DefinedItem
    //          - Our goal is to be able to resolve all defined item paths to the same path whether
    //          the item is used in the user's crate or if the defined item is used in a
    //          dependency.
    //              - We want to allow for parsing both the users crate and optionally
    //              dependencies, and connect the usage of the code item in the user's crate to the
    //              definition of the item in the dependency.
    //          - To achieve our goal I *think* we need to have both the canonical path and
    //          shortest public path, but I'm not sure. The most important thing is that we are
    //          able to take any given defined item and resolve it to the same path for both the
    //          user's crate and dependencies if parsed separately with no communication.
    //  So I'm trying to think through what methods we will want to have available in
    //  `ModuleNode`, `ImportNode`, and `CodeGraph`, and if we want to make a new struct maybe for
    //  the tree that enables the resolution of the paths.
    // You mentioned wanting to build the graph tree from relations, but we have chosen to
    // generate the relations after we create the tree. Because we are parsing in parallel, we
    // cannot resolve all re-export targets during the parsing process. Therefore we need to
    // create the tree in order to build most relations. We are currently experimenting with
    // whether it is better to build the graph by relying on a "Relation" struct that has
    // source_id/target_id/kind like `Contains` for ModuleNode->direct children, or whether this is better
    // handled by nested data structures like ModuleNode { items: NodeId }. There are different
    // tradeoffs we would need to make regarding our parsing pipeline, but for now we are trying to
    // go with nested data structures and handle all relations at once during phase 3 resolution.
    // We will need to take the NodeId::Synthetic(Uuid) of each item and resolve it into a
    // NodeId::Resolved(Uuid), using a v5 hash. You can see a description of the overall `NodeId`
    // uuid system of generating ids in `crates/ploke-core/src/lib.rs`
    //
    // Clarifying Questions:
    //
    // 1. About path Resolution:
    //
    //  - Should we prioritize implementing canonical path resolution first, then add shortest
    //    public path logic later?
    //
    //  Yes, we can prioritize canonical path first. Also, can you please define exactly what
    //  "canonical path" means? I've been using the phrase assuming it means the path that matches
    //  the file path + inline module path, but I'm not sure that is correct.
    //
    //  - How should we handle edge cases where the canonical path differs from the shortest public
    //    path due to re-exports?
    //
    //  We want to resolve to the shortest public path. However, I would like you to check my logic
    //  on this point: Our overall goal is to be able to have different partial `CodeGraph`s of two
    //  different files resolve the same path, whether the two files are both in the user's crate,
    //  both in a dependency of the user's crate, or one file is in the user's crate and one file
    //  is in the dependency. You can assume that we have the name and version of the user's
    //  dependencies, and that the versions are the same for the parsed dependency and the version
    //  of the dependency used in the user's crate.
    //
    // 2. About Edge Cases
    //
    //  - For `#[path]` attributes, should we: a) Resolve to the actual file path, or b) Maintain
    //  the logical path?
    //
    //  We should maintain the logical path. However, because we have parsed the source code target
    //  files in parallel after a file discovery phase, each partial graph is initialized with a
    //  module that uses the "best guess" module path processed from the directory and file name of
    //  its targe. Therefore our initial merged code graph will contain only these elements. We may
    //  need to resolve them into a different logical structure for the `#[path]` attribute. Give
    //  me a quick recap on how the `#[path]` attribute works.
    //
    //  - For #[cfg], should we filter out inactive items during resolution or keep them with
    //  metadata?
    //
    //  We should parse them all together. We are inserting all elements into our database, and the
    //  database later on. We want to allow the database to query for items depending on cfg.
    //
    // 3. About Architecture:
    //
    //  - Would it make sense to create a dedicated PathResolver struct that:
    //    • Takes the ModuleTree
    //    • Processes all edge cases
    //    • Produces resolved paths
    //  • Or should this logic live directly in ModuleTree?
    //
    //  Yes, it would make sense to have a PathResolver as described. The ModuleTree should have
    //  all the data required to take any valid path for an item within the module tree path
    //  structure and let it be resolved into the desired module tree. Whether the logic that
    //  handles the edge cases lives in the module tree or elsewhere may be decided as needed for
    //  the best implementation.
    //
    // 4. About Validation:
    //    • Should we add validation to detect:
    //      • Conflicting paths after resolution? Yes.
    //      • Invalid visibility chains? Yes.
    //      • Broken re-exports? No.
    //
    //  The goal is to construct a code graph to be inserted into the database. We want to abort if
    //  invalid paths are contained. We want to avoid database corruption and fail gracefully.
    //
    // 5. About Macros:
    //    • Even if we're not handling proc macros fully, should we still:
    //      • Preserve their paths in the graph? Yes.
    //      • Mark them as "unresolved" for now? No. Maybe.

    pub fn merge_new(mut graphs: Vec<Self>) -> Result<Self, Box<SynParserError>> {
        let mut new_graph = graphs.pop().ok_or(SynParserError::MergeRequiresInput)?;
        for graph in graphs {
            new_graph.append_all(graph)?;
        }

        Ok(new_graph)
    }

    pub(crate) fn append_all(&mut self, mut other: Self) -> Result<(), Box<SynParserError>> {
        self.functions.append(&mut other.functions);
        self.defined_types.append(&mut other.defined_types);
        self.type_graph.append(&mut other.type_graph);
        self.impls.append(&mut other.impls);
        self.traits.append(&mut other.traits);
        self.private_traits.append(&mut other.private_traits);
        self.relations.append(&mut other.relations);
        self.modules.append(&mut other.modules);
        self.values.append(&mut other.values);
        self.macros.append(&mut other.macros);
        self.use_statements.append(&mut other.use_statements);
        Ok(())
    }

    pub fn build_module_tree(&self) -> Result<ModuleTree, SynParserError> {
        let root_module = self.get_root_module_checked()?;
        let mut tree = ModuleTree::new_from_root(ModuleNodeId::new(root_module.id));

        // 1: Register all modules with their containment info
        for module in &self.modules {
            tree.add_module(module.clone())?;
        }

        // 2: Process direct contains relationships between files
        tree.register_containment_batch(&self.relations)?;

        // 3: Construct relations between module declarations and definitions
        tree.build_logical_paths(&self.modules)?;

        Ok(tree)
    }

    /// Filters and allocates a new Vec for direct children of module id.
    pub fn get_child_modules(&self, module_id: NodeId) -> Vec<&ModuleNode> {
        self.relations
            .iter()
            .filter(|r| r.source == GraphId::Node(module_id) && r.kind == RelationKind::Contains)
            .filter_map(|r| match r.target {
                GraphId::Node(id) => self.get_module(id),
                _ => None,
            })
            .collect()
    }

    pub fn get_child_modules_inline(&self, module_id: NodeId) -> Vec<&ModuleNode> {
        self.get_child_modules(module_id)
            .into_iter()
            .filter(|m| matches!(m.module_def, ModuleDef::Inline { .. }))
            .collect()
    }

    pub fn get_child_modules_decl(&self, module_id: NodeId) -> Vec<&ModuleNode> {
        self.get_child_modules(module_id)
            .into_iter()
            .filter(|m| matches!(m.module_def, ModuleDef::Declaration { .. }))
            .collect()
    }

    pub fn get_root_module_checked(&self) -> Result<&ModuleNode, SynParserError> {
        self.find_module_by_path(&["crate".to_string()])
            .ok_or(SynParserError::RootModuleNotFound)
    }

    pub fn get_root_module(&self) -> Option<&ModuleNode> {
        self.find_module_by_path(&["crate".to_string()])
    }

    /// Finds a module node by its full path.
    pub fn find_module_by_path(&self, path: &[String]) -> Option<&ModuleNode> {
        self.modules.iter().find(|m| m.path == path)
    }
    /// Finds a module node by its full path, returning an error if not found or if duplicates exist.
    ///
    /// Iterates through the modules, collects all matching `ModuleNode`s based on the path,
    /// and returns:
    /// - `Ok(&ModuleNode)` if exactly one match is found.
    /// - `Err(SynParserError::ModulePathNotFound)` if no matches are found.
    /// - `Err(SynParserError::DuplicateModulePath)` if more than one match is found.
    pub fn find_module_by_path_checked(
        &self,
        path: &[String],
    ) -> Result<&ModuleNode, SynParserError> {
        let mut matches = self.modules.iter().filter(|m| m.path == path);
        let first = matches.next();
        if matches.next().is_some() {
            // Convert path slice to Vec<String> for the error variant
            return Err(SynParserError::DuplicateModulePath(path.to_vec()));
        }
        first.ok_or_else(|| SynParserError::ModulePathNotFound(path.to_vec()))
    }

    /// Finds a module node by its definition path (e.g., ["crate", "module", "submodule"]),
    /// returning an error if not found or if duplicates exist.
    ///
    /// Iterates through the modules, collects all matching `ModuleNode`s based on the definition path,
    /// and returns:
    /// - `Ok(&ModuleNode)` if exactly one match is found.
    /// - `Err(SynParserError::ModulePathNotFound)` if no matches are found.
    /// - `Err(SynParserError::DuplicateModulePath)` if more than one match is found.
    pub fn find_module_by_defn_path_checked(
        &self,
        defn_path: &[String],
    ) -> Result<&ModuleNode, SynParserError> {
        let mut matches = self.modules.iter().filter(|m| m.defn_path() == defn_path);
        let first = matches.next();
        if matches.next().is_some() {
            // Convert path slice to Vec<String> for the error variant
            return Err(SynParserError::DuplicateModulePath(defn_path.to_vec()));
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
    pub fn find_module_by_file_path_checked(
        &self,
        relative_file_path: &std::path::Path,
    ) -> Result<&ModuleNode, SynParserError> {
        let mut matches = self.modules.iter().filter(|m| {
            m.file_path()
                .map(|fp| fp.ends_with(relative_file_path))
                .unwrap_or(false)
        });
        let first = matches.next();
        if let Some(_second) = matches.next() {
            // If duplicates found, return DuplicateNode error using the ID of the first match
            // as an example representative ID involved in the duplication.
            // We can safely unwrap `first` here because we know `matches.next()` returned Some.
            return Err(SynParserError::DuplicateNode(first.unwrap().id()));
        }
        // If only one or zero found, proceed.
        // Use ok_or_else to provide a more specific error if needed, but NotFound is general.
        // We need *an* ID for NotFound, so if `first` is None, we can't provide one easily.
        // Let's return a more generic error or adjust NotFound if needed.
        // For now, let's use InternalState if no matches, as it indicates a test setup issue
        // if the expected file isn't found.
        first.ok_or_else(|| {
            SynParserError::InternalState(format!(
                "ModuleNode with file path ending in '{}' not found.",
                relative_file_path.display()
            ))
        })
    }


    pub fn resolve_type(&self, type_id: TypeId) -> Option<&TypeNode> {
        self.type_graph.iter().find(|t| t.id == type_id)
    }

    pub fn get_type_kind(&self, type_id: TypeId) -> Option<&TypeKind> {
        self.resolve_type(type_id).map(|t| &t.kind)
    }

    /// Finds a struct node by its ID.
    ///
    /// Iterates through the defined types and returns a reference to the
    /// `StructNode` if a matching `TypeDefNode::Struct` is found.
    pub fn get_struct(&self, id: NodeId) -> Option<&StructNode> {
        self.defined_types.iter().find_map(|def| match def {
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
    pub fn get_struct_checked(&self, id: NodeId) -> Result<&StructNode, SynParserError> {
        let mut matches = self.defined_types.iter().filter_map(|def| match def {
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
    pub fn get_enum(&self, id: NodeId) -> Option<&EnumNode> {
        self.defined_types.iter().find_map(|def| match def {
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
    pub fn get_enum_checked(&self, id: NodeId) -> Result<&EnumNode, SynParserError> {
        let mut matches = self.defined_types.iter().filter_map(|def| match def {
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
    pub fn get_type_alias(&self, id: NodeId) -> Option<&TypeAliasNode> {
        self.defined_types.iter().find_map(|def| match def {
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
    pub fn get_type_alias_checked(&self, id: NodeId) -> Result<&TypeAliasNode, SynParserError> {
        let mut matches = self.defined_types.iter().filter_map(|def| match def {
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
    pub fn get_union(&self, id: NodeId) -> Option<&UnionNode> {
        self.defined_types.iter().find_map(|def| match def {
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
    pub fn get_union_checked(&self, id: NodeId) -> Result<&UnionNode, SynParserError> {
        let mut matches = self.defined_types.iter().filter_map(|def| match def {
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
    pub fn debug_print_all_visible(&self) {
        #[cfg(feature = "verbose_debug")]
        {
            // New implementation using NodeId enum
            let mut all_ids: Vec<(&str, NodeId)> = vec![]; // Collect NodeId enum
            all_ids.extend(self.functions.iter().map(|n| (n.name(), n.id())));
            all_ids.extend(self.impls.iter().map(|n| (n.name(), n.id())));
            all_ids.extend(self.traits.iter().map(|n| (n.name(), n.id())));
            all_ids.extend(self.private_traits.iter().map(|n| (n.name(), n.id())));
            all_ids.extend(self.modules.iter().map(|n| (n.name(), n.id())));
            all_ids.extend(self.values.iter().map(|n| (n.name(), n.id())));
            all_ids.extend(self.macros.iter().map(|n| (n.name(), n.id())));
            all_ids.extend(self.defined_types.iter().map(|def| match def {
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
        }
    }

    pub fn get_item_module_path(&self, item_id: NodeId) -> Vec<String> {
        // Find the module that contains this item
        let module_id = self
            .relations
            .iter()
            .find(|r| r.target == GraphId::Node(item_id) && r.kind == RelationKind::Contains) // Compare target with GraphId::Node
            .map(|r| r.source); // Source should be GraphId::Node(module_id)

        if let Some(GraphId::Node(mod_id)) = module_id {
            // Unwrap GraphId::Node
            // Get the module's path
            self.modules
                .iter()
                .find(|m| m.id == mod_id) // Compare NodeId == NodeId
                .map(|m| m.path.clone())
                .unwrap_or_else(|| vec!["crate".to_string()]) // Should not happen if relation exists
        } else {
            // Item not in any module (crate root) or source wasn't a Node
            vec!["crate".to_string()]
        }
    }

    pub fn get_item_module(&self, item_id: NodeId) -> &ModuleNode {
        // Find the module that contains this item
        let module_id = self
            .relations
            .iter()
            .find(|r| r.target == GraphId::Node(item_id) && r.kind == RelationKind::Contains)
            .map(|r| r.source);

        if let Some(mod_id) = module_id {
            // Get the module's path
            self.modules
                .iter()
                .find(|m| GraphId::Node(m.id) == mod_id)
                .unwrap_or_else(|| panic!("No containing module found"))
        } else {
            panic!("No containing module found");
        }
    }

    pub fn find_containing_mod_id(&self, node_id: NodeId) -> Option<NodeId> {
        self.relations
            .iter()
            .find(|m| m.target == GraphId::Node(node_id))
            .map(|r| match r.source {
                GraphId::Node(node_id) => node_id,
                GraphId::Type(_type_id) => {
                    panic!("ModuleNode should never have TypeId for containing node")
                }
            })
    }

    pub fn find_node(&self, item_id: NodeId) -> Option<&dyn GraphNode> {
        // Check all node collections for matching ID

        self.functions
            .iter()
            .find(|n| n.id == item_id)
            .map(|n| n as &dyn GraphNode)
            .or_else(|| {
                self.defined_types.iter().find_map(|n| match n {
                    TypeDefNode::Struct(s) if s.id == item_id => Some(s as &dyn GraphNode),
                    TypeDefNode::Enum(e) if e.id == item_id => Some(e as &dyn GraphNode),
                    TypeDefNode::TypeAlias(t) if t.id == item_id => Some(t as &dyn GraphNode),
                    TypeDefNode::Union(u) if u.id == item_id => Some(u as &dyn GraphNode),
                    _ => None,
                })
            })
            .or_else(|| {
                self.traits
                    .iter()
                    .chain(&self.private_traits)
                    .find(|n| n.id == item_id)
                    .map(|n| n as &dyn GraphNode)
            })
            .or_else(|| {
                self.modules
                    .iter()
                    .find(|n| n.id == item_id)
                    .map(|n| n as &dyn GraphNode)
            })
            .or_else(|| {
                self.values
                    .iter()
                    .find(|n| n.id == item_id)
                    .map(|n| n as &dyn GraphNode)
            })
            .or_else(|| {
                self.macros
                    .iter()
                    .find(|n| n.id == item_id)
                    .map(|n| n as &dyn GraphNode)
            })
            .or_else(|| {
                self.impls.iter().find_map(|i| {
                    i.methods
                        .iter()
                        .find(|n| n.id == item_id)
                        .map(|n| n as &dyn GraphNode)
                })
            })
    }

    pub fn get_nodes_by_ids(&self, ids: &[NodeId]) -> Vec<&dyn GraphNode> {
        ids.iter().filter_map(|id| self.find_node(*id)).collect()
    }

    pub fn get_children(&self, node_id: NodeId) -> Vec<&dyn GraphNode> {
        self.relations
            .iter()
            .filter(|r| r.source == GraphId::Node(node_id) && r.kind == RelationKind::Contains)
            .filter_map(|r| match r.target {
                GraphId::Node(id) => self.find_node(id),
                GraphId::Type(_) => None,
            })
            .collect()
    }

    pub fn module_contains_node(&self, module_id: NodeId, item_id: NodeId) -> bool {
        // Check if module directly contains the item
        self.modules
            .iter()
            .find(|m| m.id == module_id)
            .map(|module| module.items().is_some_and(|m| m.contains(&item_id)));

        // Check if module contains the item through nested modules
        self.relations.iter().any(|r| {
            r.source == GraphId::Node(module_id)
                && r.target == GraphId::Node(item_id)
                && r.kind == RelationKind::Contains
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
        let import_relations = self.relations.iter().filter(|r| {
            r.source == GraphId::Node(context_module_id) && r.kind == RelationKind::ModuleImports
        });

        for rel in import_relations {
            // Check if this is a glob import by looking for a module that contains the target
            let is_glob = self
                .modules
                .iter()
                .any(|m| GraphId::Node(m.id) == rel.target);

            if is_glob {
                // For glob imports, check if item is in the imported module
                match rel.target {
                    GraphId::Node(_node_id) => {
                        return VisibilityResult::Direct;
                    }
                    GraphId::Type(_type_id) => {
                        panic!("implement me!")
                    }
                }
            }
            // Direct import match
            else if rel.target == GraphId::Node(item_id) {
                return VisibilityResult::Direct;
            }
        }

        let item = match self.find_node(item_id) {
            Some(item) => item,
            None => {
                panic!("Node not in graph");
            }
        };

        // Get current module's use statements
        let current_module = self.modules.iter().find(|m| m.path == context_module);

        if let Some(module) = current_module {
            for use_stmt in &module.imports {
                // Check if use statement brings the item into scope
                if use_stmt.path.ends_with(&[item.name().to_string()]) {
                    return VisibilityResult::NeedsUse(use_stmt.path.clone());
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
    pub fn get_function(&self, id: NodeId) -> Option<&FunctionNode> {
        self.functions.iter().find(|f| f.id == id)
    }

    /// Finds a function node by its ID, returning an error if not found or if duplicates exist.
    pub fn get_function_checked(&self, id: NodeId) -> Result<&FunctionNode, SynParserError> {
        let mut matches = self.functions.iter().filter(|f| f.id == id);
        let first = matches.next();
        if matches.next().is_some() {
            return Err(SynParserError::DuplicateNode(id));
        }
        first.ok_or(SynParserError::NotFound(id))
    }

    // --- ImplNode Getters ---

    /// Finds an impl node by its ID.
    pub fn get_impl(&self, id: NodeId) -> Option<&ImplNode> {
        self.impls.iter().find(|i| i.id == id)
    }

    /// Finds an impl node by its ID, returning an error if not found or if duplicates exist.
    pub fn get_impl_checked(&self, id: NodeId) -> Result<&ImplNode, SynParserError> {
        let mut matches = self.impls.iter().filter(|i| i.id == id);
        let first = matches.next();
        if matches.next().is_some() {
            return Err(SynParserError::DuplicateNode(id));
        }
        first.ok_or(SynParserError::NotFound(id))
    }

    // --- TraitNode Getters ---

    /// Finds a trait node by its ID, searching both public and private traits.
    pub fn get_trait(&self, id: NodeId) -> Option<&TraitNode> {
        self.traits
            .iter()
            .chain(self.private_traits.iter())
            .find(|t| t.id == id)
    }

    /// Finds a trait node by its ID, searching both public and private traits,
    /// returning an error if not found or if duplicates exist across both lists.
    pub fn get_trait_checked(&self, id: NodeId) -> Result<&TraitNode, SynParserError> {
        let mut matches = self
            .traits
            .iter()
            .chain(self.private_traits.iter())
            .filter(|t| t.id == id);
        let first = matches.next();
        if matches.next().is_some() {
            return Err(SynParserError::DuplicateNode(id));
        }
        first.ok_or(SynParserError::NotFound(id))
    }

    // --- ModuleNode Getters ---

    /// Finds a module node by its ID.
    pub fn get_module(&self, id: NodeId) -> Option<&ModuleNode> {
        self.modules.iter().find(|m| m.id == id)
    }

    /// Finds a module node by its ID, returning an error if not found or if duplicates exist.
    pub fn get_module_checked(&self, id: NodeId) -> Result<&ModuleNode, SynParserError> {
        let mut matches = self.modules.iter().filter(|m| m.id == id);
        let first = matches.next();
        if matches.next().is_some() {
            return Err(SynParserError::DuplicateNode(id));
        }
        first.ok_or(SynParserError::NotFound(id))
    }

    // --- ValueNode Getters ---

    /// Finds a value node (const/static) by its ID.
    pub fn get_value(&self, id: NodeId) -> Option<&ValueNode> {
        self.values.iter().find(|v| v.id == id)
    }

    /// Finds a value node (const/static) by its ID, returning an error if not found or if duplicates exist.
    pub fn get_value_checked(&self, id: NodeId) -> Result<&ValueNode, SynParserError> {
        let mut matches = self.values.iter().filter(|v| v.id == id);
        let first = matches.next();
        if matches.next().is_some() {
            return Err(SynParserError::DuplicateNode(id));
        }
        first.ok_or(SynParserError::NotFound(id))
    }

    // --- MacroNode Getters ---

    /// Finds a macro node by its ID.
    pub fn get_macro(&self, id: NodeId) -> Option<&MacroNode> {
        self.macros.iter().find(|m| m.id == id)
    }

    /// Finds a macro node by its ID, returning an error if not found or if duplicates exist.
    pub fn get_macro_checked(&self, id: NodeId) -> Result<&MacroNode, SynParserError> {
        let mut matches = self.macros.iter().filter(|m| m.id == id);
        let first = matches.next();
        if matches.next().is_some() {
            return Err(SynParserError::DuplicateNode(id));
        }
        first.ok_or(SynParserError::NotFound(id))
    }

    // --- ImportNode Getters ---

    /// Finds an import node by its ID (searches `use_statements`).
    pub fn get_import(&self, id: NodeId) -> Option<&ImportNode> {
        self.use_statements.iter().find(|u| u.id == id)
    }

    /// Finds an import node by its ID (searches `use_statements`),
    /// returning an error if not found or if duplicates exist.
    pub fn get_import_checked(&self, id: NodeId) -> Result<&ImportNode, SynParserError> {
        let mut matches = self.use_statements.iter().filter(|u| u.id == id);
        let first = matches.next();
        if matches.next().is_some() {
            return Err(SynParserError::DuplicateNode(id));
        }
        first.ok_or(SynParserError::NotFound(id))
    }
}
