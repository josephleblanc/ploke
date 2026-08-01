//! Type-use resolution helpers for Phase 3 semantic resolution tests.
//!
//! These helpers mirror the relation-paranoid style: tests name exact owner slots and expected
//! semantic outcomes, then this module asserts that the late type-resolution report contains those
//! outcomes. Use the slot-level helpers for composite types so nested type uses cannot hide behind a
//! single positive edge assertion.

use std::collections::BTreeMap;

use ploke_core::{ItemKind, TypeId, TypeKind};
use syn_parser::error::SynParserError;
use syn_parser::parser::ParsedCodeGraph;
use syn_parser::parser::graph::GraphAccess;
use syn_parser::parser::nodes::{AnyNodeId, AsAnyNodeId, GenericParamNodeId, TypeDefNode};
use syn_parser::resolve::type_resolution::{
    AmbiguousReason, State, Target, TypeResolutionReport, TypeUseRole, UnresolvedReason,
};

use super::resolution::find_item_id_by_path_name_kind_checked;

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
    ConstType,
    StaticType,
    Any,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ExpectedTypeUseResolution<'a> {
    pub owner: TypeUseOwnerSelector<'a>,
    pub role: TypeUseRole,
    pub source: TypeUseSourceSlot,
    pub target: TypeUseOwnerSelector<'a>,
    pub expect_resolved_type_id: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ExpectedTypeUseSlot<'a> {
    pub owner: TypeUseOwnerSelector<'a>,
    pub role: TypeUseRole,
    pub source: TypeUseSourceSlot,
    pub resolutions: &'a [ExpectedTypeUseSlotResolution<'a>],
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExpectedTypeUseSlotResolution<'a> {
    Item {
        target: TypeUseOwnerSelector<'a>,
        expect_resolved_type_id: bool,
    },
    SelfType {
        target: TypeUseOwnerSelector<'a>,
        expect_resolved_type_id: bool,
    },
    GenericParam {
        name: &'a str,
    },
    Unresolved {
        path: &'a [&'a str],
        reason: UnresolvedReason,
    },
    Ambiguous {
        path: &'a [&'a str],
        reason: AmbiguousReason,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
enum SlotResolutionKey {
    Item {
        target: AnyNodeId,
        has_resolved_type_id: bool,
    },
    SelfType {
        target: AnyNodeId,
        has_resolved_type_id: bool,
    },
    GenericParam {
        name: String,
    },
    Unresolved {
        path: Vec<String>,
        reason: UnresolvedReason,
    },
    Ambiguous {
        path: Vec<String>,
        reason: AmbiguousReason,
    },
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

pub fn impl_block<'a>(
    module_path: &'a [&'a str],
    self_type_path: &'a [&'a str],
    trait_type_path: Option<&'a [&'a str]>,
) -> TypeUseOwnerSelector<'a> {
    TypeUseOwnerSelector::Impl(ImplSelector {
        module_path,
        self_type_path,
        trait_type_path,
    })
}

pub fn method<'a>(impl_selector: ImplSelector<'a>, name: &'a str) -> TypeUseOwnerSelector<'a> {
    TypeUseOwnerSelector::Method {
        impl_selector,
        name,
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

pub fn slot_item<'a>(target: TypeUseOwnerSelector<'a>) -> ExpectedTypeUseSlotResolution<'a> {
    ExpectedTypeUseSlotResolution::Item {
        target,
        expect_resolved_type_id: true,
    }
}

pub fn slot_self_type<'a>(target: TypeUseOwnerSelector<'a>) -> ExpectedTypeUseSlotResolution<'a> {
    ExpectedTypeUseSlotResolution::SelfType {
        target,
        expect_resolved_type_id: true,
    }
}

pub fn slot_generic_param(name: &str) -> ExpectedTypeUseSlotResolution<'_> {
    ExpectedTypeUseSlotResolution::GenericParam { name }
}

