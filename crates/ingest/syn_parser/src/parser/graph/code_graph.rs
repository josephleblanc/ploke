use serde::{Deserialize, Serialize};

use crate::error::SynParserError;
use crate::parser::nodes::*;

use crate::parser::{
    // Updated node types
    nodes::{
        ConstNode, FunctionNode, ImplNode, ImportNode, MacroNode, MethodNode, ModuleNode,
        StaticNode, TraitNode, TypeDefNode,
    },
    relations::SyntacticRelation, // Use new relation enum
    types::TypeNode,
};

use super::GraphAccess;

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
    // Relations between nodes
    pub relations: Vec<SyntacticRelation>, // Updated type
    // Modules defined in the code
    pub modules: Vec<ModuleNode>,
    // Constants defined in the code
    pub consts: Vec<ConstNode>, // Added
    // Static variables defined in the code
    pub statics: Vec<StaticNode>, // Added
    // Macros defined in the code
    pub macros: Vec<MacroNode>,
    pub use_statements: Vec<ImportNode>,
}

impl GraphAccess for CodeGraph {
    fn functions(&self) -> &[FunctionNode] {
        &self.functions
    }

    fn defined_types(&self) -> &[TypeDefNode] {
        &self.defined_types
    }

    fn type_graph(&self) -> &[TypeNode] {
        &self.type_graph
    }

    fn impls(&self) -> &[ImplNode] {
        &self.impls
    }

    fn traits(&self) -> &[TraitNode] {
        &self.traits
    }

    fn relations(&self) -> &[SyntacticRelation] { // Updated type
        &self.relations
    }

    fn modules(&self) -> &[ModuleNode] {
        &self.modules
    }

    // Removed values()
    fn consts(&self) -> &[ConstNode] { // Added
        &self.consts
    }

    fn statics(&self) -> &[StaticNode] { // Added
        &self.statics
    }

    fn macros(&self) -> &[MacroNode] {
        &self.macros
    }

    fn use_statements(&self) -> &[ImportNode] {
        &self.use_statements
    }

    // --- Mutable Accessors ---

    fn functions_mut(&mut self) -> &mut Vec<FunctionNode> {
        &mut self.functions
    }

    fn defined_types_mut(&mut self) -> &mut Vec<TypeDefNode> {
        &mut self.defined_types
    }

    fn type_graph_mut(&mut self) -> &mut Vec<TypeNode> {
        &mut self.type_graph
    }

    fn impls_mut(&mut self) -> &mut Vec<ImplNode> {
        &mut self.impls
    }

    fn traits_mut(&mut self) -> &mut Vec<TraitNode> {
        &mut self.traits
    }

    fn relations_mut(&mut self) -> &mut Vec<SyntacticRelation> { // Updated type
        &mut self.relations
    }

    fn modules_mut(&mut self) -> &mut Vec<ModuleNode> {
        &mut self.modules
    }

    // Removed values_mut()
    fn consts_mut(&mut self) -> &mut Vec<ConstNode> { // Added
        &mut self.consts
    }

    fn statics_mut(&mut self) -> &mut Vec<StaticNode> { // Added
        &mut self.statics
    }

    fn macros_mut(&mut self) -> &mut Vec<MacroNode> {
        &mut self.macros
    }

    fn use_statements_mut(&mut self) -> &mut Vec<ImportNode> {
        &mut self.use_statements
    }
}

impl CodeGraph {
    pub fn merge_new(mut graphs: Vec<Self>) -> Result<Self, Box<SynParserError>> {
        let mut new_graph = graphs.pop().ok_or(SynParserError::MergeRequiresInput)?;
        for graph in graphs {
            new_graph.append_all(graph)?;
        }

        Ok(new_graph)
    }

    fn append_all(&mut self, mut other: Self) -> Result<(), Box<SynParserError>> {
        self.functions.append(&mut other.functions);
        self.defined_types.append(&mut other.defined_types);
        self.type_graph.append(&mut other.type_graph);
        self.impls.append(&mut other.impls);
        self.traits.append(&mut other.traits);
        self.relations.append(&mut other.relations);
        self.modules.append(&mut other.modules);
        self.consts.append(&mut other.consts); // Added
        self.statics.append(&mut other.statics); // Added
        // Removed values append
        self.macros.append(&mut other.macros);
        self.use_statements.append(&mut other.use_statements);
        Ok(())
    }
}
