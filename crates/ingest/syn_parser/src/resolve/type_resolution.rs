//! Type-resolution artifacts for the post-merge semantic bridge.
//!
//! `syn_parser` builds a structural type graph during phase 2: each observed type
//! occurrence gets a deterministic `TypeId::Synthetic` plus a `TypeNode`
//! describing its syntax-level shape. This module is the late semantic pass that
//! answers what a `TypeKind::Named` or `TypeKind::TraitBound` refers to once the
//! merged graph, module tree, import backlinks, and canonical path state exist.
//!
//! The important constraint is correctness:
//! - a named type should only be promoted to a resolved meaning when the merged
//!   graph justifies that claim;
//! - ambiguity should remain explicit;
//! - unresolved cases should retain provenance-rich reasons rather than being
//!   flattened into "best effort".
//!
//! Terminology in this module intentionally follows the Rust Reference where
//! possible:
//! - a **scope** is the region where a named entity may be referenced;
//! - **generic parameters** are in scope within the item definition where they
//!   are declared;
//! - methods, associated types, and associated constants are **associated
//!   items** of traits or implementations;
//! - implicit `Self` is treated similarly to a generic type parameter in
//!   structs, enums, unions, traits, and implementations.
//!
//! Reference anchors:
//! - <https://doc.rust-lang.org/reference/names/scopes.html>
//! - <https://doc.rust-lang.org/reference/items/generics.html>
//! - <https://doc.rust-lang.org/reference/items/associated-items.html>
//!
//! The implementation still has a local distinction that is not a Reference term:
//! a **type-use site** is the codegraph node where the resolved edge/report row
//! should be attached, while the **resolution context** is the module/import,
//! generic-parameter, and `Self` context used to interpret the type occurrence.
//! For a function item these are often the same node. For a struct field, the
//! type-use site is the field, but the relevant generic parameter scope is the
//! struct. For an associated item in an impl, the type-use site is the method or
//! associated item, while `Self` and impl-level generics come from the enclosing
//! implementation.

use std::collections::{BTreeMap, BTreeSet, HashSet, VecDeque};

use ploke_core::{TypeId, TypeKind};
use serde::{Deserialize, Serialize};
use tracing::debug;

use crate::{
    error::SynParserError,
    parser::{
        ParsedCodeGraph,
        graph::GraphAccess,
        nodes::{
            AnyNodeId, AsAnyNodeId, GenericParamNodeId, ImportNodeId, ModuleNodeId, NamedTypeId,
            NodePath, PrimaryNodeId, TraitBoundTypeId,
        },
        relations::SyntacticRelation,
        types::{GenericParamKind, GenericParamNode},
    },
    utils::LOG_TARGET_MOD_TREE_BUILD,
};

use super::{RelationIndexer, module_tree::ModuleTree};

/// The broad syntactic shape of a named type path before semantic resolution.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum PathForm {
    Plain,
    Crate,
    SelfPath,
    SuperPath,
    FullyQualified,
}

impl PathForm {
    pub fn from_segments(path: &[String], is_fully_qualified: bool) -> Self {
        if is_fully_qualified {
            return Self::FullyQualified;
        }

        match path.first().map(String::as_str) {
            Some("crate") => Self::Crate,
            Some("self") => Self::SelfPath,
            Some("super") => Self::SuperPath,
            _ => Self::Plain,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Plain => "plain",
            Self::Crate => "crate",
            Self::SelfPath => "self",
            Self::SuperPath => "super",
            Self::FullyQualified => "fully_qualified",
        }
    }
}

/// The semantic category of a successful type-resolution target.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum Kind {
    Item,
    GenericParam,
    SelfType,
}

impl Kind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Item => "item",
            Self::GenericParam => "generic_param",
            Self::SelfType => "self_type",
        }
    }
}

/// A successfully justified semantic target for a structural `TypeId`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Target {
    /// A named type or trait resolved to a defining item in the code graph.
    Item(AnyNodeId),
    /// A generic type parameter resolved to the `GenericParamNode` that declares it.
    GenericParam(GenericParamNodeId),
    /// `Self` resolved through its `Self` scope.
    ///
    /// In an implementation this should resolve through the implementing type,
    /// matching the Rust Reference's description of method `Self`. Trait `Self`
    /// needs a distinct representation as the resolver becomes more complete.
    SelfType(AnyNodeId),
}

impl Target {
    pub fn kind(&self) -> Kind {
        match self {
            Self::Item(_) => Kind::Item,
            Self::GenericParam(_) => Kind::GenericParam,
            Self::SelfType(_) => Kind::SelfType,
        }
    }
}

/// Why a named type could not be resolved to a unique semantic target.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum UnresolvedReason {
    EmptyPath,
    PrimitiveOrBuiltin,
    MissingOwnerContext,
    MissingModuleContext,
    GenericParamNotFound,
    SelfOutsideImplOrTrait,
    SelfTargetMissing,
    PathNotFound,
    NotTypeTarget,
    ExternalDependency,
    UnsupportedQualifiedSelf,
}

impl UnresolvedReason {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::EmptyPath => "empty_path",
            Self::PrimitiveOrBuiltin => "primitive_or_builtin",
            Self::MissingOwnerContext => "missing_owner_context",
            Self::MissingModuleContext => "missing_module_context",
            Self::GenericParamNotFound => "generic_param_not_found",
            Self::SelfOutsideImplOrTrait => "self_outside_impl_or_trait",
            Self::SelfTargetMissing => "self_target_missing",
            Self::PathNotFound => "path_not_found",
            Self::NotTypeTarget => "not_type_target",
            Self::ExternalDependency => "external_dependency",
            Self::UnsupportedQualifiedSelf => "unsupported_qualified_self",
        }
    }
}

/// Why a named type resolved to more than one plausible target.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum AmbiguousReason {
    MultipleVisibleDefinitions,
    DuplicateCfgVariants,
    ImportChainConflict,
}

impl AmbiguousReason {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::MultipleVisibleDefinitions => "multiple_visible_definitions",
            Self::DuplicateCfgVariants => "duplicate_cfg_variants",
            Self::ImportChainConflict => "import_chain_conflict",
        }
    }
}