pub fn slot_unresolved<'a>(
    path: &'a [&'a str],
    reason: UnresolvedReason,
) -> ExpectedTypeUseSlotResolution<'a> {
    ExpectedTypeUseSlotResolution::Unresolved { path, reason }
}

pub fn slot_ambiguous<'a>(
    path: &'a [&'a str],
    reason: AmbiguousReason,
) -> ExpectedTypeUseSlotResolution<'a> {
    ExpectedTypeUseSlotResolution::Ambiguous { path, reason }
}

pub fn assert_type_use_resolution_once(
    graph: &ParsedCodeGraph,
    report: &TypeResolutionReport,
    expected: &ExpectedTypeUseResolution<'_>,
) -> Result<(), SynParserError> {
    let owner = resolve_owner_selector(graph, expected.owner)?;
    let target = resolve_owner_selector(graph, expected.target)?;
    let slot_root = source_slot_type_id(graph, owner, expected.source)?;

    let matches = report
        .resolutions
        .iter()
        .filter(|resolution| {
            resolution.owner == owner
                && resolution.role == expected.role
                && resolution.item_target() == Some(target)
                && slot_root
                    .is_none_or(|root| type_tree_contains(graph, root, resolution.source_type_id))
        })
        .collect::<Vec<_>>();

    assert_eq!(
        matches.len(),
        1,
        "Expected exactly one resolved type-use edge for role {:?} from {} to {}; found {}.\n\
         Owner candidates:\n{}\n\
         Target candidates:\n{}",
        expected.role,
        expected.owner.describe(),
        expected.target.describe(),
        matches.len(),
        matching_owner_rows_debug(report, owner, expected.role),
        matching_target_rows_debug(report, target),
    );

    let resolution = matches[0];
    if expected.expect_resolved_type_id {
        assert!(
            matches!(resolution.resolved_type_id, Some(TypeId::Resolved(_))),
            "Expected {:?} from {} to {} to carry TypeId::Resolved; got {:?}",
            expected.role,
            expected.owner.describe(),
            expected.target.describe(),
            resolution.resolved_type_id
        );
    }

    Ok(())
}

pub fn assert_type_use_slot_resolutions_exact(
    graph: &ParsedCodeGraph,
    report: &TypeResolutionReport,
    expected: &ExpectedTypeUseSlot<'_>,
) -> Result<(), SynParserError> {
    let owner = resolve_owner_selector(graph, expected.owner)?;
    let slot_root = source_slot_type_id(graph, owner, expected.source)?;

    let Some(slot_root) = slot_root else {
        panic!(
            "Exact slot assertions require a concrete source slot for {}::{:?}",
            expected.owner.describe(),
            expected.source
        );
    };

    let actual = report
        .resolutions
        .iter()
        .filter(|resolution| {
            resolution.owner == owner
                && resolution.role == expected.role
                && type_tree_contains(graph, slot_root, resolution.source_type_id)
        })
        .map(|resolution| slot_resolution_key(graph, resolution))
        .collect::<Result<Vec<_>, _>>()?;

    let expected_keys = expected
        .resolutions
        .iter()
        .map(|resolution| expected_slot_resolution_key(graph, *resolution))
        .collect::<Result<Vec<_>, _>>()?;

    let actual_counts = count_slot_resolution_keys(actual);
    let expected_counts = count_slot_resolution_keys(expected_keys);

    assert_eq!(
        actual_counts,
        expected_counts,
        "Expected exact type-use slot resolutions for role {:?} in {}::{:?}.\n\
         Actual owner rows:\n{}",
        expected.role,
        expected.owner.describe(),
        expected.source,
        matching_owner_rows_debug(report, owner, expected.role),
    );

    Ok(())
}

