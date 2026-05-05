//! Typed type-resolution facts for the post-merge semantic bridge.
//!
//! This module is the v2 shape of the legacy `type_resolution` pass. The older module
//! emits report rows that combine the type-use site, resolution state, broad
//! target enum, and resolved `TypeId` promotion. This module keeps those facts
//! separate and constructs [`TypeRelation`] values at the boundary where source
//! and target endpoint membership has been proven.
//!
//! The hot path should prefer:
//!
//! ```text
//! TypeId/AnyNodeId copies + ModuleTree relation indexes + typed endpoint proof
//! ```
//!
//! over broad graph scans. Heap-backed node/type payloads are still read at
//! proof boundaries: type-kind refinement, generic-parameter-kind refinement,
//! and name/path lookup.
//!
//! This pass is meant to run after [`ModuleTree`] construction has finished:
//! module declarations have been linked to definitions, `#[path]` has been
//! reconciled, unlinked file modules have been pruned, and import backlinks
//! have been indexed. At that point type resolution is a relation-construction
//! pass over an assumed-valid Rust graph, not a diagnostic pass over arbitrary
//! source. Internal contradictions are returned as [`SynParserError`]; external
//! or currently unsupported targets simply do not produce a [`TypeRelation`].

use std::collections::HashMap;

use rayon::prelude::*;
use serde::{Deserialize, Serialize};

use crate::{
    error::SynParserError,
    parser::{
        ParsedCodeGraph,
        graph::GraphAccess,
        nodes::{
            AnyNodeId, AnyTypeId, AsAnyNodeId, AssociatedItemNodeId, ImportNodeId, MethodNodeId,
            ModuleNodeId, OrdinaryTypeSourceId, OrdinaryTypeTargetId, OrdinaryTypeUseId,
            TraitTypeSourceId, TraitTypeTargetId, TypeGenericParamNodeId,
        },
        relations::{SyntacticRelation, TypeRelation},
        types::{GenericParamKind, GenericParamNode, TypeNode},
    },
};

use super::{RelationIndexer, module_tree::ModuleTree};

const MAX_IMPORT_CHAIN_DEPTH: usize = 100;
const MAX_TYPE_TREE_STACK: usize = 128;
const MAX_TYPE_TREE_STEPS: usize = 4096;

/// Owned collection adapter for the v2 typed type-relation stream.
#[derive(Debug, Default, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TypeRelationReport {
    pub relations: Vec<TypeRelation>,
    pub summary: TypeRelationSummary,
}