/// Provenance needed to explain a late named-type resolution attempt.
///
/// This is the resolution-context half of a type-use report. It records the
/// syntactic type occurrence being resolved, the path found in the `TypeNode`,
/// and enough surrounding context to apply Rust name-resolution rules.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Provenance {
    /// Structural `TypeId` for the exact type occurrence being resolved.
    pub type_id: TypeId,
    /// Path segments as parsed from `TypeKind::Named` or `TypeKind::TraitBound`.
    pub path: Vec<String>,
    /// Coarse syntactic path form used for diagnostics and summary reporting.
    pub path_form: PathForm,
    /// Module whose item scope and imports are queried for path resolution.
    pub containing_module: Option<ModuleNodeId>,
    /// Codegraph node whose generic parameter scope and `Self` context should be used.
    ///
    /// This is intentionally not always the same as the final `TypeUseResolution::owner`.
    /// Example: in `struct S<T> { field: T }`, the report row belongs to the field
    /// type-use site, but `T` is declared by `S`. In Reference language, `S` is the
    /// item definition whose generic parameters are in scope for that field type.
    pub owner: Option<AnyNodeId>,
}

impl Provenance {
    pub fn new(
        type_id: TypeId,
        path: Vec<String>,
        is_fully_qualified: bool,
        containing_module: Option<ModuleNodeId>,
        owner: Option<AnyNodeId>,
    ) -> Self {
        let path_form = PathForm::from_segments(&path, is_fully_qualified);
        Self {
            type_id,
            path,
            path_form,
            containing_module,
            owner,
        }
    }

    pub fn display_path(&self) -> String {
        if self.path.is_empty() {
            "<empty>".to_string()
        } else {
            self.path.join("::")
        }
    }
}

/// Explicit unresolved state for a named type.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Unresolved {
    pub provenance: Provenance,
    pub reason: UnresolvedReason,
}

/// Explicit ambiguous state for a named type.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Ambiguous {
    pub provenance: Provenance,
    pub reason: AmbiguousReason,
    pub candidates: Vec<Target>,
}

/// Late semantic state for a structural `TypeId`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum State {
    Resolved(Target),
    Ambiguous(Ambiguous),
    Unresolved(Unresolved),
}

/// Final artifact emitted by the future named-type resolution pass.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Ref {
    pub provenance: Provenance,
    pub state: State,
}

impl Ref {
    pub fn resolved_target_kind(&self) -> Option<Kind> {
        match &self.state {
            State::Resolved(target) => Some(target.kind()),
            _ => None,
        }
    }
}

/// The kind of type-use site where a structural type occurrence was observed.
///
/// A role describes the slot where the type was written, not the semantic target
/// it eventually resolves to. For example, `impl Trait for Type` has separate
/// `ImplTrait` and `ImplSelf` roles even though both are represented by
/// structural `TypeNode`s before resolution.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum TypeUseRole {
    FunctionReturn,
    FunctionParam,
    MethodReturn,
    MethodParam,
    Field,
    Const,
    Static,
    TypeAliasTarget,
    ImplSelf,
    ImplTrait,
    TraitSuper,
}

impl TypeUseRole {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::FunctionReturn => "function_return",
            Self::FunctionParam => "function_param",
            Self::MethodReturn => "method_return",
            Self::MethodParam => "method_param",
            Self::Field => "field",
            Self::Const => "const",
            Self::Static => "static",
            Self::TypeAliasTarget => "type_alias_target",
            Self::ImplSelf => "impl_self",
            Self::ImplTrait => "impl_trait",
            Self::TraitSuper => "trait_super",
        }
    }

    fn expects_trait_target(self) -> bool {
        matches!(self, Self::ImplTrait | Self::TraitSuper)
    }
}

/// Semantic result for one resolvable type occurrence at a type-use site.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TypeUseResolution {
    /// Codegraph node where this type-use row should be attached.
    ///
    /// This is the local "type-use site" concept. It is usually the item carrying
    /// the slot (`FunctionNode`, `MethodNode`, `ConstNode`), but for fields it is
    /// the `FieldNode`, while generic lookup may use the enclosing struct/enum/union
    /// as the `Provenance::owner` resolution context.
    pub owner: AnyNodeId,
    /// Slot kind for the observed type occurrence.
    pub role: TypeUseRole,
    /// Structural source occurrence from the phase-2 type graph.
    pub source_type_id: TypeId,
    /// Resolved, ambiguous, or unresolved semantic state plus provenance.
    pub resolved_ref: Ref,
    /// Canonical resolved type ID when the target is item-backed.
    ///
    /// Generic params, builtins, external dependencies, ambiguous paths, and
    /// unresolved paths currently do not promote to a `TypeId::Resolved`.
    pub resolved_type_id: Option<TypeId>,
}

impl TypeUseResolution {
    pub fn item_target(&self) -> Option<AnyNodeId> {
        match self.resolved_ref.state {
            State::Resolved(Target::Item(item_id)) | State::Resolved(Target::SelfType(item_id)) => {
                Some(item_id)
            }
            State::Resolved(Target::GenericParam(_))
            | State::Ambiguous(_)
            | State::Unresolved(_) => None,
        }
    }
}

/// Collected output for a complete late type-resolution pass.
#[derive(Debug, Default, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TypeResolutionReport {
    pub resolutions: Vec<TypeUseResolution>,
    pub summary: Summary,
}

