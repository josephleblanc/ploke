#![cfg(feature = "typed_type_graph")]

//! Typed type-relation helpers for Phase 3 resolution tests.
//!
//! These helpers assert the v2 resolver surface directly:
//!
//! ```text
//! Ordinary ⊆ OrdinaryTypeSourceId × OrdinaryTypeTargetId
//! Trait    ⊆ TraitTypeSourceId    × TraitTypeTargetId
//! ```
//!
//! Test cases name a source slot, optionally select a named terminal inside a
//! composite type tree, then name the expected target family. This keeps the
//! tests table-shaped without erasing the typed endpoint sets that the resolver
//! is supposed to prove.

use itertools::Itertools;
use ploke_core::ItemKind;
use syn_parser::error::SynParserError;
use syn_parser::parser::ParsedCodeGraph;
use syn_parser::parser::graph::GraphAccess;
use syn_parser::parser::nodes::{
    AnyNodeId, AnyTypeId, AsAnyNodeId, FieldNode, OrdinaryTypeSourceId, OrdinaryTypeTargetId,
    OrdinaryTypeUseId, TraitTypeSourceId, TraitTypeTargetId, TypeDefNode, TypeGenericParamNodeId,
};
use syn_parser::parser::relations::TypeRelation;
use syn_parser::parser::types::{GenericParamNode, TypeNode, TypeWherePredicate};
use syn_parser::resolve::type_resolution_v2::TypeRelationReport;

use super::resolution::find_item_id_by_path_name_kind_checked;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum TypeRelationSource {
    Ordinary(OrdinaryTypeSourceId),
    Trait(TraitTypeSourceId),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FieldSelector<'a> {
    Named(&'a str),
    Index(usize),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ImplSelector<'a> {
    pub module_path: &'a [&'a str],
    pub self_type_path: &'a [&'a str],
    pub trait_type_path: Option<&'a [&'a str]>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TypeUseOwnerSelector<'a> {
    Item {
        module_path: &'a [&'a str],
        name: &'a str,
        kind: ItemKind,
    },
    Impl(ImplSelector<'a>),
    Method {
        impl_selector: ImplSelector<'a>,
        name: &'a str,
    },
    StructField {
        module_path: &'a [&'a str],
        type_name: &'a str,
        field: FieldSelector<'a>,
    },
    UnionField {
        module_path: &'a [&'a str],
        type_name: &'a str,
        field: FieldSelector<'a>,
    },
    EnumVariantField {
        module_path: &'a [&'a str],
        enum_name: &'a str,
        variant_name: &'a str,
        field: FieldSelector<'a>,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TypeUseSourceSlot {
    FunctionReturn,
    FunctionParam(usize),
    MethodReturn,
    MethodParam(usize),
    FieldType,
    TypeAliasTarget,
    ImplSelf,
    ImplTrait,
    TraitSuper(usize),
    AssociatedTypeBound(usize),
    WherePredicateSubject(usize),
    WherePredicateBound {
        predicate_index: usize,
        bound_index: usize,
    },
    ConstType,
    StaticType,
    GenericParamBound {
        param_index: usize,
        bound_index: usize,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TypeTerminalSelector<'a> {
    Root,
    NamedPath(&'a [&'a str]),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OrdinarySourceSelector<'a> {
    pub owner: TypeUseOwnerSelector<'a>,
    pub slot: TypeUseSourceSlot,
    pub terminal: TypeTerminalSelector<'a>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TraitSourceSelector<'a> {
    pub owner: TypeUseOwnerSelector<'a>,
    pub slot: TypeUseSourceSlot,
    pub terminal: TypeTerminalSelector<'a>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OrdinaryTargetSelector<'a> {
    Item(TypeUseOwnerSelector<'a>),
    TypeGenericParam {
        owner: TypeUseOwnerSelector<'a>,
        name: &'a str,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TraitTargetSelector<'a> {
    Item(TypeUseOwnerSelector<'a>),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExpectedTypeRelation<'a> {
    Ordinary {
        source: OrdinarySourceSelector<'a>,
        target: OrdinaryTargetSelector<'a>,
    },
    Trait {
        source: TraitSourceSelector<'a>,
        target: TraitTargetSelector<'a>,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SourceSlotRoot {
    Ordinary(OrdinaryTypeUseId),
    Trait(TraitTypeSourceId),
}

#[derive(Debug, Clone, Copy)]
pub struct FixtureGraphView<'a> {
    graph: &'a ParsedCodeGraph,
}

#[derive(Debug, Clone, Copy)]
pub struct TypeRelationView<'a> {
    graph: FixtureGraphView<'a>,
    report: &'a TypeRelationReport,
}

pub fn item<'a>(
    module_path: &'a [&'a str],
    name: &'a str,
    kind: ItemKind,
) -> TypeUseOwnerSelector<'a> {
    TypeUseOwnerSelector::Item {
        module_path,
        name,
        kind,
    }
}

pub fn impl_selector<'a>(
    module_path: &'a [&'a str],
    self_type_path: &'a [&'a str],
    trait_type_path: Option<&'a [&'a str]>,
) -> ImplSelector<'a> {
    ImplSelector {
        module_path,
        self_type_path,
        trait_type_path,
    }
}