fn slot_resolution_key(
    graph: &ParsedCodeGraph,
    resolution: &syn_parser::resolve::type_resolution::TypeUseResolution,
) -> Result<SlotResolutionKey, SynParserError> {
    let has_resolved_type_id = matches!(resolution.resolved_type_id, Some(TypeId::Resolved(_)));

    match &resolution.resolved_ref.state {
        State::Resolved(Target::Item(target)) => Ok(SlotResolutionKey::Item {
            target: *target,
            has_resolved_type_id,
        }),
        State::Resolved(Target::SelfType(target)) => Ok(SlotResolutionKey::SelfType {
            target: *target,
            has_resolved_type_id,
        }),
        State::Resolved(Target::GenericParam(target)) => Ok(SlotResolutionKey::GenericParam {
            name: generic_param_name(graph, *target)?,
        }),
        State::Unresolved(unresolved) => Ok(SlotResolutionKey::Unresolved {
            path: unresolved.provenance.path.clone(),
            reason: unresolved.reason,
        }),
        State::Ambiguous(ambiguous) => Ok(SlotResolutionKey::Ambiguous {
            path: ambiguous.provenance.path.clone(),
            reason: ambiguous.reason,
        }),
    }
}

fn expected_slot_resolution_key(
    graph: &ParsedCodeGraph,
    expected: ExpectedTypeUseSlotResolution<'_>,
) -> Result<SlotResolutionKey, SynParserError> {
    match expected {
        ExpectedTypeUseSlotResolution::Item {
            target,
            expect_resolved_type_id,
        } => Ok(SlotResolutionKey::Item {
            target: resolve_owner_selector(graph, target)?,
            has_resolved_type_id: expect_resolved_type_id,
        }),
        ExpectedTypeUseSlotResolution::SelfType {
            target,
            expect_resolved_type_id,
        } => Ok(SlotResolutionKey::SelfType {
            target: resolve_owner_selector(graph, target)?,
            has_resolved_type_id: expect_resolved_type_id,
        }),
        ExpectedTypeUseSlotResolution::GenericParam { name } => {
            Ok(SlotResolutionKey::GenericParam {
                name: name.to_string(),
            })
        }
        ExpectedTypeUseSlotResolution::Unresolved { path, reason } => {
            Ok(SlotResolutionKey::Unresolved {
                path: path.iter().map(|segment| (*segment).to_string()).collect(),
                reason,
            })
        }
        ExpectedTypeUseSlotResolution::Ambiguous { path, reason } => {
            Ok(SlotResolutionKey::Ambiguous {
                path: path.iter().map(|segment| (*segment).to_string()).collect(),
                reason,
            })
        }
    }
}

fn count_slot_resolution_keys(keys: Vec<SlotResolutionKey>) -> BTreeMap<SlotResolutionKey, usize> {
    let mut counts = BTreeMap::new();
    for key in keys {
        *counts.entry(key).or_insert(0) += 1;
    }
    counts
}