/// Compact counters for validating the v2 resolver during migration.
#[derive(Debug, Default, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TypeRelationSummary {
    pub resolved: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ExpectedTarget {
    Ordinary,
    Trait,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SourceProof {
    Ordinary(OrdinaryTypeSourceId),
    Trait(TraitTypeSourceId),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TargetProof {
    Ordinary(OrdinaryTypeTargetId),
    Trait(TraitTypeTargetId),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct TypeUseSite {
    context: ResolutionContext,
    source: TypeWorkItem,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct ResolutionContext {
    resolution_context_owner: AnyNodeId,
    containing_module: Option<ModuleNodeId>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TypeWorkItem {
    Ordinary(OrdinaryTypeUseId),
    Trait(TraitTypeSourceId),
}

/// V2 resolver that treats `ModuleTree` as the indexed topology and constructs
/// typed relation facts once source and target endpoint proof succeeds.
pub struct TypeRelationResolver<'a> {
    graph: &'a ParsedCodeGraph,
    tree: &'a ModuleTree,
    type_by_id: HashMap<AnyTypeId, &'a TypeNode>,
}

impl<'a> TypeRelationResolver<'a> {
    pub fn new(graph: &'a ParsedCodeGraph, tree: &'a ModuleTree) -> Self {
        let type_by_id = graph
            .type_graph()
            .iter()
            .map(|type_node| (type_node.id(), type_node))
            .collect();

        Self {
            graph,
            tree,
            type_by_id,
        }
    }

    pub fn resolve_type_relations(&self) -> Result<TypeRelationReport, SynParserError> {
        let relations: Vec<TypeRelation> = self
            .type_relation_results()
            .collect::<Result<Vec<_>, SynParserError>>()?;

        Ok(TypeRelationReport {
            summary: TypeRelationSummary {
                resolved: relations.len(),
            },
            relations,
        })
    }

    pub fn type_relation_results(
        &'a self,
    ) -> impl ParallelIterator<Item = Result<TypeRelation, SynParserError>> + 'a {
        self.type_resolution_roots()
            .flat_map_iter(|root| DirectTypeUseIter::new(self, root))
            .flat_map_iter(|site| TypeTreeRelationIter::new(self, site))
    }

    fn type_node(&self, type_id: impl Into<AnyTypeId>) -> Result<&'a TypeNode, SynParserError> {
        let type_id = type_id.into();
        self.type_by_id.get(&type_id).copied().ok_or_else(|| {
            SynParserError::InternalState(format!("type id {type_id} was not found in type graph"))
        })
    }

    fn resolve_ordinary_source(
        &self,
        context: ResolutionContext,
        source: OrdinaryTypeSourceId,
        path: &[String],
        is_fully_qualified: bool,
    ) -> Result<Option<TypeRelation>, SynParserError> {
        self.resolve_source(
            SourceProof::Ordinary(source),
            path,
            is_fully_qualified,
            context.containing_module,
            Some(context.resolution_context_owner),
        )
    }

    fn resolve_trait_source(
        &self,
        context: ResolutionContext,
        source: TraitTypeSourceId,
        path: &[String],
        is_fully_qualified: bool,
    ) -> Result<Option<TypeRelation>, SynParserError> {
        self.resolve_source(
            SourceProof::Trait(source),
            path,
            is_fully_qualified,
            context.containing_module,
            Some(context.resolution_context_owner),
        )
    }

    fn resolve_source(
        &self,
        source: SourceProof,
        path: &[String],
        is_fully_qualified: bool,
        containing_module: Option<ModuleNodeId>,
        owner: Option<AnyNodeId>,
    ) -> Result<Option<TypeRelation>, SynParserError> {
        let expected = match source {
            SourceProof::Ordinary(_) => ExpectedTarget::Ordinary,
            SourceProof::Trait(_) => ExpectedTarget::Trait,
        };
        self.resolve_path_source(
            source,
            path,
            is_fully_qualified,
            containing_module,
            owner,
            expected,
        )
    }

    fn resolve_path_source(
        &self,
        source: SourceProof,
        path: &[String],
        is_fully_qualified: bool,
        containing_module: Option<ModuleNodeId>,
        owner: Option<AnyNodeId>,
        expected: ExpectedTarget,
    ) -> Result<Option<TypeRelation>, SynParserError> {
        if path.is_empty() {
            return Ok(None);
        }

        if self.is_builtin_path(path) || self.is_external_root(path.first().map(String::as_str)) {
            return Ok(None);
        }

        if path.len() == 1 && path[0] == "Self" {
            return Ok(None);
        }

        if let Some(owner) = owner
            && let Some(type_param_id) = self.resolve_type_generic_param(owner, path)
        {
            let target = OrdinaryTypeTargetId::from(type_param_id);
            return self.resolved_relation(source, TargetProof::Ordinary(target));
        }

        let containing_module =
            containing_module.or_else(|| owner.and_then(|owner| self.containing_module(owner)));
        let Some(mut current_module) = containing_module else {
            return Ok(None);
        };

        let start_idx = self.start_segment_index(path, is_fully_qualified, &mut current_module)?;
        if start_idx >= path.len() {
            return Ok(None);
        }

        for idx in start_idx..path.len() {
            let is_last = idx == path.len() - 1;
            let segment = path[idx].as_str();
            if is_last {
                return self.resolve_terminal_segment(source, current_module, segment, expected);
            }

            current_module = match self.resolve_module_segment(current_module, segment)? {
                Some(module_id) => module_id,
                None => return Ok(None),
            };
        }

        Ok(None)
    }

    fn resolved_relation(
        &self,
        source: SourceProof,
        target: TargetProof,
    ) -> Result<Option<TypeRelation>, SynParserError> {
        let relation = match (source, target) {
            (SourceProof::Ordinary(source), TargetProof::Ordinary(target)) => {
                TypeRelation::Ordinary { source, target }
            }
            (SourceProof::Trait(source), TargetProof::Trait(target)) => {
                TypeRelation::Trait { source, target }
            }
            _ => {
                return Ok(None);
            }
        };

        Ok(Some(relation))
    }

    fn prove_target(&self, target: AnyNodeId, expected: ExpectedTarget) -> Option<TargetProof> {
        match expected {
            ExpectedTarget::Ordinary => match target {
                AnyNodeId::Struct(id) => Some(TargetProof::Ordinary(id.into())),
                AnyNodeId::Enum(id) => Some(TargetProof::Ordinary(id.into())),
                AnyNodeId::Union(id) => Some(TargetProof::Ordinary(id.into())),
                AnyNodeId::TypeAlias(id) => Some(TargetProof::Ordinary(id.into())),
                AnyNodeId::GenericParam(id) => self
                    .generic_param_node(id)
                    .and_then(|param| TypeGenericParamNodeId::try_refine(id, &param.kind).ok())
                    .map(OrdinaryTypeTargetId::from)
                    .map(TargetProof::Ordinary),
                _ => None,
            },
            ExpectedTarget::Trait => match target {
                AnyNodeId::Trait(id) => Some(TargetProof::Trait(TraitTypeTargetId::from(id))),
                _ => None,
            },
        }
    }

    fn start_segment_index(
        &self,
        path: &[String],
        is_fully_qualified: bool,
        current_module: &mut ModuleNodeId,
    ) -> Result<usize, SynParserError> {
        if is_fully_qualified {
            return Ok(path.len());
        }

        let mut idx = 0usize;
        while let Some(segment) = path.get(idx).map(String::as_str) {
            match segment {
                "crate" => {
                    *current_module = self.tree.root();
                    idx += 1;
                }
                "self" => {
                    idx += 1;
                }
                "super" => {
                    *current_module = self
                        .tree
                        .get_parent_module_id(*current_module)
                        .ok_or_else(|| {
                            SynParserError::InternalState(format!(
                                "type resolution could not find parent module for {} while resolving {}",
                                current_module,
                                path.join("::")
                            ))
                        })?;
                    idx += 1;
                }
                _ => break,
            }
        }

        Ok(idx)
    }

    fn visit_scope_candidates(
        &self,
        module_id: ModuleNodeId,
        segment: &str,
        sink: &mut impl FnMut(AnyNodeId) -> Result<(), SynParserError>,
    ) -> Result<(), SynParserError> {
        for relation in self
            .tree
            .get_iter_relations_from(&module_id.as_any())
            .into_iter()
            .flatten()
        {
            let SyntacticRelation::Contains { target, .. } = relation.rel() else {
                continue;
            };
            let target_any = target.as_any();
            let Ok(node) = self.graph.find_node_unique(target_any) else {
                continue;
            };

            if node.name() == segment {
                self.visit_binding_terminals(target_any, 0, sink)?;
            }

            if let Some(import_node) = node.as_import()
                && import_node.is_glob
            {
                self.visit_glob_candidates(import_node.id, segment, sink)?;
            }
        }

        Ok(())
    }

    fn visit_glob_candidates(
        &self,
        import_id: ImportNodeId,
        segment: &str,
        sink: &mut impl FnMut(AnyNodeId) -> Result<(), SynParserError>,
    ) -> Result<(), SynParserError> {
        for relation in self.tree.get_iter_relations_to(&import_id.as_any()) {
            let SyntacticRelation::ImportedBy { source, target } = relation.rel() else {
                continue;
            };
            if *target != import_id {
                continue;
            }
            let source_any = source.as_any();
            let Ok(source_node) = self.graph.find_node_unique(source_any) else {
                continue;
            };
            if source_node.name() == segment {
                self.visit_binding_terminals(source_any, 0, sink)?;
            }
        }

        Ok(())
    }

    fn visit_binding_terminals(
        &self,
        start: AnyNodeId,
        depth: usize,
        sink: &mut impl FnMut(AnyNodeId) -> Result<(), SynParserError>,
    ) -> Result<(), SynParserError> {
        if depth > MAX_IMPORT_CHAIN_DEPTH {
            return Err(SynParserError::InternalState(format!(
                "type resolution exceeded import chain depth limit of {MAX_IMPORT_CHAIN_DEPTH} at {start}"
            )));
        }

        let Ok(import_id) = ImportNodeId::try_from(start) else {
            return sink(start);
        };

        let mut had_sources = false;
        for relation in self.tree.get_iter_relations_to(&import_id.as_any()) {
            let SyntacticRelation::ImportedBy { source, target } = relation.rel() else {
                continue;
            };
            if *target != import_id {
                continue;
            }
            had_sources = true;
            self.visit_binding_terminals(source.as_any(), depth + 1, sink)?;
        }

        if had_sources {
            return Ok(());
        }

        let import_node = self.graph.get_import_checked(import_id)?;
        if self.is_external_root(import_node.source_path().first().map(String::as_str)) {
            return Ok(());
        }

        Ok(())
    }

    fn resolve_module_segment(
        &self,
        module_id: ModuleNodeId,
        segment: &str,
    ) -> Result<Option<ModuleNodeId>, SynParserError> {
        let mut resolved_module = None;
        self.visit_scope_candidates(module_id, segment, &mut |candidate| {
            let Ok(candidate_module) = ModuleNodeId::try_from(candidate) else {
                return Ok(());
            };
            if let Some(existing) = resolved_module
                && existing != candidate_module
            {
                return Err(SynParserError::InternalState(format!(
                    "type resolution found multiple module candidates for segment `{segment}`: {existing} and {candidate_module}"
                )));
            }
            resolved_module = Some(candidate_module);
            Ok(())
        })?;
        Ok(resolved_module)
    }

    fn resolve_terminal_segment(
        &self,
        source: SourceProof,
        module_id: ModuleNodeId,
        segment: &str,
        expected: ExpectedTarget,
    ) -> Result<Option<TypeRelation>, SynParserError> {
        let mut resolved_target = None;
        self.visit_scope_candidates(module_id, segment, &mut |candidate| {
            if let Some(target) = self.prove_target(candidate, expected) {
                if let Some((existing_id, _)) = resolved_target
                    && existing_id != candidate
                {
                    return Err(SynParserError::InternalState(format!(
                        "type resolution found multiple type candidates for `{segment}`: {existing_id} and {candidate}"
                    )));
                }
                resolved_target = Some((candidate, target));
            }
            Ok(())
        })?;

        match resolved_target {
            Some((_, target)) => self.resolved_relation(source, target),
            None => Ok(None),
        }
    }

    fn containing_module(&self, owner: AnyNodeId) -> Option<ModuleNodeId> {
        if let Ok(module_id) = ModuleNodeId::try_from(owner) {
            return Some(module_id);
        }

        self.tree
            .get_iter_relations_to(&owner)
            .find_map(|relation| match relation.rel() {
                SyntacticRelation::Contains { source, target } if target.as_any() == owner => {
                    Some(*source)
                }
                SyntacticRelation::ImplAssociatedItem { source, target }
                    if target.as_any() == owner =>
                {
                    self.containing_module(source.as_any())
                }
                SyntacticRelation::TraitAssociatedItem { source, target }
                    if target.as_any() == owner =>
                {
                    self.containing_module(source.as_any())
                }
                _ => None,
            })
    }

    fn resolve_type_generic_param(
        &self,
        owner: AnyNodeId,
        path: &[String],
    ) -> Option<TypeGenericParamNodeId> {
        if path.len() != 1 {
            return None;
        }
        let name = path[0].as_str();

        self.resolve_type_generic_param_in_owner(owner, name)
            .or_else(|| {
                let AnyNodeId::Method(method_id) = owner else {
                    return None;
                };
                self.associated_owner_for_method(method_id)
                    .and_then(|associated_owner| {
                        self.resolve_type_generic_param_in_owner(associated_owner, name)
                    })
            })
    }

    fn resolve_type_generic_param_in_owner(
        &self,
        owner: AnyNodeId,
        name: &str,
    ) -> Option<TypeGenericParamNodeId> {
        self.generic_params_for_owner(owner)?
            .iter()
            .find_map(|param| match &param.kind {
                GenericParamKind::Type {
                    name: param_name, ..
                } if param_name == name => {
                    TypeGenericParamNodeId::try_refine(param.id, &param.kind).ok()
                }
                _ => None,
            })
    }

    fn associated_owner_for_method(&self, method_id: MethodNodeId) -> Option<AnyNodeId> {
        let target = AssociatedItemNodeId::from(method_id);
        self.tree
            .get_iter_relations_to(&target.as_any())
            .find_map(|relation| match relation.rel() {
                SyntacticRelation::ImplAssociatedItem { source, target: t } if *t == target => {
                    Some(source.as_any())
                }
                SyntacticRelation::TraitAssociatedItem { source, target: t } if *t == target => {
                    Some(source.as_any())
                }
                _ => None,
            })
    }

    fn generic_params_for_owner(&self, owner: AnyNodeId) -> Option<&'a [GenericParamNode]> {
        match owner {
            AnyNodeId::Function(id) => self
                .graph
                .functions()
                .iter()
                .find(|node| node.id == id)
                .map(|node| node.generic_params.as_slice()),
            AnyNodeId::Method(id) => self
                .find_method(id)
                .map(|node| node.generic_params.as_slice()),
            AnyNodeId::Struct(id) => {
                self.graph
                    .defined_types()
                    .iter()
                    .find_map(|node| match node {
                        crate::parser::nodes::TypeDefNode::Struct(node) if node.id == id => {
                            Some(node.generic_params.as_slice())
                        }
                        _ => None,
                    })
            }
            AnyNodeId::Enum(id) => self
                .graph
                .defined_types()
                .iter()
                .find_map(|node| match node {
                    crate::parser::nodes::TypeDefNode::Enum(node) if node.id == id => {
                        Some(node.generic_params.as_slice())
                    }
                    _ => None,
                }),
            AnyNodeId::Union(id) => self
                .graph
                .defined_types()
                .iter()
                .find_map(|node| match node {
                    crate::parser::nodes::TypeDefNode::Union(node) if node.id == id => {
                        Some(node.generic_params.as_slice())
                    }
                    _ => None,
                }),
            AnyNodeId::TypeAlias(id) => {
                self.graph
                    .defined_types()
                    .iter()
                    .find_map(|node| match node {
                        crate::parser::nodes::TypeDefNode::TypeAlias(node) if node.id == id => {
                            Some(node.generic_params.as_slice())
                        }
                        _ => None,
                    })
            }
            AnyNodeId::Trait(id) => self
                .graph
                .traits()
                .iter()
                .find(|node| node.id == id)
                .map(|node| node.generic_params.as_slice()),
            AnyNodeId::Impl(id) => self
                .graph
                .impls()
                .iter()
                .find(|node| node.id == id)
                .map(|node| node.generic_params()),
            _ => None,
        }
    }

    fn generic_param_node(
        &self,
        id: crate::parser::nodes::GenericParamNodeId,
    ) -> Option<&'a GenericParamNode> {
        self.graph
            .functions()
            .iter()
            .flat_map(|node| node.generic_params.iter())
            .chain(
                self.graph
                    .defined_types()
                    .iter()
                    .flat_map(|node| match node {
                        crate::parser::nodes::TypeDefNode::Struct(node) => {
                            node.generic_params.iter()
                        }
                        crate::parser::nodes::TypeDefNode::Enum(node) => node.generic_params.iter(),
                        crate::parser::nodes::TypeDefNode::Union(node) => {
                            node.generic_params.iter()
                        }
                        crate::parser::nodes::TypeDefNode::TypeAlias(node) => {
                            node.generic_params.iter()
                        }
                    }),
            )
            .chain(
                self.graph
                    .traits()
                    .iter()
                    .flat_map(|node| node.generic_params.iter()),
            )
            .chain(
                self.graph
                    .impls()
                    .iter()
                    .flat_map(|node| node.generic_params.iter()),
            )
            .chain(
                self.graph
                    .impls()
                    .iter()
                    .flat_map(|node| node.methods.iter())
                    .flat_map(|node| node.generic_params.iter()),
            )
            .chain(
                self.graph
                    .traits()
                    .iter()
                    .flat_map(|node| node.methods.iter())
                    .flat_map(|node| node.generic_params.iter()),
            )
            .find(|param| param.id == id)
    }

    fn find_method(&self, id: MethodNodeId) -> Option<&'a crate::parser::nodes::MethodNode> {
        self.graph
            .impls()
            .iter()
            .flat_map(|node| node.methods.iter())
            .chain(
                self.graph
                    .traits()
                    .iter()
                    .flat_map(|node| node.methods.iter()),
            )
            .find(|method| method.id == id)
    }

    fn is_builtin_path(&self, path: &[String]) -> bool {
        path.len() == 1
            && matches!(
                path[0].as_str(),
                "bool"
                    | "char"
                    | "str"
                    | "i8"
                    | "i16"
                    | "i32"
                    | "i64"
                    | "i128"
                    | "isize"
                    | "u8"
                    | "u16"
                    | "u32"
                    | "u64"
                    | "u128"
                    | "usize"
                    | "f16"
                    | "f32"
                    | "f64"
                    | "f128"
            )
    }

    fn is_external_root(&self, first_segment: Option<&str>) -> bool {
        first_segment.is_some_and(|segment| {
            matches!(segment, "std" | "core" | "alloc")
                || self
                    .graph
                    .iter_dependency_names()
                    .any(|dependency| dependency == segment)
        })
    }

    fn type_resolution_roots(
        &'a self,
    ) -> impl ParallelIterator<Item = TypeResolutionRoot<'a>> + 'a {
        self.graph
            .functions()
            .par_iter()
            .map(TypeResolutionRoot::Function)
            .chain(
                self.graph
                    .defined_types()
                    .par_iter()
                    .map(TypeResolutionRoot::DefinedType),
            )
            .chain(
                self.graph
                    .traits()
                    .par_iter()
                    .map(TypeResolutionRoot::Trait),
            )
            .chain(self.graph.impls().par_iter().map(TypeResolutionRoot::Impl))
            .chain(
                self.graph
                    .consts()
                    .par_iter()
                    .map(TypeResolutionRoot::Const),
            )
            .chain(
                self.graph
                    .statics()
                    .par_iter()
                    .map(TypeResolutionRoot::Static),
            )
    }
}