pub fn impl_block<'a>(
    module_path: &'a [&'a str],
    self_type_path: &'a [&'a str],
    trait_type_path: Option<&'a [&'a str]>,
) -> TypeUseOwnerSelector<'a> {
    TypeUseOwnerSelector::Impl(impl_selector(module_path, self_type_path, trait_type_path))
}

pub fn method<'a>(impl_selector: ImplSelector<'a>, name: &'a str) -> TypeUseOwnerSelector<'a> {
    TypeUseOwnerSelector::Method {
        impl_selector,
        name,
    }
}

pub fn struct_field<'a>(
    module_path: &'a [&'a str],
    type_name: &'a str,
    field: FieldSelector<'a>,
) -> TypeUseOwnerSelector<'a> {
    TypeUseOwnerSelector::StructField {
        module_path,
        type_name,
        field,
    }
}

pub fn enum_variant_field<'a>(
    module_path: &'a [&'a str],
    enum_name: &'a str,
    variant_name: &'a str,
    field: FieldSelector<'a>,
) -> TypeUseOwnerSelector<'a> {
    TypeUseOwnerSelector::EnumVariantField {
        module_path,
        enum_name,
        variant_name,
        field,
    }
}

pub fn union_field<'a>(
    module_path: &'a [&'a str],
    type_name: &'a str,
    field: FieldSelector<'a>,
) -> TypeUseOwnerSelector<'a> {
    TypeUseOwnerSelector::UnionField {
        module_path,
        type_name,
        field,
    }
}

pub fn root() -> TypeTerminalSelector<'static> {
    TypeTerminalSelector::Root
}

pub fn named<'a>(path: &'a [&'a str]) -> TypeTerminalSelector<'a> {
    TypeTerminalSelector::NamedPath(path)
}

pub fn ordinary_source<'a>(
    owner: TypeUseOwnerSelector<'a>,
    slot: TypeUseSourceSlot,
    terminal: TypeTerminalSelector<'a>,
) -> OrdinarySourceSelector<'a> {
    OrdinarySourceSelector {
        owner,
        slot,
        terminal,
    }
}

pub fn trait_source<'a>(
    owner: TypeUseOwnerSelector<'a>,
    slot: TypeUseSourceSlot,
    terminal: TypeTerminalSelector<'a>,
) -> TraitSourceSelector<'a> {
    TraitSourceSelector {
        owner,
        slot,
        terminal,
    }
}

pub fn ordinary_item<'a>(target: TypeUseOwnerSelector<'a>) -> OrdinaryTargetSelector<'a> {
    OrdinaryTargetSelector::Item(target)
}

pub fn ordinary_type_param<'a>(
    owner: TypeUseOwnerSelector<'a>,
    name: &'a str,
) -> OrdinaryTargetSelector<'a> {
    OrdinaryTargetSelector::TypeGenericParam { owner, name }
}

pub fn trait_item<'a>(target: TypeUseOwnerSelector<'a>) -> TraitTargetSelector<'a> {
    TraitTargetSelector::Item(target)
}

pub fn ordinary_relation<'a>(
    source: OrdinarySourceSelector<'a>,
    target: OrdinaryTargetSelector<'a>,
) -> ExpectedTypeRelation<'a> {
    ExpectedTypeRelation::Ordinary { source, target }
}

pub fn trait_relation<'a>(
    source: TraitSourceSelector<'a>,
    target: TraitTargetSelector<'a>,
) -> ExpectedTypeRelation<'a> {
    ExpectedTypeRelation::Trait { source, target }
}

impl<'a> TypeRelationView<'a> {
    pub fn new(graph: &'a ParsedCodeGraph, report: &'a TypeRelationReport) -> Self {
        Self {
            graph: FixtureGraphView::new(graph),
            report,
        }
    }

    pub fn assert_once(self, expected: ExpectedTypeRelation<'_>) -> Result<(), SynParserError> {
        let expected_relation = self.expected_relation(expected)?;
        let matches = self
            .report
            .relations
            .iter()
            .filter(|relation| **relation == expected_relation)
            .count();

        assert_eq!(
            matches,
            1,
            "Expected exactly one v2 type relation {expected_relation:?}; found {matches}.\n\
             Matching family rows:\n{}",
            self.matching_family_rows_debug(expected_relation),
        );

        Ok(())
    }