fn resolve_owner_selector(
    graph: &ParsedCodeGraph,
    selector: TypeUseOwnerSelector<'_>,
) -> Result<AnyNodeId, SynParserError> {
    match selector {
        TypeUseOwnerSelector::Item {
            module_path,
            name,
            kind,
        } => find_item_id_by_path_name_kind_checked(graph, module_path, name, kind),
        TypeUseOwnerSelector::Impl(selector) => Ok(resolve_impl(graph, selector)?.id.as_any()),
        TypeUseOwnerSelector::Method {
            impl_selector,
            name,
        } => {
            let impl_node = resolve_impl(graph, impl_selector)?;
            let matches = impl_node
                .methods
                .iter()
                .filter(|method| method.name == name)
                .collect::<Vec<_>>();
            assert_eq!(
                matches.len(),
                1,
                "Expected exactly one method `{name}` in impl {}; found {}",
                impl_selector.describe(),
                matches.len()
            );
            Ok(matches[0].id.as_any())
        }
        TypeUseOwnerSelector::StructField {
            module_path,
            type_name,
            field,
        } => {
            let id = find_item_id_by_path_name_kind_checked(
                graph,
                module_path,
                type_name,
                ItemKind::Struct,
            )?;
            let node = graph.find_node_unique(id)?.as_struct().ok_or_else(|| {
                SynParserError::InternalState(format!("{type_name} did not resolve to a struct"))
            })?;
            Ok(select_field(&node.fields, field, selector.describe())?
                .id
                .as_any())
        }
        TypeUseOwnerSelector::UnionField {
            module_path,
            type_name,
            field,
        } => {
            let id = find_item_id_by_path_name_kind_checked(
                graph,
                module_path,
                type_name,
                ItemKind::Union,
            )?;
            let node = graph.find_node_unique(id)?.as_union().ok_or_else(|| {
                SynParserError::InternalState(format!("{type_name} did not resolve to a union"))
            })?;
            Ok(select_field(&node.fields, field, selector.describe())?
                .id
                .as_any())
        }
        TypeUseOwnerSelector::EnumVariantField {
            module_path,
            enum_name,
            variant_name,
            field,
        } => {
            let id = find_item_id_by_path_name_kind_checked(
                graph,
                module_path,
                enum_name,
                ItemKind::Enum,
            )?;
            let node = graph.find_node_unique(id)?.as_enum().ok_or_else(|| {
                SynParserError::InternalState(format!("{enum_name} did not resolve to an enum"))
            })?;
            let variants = node
                .variants
                .iter()
                .filter(|variant| variant.name == variant_name)
                .collect::<Vec<_>>();
            assert_eq!(
                variants.len(),
                1,
                "Expected exactly one enum variant `{variant_name}` in {}; found {}",
                selector.describe(),
                variants.len()
            );
            Ok(
                select_field(&variants[0].fields, field, selector.describe())?
                    .id
                    .as_any(),
            )
        }
    }
}

fn resolve_impl<'a>(
    graph: &'a ParsedCodeGraph,
    selector: ImplSelector<'_>,
) -> Result<&'a syn_parser::parser::nodes::ImplNode, SynParserError> {
    let module_path = selector
        .module_path
        .iter()
        .map(|segment| (*segment).to_string())
        .collect::<Vec<_>>();
    let module = graph.find_module_by_path_checked(&module_path)?;

    let matches = graph
        .impls()
        .iter()
        .filter(|impl_node| {
            graph
                .module_for_any_id(impl_node.id.as_any())
                .is_some_and(|owner_module| owner_module.id == module.id)
        })
        .filter(|impl_node| {
            type_root_path_matches(graph, impl_node.self_type, selector.self_type_path)
        })
        .filter(
            |impl_node| match (impl_node.trait_type, selector.trait_type_path) {
                (None, None) => true,
                (Some(type_id), Some(path)) => type_root_path_matches(graph, type_id, path),
                _ => false,
            },
        )
        .collect::<Vec<_>>();

    assert_eq!(
        matches.len(),
        1,
        "Expected exactly one impl {}; found {}",
        selector.describe(),
        matches.len()
    );

    Ok(matches[0])
}

fn select_field<'a>(
    fields: &'a [syn_parser::parser::nodes::FieldNode],
    selector: FieldSelector<'_>,
    owner: String,
) -> Result<&'a syn_parser::parser::nodes::FieldNode, SynParserError> {
    match selector {
        FieldSelector::Named(name) => {
            let matches = fields
                .iter()
                .filter(|field| field.name.as_deref() == Some(name))
                .collect::<Vec<_>>();
            assert_eq!(
                matches.len(),
                1,
                "Expected exactly one field `{name}` in {owner}; found {}",
                matches.len()
            );
            Ok(matches[0])
        }
        FieldSelector::Index(index) => fields.get(index).ok_or_else(|| {
            SynParserError::InternalState(format!(
                "Field index {index} out of bounds for {owner}; field count {}",
                fields.len()
            ))
        }),
    }
}