#[derive(Debug, Clone, Copy)]
enum TypeResolutionRoot<'a> {
    Function(&'a crate::parser::nodes::FunctionNode),
    DefinedType(&'a crate::parser::nodes::TypeDefNode),
    Trait(&'a crate::parser::nodes::TraitNode),
    Impl(&'a crate::parser::nodes::ImplNode),
    Const(&'a crate::parser::nodes::ConstNode),
    Static(&'a crate::parser::nodes::StaticNode),
}

#[derive(Debug, Clone, Copy)]
struct MethodIterState {
    param_idx: usize,
    yielded_return: bool,
}

impl MethodIterState {
    fn new() -> Self {
        Self {
            param_idx: 0,
            yielded_return: false,
        }
    }
}

#[derive(Debug, Clone, Copy)]
enum DirectTypeUseIter<'a> {
    Function {
        node: &'a crate::parser::nodes::FunctionNode,
        module: Option<ModuleNodeId>,
        method_state: MethodIterState,
    },
    Struct {
        node: &'a crate::parser::nodes::StructNode,
        module: Option<ModuleNodeId>,
        field_idx: usize,
    },
    Enum {
        node: &'a crate::parser::nodes::EnumNode,
        module: Option<ModuleNodeId>,
        variant_idx: usize,
        field_idx: usize,
    },
    TypeAlias {
        node: &'a crate::parser::nodes::TypeAliasNode,
        module: Option<ModuleNodeId>,
        yielded: bool,
    },
    Union {
        node: &'a crate::parser::nodes::UnionNode,
        module: Option<ModuleNodeId>,
        field_idx: usize,
    },
    Trait {
        node: &'a crate::parser::nodes::TraitNode,
        module: Option<ModuleNodeId>,
        super_idx: usize,
        method_idx: usize,
        method_state: MethodIterState,
    },
    Impl {
        node: &'a crate::parser::nodes::ImplNode,
        module: Option<ModuleNodeId>,
        yielded_self: bool,
        yielded_trait: bool,
        method_idx: usize,
        method_state: MethodIterState,
    },
    Const {
        node: &'a crate::parser::nodes::ConstNode,
        module: Option<ModuleNodeId>,
        yielded: bool,
    },
    Static {
        node: &'a crate::parser::nodes::StaticNode,
        module: Option<ModuleNodeId>,
        yielded: bool,
    },
}

impl<'a> DirectTypeUseIter<'a> {
    fn new(resolver: &TypeRelationResolver<'a>, root: TypeResolutionRoot<'a>) -> Self {
        match root {
            TypeResolutionRoot::Function(node) => {
                let owner = node.id.as_any();
                Self::Function {
                    node,
                    module: resolver.containing_module(owner),
                    method_state: MethodIterState::new(),
                }
            }
            TypeResolutionRoot::DefinedType(node) => match node {
                crate::parser::nodes::TypeDefNode::Struct(node) => {
                    let owner = node.id.as_any();
                    Self::Struct {
                        node,
                        module: resolver.containing_module(owner),
                        field_idx: 0,
                    }
                }
                crate::parser::nodes::TypeDefNode::Enum(node) => {
                    let owner = node.id.as_any();
                    Self::Enum {
                        node,
                        module: resolver.containing_module(owner),
                        variant_idx: 0,
                        field_idx: 0,
                    }
                }
                crate::parser::nodes::TypeDefNode::TypeAlias(node) => {
                    let owner = node.id.as_any();
                    Self::TypeAlias {
                        node,
                        module: resolver.containing_module(owner),
                        yielded: false,
                    }
                }
                crate::parser::nodes::TypeDefNode::Union(node) => {
                    let owner = node.id.as_any();
                    Self::Union {
                        node,
                        module: resolver.containing_module(owner),
                        field_idx: 0,
                    }
                }
            },
            TypeResolutionRoot::Trait(node) => {
                let owner = node.id.as_any();
                Self::Trait {
                    node,
                    module: resolver.containing_module(owner),
                    super_idx: 0,
                    method_idx: 0,
                    method_state: MethodIterState::new(),
                }
            }
            TypeResolutionRoot::Impl(node) => {
                let owner = node.id.as_any();
                Self::Impl {
                    node,
                    module: resolver.containing_module(owner),
                    yielded_self: false,
                    yielded_trait: false,
                    method_idx: 0,
                    method_state: MethodIterState::new(),
                }
            }
            TypeResolutionRoot::Const(node) => {
                let owner = node.id.as_any();
                Self::Const {
                    node,
                    module: resolver.containing_module(owner),
                    yielded: false,
                }
            }
            TypeResolutionRoot::Static(node) => {
                let owner = node.id.as_any();
                Self::Static {
                    node,
                    module: resolver.containing_module(owner),
                    yielded: false,
                }
            }
        }
    }
}

impl Iterator for DirectTypeUseIter<'_> {
    type Item = TypeUseSite;

    fn next(&mut self) -> Option<Self::Item> {
        match self {
            Self::Function {
                node,
                module,
                method_state,
            } => next_callable_type_use(
                node.id.as_any(),
                *module,
                &node.parameters,
                node.return_type,
                method_state,
            ),
            Self::Struct {
                node,
                module,
                field_idx,
            } => {
                let field = node.fields.get(*field_idx)?;
                *field_idx += 1;
                Some(ordinary_type_use_site(
                    node.id.as_any(),
                    *module,
                    field.type_id,
                ))
            }
            Self::Enum {
                node,
                module,
                variant_idx,
                field_idx,
            } => loop {
                let variant = node.variants.get(*variant_idx)?;
                if let Some(field) = variant.fields.get(*field_idx) {
                    *field_idx += 1;
                    return Some(ordinary_type_use_site(
                        node.id.as_any(),
                        *module,
                        field.type_id,
                    ));
                }
                *variant_idx += 1;
                *field_idx = 0;
            },
            Self::TypeAlias {
                node,
                module,
                yielded,
            } => {
                if *yielded {
                    return None;
                }
                *yielded = true;
                Some(ordinary_type_use_site(
                    node.id.as_any(),
                    *module,
                    node.type_id,
                ))
            }
            Self::Union {
                node,
                module,
                field_idx,
            } => {
                let field = node.fields.get(*field_idx)?;
                *field_idx += 1;
                Some(ordinary_type_use_site(
                    node.id.as_any(),
                    *module,
                    field.type_id,
                ))
            }
            Self::Trait {
                node,
                module,
                super_idx,
                method_idx,
                method_state,
            } => {
                if let Some(type_id) = node.super_traits.get(*super_idx).copied() {
                    *super_idx += 1;
                    return Some(trait_type_use_site(node.id.as_any(), *module, type_id));
                }
                loop {
                    let method = node.methods.get(*method_idx)?;
                    if let Some(site) = next_callable_type_use(
                        method.id.as_any(),
                        *module,
                        &method.parameters,
                        method.return_type,
                        method_state,
                    ) {
                        return Some(site);
                    }
                    *method_idx += 1;
                    *method_state = MethodIterState::new();
                }
            }
            Self::Impl {
                node,
                module,
                yielded_self,
                yielded_trait,
                method_idx,
                method_state,
            } => {
                if !*yielded_self {
                    *yielded_self = true;
                    return Some(ordinary_type_use_site(
                        node.id.as_any(),
                        *module,
                        node.self_type,
                    ));
                }
                if !*yielded_trait {
                    *yielded_trait = true;
                    if let Some(type_id) = node.trait_type {
                        return Some(trait_type_use_site(node.id.as_any(), *module, type_id));
                    }
                }
                loop {
                    let method = node.methods.get(*method_idx)?;
                    if let Some(site) = next_callable_type_use(
                        method.id.as_any(),
                        *module,
                        &method.parameters,
                        method.return_type,
                        method_state,
                    ) {
                        return Some(site);
                    }
                    *method_idx += 1;
                    *method_state = MethodIterState::new();
                }
            }
            Self::Const {
                node,
                module,
                yielded,
            } => {
                if *yielded {
                    return None;
                }
                *yielded = true;
                Some(ordinary_type_use_site(
                    node.id.as_any(),
                    *module,
                    node.type_id,
                ))
            }
            Self::Static {
                node,
                module,
                yielded,
            } => {
                if *yielded {
                    return None;
                }
                *yielded = true;
                Some(ordinary_type_use_site(
                    node.id.as_any(),
                    *module,
                    node.type_id,
                ))
            }
        }
    }
}

fn next_callable_type_use(
    resolution_context_owner: AnyNodeId,
    module: Option<ModuleNodeId>,
    parameters: &[crate::parser::nodes::ParamData],
    return_type: Option<OrdinaryTypeUseId>,
    state: &mut MethodIterState,
) -> Option<TypeUseSite> {
    if let Some(param) = parameters.get(state.param_idx) {
        state.param_idx += 1;
        return Some(ordinary_type_use_site(
            resolution_context_owner,
            module,
            param.type_id,
        ));
    }
    if !state.yielded_return {
        state.yielded_return = true;
        if let Some(type_id) = return_type {
            return Some(ordinary_type_use_site(
                resolution_context_owner,
                module,
                type_id,
            ));
        }
    }
    None
}

fn ordinary_type_use_site(
    resolution_context_owner: AnyNodeId,
    containing_module: Option<ModuleNodeId>,
    source: OrdinaryTypeUseId,
) -> TypeUseSite {
    TypeUseSite {
        context: ResolutionContext {
            resolution_context_owner,
            containing_module,
        },
        source: TypeWorkItem::Ordinary(source),
    }
}

fn trait_type_use_site(
    resolution_context_owner: AnyNodeId,
    containing_module: Option<ModuleNodeId>,
    source: TraitTypeSourceId,
) -> TypeUseSite {
    TypeUseSite {
        context: ResolutionContext {
            resolution_context_owner,
            containing_module,
        },
        source: TypeWorkItem::Trait(source),
    }
}

struct TypeTreeRelationIter<'a, 'resolver> {
    resolver: &'resolver TypeRelationResolver<'a>,
    context: ResolutionContext,
    stack: [Option<TypeWorkItem>; MAX_TYPE_TREE_STACK],
    len: usize,
    steps: usize,
    terminal_error: Option<SynParserError>,
}

impl<'a, 'resolver> TypeTreeRelationIter<'a, 'resolver> {
    fn new(resolver: &'resolver TypeRelationResolver<'a>, site: TypeUseSite) -> Self {
        let mut iter = Self {
            resolver,
            context: site.context,
            stack: [None; MAX_TYPE_TREE_STACK],
            len: 0,
            steps: 0,
            terminal_error: None,
        };
        iter.push(site.source);
        iter
    }

    fn push(&mut self, work_item: TypeWorkItem) {
        if self.len >= MAX_TYPE_TREE_STACK {
            self.terminal_error = Some(SynParserError::InternalState(format!(
                "type resolution exceeded type-tree stack limit of {MAX_TYPE_TREE_STACK}"
            )));
            return;
        }
        self.stack[self.len] = Some(work_item);
        self.len += 1;
    }

    fn pop(&mut self) -> Option<TypeWorkItem> {
        if self.len == 0 {
            return None;
        }
        self.len -= 1;
        self.stack[self.len].take()
    }

    fn push_ordinary_children_rev(&mut self, ids: &[OrdinaryTypeUseId]) {
        for id in ids.iter().rev().copied() {
            self.push(TypeWorkItem::Ordinary(id));
        }
    }

    fn push_trait_children_rev(&mut self, ids: &[TraitTypeSourceId]) {
        for id in ids.iter().rev().copied() {
            self.push(TypeWorkItem::Trait(id));
        }
    }

    fn visit_ordinary(
        &mut self,
        resolver: &TypeRelationResolver<'a>,
        source: OrdinaryTypeUseId,
    ) -> Result<Option<TypeRelation>, SynParserError> {
        match resolver.type_node(source)? {
            TypeNode::Named(node) => {
                self.push_ordinary_children_rev(&node.arguments);
                resolver.resolve_ordinary_source(
                    self.context,
                    OrdinaryTypeSourceId::from(node.id),
                    &node.path,
                    node.is_fully_qualified,
                )
            }
            TypeNode::Reference(node) => {
                self.push(TypeWorkItem::Ordinary(node.referenced));
                Ok(None)
            }
            TypeNode::Slice(node) => {
                self.push(TypeWorkItem::Ordinary(node.element));
                Ok(None)
            }
            TypeNode::Array(node) => {
                self.push(TypeWorkItem::Ordinary(node.element));
                Ok(None)
            }
            TypeNode::Tuple(node) => {
                self.push_ordinary_children_rev(&node.elements);
                Ok(None)
            }
            TypeNode::Function(node) => {
                if let Some(return_type) = node.return_type {
                    self.push(TypeWorkItem::Ordinary(return_type));
                }
                self.push_ordinary_children_rev(&node.parameters);
                Ok(None)
            }
            TypeNode::Never(_)
            | TypeNode::Inferred(_)
            | TypeNode::Macro(_)
            | TypeNode::Unknown(_) => Ok(None),
            TypeNode::RawPointer(node) => {
                self.push(TypeWorkItem::Ordinary(node.pointee));
                Ok(None)
            }
            TypeNode::TraitObject(node) => {
                self.push_trait_children_rev(&node.bounds);
                Ok(None)
            }
            TypeNode::ImplTrait(node) => {
                self.push_trait_children_rev(&node.bounds);
                Ok(None)
            }
            TypeNode::TraitBound(_) => Err(SynParserError::InternalState(format!(
                "ordinary type-use work item resolved to trait-bound node {source}"
            ))),
            TypeNode::Paren(node) => {
                self.push(TypeWorkItem::Ordinary(node.inner));
                Ok(None)
            }
        }
    }

    fn visit_trait(
        &mut self,
        resolver: &TypeRelationResolver<'a>,
        source: TraitTypeSourceId,
    ) -> Result<Option<TypeRelation>, SynParserError> {
        match resolver.type_node(source)? {
            TypeNode::Named(node) => {
                self.push_ordinary_children_rev(&node.arguments);
                resolver.resolve_trait_source(
                    self.context,
                    TraitTypeSourceId::from(node.id),
                    &node.path,
                    node.is_fully_qualified,
                )
            }
            TypeNode::TraitBound(node) => {
                self.push_ordinary_children_rev(&node.arguments);
                resolver.resolve_trait_source(
                    self.context,
                    TraitTypeSourceId::from(node.id),
                    &node.path,
                    node.is_fully_qualified,
                )
            }
            _ => Err(SynParserError::InternalState(format!(
                "trait type-use work item resolved to non-trait-source node {source}"
            ))),
        }
    }
}

impl Iterator for TypeTreeRelationIter<'_, '_> {
    type Item = Result<TypeRelation, SynParserError>;

    fn next(&mut self) -> Option<Self::Item> {
        if let Some(err) = self.terminal_error.take() {
            return Some(Err(err));
        }

        let resolver = self.resolver;
        while let Some(work_item) = self.pop() {
            self.steps += 1;
            if self.steps > MAX_TYPE_TREE_STEPS {
                return Some(Err(SynParserError::InternalState(format!(
                    "type resolution exceeded type-tree step limit of {MAX_TYPE_TREE_STEPS}"
                ))));
            }

            let result = match work_item {
                TypeWorkItem::Ordinary(source) => self.visit_ordinary(resolver, source),
                TypeWorkItem::Trait(source) => self.visit_trait(resolver, source),
            };
            if let Some(err) = self.terminal_error.take() {
                return Some(Err(err));
            }
            match result {
                Ok(Some(relation)) => return Some(Ok(relation)),
                Ok(None) => {}
                Err(err) => return Some(Err(err)),
            }
        }

        None
    }
}

/// Resolves type uses into typed v2 relation facts after the `ModuleTree` has
/// been built.
pub fn resolve_type_relations_after_tree(
    graph: &ParsedCodeGraph,
    tree: &ModuleTree,
) -> Result<TypeRelationReport, SynParserError> {
    TypeRelationResolver::new(graph, tree).resolve_type_relations()
}