    pub fn assert_present(
        self,
        expected: &[ExpectedTypeRelation<'_>],
    ) -> Result<(), SynParserError> {
        for relation in expected {
            self.assert_once(*relation)?;
        }
        Ok(())
    }

    pub fn assert_exact_sources(
        self,
        expected: &[ExpectedTypeRelation<'_>],
    ) -> Result<(), SynParserError> {
        let mut expected_relations = expected
            .iter()
            .map(|relation| self.expected_relation(*relation))
            .collect::<Result<Vec<_>, _>>()?;

        expected_relations.sort();
        expected_relations.dedup();

        let expected_sources = expected_relations
            .iter()
            .map(|relation| relation_source(*relation))
            .unique()
            .collect::<Vec<_>>();

        for source in expected_sources {
            let mut expected_for_source = expected_relations
                .iter()
                .copied()
                .filter(|relation| relation_source(*relation) == source)
                .collect::<Vec<_>>();
            let mut actual_for_source = self
                .report
                .relations
                .iter()
                .copied()
                .filter(|relation| relation_source(*relation) == source)
                .collect::<Vec<_>>();

            expected_for_source.sort();
            actual_for_source.sort();

            assert_eq!(
                actual_for_source,
                expected_for_source,
                "Expected exact v2 type relations for source {source:?}.\n\
                 Expected:\n{}\n\
                 Actual:\n{}",
                format_relation_rows(&expected_for_source),
                format_relation_rows(&actual_for_source),
            );
        }

        Ok(())
    }

    fn expected_relation(
        self,
        expected: ExpectedTypeRelation<'_>,
    ) -> Result<TypeRelation, SynParserError> {
        match expected {
            ExpectedTypeRelation::Ordinary { source, target } => Ok(TypeRelation::Ordinary {
                source: self.graph.ordinary_source(source)?,
                target: self.graph.ordinary_target(target)?,
            }),
            ExpectedTypeRelation::Trait { source, target } => Ok(TypeRelation::Trait {
                source: self.graph.trait_source(source)?,
                target: self.graph.trait_target(target)?,
            }),
        }
    }

    fn matching_family_rows_debug(self, relation: TypeRelation) -> String {
        let rows = self
            .report
            .relations
            .iter()
            .filter(|actual| actual.kind_str() == relation.kind_str())
            .map(|actual| format!("  {actual:?}"))
            .collect::<Vec<_>>();

        if rows.is_empty() {
            "  <none>".to_string()
        } else {
            rows.join("\n")
        }
    }
}

fn relation_source(relation: TypeRelation) -> TypeRelationSource {
    match relation {
        TypeRelation::Ordinary { source, .. } => TypeRelationSource::Ordinary(source),
        TypeRelation::Trait { source, .. } => TypeRelationSource::Trait(source),
    }
}

impl<'a> FixtureGraphView<'a> {
    pub fn new(graph: &'a ParsedCodeGraph) -> Self {
        Self { graph }
    }

    fn ordinary_source(
        self,
        selector: OrdinarySourceSelector<'_>,
    ) -> Result<OrdinaryTypeSourceId, SynParserError> {
        let owner = self.owner(selector.owner)?;
        let root = self.source_slot_root(owner, selector.slot)?;

        match (root, selector.terminal) {
            (SourceSlotRoot::Ordinary(root), TypeTerminalSelector::Root) => {
                OrdinaryTypeSourceId::try_from(root).map_err(|_| {
                    SynParserError::InternalState(format!(
                        "ordinary source root in {}::{:?} is not a direct source",
                        selector.owner.describe(),
                        selector.slot
                    ))
                })
            }
            (SourceSlotRoot::Ordinary(root), TypeTerminalSelector::NamedPath(path)) => {
                self.ordinary_source_terminal(AnyTypeId::from(root), path)
            }
            (SourceSlotRoot::Trait(root), TypeTerminalSelector::NamedPath(path)) => {
                self.ordinary_source_terminal(AnyTypeId::from(root), path)
            }
            (SourceSlotRoot::Trait(_), TypeTerminalSelector::Root) => {
                panic!(
                    "ordinary root source selector used with trait-position slot {}::{:?}",
                    selector.owner.describe(),
                    selector.slot
                );
            }
        }
    }

    fn trait_source(
        self,
        selector: TraitSourceSelector<'_>,
    ) -> Result<TraitTypeSourceId, SynParserError> {
        let owner = self.owner(selector.owner)?;
        let root = self.source_slot_root(owner, selector.slot)?;

        match (root, selector.terminal) {
            (SourceSlotRoot::Trait(root), TypeTerminalSelector::Root) => Ok(root),
            (SourceSlotRoot::Trait(root), TypeTerminalSelector::NamedPath(path)) => {
                self.trait_source_terminal(AnyTypeId::from(root), path)
            }
            (SourceSlotRoot::Ordinary(root), TypeTerminalSelector::NamedPath(path)) => {
                self.trait_source_terminal(AnyTypeId::from(root), path)
            }
            (SourceSlotRoot::Ordinary(_), TypeTerminalSelector::Root) => {
                panic!(
                    "trait root source selector used with ordinary-position slot {}::{:?}",
                    selector.owner.describe(),
                    selector.slot
                );
            }
        }
    }

    fn ordinary_target(
        self,
        selector: OrdinaryTargetSelector<'_>,
    ) -> Result<OrdinaryTypeTargetId, SynParserError> {
        match selector {
            OrdinaryTargetSelector::Item(selector) => match self.owner(selector)? {
                AnyNodeId::Struct(id) => Ok(OrdinaryTypeTargetId::from(id)),
                AnyNodeId::Enum(id) => Ok(OrdinaryTypeTargetId::from(id)),
                AnyNodeId::Union(id) => Ok(OrdinaryTypeTargetId::from(id)),
                AnyNodeId::TypeAlias(id) => Ok(OrdinaryTypeTargetId::from(id)),
                other => Err(SynParserError::InternalState(format!(
                    "{} resolved to {other:?}, which is not an ordinary type target",
                    selector.describe()
                ))),
            },
            OrdinaryTargetSelector::TypeGenericParam { owner, name } => Ok(
                OrdinaryTypeTargetId::from(self.type_generic_param(owner, name)?),
            ),
        }
    }

    fn trait_target(
        self,
        selector: TraitTargetSelector<'_>,
    ) -> Result<TraitTypeTargetId, SynParserError> {
        match selector {
            TraitTargetSelector::Item(selector) => match self.owner(selector)? {
                AnyNodeId::Trait(id) => Ok(TraitTypeTargetId::from(id)),
                other => Err(SynParserError::InternalState(format!(
                    "{} resolved to {other:?}, which is not a trait target",
                    selector.describe()
                ))),
            },
        }
    }

    fn owner(self, selector: TypeUseOwnerSelector<'_>) -> Result<AnyNodeId, SynParserError> {
        match selector {
            TypeUseOwnerSelector::Item {
                module_path,
                name,
                kind,
            } => find_item_id_by_path_name_kind_checked(self.graph, module_path, name, kind),
            TypeUseOwnerSelector::Impl(selector) => Ok(self.impl_node(selector)?.id.as_any()),
            TypeUseOwnerSelector::Method {
                impl_selector,
                name,
            } => {
                let impl_node = self.impl_node(impl_selector)?;
                let method = exactly_one_or_panic(
                    impl_node
                        .methods
                        .iter()
                        .filter(|method| method.name == name),
                    || format!("method `{name}` in impl {}", impl_selector.describe()),
                );
                Ok(method.id.as_any())
            }
            TypeUseOwnerSelector::StructField {
                module_path,
                type_name,
                field,
            } => {
                let id = find_item_id_by_path_name_kind_checked(
                    self.graph,
                    module_path,
                    type_name,
                    ItemKind::Struct,
                )?;
                let node = self
                    .graph
                    .find_node_unique(id)?
                    .as_struct()
                    .ok_or_else(|| {
                        SynParserError::InternalState(format!(
                            "{type_name} did not resolve to a struct"
                        ))
                    })?;
                Ok(
                    Self::select_field(&node.fields, field, selector.describe())?
                        .id
                        .as_any(),
                )
            }
            TypeUseOwnerSelector::UnionField {
                module_path,
                type_name,
                field,
            } => {
                let id = find_item_id_by_path_name_kind_checked(
                    self.graph,
                    module_path,
                    type_name,
                    ItemKind::Union,
                )?;
                let node = self.graph.find_node_unique(id)?.as_union().ok_or_else(|| {
                    SynParserError::InternalState(format!("{type_name} did not resolve to a union"))
                })?;
                Ok(
                    Self::select_field(&node.fields, field, selector.describe())?
                        .id
                        .as_any(),
                )
            }
            TypeUseOwnerSelector::EnumVariantField {
                module_path,
                enum_name,
                variant_name,
                field,
            } => {
                let id = find_item_id_by_path_name_kind_checked(
                    self.graph,
                    module_path,
                    enum_name,
                    ItemKind::Enum,
                )?;
                let node = self.graph.find_node_unique(id)?.as_enum().ok_or_else(|| {
                    SynParserError::InternalState(format!("{enum_name} did not resolve to an enum"))
                })?;
                let variant = exactly_one_or_panic(
                    node.variants
                        .iter()
                        .filter(|variant| variant.name == variant_name),
                    || format!("enum variant `{variant_name}` in {}", selector.describe()),
                );
                Ok(
                    Self::select_field(&variant.fields, field, selector.describe())?
                        .id
                        .as_any(),
                )
            }
        }
    }

    fn source_slot_root(
        self,
        owner: AnyNodeId,
        source: TypeUseSourceSlot,
    ) -> Result<SourceSlotRoot, SynParserError> {
        match source {
            TypeUseSourceSlot::FunctionReturn => {
                let AnyNodeId::Function(id) = owner else {
                    panic!("FunctionReturn source used with non-function owner {owner:?}");
                };
                let function = self
                    .graph
                    .functions()
                    .iter()
                    .find(|function| function.id == id)
                    .expect("function owner should exist");
                function
                    .return_type
                    .map(SourceSlotRoot::Ordinary)
                    .ok_or_else(|| {
                        SynParserError::InternalState("function has no return type".into())
                    })
            }
            TypeUseSourceSlot::FunctionParam(index) => {
                let AnyNodeId::Function(id) = owner else {
                    panic!("FunctionParam source used with non-function owner {owner:?}");
                };
                let function = self
                    .graph
                    .functions()
                    .iter()
                    .find(|function| function.id == id)
                    .expect("function owner should exist");
                Ok(SourceSlotRoot::Ordinary(function.parameters[index].type_id))
            }
            TypeUseSourceSlot::MethodReturn => {
                let method = self
                    .method_by_owner(owner)
                    .expect("method owner should exist");
                method
                    .return_type
                    .map(SourceSlotRoot::Ordinary)
                    .ok_or_else(|| {
                        SynParserError::InternalState("method has no return type".into())
                    })
            }
            TypeUseSourceSlot::MethodParam(index) => {
                let method = self
                    .method_by_owner(owner)
                    .expect("method owner should exist");
                Ok(SourceSlotRoot::Ordinary(method.parameters[index].type_id))
            }
            TypeUseSourceSlot::FieldType => Ok(SourceSlotRoot::Ordinary(
                self.field_by_owner(owner)?.type_id,
            )),
            TypeUseSourceSlot::TypeAliasTarget => {
                let AnyNodeId::TypeAlias(id) = owner else {
                    panic!("TypeAliasTarget source used with non-type-alias owner {owner:?}");
                };
                let alias = self
                    .graph
                    .defined_types()
                    .iter()
                    .find_map(|node| match node {
                        TypeDefNode::TypeAlias(alias) if alias.id == id => Some(alias),
                        _ => None,
                    })
                    .expect("type alias owner should exist");
                Ok(SourceSlotRoot::Ordinary(alias.type_id))
            }
            TypeUseSourceSlot::ImplSelf => {
                let AnyNodeId::Impl(id) = owner else {
                    panic!("ImplSelf source used with non-impl owner {owner:?}");
                };
                let impl_node = self
                    .graph
                    .impls()
                    .iter()
                    .find(|impl_node| impl_node.id == id)
                    .expect("impl owner should exist");
                Ok(SourceSlotRoot::Ordinary(impl_node.self_type))
            }
            TypeUseSourceSlot::ImplTrait => {
                let AnyNodeId::Impl(id) = owner else {
                    panic!("ImplTrait source used with non-impl owner {owner:?}");
                };
                let impl_node = self
                    .graph
                    .impls()
                    .iter()
                    .find(|impl_node| impl_node.id == id)
                    .expect("impl owner should exist");
                impl_node
                    .trait_type
                    .map(SourceSlotRoot::Trait)
                    .ok_or_else(|| SynParserError::InternalState("impl has no trait type".into()))
            }
            TypeUseSourceSlot::TraitSuper(index) => {
                let AnyNodeId::Trait(id) = owner else {
                    panic!("TraitSuper source used with non-trait owner {owner:?}");
                };
                let trait_node = self
                    .graph
                    .traits()
                    .iter()
                    .find(|trait_node| trait_node.id == id)
                    .expect("trait owner should exist");
                Ok(SourceSlotRoot::Trait(trait_node.super_traits[index]))
            }
            TypeUseSourceSlot::AssociatedTypeBound(index) => {
                let AnyNodeId::Trait(id) = owner else {
                    panic!("AssociatedTypeBound source used with non-trait owner {owner:?}");
                };
                let trait_node = self
                    .graph
                    .traits()
                    .iter()
                    .find(|trait_node| trait_node.id == id)
                    .expect("trait owner should exist");
                Ok(SourceSlotRoot::Trait(
                    trait_node.associated_type_bounds[index],
                ))
            }
            TypeUseSourceSlot::WherePredicateSubject(index) => {
                let predicates = self.where_predicates_for_owner(owner);
                Ok(SourceSlotRoot::Ordinary(
                    predicates.get(index).unwrap_or_else(|| {
                        panic!(
                            "where predicate index {index} out of bounds for owner {owner:?}; predicate count {}",
                            predicates.len()
                        )
                    }).subject,
                ))
            }
            TypeUseSourceSlot::WherePredicateBound {
                predicate_index,
                bound_index,
            } => {
                let predicates = self.where_predicates_for_owner(owner);
                let predicate = predicates.get(predicate_index).unwrap_or_else(|| {
                    panic!(
                        "where predicate index {predicate_index} out of bounds for owner {owner:?}; predicate count {}",
                        predicates.len()
                    )
                });
                Ok(SourceSlotRoot::Trait(*predicate.bounds.get(bound_index).unwrap_or_else(|| {
                    panic!(
                        "bound index {bound_index} out of bounds for where predicate {predicate:?}; bound count {}",
                        predicate.bounds.len()
                    )
                })))
            }
            TypeUseSourceSlot::ConstType => {
                let AnyNodeId::Const(id) = owner else {
                    panic!("ConstType source used with non-const owner {owner:?}");
                };
                let const_node = self
                    .graph
                    .consts()
                    .iter()
                    .find(|const_node| const_node.id == id)
                    .expect("const owner should exist");
                Ok(SourceSlotRoot::Ordinary(const_node.type_id))
            }
            TypeUseSourceSlot::StaticType => {
                let AnyNodeId::Static(id) = owner else {
                    panic!("StaticType source used with non-static owner {owner:?}");
                };
                let static_node = self
                    .graph
                    .statics()
                    .iter()
                    .find(|static_node| static_node.id == id)
                    .expect("static owner should exist");
                Ok(SourceSlotRoot::Ordinary(static_node.type_id))
            }
            TypeUseSourceSlot::GenericParamBound {
                param_index,
                bound_index,
            } => {
                let generic_params = self.generic_params_for_owner(owner);
                let generic_param = generic_params.get(param_index).unwrap_or_else(|| {
                    panic!(
                        "generic param index {param_index} out of bounds for owner {owner:?}; param count {}",
                        generic_params.len()
                    )
                });
                let bounds = generic_param.kind.bounds().unwrap_or_else(|| {
                    panic!("generic param {generic_param:?} does not have trait bounds")
                });
                Ok(SourceSlotRoot::Trait(*bounds.get(bound_index).unwrap_or_else(|| {
                    panic!(
                        "bound index {bound_index} out of bounds for generic param {generic_param:?}; bound count {}",
                        bounds.len()
                    )
                })))
            }
        }
    }

    fn impl_node(
        self,
        selector: ImplSelector<'_>,
    ) -> Result<&'a syn_parser::parser::nodes::ImplNode, SynParserError> {
        let module_path = selector
            .module_path
            .iter()
            .map(|segment| (*segment).to_string())
            .collect::<Vec<_>>();
        let module = self.graph.find_module_by_path_checked(&module_path)?;

        Ok(exactly_one_or_panic(
            self.graph
                .impls()
                .iter()
                .filter(|impl_node| {
                    self.graph
                        .module_for_any_id(impl_node.id.as_any())
                        .is_some_and(|owner_module| owner_module.id == module.id)
                })
                .filter(|impl_node| {
                    self.ordinary_root_path_matches(impl_node.self_type, selector.self_type_path)
                })
                .filter(
                    |impl_node| match (impl_node.trait_type, selector.trait_type_path) {
                        (None, None) => true,
                        (Some(type_id), Some(path)) => self.trait_root_path_matches(type_id, path),
                        _ => false,
                    },
                ),
            || format!("impl {}", selector.describe()),
        ))
    }

    fn type_generic_param(
        self,
        owner: TypeUseOwnerSelector<'_>,
        name: &str,
    ) -> Result<TypeGenericParamNodeId, SynParserError> {
        let owner = self.owner(owner)?;
        let generic_param = self
            .generic_params_for_owner(owner)
            .into_iter()
            .find(|param| param.kind.name() == Some(name))
            .ok_or_else(|| {
                SynParserError::InternalState(format!(
                    "type generic parameter `{name}` was not found on owner {owner:?}"
                ))
            })?;

        TypeGenericParamNodeId::try_refine(generic_param.id, &generic_param.kind).map_err(|err| {
            SynParserError::InternalState(format!(
                "generic parameter `{name}` on {owner:?} was not a type parameter: {err}"
            ))
        })
    }

    fn generic_params_for_owner(self, owner: AnyNodeId) -> Vec<&'a GenericParamNode> {
        match owner {
            AnyNodeId::Function(id) => self
                .graph
                .functions()
                .iter()
                .find(|node| node.id == id)
                .map(|node| node.generic_params.iter().collect())
                .unwrap_or_default(),
            AnyNodeId::Struct(id) => self
                .graph
                .defined_types()
                .iter()
                .find_map(|node| match node {
                    TypeDefNode::Struct(node) if node.id == id => {
                        Some(node.generic_params.iter().collect())
                    }
                    _ => None,
                })
                .unwrap_or_default(),
            AnyNodeId::Enum(id) => self
                .graph
                .defined_types()
                .iter()
                .find_map(|node| match node {
                    TypeDefNode::Enum(node) if node.id == id => {
                        Some(node.generic_params.iter().collect())
                    }
                    _ => None,
                })
                .unwrap_or_default(),
            AnyNodeId::Union(id) => self
                .graph
                .defined_types()
                .iter()
                .find_map(|node| match node {
                    TypeDefNode::Union(node) if node.id == id => {
                        Some(node.generic_params.iter().collect())
                    }
                    _ => None,
                })
                .unwrap_or_default(),
            AnyNodeId::TypeAlias(id) => self
                .graph
                .defined_types()
                .iter()
                .find_map(|node| match node {
                    TypeDefNode::TypeAlias(node) if node.id == id => {
                        Some(node.generic_params.iter().collect())
                    }
                    _ => None,
                })
                .unwrap_or_default(),
            AnyNodeId::Trait(id) => self
                .graph
                .traits()
                .iter()
                .find(|node| node.id == id)
                .map(|node| node.generic_params.iter().collect())
                .unwrap_or_default(),
            AnyNodeId::Impl(id) => self
                .graph
                .impls()
                .iter()
                .find(|node| node.id == id)
                .map(|node| node.generic_params.iter().collect())
                .unwrap_or_default(),
            AnyNodeId::Method(id) => self
                .graph
                .impls()
                .iter()
                .flat_map(|node| node.methods.iter())
                .chain(
                    self.graph
                        .traits()
                        .iter()
                        .flat_map(|node| node.methods.iter()),
                )
                .find(|node| node.id == id)
                .map(|node| node.generic_params.iter().collect())
                .unwrap_or_default(),
            _ => Vec::new(),
        }
    }