fn source_slot_type_id(
    graph: &ParsedCodeGraph,
    owner: AnyNodeId,
    source: TypeUseSourceSlot,
) -> Result<Option<TypeId>, SynParserError> {
    match source {
        TypeUseSourceSlot::Any => Ok(None),
        TypeUseSourceSlot::FunctionReturn => {
            let AnyNodeId::Function(id) = owner else {
                panic!("FunctionReturn source used with non-function owner {owner:?}");
            };
            Ok(graph
                .functions()
                .iter()
                .find(|function| function.id == id)
                .and_then(|function| function.return_type))
        }
        TypeUseSourceSlot::FunctionParam(index) => {
            let AnyNodeId::Function(id) = owner else {
                panic!("FunctionParam source used with non-function owner {owner:?}");
            };
            let function = graph
                .functions()
                .iter()
                .find(|function| function.id == id)
                .expect("function owner should exist");
            Ok(Some(function.parameters[index].type_id))
        }
        TypeUseSourceSlot::MethodReturn => {
            let method = find_method_by_owner(graph, owner);
            Ok(method.and_then(|method| method.return_type))
        }
        TypeUseSourceSlot::MethodParam(index) => {
            let method = find_method_by_owner(graph, owner).expect("method owner should exist");
            Ok(Some(method.parameters[index].type_id))
        }
        TypeUseSourceSlot::FieldType => Ok(Some(find_field_by_owner(graph, owner)?.type_id)),
        TypeUseSourceSlot::TypeAliasTarget => {
            let AnyNodeId::TypeAlias(id) = owner else {
                panic!("TypeAliasTarget source used with non-type-alias owner {owner:?}");
            };
            let alias = graph
                .defined_types()
                .iter()
                .find_map(|node| match node {
                    TypeDefNode::TypeAlias(alias) if alias.id == id => Some(alias),
                    _ => None,
                })
                .expect("type alias owner should exist");
            Ok(Some(alias.type_id))
        }
        TypeUseSourceSlot::ImplSelf => {
            let AnyNodeId::Impl(id) = owner else {
                panic!("ImplSelf source used with non-impl owner {owner:?}");
            };
            let impl_node = graph
                .impls()
                .iter()
                .find(|impl_node| impl_node.id == id)
                .expect("impl owner should exist");
            Ok(Some(impl_node.self_type))
        }
        TypeUseSourceSlot::ImplTrait => {
            let AnyNodeId::Impl(id) = owner else {
                panic!("ImplTrait source used with non-impl owner {owner:?}");
            };
            let impl_node = graph
                .impls()
                .iter()
                .find(|impl_node| impl_node.id == id)
                .expect("impl owner should exist");
            Ok(impl_node.trait_type)
        }
        TypeUseSourceSlot::TraitSuper(index) => {
            let AnyNodeId::Trait(id) = owner else {
                panic!("TraitSuper source used with non-trait owner {owner:?}");
            };
            let trait_node = graph
                .traits()
                .iter()
                .find(|trait_node| trait_node.id == id)
                .expect("trait owner should exist");
            Ok(Some(trait_node.super_traits[index]))
        }
        TypeUseSourceSlot::ConstType => {
            let AnyNodeId::Const(id) = owner else {
                panic!("ConstType source used with non-const owner {owner:?}");
            };
            let const_node = graph
                .consts()
                .iter()
                .find(|const_node| const_node.id == id)
                .expect("const owner should exist");
            Ok(Some(const_node.type_id))
        }
        TypeUseSourceSlot::StaticType => {
            let AnyNodeId::Static(id) = owner else {
                panic!("StaticType source used with non-static owner {owner:?}");
            };
            let static_node = graph
                .statics()
                .iter()
                .find(|static_node| static_node.id == id)
                .expect("static owner should exist");
            Ok(Some(static_node.type_id))
        }
    }
}

