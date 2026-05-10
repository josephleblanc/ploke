#![cfg(feature = "typed_type_graph")]

//! Integration tests for the typed type-resolution v2 relation surface.
//!
//! These cases assert exact typed relation endpoints rather than legacy
//! role/provenance rows. The macro table is intentionally broad: adding one
//! relation shape should mean adding one row, not another hand-written test
//! body that may accidentally weaken the source/target families.

use lazy_static::lazy_static;
use ploke_core::ItemKind;
use syn_parser::{
    parser::ParsedCodeGraph,
    resolve::{
        module_tree::ModuleTree,
        type_resolution_v2::{TypeRelationReport, resolve_type_relations_after_tree},
    },
};

use crate::common::build_tree_for_tests;
use crate::common::type_relation_resolution::{
    FieldSelector, TypeUseSourceSlot, enum_variant_field, impl_block, impl_selector, item, method,
    named, ordinary_item, ordinary_relation, ordinary_source, ordinary_type_param, root,
    struct_field, trait_item, trait_relation, trait_source, union_field,
};
use crate::{type_relation_cases, type_relations_present_case};

lazy_static! {
    static ref FIXTURE_TYPE_RESOLUTION_V2: (ParsedCodeGraph, ModuleTree) =
        build_tree_for_tests("fixture_type_resolution_v2");
    static ref FIXTURE_TYPE_RESOLUTION_V2_REPORT: TypeRelationReport =
        resolve_type_relations_after_tree(
            &FIXTURE_TYPE_RESOLUTION_V2.0,
            &FIXTURE_TYPE_RESOLUTION_V2.1
        )
        .expect("fixture_type_resolution_v2 type relation report");
    static ref FIXTURE_TYPES: (ParsedCodeGraph, ModuleTree) = build_tree_for_tests("fixture_types");
    static ref FIXTURE_TYPES_REPORT: TypeRelationReport =
        resolve_type_relations_after_tree(&FIXTURE_TYPES.0, &FIXTURE_TYPES.1)
            .expect("fixture_types type relation report");
    static ref FIXTURE_NODES: (ParsedCodeGraph, ModuleTree) = build_tree_for_tests("fixture_nodes");
    static ref FIXTURE_NODES_REPORT: TypeRelationReport =
        resolve_type_relations_after_tree(&FIXTURE_NODES.0, &FIXTURE_NODES.1)
            .expect("fixture_nodes type relation report");
    static ref FIXTURE_CONFLATION: (ParsedCodeGraph, ModuleTree) =
        build_tree_for_tests("fixture_conflation");
    static ref FIXTURE_CONFLATION_REPORT: TypeRelationReport =
        resolve_type_relations_after_tree(&FIXTURE_CONFLATION.0, &FIXTURE_CONFLATION.1)
            .expect("fixture_conflation type relation report");
}

type_relation_cases!(
    graph: &FIXTURE_TYPE_RESOLUTION_V2.0,
    report: &FIXTURE_TYPE_RESOLUTION_V2_REPORT,
    cases: [
        v2_generic_param_shadows_module_item_with_same_name => ordinary_relation(
            ordinary_source(
                item(&["crate"], "generic_shadow", ItemKind::Function),
                TypeUseSourceSlot::FunctionParam(0),
                root()
            ),
            ordinary_type_param(item(&["crate"], "generic_shadow", ItemKind::Function), "T")
        ),
        v2_resolves_ordinary_item_path_when_no_generic_param_shadows_it => ordinary_relation(
            ordinary_source(
                item(&["crate"], "concrete", ItemKind::Function),
                TypeUseSourceSlot::FunctionParam(0),
                root()
            ),
            ordinary_item(item(&["crate"], "T", ItemKind::Struct))
        ),
        v2_resolves_trait_position_supertrait => trait_relation(
            trait_source(
                item(&["crate"], "ChildTrait", ItemKind::Trait),
                TypeUseSourceSlot::TraitSuper(0),
                root()
            ),
            trait_item(item(&["crate"], "LocalTrait", ItemKind::Trait))
        ),
        v2_resolves_generic_declaration_bound => trait_relation(
            trait_source(
                item(&["crate"], "LocallyBound", ItemKind::Struct),
                TypeUseSourceSlot::GenericParamBound {
                    param_index: 0,
                    bound_index: 0
                },
                root()
            ),
            trait_item(item(&["crate"], "LocalTrait", ItemKind::Trait))
        ),
    ]
);