impl TypeResolutionReport {
    pub fn item_backed_resolutions(&self) -> impl Iterator<Item = &TypeUseResolution> {
        self.resolutions
            .iter()
            .filter(|resolution| resolution.item_target().is_some())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ExpectedTarget {
    Ordinary,
    Trait,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum BindingFailure {
    Unresolved(UnresolvedReason),
    Ambiguous(AmbiguousReason),
}

/// Late semantic resolver for named type paths once the merged module tree exists.
///
/// The important distinction is between:
/// - structural type nodes (`TypeId::Synthetic`, `TypeKind::Named`, `TypeKind::TraitBound`)
/// - semantic targets justified by module/import visibility
///
/// `LateResolver` works on one resolution context at a time. The caller supplies
/// the containing module plus the item/associated-item context used for generic
/// parameter and `Self` lookup. This mirrors Reference terminology: item scopes
/// and generic parameter scopes decide which names are in scope, while associated
/// items inside impls/traits inherit important context from their enclosing
/// implementation or trait.
///
/// This resolver does not guess. It walks the existing scope/import graph, including glob
/// bindings and re-export chains, and produces explicit resolved/ambiguous/unresolved states.
pub struct LateResolver<'a> {
    graph: &'a ParsedCodeGraph,
    tree: &'a ModuleTree,
}

impl<'a> LateResolver<'a> {
    pub fn new(graph: &'a ParsedCodeGraph, tree: &'a ModuleTree) -> Self {
        Self { graph, tree }
    }

    pub fn resolve_named(
        &self,
        type_id: NamedTypeId,
        containing_module: Option<ModuleNodeId>,
        owner: Option<AnyNodeId>,
    ) -> Ref {
        let type_node = self
            .graph
            .resolve_type(type_id.into())
            .expect("NamedTypeId should exist in the graph type table");
        let (path, is_fully_qualified) = match &type_node.kind {
            ploke_core::TypeKind::Named {
                path,
                is_fully_qualified,
            } => (path.clone(), *is_fully_qualified),
            other => panic!("NamedTypeId points to unexpected TypeKind: {other:?}"),
        };

        let provenance = Provenance::new(
            type_id.into(),
            path,
            is_fully_qualified,
            containing_module,
            owner,
        );
        self.resolve_named_path(provenance)
    }

    pub fn resolve_named_trait(
        &self,
        type_id: NamedTypeId,
        containing_module: Option<ModuleNodeId>,
        owner: Option<AnyNodeId>,
    ) -> Ref {
        let type_node = self
            .graph
            .resolve_type(type_id.into())
            .expect("NamedTypeId should exist in the graph type table");
        let (path, is_fully_qualified) = match &type_node.kind {
            ploke_core::TypeKind::Named {
                path,
                is_fully_qualified,
            } => (path.clone(), *is_fully_qualified),
            other => panic!("NamedTypeId points to unexpected TypeKind: {other:?}"),
        };

        let provenance = Provenance::new(
            type_id.into(),
            path,
            is_fully_qualified,
            containing_module,
            owner,
        );
        self.resolve_trait_path(provenance)
    }

    pub fn resolve_trait_bound(
        &self,
        type_id: TraitBoundTypeId,
        containing_module: Option<ModuleNodeId>,
        owner: Option<AnyNodeId>,
    ) -> Ref {
        let type_node = self
            .graph
            .resolve_type(type_id.into())
            .expect("TraitBoundTypeId should exist in the graph type table");
        let (path, is_fully_qualified) = match &type_node.kind {
            ploke_core::TypeKind::TraitBound {
                path,
                is_fully_qualified,
            } => (path.clone(), *is_fully_qualified),
            other => panic!("TraitBoundTypeId points to unexpected TypeKind: {other:?}"),
        };

        let provenance = Provenance::new(
            type_id.into(),
            path,
            is_fully_qualified,
            containing_module,
            owner,
        );
        self.resolve_trait_path(provenance)
    }

    pub fn resolve_named_path(&self, provenance: Provenance) -> Ref {
        self.resolve_provenance(provenance, ExpectedTarget::Ordinary)
    }

    pub fn resolve_trait_path(&self, provenance: Provenance) -> Ref {
        self.resolve_provenance(provenance, ExpectedTarget::Trait)
    }

    pub fn promote_resolved_type_id(
        &self,
        resolved: &Ref,
    ) -> Result<Option<TypeId>, SynParserError> {
        let item_id = match &resolved.state {
            State::Resolved(Target::Item(item_id)) | State::Resolved(Target::SelfType(item_id)) => {
                *item_id
            }
            State::Resolved(Target::GenericParam(_)) => return Ok(None),
            State::Ambiguous(_) | State::Unresolved(_) => return Ok(None),
        };

        let primary_id = PrimaryNodeId::try_from(item_id).map_err(|_| {
            SynParserError::InternalState(format!(
                "resolved type target {item_id:?} is not a primary node id"
            ))
        })?;
        let graph_node = self.graph.find_node_unique(item_id)?;
        let canonical_path = self.canonical_item_path(primary_id, graph_node.name())?;
        let file_path = self
            .tree
            .find_defining_file_path_ref_seq(primary_id)
            .map_err(SynParserError::from)?;

        Ok(Some(
            TypeId::generate_resolved(
                self.graph.crate_namespace,
                file_path,
                canonical_path.as_segments(),
                graph_node.cfgs(),
            )
            .map_err(SynParserError::from)?,
        ))
    }

    fn resolve_provenance(&self, provenance: Provenance, expected: ExpectedTarget) -> Ref {
        if provenance.path.is_empty() {
            return Ref {
                provenance: provenance.clone(),
                state: State::Unresolved(Unresolved {
                    provenance,
                    reason: UnresolvedReason::EmptyPath,
                }),
            };
        }

        if self.is_builtin_path(&provenance.path) {
            return Ref {
                provenance: provenance.clone(),
                state: State::Unresolved(Unresolved {
                    provenance,
                    reason: UnresolvedReason::PrimitiveOrBuiltin,
                }),
            };
        }

        if provenance.path == ["Self".to_string()] {
            return self.resolve_self_type(provenance);
        }

        if let Some(owner) = provenance.owner {
            if let Some(generic_param_id) =
                self.resolve_generic_param(owner, provenance.path.as_slice(), expected)
            {
                return Ref {
                    provenance,
                    state: State::Resolved(Target::GenericParam(generic_param_id)),
                };
            }
        }

        let containing_module = provenance
            .containing_module
            .or_else(|| provenance.owner.and_then(|owner| self.owner_module(owner)));
        let Some(start_module) = containing_module else {
            return Ref {
                provenance: provenance.clone(),
                state: State::Unresolved(Unresolved {
                    provenance,
                    reason: UnresolvedReason::MissingModuleContext,
                }),
            };
        };

        let (mut active_modules, segments) = match self.start_modules(&provenance, start_module) {
            Ok(start) => start,
            Err(reason) => {
                return Ref {
                    provenance: provenance.clone(),
                    state: State::Unresolved(Unresolved { provenance, reason }),
                };
            }
        };

        if segments.is_empty() {
            return Ref {
                provenance: provenance.clone(),
                state: State::Unresolved(Unresolved {
                    provenance,
                    reason: UnresolvedReason::EmptyPath,
                }),
            };
        }

        for (idx, segment) in segments.iter().enumerate() {
            let is_last = idx == segments.len() - 1;
            let mut next_targets = BTreeSet::new();
            let mut binding_failure = None;

            for module_id in &active_modules {
                let bindings = self.scope_candidates(*module_id, segment.as_str());
                for binding in bindings {
                    match self.resolve_binding_terminals(binding) {
                        Ok(terminals) => {
                            for terminal in terminals {
                                next_targets.insert(terminal);
                            }
                        }
                        Err(failure) => {
                            binding_failure.get_or_insert(failure);
                        }
                    }
                }
            }

            if next_targets.is_empty() {
                let reason = binding_failure
                    .map(|failure| match failure {
                        BindingFailure::Unresolved(reason) => reason,
                        BindingFailure::Ambiguous(_) => UnresolvedReason::PathNotFound,
                    })
                    .unwrap_or_else(|| self.unresolved_reason_for_missing_path(segment));
                return Ref {
                    provenance: provenance.clone(),
                    state: State::Unresolved(Unresolved { provenance, reason }),
                };
            }

            if is_last {
                return self.classify_terminal_targets(
                    provenance,
                    next_targets.into_iter().collect(),
                    expected,
                );
            }

            active_modules = next_targets
                .into_iter()
                .filter_map(|target| ModuleNodeId::try_from(target).ok())
                .collect();

            if active_modules.is_empty() {
                return Ref {
                    provenance: provenance.clone(),
                    state: State::Unresolved(Unresolved {
                        provenance,
                        reason: UnresolvedReason::PathNotFound,
                    }),
                };
            }

            if active_modules.len() > 1 {
                return Ref {
                    provenance: provenance.clone(),
                    state: State::Ambiguous(Ambiguous {
                        provenance,
                        reason: AmbiguousReason::MultipleVisibleDefinitions,
                        candidates: active_modules
                            .iter()
                            .copied()
                            .map(|module_id| Target::Item(module_id.as_any()))
                            .collect(),
                    }),
                };
            }
        }

        Ref {
            provenance: provenance.clone(),
            state: State::Unresolved(Unresolved {
                provenance,
                reason: UnresolvedReason::PathNotFound,
            }),
        }
    }

    fn resolve_self_type(&self, provenance: Provenance) -> Ref {
        let Some(owner) = provenance.owner else {
            return Ref {
                provenance: provenance.clone(),
                state: State::Unresolved(Unresolved {
                    provenance,
                    reason: UnresolvedReason::MissingOwnerContext,
                }),
            };
        };

        let Some(owner_node) = self.graph.find_node_unique(owner).ok() else {
            return Ref {
                provenance: provenance.clone(),
                state: State::Unresolved(Unresolved {
                    provenance,
                    reason: UnresolvedReason::SelfTargetMissing,
                }),
            };
        };

        let impl_node = if let Some(impl_node) = owner_node.as_impl() {
            impl_node
        } else if let Some(impl_node) = self.impl_for_method_owner(owner) {
            impl_node
        } else {
            return Ref {
                provenance: provenance.clone(),
                state: State::Unresolved(Unresolved {
                    provenance,
                    reason: UnresolvedReason::SelfOutsideImplOrTrait,
                }),
            };
        };

        let Some(self_type_node) = self.graph.resolve_type(impl_node.self_type()) else {
            return Ref {
                provenance: provenance.clone(),
                state: State::Unresolved(Unresolved {
                    provenance,
                    reason: UnresolvedReason::SelfTargetMissing,
                }),
            };
        };

        let ploke_core::TypeKind::Named {
            path,
            is_fully_qualified,
        } = &self_type_node.kind
        else {
            return Ref {
                provenance: provenance.clone(),
                state: State::Unresolved(Unresolved {
                    provenance,
                    reason: UnresolvedReason::SelfTargetMissing,
                }),
            };
        };

        let nested = Provenance::new(
            provenance.type_id,
            path.clone(),
            *is_fully_qualified,
            provenance
                .containing_module
                .or_else(|| self.owner_module(owner)),
            Some(owner),
        );
        let resolved = self.resolve_provenance(nested.clone(), ExpectedTarget::Ordinary);

        match resolved.state {
            State::Resolved(Target::Item(item_id)) => Ref {
                provenance,
                state: State::Resolved(Target::SelfType(item_id)),
            },
            State::Resolved(Target::SelfType(item_id)) => Ref {
                provenance,
                state: State::Resolved(Target::SelfType(item_id)),
            },
            State::Resolved(Target::GenericParam(_))
            | State::Ambiguous(_)
            | State::Unresolved(_) => Ref {
                provenance: provenance.clone(),
                state: State::Unresolved(Unresolved {
                    provenance,
                    reason: UnresolvedReason::SelfTargetMissing,
                }),
            },
        }
    }

    fn start_modules(
        &self,
        provenance: &Provenance,
        start_module: ModuleNodeId,
    ) -> Result<(BTreeSet<ModuleNodeId>, Vec<String>), UnresolvedReason> {
        if provenance.path_form == PathForm::FullyQualified {
            return Err(UnresolvedReason::UnsupportedQualifiedSelf);
        }

        let mut current_module = start_module;
        let mut idx = 0usize;
        while let Some(segment) = provenance.path.get(idx).map(String::as_str) {
            match segment {
                "crate" => {
                    current_module = self.tree.root();
                    idx += 1;
                }
                "self" => {
                    idx += 1;
                }
                "super" => {
                    current_module = self
                        .tree
                        .get_parent_module_id(current_module)
                        .ok_or(UnresolvedReason::PathNotFound)?;
                    idx += 1;
                }
                _ => break,
            }
        }

        let mut modules = BTreeSet::new();
        modules.insert(current_module);
        Ok((modules, provenance.path[idx..].to_vec()))
    }

    fn scope_candidates(&self, module_id: ModuleNodeId, segment: &str) -> Vec<AnyNodeId> {
        let mut candidates = BTreeSet::new();

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
                candidates.insert(target_any);
            }

            if let Some(import_node) = node.as_import() {
                if import_node.is_glob {
                    for glob_match in self.glob_candidates(import_node.id, segment) {
                        candidates.insert(glob_match);
                    }
                }
            }
        }

        candidates.into_iter().collect()
    }

