//! Helpers for associated-item ([`MethodNode`]) paranoid ID regeneration tests.
//!
//! Synthetic IDs for methods use the owning [`ImplNodeId`] or [`TraitNodeId`] as
//! `parent_scope_id`, matching the visitor stack when methods are parsed (see
//! [`syn_parser::parser::nodes::ids::internal::GeneratesAnyNodeId`]).
//!
//! Phase 1 assumes merged cfgs are empty (no `#[cfg]` on enclosing module/impl/trait/method), so
//! `expected_cfg` should be `None` or `Some(&[])`.

use itertools::Itertools;
use ploke_core::{ItemKind, NodeId};
use syn_parser::TestIds;
use syn_parser::error::SynParserError;
use syn_parser::parser::graph::GraphAccess;
use syn_parser::parser::visitor::calculate_cfg_hash_bytes;
use syn_parser::parser::{ParsedCodeGraph, nodes::*};
use syn_parser::utils::logging::LOG_TEST_ID_REGEN;

/// Owning scope for a [`MethodNode`]: an inherent / trait impl block, or a trait definition.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AssocOwner<'a> {
    /// Match exactly one [`ImplNode`] in the target file by byte span.
    Impl { span: (usize, usize) },
    /// Match a [`TraitNode`] by name within the module at [`AssocParanoidArgs::expected_path`].
    Trait { trait_name: &'a str },
}

/// Arguments to regenerate a [`MethodNode`]'s synthetic [`NodeId`] the same way phase-2 parsing does.
#[derive(Debug, Clone)]
pub struct AssocParanoidArgs<'a> {
    /// Fixture directory name under the fixtures crates root (e.g. `"fixture_nodes"`).
    pub fixture: &'a str,
    /// Source file path relative to the fixture root (e.g. `"src/impls.rs"`).
    pub relative_file_path: &'a str,
    /// Module path segments for the item’s ID context (`current_module_path` in the visitor),
    /// e.g. `&["crate", "impls"]`.
    pub expected_path: &'a [&'a str],
    /// Owning `impl` or `trait` block.
    pub owner: AssocOwner<'a>,
    /// Method name (`ident` in `NodeId::generate_synthetic`).
    pub ident: &'a str,
    /// Optional cfg strings for hashing; Phase 1: `None` or empty (merged cfgs empty).
    pub expected_cfg: Option<&'a [&'a str]>,
}

/// Output of [`AssocParanoidArgs::generate_method_pid`].
#[derive(Debug, Clone)]
pub struct AssocTestInfo<'a> {
    args: &'a AssocParanoidArgs<'a>,
    target_data: &'a ParsedCodeGraph,
    test_method_id: MethodNodeId,
}

impl<'a> AssocTestInfo<'a> {
    pub fn args(&self) -> &'a AssocParanoidArgs<'a> {
        self.args
    }

    pub fn target_data(&self) -> &'a ParsedCodeGraph {
        self.target_data
    }

    pub fn test_method_id(&self) -> MethodNodeId {
        self.test_method_id
    }
}

fn strs_to_strings(strs: &[&str]) -> Vec<String> {
    strs.iter().copied().map(String::from).collect()
}

fn find_impl_by_span_checked<'a, G: GraphAccess + ?Sized>(
    graph: &'a G,
    span: (usize, usize),
) -> Result<&'a ImplNode, SynParserError> {
    let matches: Vec<&ImplNode> = graph
        .impls()
        .iter()
        .filter(|imp| imp.span == span)
        .collect();
    match matches.as_slice() {
        [] => Err(SynParserError::InternalState(format!(
            "No ImplNode with span {span:?}"
        ))),
        [one] => Ok(*one),
        many => Err(SynParserError::InternalState(format!(
            "Expected exactly one ImplNode with span {span:?}, found {}",
            many.len()
        ))),
    }
}

fn find_trait_in_module_by_name_checked<'a, G: GraphAccess + ?Sized>(
    graph: &'a G,
    module_path: &[String],
    trait_name: &str,
) -> Result<&'a TraitNode, SynParserError> {
    let module = graph.find_module_by_path_checked(module_path)?;
        let matches: Vec<&TraitNode> = graph
        .traits()
        .iter()
        .filter(|t| {
            t.name == trait_name
                && graph.module_contains_node(module.id, t.id.into())
        })
        .collect();
    match matches.as_slice() {
        [] => Err(SynParserError::InternalState(format!(
            "No TraitNode named {:?} in module path [{}]",
            trait_name,
            module_path.join("::")
        ))),
        [one] => Ok(*one),
        many => Err(SynParserError::InternalState(format!(
            "Expected exactly one TraitNode named {:?} in module path [{}], found {}",
            trait_name,
            module_path.join("::"),
            many.len()
        ))),
    }
}

impl<'a> AssocParanoidArgs<'a> {
    /// Regenerates the synthetic ID for an associated [`MethodNode`] using the owning impl or trait
    /// scope and the same `relative_path` / cfg inputs as [`super::ParanoidArgs::generate_pid`].
    pub fn generate_method_pid(
        &'a self,
        parsed_graphs: &'a [ParsedCodeGraph],
    ) -> Result<AssocTestInfo<'a>, SynParserError> {
        let fixture_root = ploke_common::fixtures_crates_dir().join(self.fixture);
        let target_file_path = fixture_root.join(self.relative_file_path);

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

        let exp_path_string: Vec<String> = self
            .expected_path
            .iter()
            .copied()
            .map(|s| s.to_string())
            .collect_vec();

        let cfgs_bytes_option: Option<Vec<u8>> = self
            .expected_cfg
            .filter(|cfgs_slice| !cfgs_slice.is_empty())
            .and_then(|cfgs_slice| calculate_cfg_hash_bytes(&strs_to_strings(cfgs_slice)));
        let actual_cfg_bytes_for_id_gen = cfgs_bytes_option.as_deref();

        let parent_scope_id: Option<NodeId> = match self.owner {
            AssocOwner::Impl { span } => {
                let imp = find_impl_by_span_checked(graph, span)?;
                Some(imp.id().base_tid())
            }
            AssocOwner::Trait { trait_name } => {
                let tr = find_trait_in_module_by_name_checked(graph, &exp_path_string, trait_name)?;
                Some(tr.trait_id().base_tid())
            }
        };

        if log::log_enabled!(target: LOG_TEST_ID_REGEN, log::Level::Debug) {
            log::debug!(target: LOG_TEST_ID_REGEN, "AssocParanoidArgs::generate_method_pid");
            log::debug!(target: LOG_TEST_ID_REGEN,
                "  Inputs for {} ({:?}):\n    crate_namespace: {}\n    file_path: {:?}\n    relative_path: {:?}\n    item_name: {}\n    item_kind: {:?}\n    parent_scope_id: {:?}\n    cfg_bytes: {:?}\n    owner: {:?}",
                self.ident,
                ItemKind::Method,
                target_data.crate_namespace,
                &target_file_path,
                &exp_path_string,
                self.ident,
                ItemKind::Method,
                parent_scope_id,
                actual_cfg_bytes_for_id_gen,
                self.owner,
            );
        }

        let generated_id = NodeId::generate_synthetic(
            target_data.crate_namespace,
            &target_file_path,
            &exp_path_string,
            self.ident,
            ItemKind::Method,
            parent_scope_id,
            actual_cfg_bytes_for_id_gen,
        );

        let test_method_id = MethodNodeId::new_test(generated_id);

        Ok(AssocTestInfo {
            args: self,
            target_data,
            test_method_id,
        })
    }
}
