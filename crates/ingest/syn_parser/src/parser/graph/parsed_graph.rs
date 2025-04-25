use std::path::PathBuf;

use super::CodeGraph;

use crate::discovery::CrateContext;
use crate::error::SynParserError;
use crate::parser::nodes::*;
use crate::parser::relations::RelationKind;
use crate::resolve::module_tree;
use crate::resolve::module_tree::ModuleTree;
use crate::resolve::module_tree::ModuleTreeError;
use crate::utils::LogStyle;
use ploke_core::{NodeId, TypeId, TypeKind};
use serde::Deserialize;
use uuid::Uuid;

use crate::parser::visibility::VisibilityResult;
use crate::parser::{
    nodes::{FunctionNode, ImplNode, MacroNode, ModuleNode, TraitNode, TypeDefNode, ValueNode},
    relations::Relation,
    types::TypeNode,
};

#[derive(Debug, Deserialize, Clone)]
// AI: ParsedCodeGraph here. I want to be able to use all the methods from CodeGraph, but I don't
// know how I should implement this functionality. The most straightforward way seems to be using
// Deref, but I'm not sure that is really a clean approach. Another option would be to use a trait
// and move all the methods from the `CodeGraph` into the trait as default methods? Maybe then have
// a method that each struct would need to implement to get a reference or mutable reference to the
// underlying `CodeGraph`? I'm really not sure how to handle this.
//
// The main reason to do this is because I need more context than is immediately available to
// `CodeGraph` for the `shortest_public_path` and later, similar methods. It is awkward to manage
// things like `crate_context` between these items, and it is not ideal to need to write `.graph`
// after almost every use of the `ParsedCodeGraph`.
//
// AI?
pub struct ParsedCodeGraph {
    /// The absolute path of the file that was parsed.
    pub file_path: PathBuf,
    /// The UUID namespace of the crate this file belongs to.
    pub crate_namespace: Uuid,
    /// The resulting code graph from parsing the file.
    pub graph: CodeGraph,
    // TODO: Replace filepath above with CrateContext once I'm ready to refactor other
    // examples/tests
    //  - Option for now
    pub crate_context: Option<CrateContext>,
}