    fn glob_candidates(&self, import_id: ImportNodeId, segment: &str) -> Vec<AnyNodeId> {
        let mut matches = BTreeSet::new();

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
                matches.insert(source_any);
            }
        }

        matches.into_iter().collect()
    }

    fn resolve_binding_terminals(
        &self,
        start: AnyNodeId,
    ) -> Result<Vec<AnyNodeId>, BindingFailure> {
        let mut queue = VecDeque::from([start]);
        let mut visited_imports = HashSet::new();
        let mut terminals = BTreeSet::new();

        while let Some(current) = queue.pop_front() {
            if let Ok(import_id) = ImportNodeId::try_from(current) {
                if !visited_imports.insert(import_id) {
                    return Err(BindingFailure::Ambiguous(
                        AmbiguousReason::ImportChainConflict,
                    ));
                }

                let mut had_sources = false;
                for relation in self.tree.get_iter_relations_to(&import_id.as_any()) {
                    let SyntacticRelation::ImportedBy { source, target } = relation.rel() else {
                        continue;
                    };
                    if *target != import_id {
                        continue;
                    }
                    had_sources = true;
                    queue.push_back(source.as_any());
                }

                if had_sources {
                    continue;
                }

                let import_node = self
                    .graph
                    .get_import_checked(import_id)
                    .map_err(|_| BindingFailure::Unresolved(UnresolvedReason::PathNotFound))?;
                if self.is_external_root(import_node.source_path().first().map(String::as_str)) {
                    return Err(BindingFailure::Unresolved(
                        UnresolvedReason::ExternalDependency,
                    ));
                }
                return Err(BindingFailure::Unresolved(UnresolvedReason::PathNotFound));
            }

            terminals.insert(current);
        }

        Ok(terminals.into_iter().collect())
    }

    fn classify_terminal_targets(
        &self,
        provenance: Provenance,
        terminals: Vec<AnyNodeId>,
        expected: ExpectedTarget,
    ) -> Ref {
        let mut resolved_targets = BTreeSet::new();
        let mut saw_unresolved = false;

        for terminal in terminals {
            let Ok(node) = self.graph.find_node_unique(terminal) else {
                continue;
            };

            if node.as_unresolved().is_some() {
                saw_unresolved = true;
                continue;
            }

            let accepted = match expected {
                ExpectedTarget::Ordinary => {
                    node.as_struct().is_some()
                        || node.as_enum().is_some()
                        || node.as_union().is_some()
                        || node.as_type_alias().is_some()
                }
                ExpectedTarget::Trait => node.as_trait().is_some(),
            };

            if accepted {
                resolved_targets.insert(terminal);
            }
        }

        match resolved_targets.len() {
            0 if saw_unresolved => Ref {
                provenance: provenance.clone(),
                state: State::Unresolved(Unresolved {
                    provenance,
                    reason: UnresolvedReason::PathNotFound,
                }),
            },
            0 => Ref {
                provenance: provenance.clone(),
                state: State::Unresolved(Unresolved {
                    provenance,
                    reason: UnresolvedReason::NotTypeTarget,
                }),
            },
            1 => Ref {
                provenance,
                state: State::Resolved(Target::Item(
                    resolved_targets
                        .into_iter()
                        .next()
                        .expect("one resolved target"),
                )),
            },
            _ => Ref {
                provenance: provenance.clone(),
                state: State::Ambiguous(Ambiguous {
                    provenance,
                    reason: AmbiguousReason::MultipleVisibleDefinitions,
                    candidates: resolved_targets.into_iter().map(Target::Item).collect(),
                }),
            },
        }
    }

    fn owner_module(&self, owner: AnyNodeId) -> Option<ModuleNodeId> {
        self.tree
            .get_iter_relations_to(&owner)
            .find_map(|relation| match relation.rel() {
                SyntacticRelation::Contains { source, target } if target.as_any() == owner => {
                    Some(*source)
                }
                _ => None,
            })
    }

    fn resolve_generic_param(
        &self,
        owner: AnyNodeId,
        path: &[String],
        expected: ExpectedTarget,
    ) -> Option<GenericParamNodeId> {
        // The Rust Reference says generic parameters are in scope within the item definition
        // where they are declared. This lookup currently checks one declaring item at a time.
        // Associated items may need an enclosing impl/trait lookup before this fully matches
        // Rust's effective resolution context for method signatures.
        if expected != ExpectedTarget::Ordinary || path.len() != 1 {
            return None;
        }

        let owner_node = self.graph.find_node_unique(owner).ok()?;
        let generic_params: &[GenericParamNode] = if let Some(node) = owner_node.as_function() {
            &node.generic_params
        } else if let Some(node) = owner_node.as_method() {
            &node.generic_params
        } else if let Some(node) = owner_node.as_struct() {
            &node.generic_params
        } else if let Some(node) = owner_node.as_enum() {
            &node.generic_params
        } else if let Some(node) = owner_node.as_union() {
            &node.generic_params
        } else if let Some(node) = owner_node.as_type_alias() {
            &node.generic_params
        } else if let Some(node) = owner_node.as_trait() {
            &node.generic_params
        } else if let Some(node) = owner_node.as_impl() {
            node.generic_params()
        } else {
            &[]
        };

        generic_params.iter().find_map(|param| match &param.kind {
            GenericParamKind::Type { name, .. } if name == &path[0] => Some(param.id),
            _ => None,
        })
    }

    fn impl_for_method_owner(&self, owner: AnyNodeId) -> Option<&crate::parser::nodes::ImplNode> {
        let AnyNodeId::Method(method_id) = owner else {
            return None;
        };

        self.graph.impls().iter().find(|impl_node| {
            impl_node
                .methods
                .iter()
                .any(|method| method.id == method_id)
        })
    }

    fn canonical_item_path(
        &self,
        item_id: PrimaryNodeId,
        name: &str,
    ) -> Result<NodePath, SynParserError> {
        let containing_module = self
            .tree
            .get_iter_relations_to(&item_id.as_any())
            .find_map(|relation| match relation.rel() {
                SyntacticRelation::Contains { source, target } if *target == item_id => {
                    Some(*source)
                }
                _ => None,
            })
            .ok_or_else(|| {
                SynParserError::InternalState(format!(
                    "containing module not found for item {} while generating canonical type path",
                    item_id.as_any()
                ))
            })?;

        let module = self.tree.modules().get(&containing_module).ok_or_else(|| {
            SynParserError::InternalState(format!(
                "module {} missing from ModuleTree while generating canonical type path",
                containing_module
            ))
        })?;
        let mut path = module.path().clone();
        path.push(name.to_string());
        NodePath::try_from(path)
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

    fn unresolved_reason_for_missing_path(&self, segment: &str) -> UnresolvedReason {
        if self.is_external_root(Some(segment)) {
            UnresolvedReason::ExternalDependency
        } else {
            UnresolvedReason::PathNotFound
        }
    }
}

