//! Relation-focused helpers for paranoid integration tests.
//!
//! The larger object here is not "did some relation exist somewhere in the tree?".
//! It is "for a fixture with stable node identity, did this exact source node and this exact
//! target node participate in this exact relation variant, once and only once?".
//!
//! These helpers are intended for Phase 3 tree-relation assertions where source/target lookup
//! should be typed and provenance-preserving in the same spirit as the node paranoid suites.

use ploke_common::fixtures_crates_dir;
use ploke_core::{ItemKind, NodeId};
use syn_parser::TestIds;
use syn_parser::error::SynParserError;
use syn_parser::parser::ParsedCodeGraph;
use syn_parser::parser::graph::GraphAccess;
use syn_parser::parser::nodes::{AnyNodeId, GraphNode, ImportNodeId};
use syn_parser::parser::relations::SyntacticRelation;
use syn_parser::parser::visitor::calculate_cfg_hash_bytes;
use syn_parser::resolve::module_tree::ModuleTree;

use super::ParanoidArgs;
use super::paranoid::find_import_node_paranoid;
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ImportParanoidArgs<'a> {
    pub fixture: &'a str,
    pub relative_file_path: &'a str,
    pub expected_module_path: &'a [&'a str],
    pub visible_name: &'a str,
    pub expected_path: &'a [&'a str],
    pub expected_original_name: Option<&'a str>,
    pub expected_is_glob: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModuleParanoidKind {
    Declaration,
    FileDefinition,
    InlineDefinition,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ModuleParanoidArgs<'a> {
    pub fixture: &'a str,
    pub relative_file_path: &'a str,
    pub expected_module_path: &'a [&'a str],
    pub kind: ModuleParanoidKind,
    pub expected_cfg: Option<&'a [&'a str]>,
}

#[derive(Debug, Clone)]
pub enum ParanoidRelationEndpoint<'a> {
    Item(ParanoidArgs<'a>),
    Import(ImportParanoidArgs<'a>),
    Module(ModuleParanoidArgs<'a>),
}

impl ParanoidRelationEndpoint<'_> {
    fn describe(&self) -> String {
        match self {
            Self::Item(args) => format!(
                "item {}::{} ({:?})",
                args.expected_path.join("::"),
                args.ident,
                args.item_kind
            ),
            Self::Import(args) => format!(
                "import {}::{}",
                args.expected_module_path.join("::"),
                args.visible_name
            ),
            Self::Module(args) => format!(
                "module {} ({:?})",
                args.expected_module_path.join("::"),
                args.kind
            ),
        }
    }
}

pub fn paranoid_item<'a>(args: ParanoidArgs<'a>) -> ParanoidRelationEndpoint<'a> {
    ParanoidRelationEndpoint::Item(args)
}

pub fn paranoid_import<'a>(args: ImportParanoidArgs<'a>) -> ParanoidRelationEndpoint<'a> {
    ParanoidRelationEndpoint::Import(args)
}

pub fn paranoid_module<'a>(args: ModuleParanoidArgs<'a>) -> ParanoidRelationEndpoint<'a> {
    ParanoidRelationEndpoint::Module(args)
}

fn resolve_exact_endpoint(
    parsed_graphs: &[ParsedCodeGraph],
    endpoint: &ParanoidRelationEndpoint<'_>,
) -> AnyNodeId {
    match endpoint {
        ParanoidRelationEndpoint::Item(args) => args
            .generate_pid(parsed_graphs)
            .expect("failed to regenerate item ID for relation endpoint")
            .test_pid()
            .into(),
        ParanoidRelationEndpoint::Import(args) => {
            let expected_module_path = args
                .expected_module_path
                .iter()
                .map(|segment| (*segment).to_string())
                .collect::<Vec<_>>();
            let expected_path = args
                .expected_path
                .iter()
                .map(|segment| (*segment).to_string())
                .collect::<Vec<_>>();
            find_import_node_paranoid(
                parsed_graphs,
                args.fixture,
                args.relative_file_path,
                &expected_module_path,
                args.visible_name,
                &expected_path,
                args.expected_original_name,
                args.expected_is_glob,
            )
            .id
            .into()
        }
        ParanoidRelationEndpoint::Module(args) => {
            resolve_exact_module_endpoint(parsed_graphs, args)
        }
    }
}