    fn where_predicates_for_owner(self, owner: AnyNodeId) -> Vec<&'a TypeWherePredicate> {
        match owner {
            AnyNodeId::Function(id) => self
                .graph
                .functions()
                .iter()
                .find(|node| node.id == id)
                .map(|node| node.where_predicates.iter().collect())
                .unwrap_or_default(),
            AnyNodeId::Struct(id) => self
                .graph
                .defined_types()
                .iter()
                .find_map(|node| match node {
                    TypeDefNode::Struct(node) if node.id == id => {
                        Some(node.where_predicates.iter().collect())
                    }
                    _ => None,
                })
                .unwrap_or_default(),
            AnyNodeId::Enum(id) => self
                .graph
                .defined_types()
                .iter()
                .find_map(|node| match node {
                    TypeDefNode::Enum(node) if node.id == id => {
                        Some(node.where_predicates.iter().collect())
                    }
                    _ => None,
                })
                .unwrap_or_default(),
            AnyNodeId::Union(id) => self
                .graph
                .defined_types()
                .iter()
                .find_map(|node| match node {
                    TypeDefNode::Union(node) if node.id == id => {
                        Some(node.where_predicates.iter().collect())
                    }
                    _ => None,
                })
                .unwrap_or_default(),
            AnyNodeId::TypeAlias(id) => self
                .graph
                .defined_types()
                .iter()
                .find_map(|node| match node {
                    TypeDefNode::TypeAlias(node) if node.id == id => {
                        Some(node.where_predicates.iter().collect())
                    }
                    _ => None,
                })
                .unwrap_or_default(),
            AnyNodeId::Trait(id) => self
                .graph
                .traits()
                .iter()
                .find(|node| node.id == id)
                .map(|node| node.where_predicates.iter().collect())
                .unwrap_or_default(),
            AnyNodeId::Impl(id) => self
                .graph
                .impls()
                .iter()
                .find(|node| node.id == id)
                .map(|node| node.where_predicates.iter().collect())
                .unwrap_or_default(),
            AnyNodeId::Method(id) => self
                .graph
                .impls()
                .iter()
                .flat_map(|node| node.methods.iter())
                .chain(
                    self.graph
                        .traits()
                        .iter()
                        .flat_map(|node| node.methods.iter()),
                )
                .find(|node| node.id == id)
                .map(|node| node.where_predicates.iter().collect())
                .unwrap_or_default(),
            _ => Vec::new(),
        }
    }

    fn ordinary_source_terminal(
        self,
        root: AnyTypeId,
        path: &[&str],
    ) -> Result<OrdinaryTypeSourceId, SynParserError> {
        Ok(exactly_one_or_panic(
            self.type_tree_vertices(root)
                .into_iter()
                .filter_map(|id| match id {
                    AnyTypeId::Named(id) => Some(OrdinaryTypeSourceId::from(id)),
                    _ => None,
                })
                .filter(|source| self.source_path_matches(AnyTypeId::from(*source), path)),
            || {
                format!(
                    "ordinary source terminal `{}` under root {root:?}",
                    path.join("::")
                )
            },
        ))
    }

    fn trait_source_terminal(
        self,
        root: AnyTypeId,
        path: &[&str],
    ) -> Result<TraitTypeSourceId, SynParserError> {
        Ok(exactly_one_or_panic(
            self.type_tree_vertices(root)
                .into_iter()
                .filter_map(|id| match id {
                    AnyTypeId::Named(id) => Some(TraitTypeSourceId::from(id)),
                    AnyTypeId::TraitBound(id) => Some(TraitTypeSourceId::from(id)),
                    _ => None,
                })
                .filter(|source| self.source_path_matches(AnyTypeId::from(*source), path)),
            || {
                format!(
                    "trait source terminal `{}` under root {root:?}",
                    path.join("::")
                )
            },
        ))
    }

    fn type_tree_vertices(self, root: AnyTypeId) -> Vec<AnyTypeId> {
        let mut out = Vec::new();
        let mut stack = vec![root];
        while let Some(id) = stack.pop() {
            out.push(id);
            if let Some(node) = self.type_node(id) {
                stack.extend(node.child_type_ids());
            }
        }
        out
    }

    fn source_path_matches(self, source: AnyTypeId, expected_path: &[&str]) -> bool {
        self.type_node(source).is_some_and(|node| match node {
            TypeNode::Named(node) => path_matches(&node.path, expected_path),
            TypeNode::TraitBound(node) => path_matches(&node.path, expected_path),
            _ => false,
        })
    }

    fn ordinary_root_path_matches(
        self,
        type_id: OrdinaryTypeUseId,
        expected_path: &[&str],
    ) -> bool {
        self.source_path_matches(AnyTypeId::from(type_id), expected_path)
    }

    fn trait_root_path_matches(self, type_id: TraitTypeSourceId, expected_path: &[&str]) -> bool {
        self.source_path_matches(AnyTypeId::from(type_id), expected_path)
    }

    fn type_node(self, type_id: AnyTypeId) -> Option<&'a TypeNode> {
        self.graph
            .type_graph()
            .iter()
            .find(|node| node.id() == type_id)
    }

    fn select_field<'b>(
        fields: &'b [FieldNode],
        selector: FieldSelector<'_>,
        owner: String,
    ) -> Result<&'b FieldNode, SynParserError> {
        match selector {
            FieldSelector::Named(name) => Ok(exactly_one_or_panic(
                fields
                    .iter()
                    .filter(|field| field.name.as_deref() == Some(name)),
                || format!("field `{name}` in {owner}"),
            )),
            FieldSelector::Index(index) => fields.get(index).ok_or_else(|| {
                SynParserError::InternalState(format!(
                    "Field index {index} out of bounds for {owner}; field count {}",
                    fields.len()
                ))
            }),
        }
    }

    fn method_by_owner(
        self,
        owner: AnyNodeId,
    ) -> Option<&'a syn_parser::parser::nodes::MethodNode> {
        let AnyNodeId::Method(id) = owner else {
            panic!("method source used with non-method owner {owner:?}");
        };

        self.graph
            .impls()
            .iter()
            .flat_map(|impl_node| &impl_node.methods)
            .chain(
                self.graph
                    .traits()
                    .iter()
                    .flat_map(|trait_node| &trait_node.methods),
            )
            .find(|method| method.id == id)
    }

    fn field_by_owner(self, owner: AnyNodeId) -> Result<&'a FieldNode, SynParserError> {
        let AnyNodeId::Field(id) = owner else {
            panic!("FieldType source used with non-field owner {owner:?}");
        };

        self.graph
            .defined_types()
            .iter()
            .find_map(|node| match node {
                TypeDefNode::Struct(node) => node.fields.iter().find(|field| field.id == id),
                TypeDefNode::Enum(node) => node
                    .variants
                    .iter()
                    .flat_map(|variant| &variant.fields)
                    .find(|field| field.id == id),
                TypeDefNode::Union(node) => node.fields.iter().find(|field| field.id == id),
                TypeDefNode::TypeAlias(_) => None,
            })
            .ok_or(SynParserError::NotFound(owner))
    }
}