pub fn resolve_type_uses_after_tree(
    graph: &ParsedCodeGraph,
    tree: &ModuleTree,
) -> Result<TypeResolutionReport, SynParserError> {
    let mut walker = TypeUseWalker::new(graph, tree);
    walker.collect()?;
    let summary = Summary::from_results(
        walker
            .resolutions
            .iter()
            .map(|resolution| &resolution.resolved_ref),
    );

    Ok(TypeResolutionReport {
        resolutions: walker.resolutions,
        summary,
    })
}

struct TypeUseWalker<'a> {
    graph: &'a ParsedCodeGraph,
    resolver: LateResolver<'a>,
    resolutions: Vec<TypeUseResolution>,
}

impl<'a> TypeUseWalker<'a> {
    fn new(graph: &'a ParsedCodeGraph, tree: &'a ModuleTree) -> Self {
        Self {
            graph,
            resolver: LateResolver::new(graph, tree),
            resolutions: Vec::new(),
        }
    }

    fn collect(&mut self) -> Result<(), SynParserError> {
        for function in self.graph.functions() {
            let owner = function.id.as_any();
            let module = self.containing_module(owner);
            for param in &function.parameters {
                self.visit_type_use(
                    owner,
                    owner,
                    module,
                    TypeUseRole::FunctionParam,
                    param.type_id,
                )?;
            }
            if let Some(type_id) = function.return_type {
                self.visit_type_use(owner, owner, module, TypeUseRole::FunctionReturn, type_id)?;
            }
        }

        for defined_type in self.graph.defined_types() {
            match defined_type {
                crate::parser::nodes::TypeDefNode::Struct(node) => {
                    let resolution_context_owner = node.id.as_any();
                    let module = self.containing_module(resolution_context_owner);
                    for field in &node.fields {
                        self.visit_type_use(
                            field.id.as_any(),
                            resolution_context_owner,
                            module,
                            TypeUseRole::Field,
                            field.type_id,
                        )?;
                    }
                }
                crate::parser::nodes::TypeDefNode::Enum(node) => {
                    let resolution_context_owner = node.id.as_any();
                    let module = self.containing_module(resolution_context_owner);
                    for variant in &node.variants {
                        for field in &variant.fields {
                            self.visit_type_use(
                                field.id.as_any(),
                                resolution_context_owner,
                                module,
                                TypeUseRole::Field,
                                field.type_id,
                            )?;
                        }
                    }
                }
                crate::parser::nodes::TypeDefNode::TypeAlias(node) => {
                    let owner = node.id.as_any();
                    let module = self.containing_module(owner);
                    self.visit_type_use(
                        owner,
                        owner,
                        module,
                        TypeUseRole::TypeAliasTarget,
                        node.type_id,
                    )?;
                }
                crate::parser::nodes::TypeDefNode::Union(node) => {
                    let resolution_context_owner = node.id.as_any();
                    let module = self.containing_module(resolution_context_owner);
                    for field in &node.fields {
                        self.visit_type_use(
                            field.id.as_any(),
                            resolution_context_owner,
                            module,
                            TypeUseRole::Field,
                            field.type_id,
                        )?;
                    }
                }
            }
        }

        for trait_node in self.graph.traits() {
            let owner = trait_node.id.as_any();
            let module = self.containing_module(owner);
            for type_id in &trait_node.super_traits {
                self.visit_type_use(owner, owner, module, TypeUseRole::TraitSuper, *type_id)?;
            }
            for method in &trait_node.methods {
                self.collect_method(method, method.id.as_any(), module, true)?;
            }
        }

        for impl_node in self.graph.impls() {
            let owner = impl_node.id.as_any();
            let module = self.containing_module(owner);
            self.visit_type_use(
                owner,
                owner,
                module,
                TypeUseRole::ImplSelf,
                impl_node.self_type,
            )?;
            if let Some(type_id) = impl_node.trait_type {
                self.visit_type_use(owner, owner, module, TypeUseRole::ImplTrait, type_id)?;
            }
            for method in &impl_node.methods {
                self.collect_method(method, method.id.as_any(), module, false)?;
            }
        }

        for const_node in self.graph.consts() {
            let owner = const_node.id.as_any();
            let module = self.containing_module(owner);
            self.visit_type_use(owner, owner, module, TypeUseRole::Const, const_node.type_id)?;
        }

        for static_node in self.graph.statics() {
            let owner = static_node.id.as_any();
            let module = self.containing_module(owner);
            self.visit_type_use(
                owner,
                owner,
                module,
                TypeUseRole::Static,
                static_node.type_id,
            )?;
        }

        Ok(())
    }