type_relation_cases!(
    graph: &FIXTURE_TYPES.0,
    report: &FIXTURE_TYPES_REPORT,
    cases: [
        fixture_types_v2_imported_point_function_param => ordinary_relation(
            ordinary_source(
                item(&["crate", "func", "return_types"], "consumes_point", ItemKind::Function),
                TypeUseSourceSlot::FunctionParam(0),
                root()
            ),
            ordinary_item(item(&["crate"], "Point", ItemKind::TypeAlias))
        ),
        fixture_types_v2_imported_math_operation_function_param => ordinary_relation(
            ordinary_source(
                item(
                    &["crate", "func", "return_types"],
                    "math_operation_consumer",
                    ItemKind::Function
                ),
                TypeUseSourceSlot::FunctionParam(0),
                root()
            ),
            ordinary_item(item(&["crate"], "MathOperation", ItemKind::TypeAlias))
        ),
        fixture_types_v2_imported_math_operation_function_return => ordinary_relation(
            ordinary_source(
                item(
                    &["crate", "func", "return_types"],
                    "math_operation_producer",
                    ItemKind::Function
                ),
                TypeUseSourceSlot::FunctionReturn,
                root()
            ),
            ordinary_item(item(&["crate"], "MathOperation", ItemKind::TypeAlias))
        ),
        fixture_types_v2_nested_imported_point_function_param => ordinary_relation(
            ordinary_source(
                item(
                    &["crate", "func", "return_types", "restricted_duplicate"],
                    "consumes_point",
                    ItemKind::Function
                ),
                TypeUseSourceSlot::FunctionParam(0),
                root()
            ),
            ordinary_item(item(&["crate"], "Point", ItemKind::TypeAlias))
        ),
    ]
);

