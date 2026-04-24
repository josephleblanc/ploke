//! Relation-focused helpers for paranoid integration tests.
//!
//! The larger object here is not "did some relation exist somewhere in the tree?".
//! It is "for a fixture with stable node identity, did this exact source node and this exact
//! target node participate in this exact relation variant, once and only once?".
//!
//! These helpers are intended for Phase 3 tree-relation assertions where source/target lookup
//! should be typed and provenance-preserving in the same spirit as the node paranoid suites.

use ploke_core::ItemKind;
use syn_parser::error::SynParserError;
use syn_parser::parser::ParsedCodeGraph;
use syn_parser::parser::graph::GraphAccess;
use syn_parser::parser::nodes::{AnyNodeId, ImportNodeId};
use syn_parser::parser::relations::SyntacticRelation;
use syn_parser::resolve::module_tree::ModuleTree;

use super::resolution::find_item_id_by_path_name_kind_checked;

/// Typed selector for a relation endpoint in tests.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RelationNodeSelector<'a> {
    Item {
        module_path: &'a [&'a str],
        name: &'a str,
        kind: ItemKind,
    },
    Import {
        module_path: &'a [&'a str],
        visible_name: &'a str,
    },
    Module {
        module_path: &'a [&'a str],
    },
}

impl RelationNodeSelector<'_> {
    fn describe(&self) -> String {
        match self {
            Self::Item {
                module_path,
                name,
                kind,
            } => format!("item {}::{} ({kind:?})", module_path.join("::"), name),
            Self::Import {
                module_path,
                visible_name,
            } => format!("import {}::{}", module_path.join("::"), visible_name),
            Self::Module { module_path } => format!("module {}", module_path.join("::")),
        }
    }
}

/// Exact tree relation expectation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExpectedTreeRelation<'a> {
    Contains {
        source: RelationNodeSelector<'a>,
        target: RelationNodeSelector<'a>,
    },
    ResolvesToDefinition {
        source: RelationNodeSelector<'a>,
        target: RelationNodeSelector<'a>,
    },
    CustomPath {
        source: RelationNodeSelector<'a>,
        target: RelationNodeSelector<'a>,
    },
    Sibling {
        source: RelationNodeSelector<'a>,
        target: RelationNodeSelector<'a>,
    },
    ModuleImports {
        source: RelationNodeSelector<'a>,
        target: RelationNodeSelector<'a>,
    },
    ReExports {
        source: RelationNodeSelector<'a>,
        target: RelationNodeSelector<'a>,
    },
    ImportedBy {
        source: RelationNodeSelector<'a>,
        target: RelationNodeSelector<'a>,
    },
}

impl ExpectedTreeRelation<'_> {
    fn source_selector(&self) -> RelationNodeSelector<'_> {
        match *self {
            Self::Contains { source, .. }
            | Self::ResolvesToDefinition { source, .. }
            | Self::CustomPath { source, .. }
            | Self::Sibling { source, .. }
            | Self::ModuleImports { source, .. }
            | Self::ReExports { source, .. }
            | Self::ImportedBy { source, .. } => source,
        }
    }

    fn target_selector(&self) -> RelationNodeSelector<'_> {
        match *self {
            Self::Contains { target, .. }
            | Self::ResolvesToDefinition { target, .. }
            | Self::CustomPath { target, .. }
            | Self::Sibling { target, .. }
            | Self::ModuleImports { target, .. }
            | Self::ReExports { target, .. }
            | Self::ImportedBy { target, .. } => target,
        }
    }

    fn kind_name(&self) -> &'static str {
        match self {
            Self::Contains { .. } => "Contains",
            Self::ResolvesToDefinition { .. } => "ResolvesToDefinition",
            Self::CustomPath { .. } => "CustomPath",
            Self::Sibling { .. } => "Sibling",
            Self::ModuleImports { .. } => "ModuleImports",
            Self::ReExports { .. } => "ReExports",
            Self::ImportedBy { .. } => "ImportedBy",
        }
    }
}

pub fn item<'a>(
    module_path: &'a [&'a str],
    name: &'a str,
    kind: ItemKind,
) -> RelationNodeSelector<'a> {
    RelationNodeSelector::Item {
        module_path,
        name,
        kind,
    }
}

pub fn import<'a>(module_path: &'a [&'a str], visible_name: &'a str) -> RelationNodeSelector<'a> {
    RelationNodeSelector::Import {
        module_path,
        visible_name,
    }
}