    fn collect_method(
        &mut self,
        method: &crate::parser::nodes::MethodNode,
        owner: AnyNodeId,
        module: Option<ModuleNodeId>,
        _in_trait: bool,
    ) -> Result<(), SynParserError> {
        for param in &method.parameters {
            self.visit_type_use(
                owner,
                owner,
                module,
                TypeUseRole::MethodParam,
                param.type_id,
            )?;
        }
        if let Some(type_id) = method.return_type {
            self.visit_type_use(owner, owner, module, TypeUseRole::MethodReturn, type_id)?;
        }
        Ok(())
    }

    fn visit_type_use(
        &mut self,
        type_use_site: AnyNodeId,
        resolution_context_owner: AnyNodeId,
        containing_module: Option<ModuleNodeId>,
        role: TypeUseRole,
        type_id: TypeId,
    ) -> Result<(), SynParserError> {
        // `type_use_site` is where the semantic row is attached. `resolution_context_owner`
        // is where generic parameters and `Self` are looked up. These are often the same
        // node, but they intentionally differ for fields and should eventually differ for
        // associated items whose signatures use impl/trait-level generic parameters.
        let mut seen = HashSet::new();
        self.visit_type_tree(
            type_use_site,
            resolution_context_owner,
            containing_module,
            role,
            type_id,
            role.expects_trait_target(),
            &mut seen,
        )
    }