type_relation_cases!(
    graph: &FIXTURE_NODES.0,
    report: &FIXTURE_NODES_REPORT,
    cases: [
        fixture_nodes_v2_inner_imported_simple_struct_impl_self => ordinary_relation(
            ordinary_source(
                impl_block(&["crate", "impls", "inner"], &["SimpleStruct"], None),
                TypeUseSourceSlot::ImplSelf,
                root()
            ),
            ordinary_item(item(&["crate", "impls"], "SimpleStruct", ItemKind::Struct))
        ),
        fixture_nodes_v2_inner_imported_simple_trait_impl_trait => trait_relation(
            trait_source(
                impl_block(
                    &["crate", "impls", "inner"],
                    &["InnerStruct"],
                    Some(&["SimpleTrait"])
                ),
                TypeUseSourceSlot::ImplTrait,
                root()
            ),
            trait_item(item(&["crate", "impls"], "SimpleTrait", ItemKind::Trait))
        ),
        fixture_nodes_v2_local_simple_trait_impl_self => ordinary_relation(
            ordinary_source(
                impl_block(&["crate", "impls"], &["SimpleStruct"], Some(&["SimpleTrait"])),
                TypeUseSourceSlot::ImplSelf,
                root()
            ),
            ordinary_item(item(&["crate", "impls"], "SimpleStruct", ItemKind::Struct))
        ),
        fixture_nodes_v2_local_simple_trait_impl_trait => trait_relation(
            trait_source(
                impl_block(&["crate", "impls"], &["SimpleStruct"], Some(&["SimpleTrait"])),
                TypeUseSourceSlot::ImplTrait,
                root()
            ),
            trait_item(item(&["crate", "impls"], "SimpleTrait", ItemKind::Trait))
        ),
        fixture_nodes_v2_simple_trait_supertrait => trait_relation(
            trait_source(
                item(&["crate", "traits"], "SuperTrait", ItemKind::Trait),
                TypeUseSourceSlot::TraitSuper(0),
                root()
            ),
            trait_item(item(&["crate", "traits"], "SimpleTrait", ItemKind::Trait))
        ),
        fixture_nodes_v2_multi_supertrait_simple_trait => trait_relation(
            trait_source(
                item(&["crate", "traits"], "MultiSuperTrait", ItemKind::Trait),
                TypeUseSourceSlot::TraitSuper(0),
                root()
            ),
            trait_item(item(&["crate", "traits"], "SimpleTrait", ItemKind::Trait))
        ),
        fixture_nodes_v2_multi_supertrait_internal_trait => trait_relation(
            trait_source(
                item(&["crate", "traits"], "MultiSuperTrait", ItemKind::Trait),
                TypeUseSourceSlot::TraitSuper(1),
                root()
            ),
            trait_item(item(&["crate", "traits"], "InternalTrait", ItemKind::Trait))
        ),
        fixture_nodes_v2_generic_trait_supertrait => trait_relation(
            trait_source(
                item(&["crate", "traits"], "GenericSuperTrait", ItemKind::Trait),
                TypeUseSourceSlot::TraitSuper(0),
                root()
            ),
            trait_item(item(&["crate", "traits"], "GenericTrait", ItemKind::Trait))
        ),
        fixture_nodes_v2_type_alias_to_type_alias => ordinary_relation(
            ordinary_source(
                item(&["crate", "type_alias"], "IdAlias", ItemKind::TypeAlias),
                TypeUseSourceSlot::TypeAliasTarget,
                root()
            ),
            ordinary_item(item(&["crate", "type_alias"], "SimpleId", ItemKind::TypeAlias))
        ),
        fixture_nodes_v2_inner_outer_point_type_alias_target => ordinary_relation(
            ordinary_source(
                item(&["crate", "type_alias", "inner"], "OuterPoint", ItemKind::TypeAlias),
                TypeUseSourceSlot::TypeAliasTarget,
                root()
            ),
            ordinary_item(item(&["crate", "type_alias"], "Point", ItemKind::TypeAlias))
        ),
        fixture_nodes_v2_use_inner_type_alias_target => ordinary_relation(
            ordinary_source(
                item(&["crate", "type_alias"], "UseInner", ItemKind::TypeAlias),
                TypeUseSourceSlot::TypeAliasTarget,
                root()
            ),
            ordinary_item(item(
                &["crate", "type_alias", "inner"],
                "InnerPublic",
                ItemKind::TypeAlias
            ))
        ),
        fixture_nodes_v2_const_struct_type => ordinary_relation(
            ordinary_source(
                item(&["crate", "const_static"], "STRUCT_CONST", ItemKind::Const),
                TypeUseSourceSlot::ConstType,
                root()
            ),
            ordinary_item(item(&["crate", "const_static"], "SimpleStruct", ItemKind::Struct))
        ),
        fixture_nodes_v2_const_alias_type => ordinary_relation(
            ordinary_source(
                item(&["crate", "const_static"], "ALIASED_CONST", ItemKind::Const),
                TypeUseSourceSlot::ConstType,
                root()
            ),
            ordinary_item(item(&["crate", "const_static"], "MyInt", ItemKind::TypeAlias))
        ),
        fixture_nodes_v2_generic_struct_field_type_param => ordinary_relation(
            ordinary_source(
                struct_field(&["crate", "structs"], "GenericStruct", FieldSelector::Index(0)),
                TypeUseSourceSlot::FieldType,
                root()
            ),
            ordinary_type_param(item(&["crate", "structs"], "GenericStruct", ItemKind::Struct), "T")
        ),
        fixture_nodes_v2_enum_variant_field_type_param => ordinary_relation(
            ordinary_source(
                enum_variant_field(
                    &["crate", "enums"],
                    "JustTypeGeneric",
                    "VariantA",
                    FieldSelector::Index(0)
                ),
                TypeUseSourceSlot::FieldType,
                root()
            ),
            ordinary_type_param(item(&["crate", "enums"], "JustTypeGeneric", ItemKind::Enum), "A")
        ),
        fixture_nodes_v2_generic_union_field_nested_type_param => ordinary_relation(
            ordinary_source(
                union_field(&["crate", "unions"], "GenericUnion", FieldSelector::Index(0)),
                TypeUseSourceSlot::FieldType,
                named(&["T"])
            ),
            ordinary_type_param(item(&["crate", "unions"], "GenericUnion", ItemKind::Union), "T")
        ),
        fixture_nodes_v2_trait_impl_method_param_type_param => ordinary_relation(
            ordinary_source(
                method(
                    impl_selector(
                        &["crate", "impls"],
                        &["GenericStruct"],
                        Some(&["GenericTrait"])
                    ),
                    "generic_trait_method"
                ),
                TypeUseSourceSlot::MethodParam(1),
                root()
            ),
            ordinary_type_param(
                impl_block(
                    &["crate", "impls"],
                    &["GenericStruct"],
                    Some(&["GenericTrait"])
                ),
                "T"
            )
        ),
    ]
);