fn find_method_by_owner(
    graph: &ParsedCodeGraph,
    owner: AnyNodeId,
) -> Option<&syn_parser::parser::nodes::MethodNode> {
    let AnyNodeId::Method(id) = owner else {
        panic!("method source used with non-method owner {owner:?}");
    };

    graph
        .impls()
        .iter()
        .flat_map(|impl_node| &impl_node.methods)
        .chain(
            graph
                .traits()
                .iter()
                .flat_map(|trait_node| &trait_node.methods),
        )
        .find(|method| method.id == id)
}

fn find_field_by_owner(
    graph: &ParsedCodeGraph,
    owner: AnyNodeId,
) -> Result<&syn_parser::parser::nodes::FieldNode, SynParserError> {
    let AnyNodeId::Field(id) = owner else {
        panic!("FieldType source used with non-field owner {owner:?}");
    };

    graph
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

fn generic_param_name(
    graph: &ParsedCodeGraph,
    target: GenericParamNodeId,
) -> Result<String, SynParserError> {
    graph
        .functions()
        .iter()
        .flat_map(|node| node.generic_params.iter())
        .chain(
            graph
                .impls()
                .iter()
                .flat_map(|node| node.generic_params.iter()),
        )
        .chain(
            graph
                .impls()
                .iter()
                .flat_map(|node| node.methods.iter())
                .flat_map(|node| node.generic_params.iter()),
        )
        .chain(
            graph
                .traits()
                .iter()
                .flat_map(|node| node.generic_params.iter()),
        )
        .chain(
            graph
                .traits()
                .iter()
                .flat_map(|node| node.methods.iter())
                .flat_map(|node| node.generic_params.iter()),
        )
        .chain(graph.defined_types().iter().flat_map(|node| match node {
            TypeDefNode::Struct(node) => node.generic_params.iter(),
            TypeDefNode::Enum(node) => node.generic_params.iter(),
            TypeDefNode::Union(node) => node.generic_params.iter(),
            TypeDefNode::TypeAlias(node) => node.generic_params.iter(),
        }))
        .find(|param| param.id == target)
        .and_then(|param| param.kind.name())
        .map(str::to_string)
        .ok_or_else(|| {
            SynParserError::InternalState(format!(
                "generic param target {target:?} was not found in graph"
            ))
        })
}

fn type_tree_contains(graph: &ParsedCodeGraph, root: TypeId, needle: TypeId) -> bool {
    if root == needle {
        return true;
    }

    graph.resolve_type(root).is_some_and(|node| {
        node.related_types
            .iter()
            .any(|related| type_tree_contains(graph, *related, needle))
    })
}

fn type_root_path_matches(
    graph: &ParsedCodeGraph,
    type_id: TypeId,
    expected_path: &[&str],
) -> bool {
    let Some(type_node) = graph.resolve_type(type_id) else {
        return false;
    };

    match &type_node.kind {
        TypeKind::Named { path, .. } | TypeKind::TraitBound { path, .. } => {
            path_matches(path, expected_path)
        }
        _ => false,
    }
}

fn path_matches(actual: &[String], expected: &[&str]) -> bool {
    actual.len() == expected.len()
        && actual
            .iter()
            .map(String::as_str)
            .eq(expected.iter().copied())
}

fn matching_owner_rows_debug(
    report: &TypeResolutionReport,
    owner: AnyNodeId,
    role: TypeUseRole,
) -> String {
    let rows = report
        .resolutions
        .iter()
        .filter(|resolution| resolution.owner == owner && resolution.role == role)
        .map(|resolution| format!("  {resolution:?}"))
        .collect::<Vec<_>>();

    if rows.is_empty() {
        "  <none>".to_string()
    } else {
        rows.join("\n")
    }
}

fn matching_target_rows_debug(report: &TypeResolutionReport, target: AnyNodeId) -> String {
    let rows = report
        .resolutions
        .iter()
        .filter(|resolution| resolution.item_target() == Some(target))
        .map(|resolution| format!("  {resolution:?}"))
        .collect::<Vec<_>>();

    if rows.is_empty() {
        "  <none>".to_string()
    } else {
        rows.join("\n")
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