    fn visit_type_tree(
        &mut self,
        type_use_site: AnyNodeId,
        resolution_context_owner: AnyNodeId,
        containing_module: Option<ModuleNodeId>,
        role: TypeUseRole,
        type_id: TypeId,
        expects_trait_target: bool,
        seen: &mut HashSet<TypeId>,
    ) -> Result<(), SynParserError> {
        if !seen.insert(type_id) {
            return Ok(());
        }

        let type_node = self.graph.resolve_type(type_id).ok_or_else(|| {
            SynParserError::InternalState(format!(
                "type use {type_id} referenced by {type_use_site:?} was not found in type graph"
            ))
        })?;
        let related_types = type_node.related_types.clone();

        let resolved_ref = match &type_node.kind {
            TypeKind::Named { .. } => {
                let named_id = NamedTypeId::try_from(type_node).map_err(|err| {
                    SynParserError::InternalState(format!(
                        "failed to refine named type {type_id}: {err}"
                    ))
                })?;
                if expects_trait_target {
                    Some(self.resolver.resolve_named_trait(
                        named_id,
                        containing_module,
                        Some(resolution_context_owner),
                    ))
                } else {
                    Some(self.resolver.resolve_named(
                        named_id,
                        containing_module,
                        Some(resolution_context_owner),
                    ))
                }
            }
            TypeKind::TraitBound { .. } => {
                let trait_bound_id = TraitBoundTypeId::try_from(type_node).map_err(|err| {
                    SynParserError::InternalState(format!(
                        "failed to refine trait-bound type {type_id}: {err}"
                    ))
                })?;
                Some(self.resolver.resolve_trait_bound(
                    trait_bound_id,
                    containing_module,
                    Some(resolution_context_owner),
                ))
            }
            _ => None,
        };

        if let Some(resolved_ref) = resolved_ref {
            let resolved_type_id = self.resolver.promote_resolved_type_id(&resolved_ref)?;
            self.resolutions.push(TypeUseResolution {
                owner: type_use_site,
                role,
                source_type_id: type_id,
                resolved_ref,
                resolved_type_id,
            });
        }

        for related_type_id in related_types {
            self.visit_type_tree(
                type_use_site,
                resolution_context_owner,
                containing_module,
                role,
                related_type_id,
                false,
                seen,
            )?;
        }

        Ok(())
    }

    fn containing_module(&self, resolution_context_owner: AnyNodeId) -> Option<ModuleNodeId> {
        self.graph
            .module_for_any_id(resolution_context_owner)
            .map(|module| module.id)
    }
}

/// Debug-focused summary surface for the late named-type resolution pass.
#[derive(Debug, Default, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Summary {
    pub total: usize,
    pub resolved: usize,
    pub ambiguous: usize,
    pub unresolved: usize,
    pub by_path_form: BTreeMap<PathForm, usize>,
    pub by_target_kind: BTreeMap<Kind, usize>,
    pub by_unresolved_reason: BTreeMap<UnresolvedReason, usize>,
    pub by_ambiguous_reason: BTreeMap<AmbiguousReason, usize>,
}

impl Summary {
    pub fn record(&mut self, resolved: &Ref) {
        self.total += 1;
        *self
            .by_path_form
            .entry(resolved.provenance.path_form)
            .or_default() += 1;

        match &resolved.state {
            State::Resolved(target) => {
                self.resolved += 1;
                *self.by_target_kind.entry(target.kind()).or_default() += 1;
            }
            State::Ambiguous(ambiguous) => {
                self.ambiguous += 1;
                *self
                    .by_ambiguous_reason
                    .entry(ambiguous.reason)
                    .or_default() += 1;
            }
            State::Unresolved(unresolved) => {
                self.unresolved += 1;
                *self
                    .by_unresolved_reason
                    .entry(unresolved.reason)
                    .or_default() += 1;
            }
        }
    }

    pub fn from_results<'a>(results: impl IntoIterator<Item = &'a Ref>) -> Self {
        let mut summary = Self::default();
        for result in results {
            summary.record(result);
        }
        summary
    }

    /// Emits a compact debug summary to help validate the pass during development.
    pub fn log_debug(&self, root: Option<&NodePath>) {
        debug!(
            target: LOG_TARGET_MOD_TREE_BUILD,
            root = ?root.map(ToString::to_string),
            total = self.total,
            resolved = self.resolved,
            ambiguous = self.ambiguous,
            unresolved = self.unresolved,
            by_path_form = ?self
                .by_path_form
                .iter()
                .map(|(kind, count)| (kind.as_str(), count))
                .collect::<Vec<_>>(),
            by_target_kind = ?self
                .by_target_kind
                .iter()
                .map(|(kind, count)| (kind.as_str(), count))
                .collect::<Vec<_>>(),
            by_unresolved_reason = ?self
                .by_unresolved_reason
                .iter()
                .map(|(reason, count)| (reason.as_str(), count))
                .collect::<Vec<_>>(),
            by_ambiguous_reason = ?self
                .by_ambiguous_reason
                .iter()
                .map(|(reason, count)| (reason.as_str(), count))
                .collect::<Vec<_>>(),
            "type resolution summary"
        );
    }
}

#[cfg(test)]
mod tests {
    use ploke_core::IdTrait;
    use ploke_core::TypeId;
    use uuid::Uuid;

    use super::*;
    use crate::{
        parser::{
            graph::GraphAccess,
            nodes::{GraphNode, TypeDefNode},
        },
        utils::test_setup::build_tree_for_tests,
    };

    fn trait_id_in_module(
        graph: &ParsedCodeGraph,
        tree: &ModuleTree,
        module_path: &[&str],
        name: &str,
    ) -> AnyNodeId {
        let module = graph
            .find_module_by_path_checked(
                &module_path
                    .iter()
                    .map(|seg| (*seg).to_string())
                    .collect::<Vec<_>>(),
            )
            .expect("module for trait lookup");

        tree.get_iter_relations_from(&module.id.as_any())
            .into_iter()
            .flatten()
            .find_map(|relation| match relation.rel() {
                SyntacticRelation::Contains { target, .. } => {
                    let node = graph.find_node_unique(target.as_any()).ok()?;
                    node.as_trait()
                        .filter(|trait_node| trait_node.name == name)
                        .map(|trait_node| trait_node.id.as_any())
                }
                _ => None,
            })
            .expect("trait in module")
    }