pub fn module<'a>(module_path: &'a [&'a str]) -> RelationNodeSelector<'a> {
    RelationNodeSelector::Module { module_path }
}

fn resolve_selector(
    graph: &ParsedCodeGraph,
    selector: RelationNodeSelector<'_>,
) -> Result<AnyNodeId, SynParserError> {
    match selector {
        RelationNodeSelector::Item {
            module_path,
            name,
            kind,
        } => find_item_id_by_path_name_kind_checked(graph, module_path, name, kind),
        RelationNodeSelector::Import {
            module_path,
            visible_name,
        } => {
            let module_path_vec = module_path
                .iter()
                .map(|segment| (*segment).to_string())
                .collect::<Vec<_>>();
            let imports_module = graph.find_module_by_path_checked(&module_path_vec)?;
            let matches: Vec<ImportNodeId> = imports_module
                .imports
                .iter()
                .filter(|imp| imp.visible_name == visible_name)
                .map(|imp| imp.id)
                .collect();

            match matches.as_slice() {
                [] => Err(SynParserError::InternalState(format!(
                    "Import `{visible_name}` not found in module `{}` while resolving relation endpoint",
                    module_path.join("::")
                ))),
                [id] => Ok((*id).into()),
                many => Err(SynParserError::InternalState(format!(
                    "Import selector `{}` was ambiguous in module `{}`; matched {} imports",
                    visible_name,
                    module_path.join("::"),
                    many.len()
                ))),
            }
        }
        RelationNodeSelector::Module { module_path } => {
            let module_path_vec = module_path
                .iter()
                .map(|segment| (*segment).to_string())
                .collect::<Vec<_>>();
            let module = graph.find_module_by_path_checked(&module_path_vec)?;
            Ok(module.id.into())
        }
    }
}

fn relation_matches(
    expected: &ExpectedTreeRelation<'_>,
    relation: &SyntacticRelation,
    source: AnyNodeId,
    target: AnyNodeId,
) -> bool {
    match expected {
        ExpectedTreeRelation::Contains { .. } => {
            matches!(relation, SyntacticRelation::Contains { .. })
                && relation.source() == source
                && relation.target() == target
        }
        ExpectedTreeRelation::ResolvesToDefinition { .. } => {
            matches!(relation, SyntacticRelation::ResolvesToDefinition { .. })
                && relation.source() == source
                && relation.target() == target
        }
        ExpectedTreeRelation::CustomPath { .. } => {
            matches!(relation, SyntacticRelation::CustomPath { .. })
                && relation.source() == source
                && relation.target() == target
        }
        ExpectedTreeRelation::Sibling { .. } => {
            matches!(relation, SyntacticRelation::Sibling { .. })
                && relation.source() == source
                && relation.target() == target
        }
        ExpectedTreeRelation::ModuleImports { .. } => {
            matches!(relation, SyntacticRelation::ModuleImports { .. })
                && relation.source() == source
                && relation.target() == target
        }
        ExpectedTreeRelation::ReExports { .. } => {
            matches!(relation, SyntacticRelation::ReExports { .. })
                && relation.source() == source
                && relation.target() == target
        }
        ExpectedTreeRelation::ImportedBy { .. } => {
            matches!(relation, SyntacticRelation::ImportedBy { .. })
                && relation.source() == source
                && relation.target() == target
        }
    }
}

fn adjacent_relations_debug(tree: &ModuleTree, node: AnyNodeId) -> String {
    let mut lines = Vec::new();
    for relation in tree.tree_relations() {
        let rel = relation.rel();
        if rel.source() == node || rel.target() == node {
            lines.push(format!("{rel:?}"));
        }
    }

    if lines.is_empty() {
        "  <none>".to_string()
    } else {
        lines
            .into_iter()
            .map(|line| format!("  {line}"))
            .collect::<Vec<_>>()
            .join("\n")
    }
}

/// Assert that an exact tree relation exists once and only once.
pub fn assert_tree_relation_once(
    graph: &ParsedCodeGraph,
    tree: &ModuleTree,
    expected: &ExpectedTreeRelation<'_>,
) -> Result<(), SynParserError> {
    let source = resolve_selector(graph, expected.source_selector())?;
    let target = resolve_selector(graph, expected.target_selector())?;

    let matches: Vec<_> = tree
        .tree_relations()
        .iter()
        .filter(|relation| relation_matches(expected, relation.rel(), source, target))
        .collect();

    assert_eq!(
        matches.len(),
        1,
        "Expected exactly one {} relation from {} to {}, found {}.\n\
         Source adjacent relations:\n{}\n\
         Target adjacent relations:\n{}",
        expected.kind_name(),
        expected.source_selector().describe(),
        expected.target_selector().describe(),
        matches.len(),
        adjacent_relations_debug(tree, source),
        adjacent_relations_debug(tree, target),
    );

    Ok(())
}