fn exactly_one_or_panic<I>(iter: I, label: impl FnOnce() -> String) -> I::Item
where
    I: Iterator,
{
    iter.exactly_one()
        .unwrap_or_else(|err| panic!("Expected exactly one {}; found {}", label(), err.count()))
}

fn path_matches(actual: &[String], expected: &[&str]) -> bool {
    actual.len() == expected.len()
        && actual
            .iter()
            .map(String::as_str)
            .eq(expected.iter().copied())
}

fn format_relation_rows(rows: &[TypeRelation]) -> String {
    if rows.is_empty() {
        "  <none>".to_string()
    } else {
        rows.iter()
            .map(|relation| format!("  {relation:?}"))
            .collect::<Vec<_>>()
            .join("\n")
    }
}

impl TypeUseOwnerSelector<'_> {
    fn describe(self) -> String {
        match self {
            TypeUseOwnerSelector::Item {
                module_path,
                name,
                kind,
            } => format!("item {}::{} ({kind:?})", module_path.join("::"), name),
            TypeUseOwnerSelector::Impl(selector) => format!("impl {}", selector.describe()),
            TypeUseOwnerSelector::Method {
                impl_selector,
                name,
            } => format!("method {}::{name}", impl_selector.describe()),
            TypeUseOwnerSelector::StructField {
                module_path,
                type_name,
                field,
            } => format!(
                "struct field {}::{}::{field:?}",
                module_path.join("::"),
                type_name
            ),
            TypeUseOwnerSelector::UnionField {
                module_path,
                type_name,
                field,
            } => format!(
                "union field {}::{}::{field:?}",
                module_path.join("::"),
                type_name
            ),
            TypeUseOwnerSelector::EnumVariantField {
                module_path,
                enum_name,
                variant_name,
                field,
            } => format!(
                "enum field {}::{}::{}::{field:?}",
                module_path.join("::"),
                enum_name,
                variant_name
            ),
        }
    }
}

impl ImplSelector<'_> {
    fn describe(self) -> String {
        let trait_part = self
            .trait_type_path
            .map(|path| format!("{} for ", path.join("::")))
            .unwrap_or_default();
        format!(
            "{}{} in {}",
            trait_part,
            self.self_type_path.join("::"),
            self.module_path.join("::")
        )
    }
}