    #[test]
    fn path_form_classifies_common_named_paths() {
        assert_eq!(
            PathForm::from_segments(&["crate".into(), "Thing".into()], false),
            PathForm::Crate
        );
        assert_eq!(
            PathForm::from_segments(&["self".into(), "Thing".into()], false),
            PathForm::SelfPath
        );
        assert_eq!(
            PathForm::from_segments(&["super".into(), "Thing".into()], false),
            PathForm::SuperPath
        );
        assert_eq!(
            PathForm::from_segments(&["Thing".into()], false),
            PathForm::Plain
        );
        assert_eq!(
            PathForm::from_segments(&["Thing".into()], true),
            PathForm::FullyQualified
        );
    }

    #[test]
    fn summary_counts_resolution_states_and_kinds() {
        let type_id = TypeId::Synthetic(Uuid::nil());
        let resolved = Ref {
            provenance: Provenance::new(
                type_id,
                vec!["crate".into(), "Thing".into()],
                false,
                None,
                None,
            ),
            state: State::Resolved(Target::Item(AnyNodeId::Unresolved(
                crate::parser::nodes::UnresolvedNodeId::new(ploke_core::NodeId::Synthetic(
                    Uuid::nil(),
                )),
            ))),
        };
        let unresolved = Ref {
            provenance: Provenance::new(type_id, vec!["T".into()], false, None, None),
            state: State::Unresolved(Unresolved {
                provenance: Provenance::new(type_id, vec!["T".into()], false, None, None),
                reason: UnresolvedReason::GenericParamNotFound,
            }),
        };

        let summary = Summary::from_results([&resolved, &unresolved]);
        assert_eq!(summary.total, 2);
        assert_eq!(summary.resolved, 1);
        assert_eq!(summary.unresolved, 1);
        assert_eq!(summary.ambiguous, 0);
        assert_eq!(summary.by_target_kind.get(&Kind::Item), Some(&1));
        assert_eq!(
            summary
                .by_unresolved_reason
                .get(&UnresolvedReason::GenericParamNotFound),
            Some(&1)
        );
    }

    #[test]
    fn resolves_named_type_through_renamed_import_and_promotes_type_id() {
        let (graph, tree) = build_tree_for_tests("fixture_nodes");
        let resolver = LateResolver::new(&graph, &tree);
        let imports_module = graph
            .find_module_by_path_checked(&["crate".to_string(), "imports".to_string()])
            .expect("imports module");
        let sample_struct_id = graph
            .defined_types()
            .iter()
            .find_map(|def| match def {
                TypeDefNode::Struct(node) if node.name == "SampleStruct" => Some(node.id.as_any()),
                _ => None,
            })
            .expect("SampleStruct");

        let named_type = Ref {
            provenance: Provenance::new(
                TypeId::Synthetic(Uuid::nil()),
                vec!["MySimpleStruct".to_string()],
                false,
                Some(imports_module.id),
                None,
            ),
            state: State::Unresolved(Unresolved {
                provenance: Provenance::new(
                    TypeId::Synthetic(Uuid::nil()),
                    vec!["MySimpleStruct".to_string()],
                    false,
                    Some(imports_module.id),
                    None,
                ),
                reason: UnresolvedReason::PathNotFound,
            }),
        };

        let resolved = resolver.resolve_named_path(named_type.provenance.clone());
        assert_eq!(
            resolved.state,
            State::Resolved(Target::Item(sample_struct_id))
        );

        let promoted = resolver
            .promote_resolved_type_id(&resolved)
            .expect("promote resolved item type id")
            .expect("item-backed type should promote");
        assert!(matches!(promoted, TypeId::Resolved(_)));
    }

    #[test]
    fn resolves_trait_bound_through_glob_import() {
        let (graph, tree) = build_tree_for_tests("fixture_nodes");
        let resolver = LateResolver::new(&graph, &tree);
        let imports_module = graph
            .find_module_by_path_checked(&["crate".to_string(), "imports".to_string()])
            .expect("imports module");
        let documented_trait_id =
            trait_id_in_module(&graph, &tree, &["crate", "traits"], "DocumentedTrait");

        let resolved = resolver.resolve_trait_path(Provenance::new(
            TypeId::Synthetic(Uuid::nil()),
            vec!["DocumentedTrait".to_string()],
            false,
            Some(imports_module.id),
            None,
        ));

        assert_eq!(
            resolved.state,
            State::Resolved(Target::Item(documented_trait_id))
        );
    }

    #[test]
    fn resolves_trait_bound_through_multi_hop_reexport_chain() {
        let (graph, tree) = build_tree_for_tests("fixture_nodes");
        let resolver = LateResolver::new(&graph, &tree);
        let chain_module = graph
            .find_module_by_path_checked(&[
                "crate".to_string(),
                "imports".to_string(),
                "trait_chain".to_string(),
            ])
            .expect("trait_chain module");
        let simple_trait_id =
            trait_id_in_module(&graph, &tree, &["crate", "traits"], "SimpleTrait");

        let resolved = resolver.resolve_trait_path(Provenance::new(
            TypeId::Synthetic(Uuid::nil()),
            vec!["ChainPublicTraitAlias".to_string()],
            false,
            Some(chain_module.id),
            None,
        ));

        assert_eq!(
            resolved.state,
            State::Resolved(Target::Item(simple_trait_id))
        );
    }

    #[test]
    fn report_resolves_impl_method_self_return_types() {
        let (graph, tree) = build_tree_for_tests("fixture_nodes");
        let report =
            resolve_type_uses_after_tree(&graph, &tree).expect("late type-use resolution report");
        let method = graph
            .impls()
            .iter()
            .flat_map(|impl_node| &impl_node.methods)
            .find(|method| method.name == "new")
            .expect("fixture method");

        let resolved_method_return_targets = report
            .resolutions
            .iter()
            .filter(|resolution| {
                resolution.owner == method.id.as_any()
                    && resolution.role == TypeUseRole::MethodReturn
                    && resolution.resolved_type_id.is_some()
            })
            .filter_map(TypeUseResolution::item_target)
            .filter_map(|target| graph.find_node_unique(target).ok())
            .map(|node| node.name().to_string())
            .collect::<BTreeSet<_>>();

        assert!(
            resolved_method_return_targets.contains("SimpleStruct"),
            "Self return should include resolved SimpleStruct target; got {resolved_method_return_targets:?}"
        );
    }
}