type_relations_present_case!(
    fixture_nodes_v2_generic_trait_supertrait_complete_slot,
    graph: &FIXTURE_NODES.0,
    report: &FIXTURE_NODES_REPORT,
    expected: [
        trait_relation(
            trait_source(
                item(&["crate", "traits"], "GenericSuperTrait", ItemKind::Trait),
                TypeUseSourceSlot::TraitSuper(0),
                root()
            ),
            trait_item(item(&["crate", "traits"], "GenericTrait", ItemKind::Trait))
        ),
        ordinary_relation(
            ordinary_source(
                item(&["crate", "traits"], "GenericSuperTrait", ItemKind::Trait),
                TypeUseSourceSlot::TraitSuper(0),
                named(&["T"])
            ),
            ordinary_type_param(item(&["crate", "traits"], "GenericSuperTrait", ItemKind::Trait), "T")
        ),
    ]
);

type_relation_cases!(
    graph: &FIXTURE_CONFLATION.0,
    report: &FIXTURE_CONFLATION_REPORT,
    cases: [
        fixture_conflation_v2_imported_top_level_trait_impl_trait => trait_relation(
            trait_source(
                impl_block(
                    &["crate", "inner_mod"],
                    &["InnerStruct"],
                    Some(&["TopLevelTrait"])
                ),
                TypeUseSourceSlot::ImplTrait,
                root()
            ),
            trait_item(item(&["crate"], "TopLevelTrait", ItemKind::Trait))
        ),
        fixture_conflation_v2_inner_struct_self_return => ordinary_relation(
            ordinary_source(
                method(
                    impl_selector(&["crate", "inner_mod"], &["InnerStruct"], None),
                    "inner_method"
                ),
                TypeUseSourceSlot::MethodReturn,
                root()
            ),
            ordinary_item(item(&["crate", "inner_mod"], "InnerStruct", ItemKind::Struct))
        ),
    ]
);

type_relations_present_case!(
    fixture_conflation_v2_nested_generic_field_complete_slot,
    graph: &FIXTURE_CONFLATION.0,
    report: &FIXTURE_CONFLATION_REPORT,
    expected: [
        ordinary_relation(
            ordinary_source(
                struct_field(&["crate"], "NestedGeneric", FieldSelector::Index(0)),
                TypeUseSourceSlot::FieldType,
                named(&["TopLevelStruct"])
            ),
            ordinary_item(item(&["crate"], "TopLevelStruct", ItemKind::Struct))
        ),
        ordinary_relation(
            ordinary_source(
                struct_field(&["crate"], "NestedGeneric", FieldSelector::Index(0)),
                TypeUseSourceSlot::FieldType,
                named(&["T"])
            ),
            ordinary_type_param(item(&["crate"], "NestedGeneric", ItemKind::Struct), "T")
        ),
    ]
);