fn resolve_exact_module_endpoint(
    parsed_graphs: &[ParsedCodeGraph],
    args: &ModuleParanoidArgs<'_>,
) -> AnyNodeId {
    let fixture_root = fixtures_crates_dir().join(args.fixture);
    let target_file_path = fixture_root.join(args.relative_file_path);
    let target_data = parsed_graphs
        .iter()
        .find(|data| data.file_path == target_file_path)
        .unwrap_or_else(|| {
            panic!(
                "ParsedCodeGraph for '{}' not found in results",
                target_file_path.display()
            )
        });

    let graph = &target_data.graph;
    let expected_module_path = args
        .expected_module_path
        .iter()
        .map(|segment| (*segment).to_string())
        .collect::<Vec<_>>();
    let candidates = graph
        .modules
        .iter()
        .filter(|module| {
            module.path == expected_module_path
                && match args.kind {
                    ModuleParanoidKind::Declaration => module.is_decl(),
                    ModuleParanoidKind::FileDefinition => module.is_file_based(),
                    ModuleParanoidKind::InlineDefinition => module.is_inline(),
                }
                && match args.expected_cfg {
                    Some(expected_cfg) => module.cfgs() == expected_cfg,
                    None => true,
                }
        })
        .collect::<Vec<_>>();

    assert_eq!(
        candidates.len(),
        1,
        "Expected exactly one {:?} module with path {:?} in file '{}', found {}",
        args.kind,
        expected_module_path,
        target_file_path.display(),
        candidates.len()
    );

    let module = candidates[0];
    let item_cfgs = module.cfgs();
    let parent_path_vec = expected_module_path
        .iter()
        .take(expected_module_path.len().saturating_sub(1))
        .cloned()
        .collect::<Vec<_>>();

    let (parent_scope_id, scope_cfgs): (Option<NodeId>, Vec<String>) = match args.kind {
        ModuleParanoidKind::FileDefinition => (None, vec![]),
        ModuleParanoidKind::Declaration | ModuleParanoidKind::InlineDefinition => {
            let parent_mod = graph
                .modules
                .iter()
                .find(|m| m.path == parent_path_vec)
                .unwrap_or_else(|| {
                    panic!(
                        "Parent ModuleNode not found for path {:?} in file '{}'",
                        parent_path_vec,
                        target_file_path.display()
                    )
                });
            (Some(parent_mod.id.base_tid()), parent_mod.cfgs().to_vec())
        }
    };

    let mut provisional_effective_cfgs = scope_cfgs.clone();
    provisional_effective_cfgs.extend(item_cfgs.iter().cloned());
    provisional_effective_cfgs.sort_unstable();
    let cfg_bytes = calculate_cfg_hash_bytes(&provisional_effective_cfgs);

    let regenerated_id = NodeId::generate_synthetic(
        target_data.crate_namespace,
        &target_data.file_path,
        &parent_path_vec,
        module.name(),
        ItemKind::Module,
        parent_scope_id,
        cfg_bytes.as_deref(),
    );

    assert_eq!(
        module.id.base_tid(),
        regenerated_id,
        "Mismatch between module node's actual ID ({}) and regenerated ID ({}) for module path {:?} ({:?}) in file '{}'",
        module.id,
        regenerated_id,
        expected_module_path,
        args.kind,
        target_file_path.display(),
    );

    module.id.into()
}

fn imported_by_sources_debug(tree: &ModuleTree, target: AnyNodeId) -> Vec<AnyNodeId> {
    tree.tree_relations()
        .iter()
        .filter_map(|relation| match relation.rel() {
            SyntacticRelation::ImportedBy {
                source,
                target: relation_target,
            } if AnyNodeId::from(*relation_target) == target => Some(AnyNodeId::from(*source)),
            _ => None,
        })
        .collect()
}

pub fn assert_imported_by_sources_exact(
    parsed_graphs: &[ParsedCodeGraph],
    tree: &ModuleTree,
    target_import: ImportParanoidArgs<'_>,
    expected_sources: &[ParanoidRelationEndpoint<'_>],
) -> Result<(), SynParserError> {
    let target = resolve_exact_endpoint(
        parsed_graphs,
        &ParanoidRelationEndpoint::Import(target_import),
    );
    let expected_ids = expected_sources
        .iter()
        .map(|endpoint| resolve_exact_endpoint(parsed_graphs, endpoint))
        .collect::<Vec<_>>();
    let actual_ids = imported_by_sources_debug(tree, target);

    assert_eq!(
        actual_ids.len(),
        expected_ids.len(),
        "Expected exactly {} ImportedBy sources for import {}::{}, found {}.\nExpected sources:\n{}\nActual source IDs:\n{:#?}",
        expected_ids.len(),
        target_import.expected_module_path.join("::"),
        target_import.visible_name,
        actual_ids.len(),
        expected_sources
            .iter()
            .map(ParanoidRelationEndpoint::describe)
            .map(|line| format!("  {line}"))
            .collect::<Vec<_>>()
            .join("\n"),
        actual_ids,
    );

    for expected in &expected_ids {
        assert!(
            actual_ids.contains(expected),
            "Missing ImportedBy source {:?} for import {}::{}.\nExpected sources:\n{}\nActual source IDs:\n{:#?}",
            expected,
            target_import.expected_module_path.join("::"),
            target_import.visible_name,
            expected_sources
                .iter()
                .map(ParanoidRelationEndpoint::describe)
                .map(|line| format!("  {line}"))
                .collect::<Vec<_>>()
                .join("\n"),
            actual_ids,
        );
    }

    Ok(())
}
