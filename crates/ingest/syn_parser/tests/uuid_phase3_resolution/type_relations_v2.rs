#![cfg(feature = "typed_type_graph")]

//! Integration tests for the typed type-resolution v2 relation surface.
//!
//! These cases assert exact typed relation endpoints rather than legacy
//! role/provenance rows. The macro table is intentionally broad: adding one
//! relation shape should mean adding one row, not another hand-written test
//! body that may accidentally weaken the source/target families.
//!
//! Where-clause coverage contract:
//!
//! The parser/resolver layer models where predicates as source-coordinate
//! facts over structural type uses. It does not model DB ownership or
//! graphRAG expansion here; those are tested in `ploke-db`.
//!
//! Covered where shapes in `fixture_type_resolution_v2`:
//!
//! - Direct type-param subject: `where T: LocalTrait`.
//! - Multiple bounds on one predicate: `where T: LocalTrait + ExtraTrait`.
//! - Multiple predicates: `where T: LocalTrait, U: ExtraTrait`.
//! - Repeated same-subject predicates: `where T: LocalTrait, T: ExtraTrait`.
//! - Enum, function, method, trait, type-alias, union, and impl owners with
//!   where predicates.
//! - Composite subject: `where Vec<T>: LocalTrait`.
//! - Composite subject with multiple bounds:
//!   `where Vec<T>: LocalTrait + ExtraTrait`.
//! - Projection subject: `where <T as LocalAssocBound>::Output: LocalTrait`.
//! - Projection subject with multiple bounds:
//!   `where <T as LocalAssocBound>::Output: LocalTrait + ExtraTrait`.
//!
//! For each covered shape, the tests assert exact `TypeUseSourceSlot`
//! coordinates: where-subject `predicate_index`, where-bound
//! `{ predicate_index, bound_index }`, and the terminal selector reached
//! through the structural type tree.
//! The corpus-backed `TypeShapeCase` matrix in `ploke-test-utils` consumes the
//! same coordinate model for DB/RAG/TUI coverage; this parser file remains the
//! focused source-coordinate authority for the synthetic fixture.
//!
//! Not covered at this parser layer:
//!
//! - Direct generic-param-owned where-bound roots. Those are transform/DB
//!   projections derived only for simple direct type-param subjects.
//! - Nested recursive subject examples beyond the deepest focused fixtures:
//!   `Vec<T>` and `<T as LocalAssocBound>::Output`. If deeper recursive
//!   subject fixtures are added, add exact-source assertions for the deepest
//!   available nesting rather than a looser existence check.
//! - Semantic non-node targets such as primitives, external dependency items,
//!   `Self`, lifetimes, and const generic values. These require explicit target
//!   modeling before they can be asserted as resolved endpoints.

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
    FieldSelector, TypeUseSourceSlot, enum_variant_field, impl_associated_const,
    impl_associated_type, impl_block, impl_selector, item, method, named, ordinary_item,
    ordinary_relation, ordinary_source, ordinary_type_param, root, struct_field,
    trait_associated_const, trait_associated_type, trait_item, trait_relation, trait_source,
    union_field,
};
use crate::{type_relation_cases, type_relations_exact_sources_case};

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
        v2_resolves_generic_type_parameter_default => ordinary_relation(
            ordinary_source(
                item(&["crate"], "GenericDefault", ItemKind::Struct),
                TypeUseSourceSlot::GenericParamDefault(0),
                root()
            ),
            ordinary_item(item(&["crate"], "LocalType", ItemKind::Struct))
        ),
        v2_resolves_const_generic_parameter_type_annotation => ordinary_relation(
            ordinary_source(
                item(&["crate"], "ConstGenericAnnotated", ItemKind::Struct),
                TypeUseSourceSlot::ConstGenericParamType(0),
                root()
            ),
            ordinary_item(item(&["crate"], "AliasOrPrimitive", ItemKind::TypeAlias))
        ),
        v2_resolves_qualified_projection_trait_qualifier => trait_relation(
            trait_source(
                item(&["crate"], "ProjectedArrayLength", ItemKind::TypeAlias),
                TypeUseSourceSlot::TypeAliasTarget,
                named(&["IntoArrayLength"])
            ),
            trait_item(item(&["crate"], "IntoArrayLength", ItemKind::Trait))
        ),
        v2_resolves_associated_type_bound => trait_relation(
            trait_source(
                item(&["crate"], "LocalAssocBound", ItemKind::Trait),
                TypeUseSourceSlot::AssociatedTypeBound(0),
                root()
            ),
            trait_item(item(&["crate"], "LocalTrait", ItemKind::Trait))
        ),
        v2_resolves_trait_associated_type_default => ordinary_relation(
            ordinary_source(
                trait_associated_type(&["crate"], "AssociatedDefaults", "Defaulted"),
                TypeUseSourceSlot::TypeAliasTarget,
                root()
            ),
            ordinary_item(item(&["crate"], "LocalType", ItemKind::Struct))
        ),
        v2_resolves_impl_associated_type_definition => ordinary_relation(
            ordinary_source(
                impl_associated_type(
                    impl_selector(&["crate"], &["AssociatedImpl"], Some(&["AssociatedDefaults"])),
                    "Defaulted"
                ),
                TypeUseSourceSlot::TypeAliasTarget,
                root()
            ),
            ordinary_item(item(&["crate"], "LocalType", ItemKind::Struct))
        ),
        v2_resolves_trait_associated_const_type_annotation => ordinary_relation(
            ordinary_source(
                trait_associated_const(&["crate"], "AssociatedDefaults", "TRAIT_CONST"),
                TypeUseSourceSlot::ConstType,
                root()
            ),
            ordinary_item(item(&["crate"], "AliasOrPrimitive", ItemKind::TypeAlias))
        ),
        v2_resolves_impl_associated_const_type_annotation => ordinary_relation(
            ordinary_source(
                impl_associated_const(
                    impl_selector(&["crate"], &["AssociatedImpl"], Some(&["AssociatedDefaults"])),
                    "TRAIT_CONST"
                ),
                TypeUseSourceSlot::ConstType,
                root()
            ),
            ordinary_item(item(&["crate"], "AliasOrPrimitive", ItemKind::TypeAlias))
        ),
        v2_resolves_trait_associated_type_default_to_enclosing_generic_param => ordinary_relation(
            ordinary_source(
                trait_associated_type(&["crate"], "AssociatedGenericDefaults", "Defaulted"),
                TypeUseSourceSlot::TypeAliasTarget,
                root()
            ),
            ordinary_type_param(
                item(&["crate"], "AssociatedGenericDefaults", ItemKind::Trait),
                "T"
            )
        ),
        v2_resolves_impl_associated_type_definition_to_enclosing_generic_param => ordinary_relation(
            ordinary_source(
                impl_associated_type(
                    impl_selector(
                        &["crate"],
                        &["GenericAssociatedImpl"],
                        Some(&["AssociatedGenericDefaults"])
                    ),
                    "Defaulted"
                ),
                TypeUseSourceSlot::TypeAliasTarget,
                root()
            ),
            ordinary_type_param(
                impl_block(
                    &["crate"],
                    &["GenericAssociatedImpl"],
                    Some(&["AssociatedGenericDefaults"])
                ),
                "T"
            )
        ),
        v2_resolves_trait_associated_const_type_to_enclosing_generic_param => ordinary_relation(
            ordinary_source(
                trait_associated_const(&["crate"], "AssociatedGenericDefaults", "TRAIT_CONST"),
                TypeUseSourceSlot::ConstType,
                root()
            ),
            ordinary_type_param(
                item(&["crate"], "AssociatedGenericDefaults", ItemKind::Trait),
                "T"
            )
        ),
        v2_resolves_impl_associated_const_type_to_enclosing_generic_param => ordinary_relation(
            ordinary_source(
                impl_associated_const(
                    impl_selector(
                        &["crate"],
                        &["GenericAssociatedImpl"],
                        Some(&["AssociatedGenericDefaults"])
                    ),
                    "TRAIT_CONST"
                ),
                TypeUseSourceSlot::ConstType,
                root()
            ),
            ordinary_type_param(
                impl_block(
                    &["crate"],
                    &["GenericAssociatedImpl"],
                    Some(&["AssociatedGenericDefaults"])
                ),
                "T"
            )
        ),
        v2_resolves_where_direct_type_param_bound => trait_relation(
            trait_source(
                item(&["crate"], "WhereLocal", ItemKind::Struct),
                TypeUseSourceSlot::WherePredicateBound {
                    predicate_index: 0,
                    bound_index: 0
                },
                root()
            ),
            trait_item(item(&["crate"], "LocalTrait", ItemKind::Trait))
        ),
        v2_resolves_where_direct_type_param_subject => ordinary_relation(
            ordinary_source(
                item(&["crate"], "WhereLocal", ItemKind::Struct),
                TypeUseSourceSlot::WherePredicateSubject(0),
                root()
            ),
            ordinary_type_param(item(&["crate"], "WhereLocal", ItemKind::Struct), "T")
        ),
        v2_resolves_where_multi_bound_first_trait => trait_relation(
            trait_source(
                item(&["crate"], "WhereMultiBound", ItemKind::Struct),
                TypeUseSourceSlot::WherePredicateBound {
                    predicate_index: 0,
                    bound_index: 0
                },
                root()
            ),
            trait_item(item(&["crate"], "LocalTrait", ItemKind::Trait))
        ),
        v2_resolves_where_multi_bound_second_trait => trait_relation(
            trait_source(
                item(&["crate"], "WhereMultiBound", ItemKind::Struct),
                TypeUseSourceSlot::WherePredicateBound {
                    predicate_index: 0,
                    bound_index: 1
                },
                root()
            ),
            trait_item(item(&["crate"], "ExtraTrait", ItemKind::Trait))
        ),
        v2_resolves_where_multi_bound_subject => ordinary_relation(
            ordinary_source(
                item(&["crate"], "WhereMultiBound", ItemKind::Struct),
                TypeUseSourceSlot::WherePredicateSubject(0),
                root()
            ),
            ordinary_type_param(item(&["crate"], "WhereMultiBound", ItemKind::Struct), "T")
        ),
        v2_resolves_where_multi_predicate_first_subject => ordinary_relation(
            ordinary_source(
                item(&["crate"], "WhereMultiPredicate", ItemKind::Struct),
                TypeUseSourceSlot::WherePredicateSubject(0),
                root()
            ),
            ordinary_type_param(item(&["crate"], "WhereMultiPredicate", ItemKind::Struct), "T")
        ),
        v2_resolves_where_multi_predicate_second_subject => ordinary_relation(
            ordinary_source(
                item(&["crate"], "WhereMultiPredicate", ItemKind::Struct),
                TypeUseSourceSlot::WherePredicateSubject(1),
                root()
            ),
            ordinary_type_param(item(&["crate"], "WhereMultiPredicate", ItemKind::Struct), "U")
        ),
        v2_resolves_where_multi_predicate_first_bound => trait_relation(
            trait_source(
                item(&["crate"], "WhereMultiPredicate", ItemKind::Struct),
                TypeUseSourceSlot::WherePredicateBound {
                    predicate_index: 0,
                    bound_index: 0
                },
                root()
            ),
            trait_item(item(&["crate"], "LocalTrait", ItemKind::Trait))
        ),
        v2_resolves_where_multi_predicate_second_bound => trait_relation(
            trait_source(
                item(&["crate"], "WhereMultiPredicate", ItemKind::Struct),
                TypeUseSourceSlot::WherePredicateBound {
                    predicate_index: 1,
                    bound_index: 0
                },
                root()
            ),
            trait_item(item(&["crate"], "ExtraTrait", ItemKind::Trait))
        ),
        v2_resolves_where_repeated_subject_first_bound => trait_relation(
            trait_source(
                item(&["crate"], "WhereRepeatedSubject", ItemKind::Struct),
                TypeUseSourceSlot::WherePredicateBound {
                    predicate_index: 0,
                    bound_index: 0
                },
                root()
            ),
            trait_item(item(&["crate"], "LocalTrait", ItemKind::Trait))
        ),
        v2_resolves_where_repeated_subject_second_bound => trait_relation(
            trait_source(
                item(&["crate"], "WhereRepeatedSubject", ItemKind::Struct),
                TypeUseSourceSlot::WherePredicateBound {
                    predicate_index: 1,
                    bound_index: 0
                },
                root()
            ),
            trait_item(item(&["crate"], "ExtraTrait", ItemKind::Trait))
        ),
        v2_resolves_where_repeated_subject_first_subject => ordinary_relation(
            ordinary_source(
                item(&["crate"], "WhereRepeatedSubject", ItemKind::Struct),
                TypeUseSourceSlot::WherePredicateSubject(0),
                root()
            ),
            ordinary_type_param(item(&["crate"], "WhereRepeatedSubject", ItemKind::Struct), "T")
        ),
        v2_resolves_where_repeated_subject_second_subject => ordinary_relation(
            ordinary_source(
                item(&["crate"], "WhereRepeatedSubject", ItemKind::Struct),
                TypeUseSourceSlot::WherePredicateSubject(1),
                root()
            ),
            ordinary_type_param(item(&["crate"], "WhereRepeatedSubject", ItemKind::Struct), "T")
        ),
        v2_resolves_where_enum_bound => trait_relation(
            trait_source(
                item(&["crate"], "WhereEnum", ItemKind::Enum),
                TypeUseSourceSlot::WherePredicateBound {
                    predicate_index: 0,
                    bound_index: 0
                },
                root()
            ),
            trait_item(item(&["crate"], "LocalTrait", ItemKind::Trait))
        ),
        v2_resolves_where_enum_subject => ordinary_relation(
            ordinary_source(
                item(&["crate"], "WhereEnum", ItemKind::Enum),
                TypeUseSourceSlot::WherePredicateSubject(0),
                root()
            ),
            ordinary_type_param(item(&["crate"], "WhereEnum", ItemKind::Enum), "T")
        ),
        v2_resolves_where_fn_bound => trait_relation(
            trait_source(
                item(&["crate"], "where_fn", ItemKind::Function),
                TypeUseSourceSlot::WherePredicateBound {
                    predicate_index: 0,
                    bound_index: 0
                },
                root()
            ),
            trait_item(item(&["crate"], "LocalTrait", ItemKind::Trait))
        ),
        v2_resolves_where_fn_subject => ordinary_relation(
            ordinary_source(
                item(&["crate"], "where_fn", ItemKind::Function),
                TypeUseSourceSlot::WherePredicateSubject(0),
                root()
            ),
            ordinary_type_param(item(&["crate"], "where_fn", ItemKind::Function), "T")
        ),
        v2_resolves_where_method_bound => trait_relation(
            trait_source(
                method(
                    impl_selector(&["crate"], &["WhereMethod"], None),
                    "where_method"
                ),
                TypeUseSourceSlot::WherePredicateBound {
                    predicate_index: 0,
                    bound_index: 0
                },
                root()
            ),
            trait_item(item(&["crate"], "LocalTrait", ItemKind::Trait))
        ),
        v2_resolves_where_method_subject => ordinary_relation(
            ordinary_source(
                method(
                    impl_selector(&["crate"], &["WhereMethod"], None),
                    "where_method"
                ),
                TypeUseSourceSlot::WherePredicateSubject(0),
                root()
            ),
            ordinary_type_param(
                method(
                    impl_selector(&["crate"], &["WhereMethod"], None),
                    "where_method"
                ),
                "T"
            )
        ),
        v2_resolves_where_trait_bound => trait_relation(
            trait_source(
                item(&["crate"], "WhereTrait", ItemKind::Trait),
                TypeUseSourceSlot::WherePredicateBound {
                    predicate_index: 0,
                    bound_index: 0
                },
                root()
            ),
            trait_item(item(&["crate"], "LocalTrait", ItemKind::Trait))
        ),
        v2_resolves_where_trait_subject => ordinary_relation(
            ordinary_source(
                item(&["crate"], "WhereTrait", ItemKind::Trait),
                TypeUseSourceSlot::WherePredicateSubject(0),
                root()
            ),
            ordinary_type_param(item(&["crate"], "WhereTrait", ItemKind::Trait), "T")
        ),
        v2_resolves_where_union_bound => trait_relation(
            trait_source(
                item(&["crate"], "WhereUnion", ItemKind::Union),
                TypeUseSourceSlot::WherePredicateBound {
                    predicate_index: 0,
                    bound_index: 0
                },
                root()
            ),
            trait_item(item(&["crate"], "LocalTrait", ItemKind::Trait))
        ),
        v2_resolves_where_union_subject => ordinary_relation(
            ordinary_source(
                item(&["crate"], "WhereUnion", ItemKind::Union),
                TypeUseSourceSlot::WherePredicateSubject(0),
                root()
            ),
            ordinary_type_param(item(&["crate"], "WhereUnion", ItemKind::Union), "T")
        ),
        v2_resolves_where_type_alias_multi_bound_first_trait => trait_relation(
            trait_source(
                item(&["crate"], "WhereAliasMulti", ItemKind::TypeAlias),
                TypeUseSourceSlot::WherePredicateBound {
                    predicate_index: 0,
                    bound_index: 0
                },
                root()
            ),
            trait_item(item(&["crate"], "LocalTrait", ItemKind::Trait))
        ),
        v2_resolves_where_type_alias_multi_bound_second_trait => trait_relation(
            trait_source(
                item(&["crate"], "WhereAliasMulti", ItemKind::TypeAlias),
                TypeUseSourceSlot::WherePredicateBound {
                    predicate_index: 0,
                    bound_index: 1
                },
                root()
            ),
            trait_item(item(&["crate"], "ExtraTrait", ItemKind::Trait))
        ),
        v2_resolves_where_type_alias_subject => ordinary_relation(
            ordinary_source(
                item(&["crate"], "WhereAliasMulti", ItemKind::TypeAlias),
                TypeUseSourceSlot::WherePredicateSubject(0),
                root()
            ),
            ordinary_type_param(item(&["crate"], "WhereAliasMulti", ItemKind::TypeAlias), "T")
        ),
        v2_resolves_where_impl_multi_bound_first_trait => trait_relation(
            trait_source(
                impl_block(&["crate"], &["WhereImpl"], Some(&["LocalTrait"])),
                TypeUseSourceSlot::WherePredicateBound {
                    predicate_index: 0,
                    bound_index: 0
                },
                root()
            ),
            trait_item(item(&["crate"], "ExtraTrait", ItemKind::Trait))
        ),
        v2_resolves_where_impl_multi_bound_second_trait => trait_relation(
            trait_source(
                impl_block(&["crate"], &["WhereImpl"], Some(&["LocalTrait"])),
                TypeUseSourceSlot::WherePredicateBound {
                    predicate_index: 0,
                    bound_index: 1
                },
                root()
            ),
            trait_item(item(&["crate"], "AnotherTrait", ItemKind::Trait))
        ),
        v2_resolves_where_impl_subject => ordinary_relation(
            ordinary_source(
                impl_block(&["crate"], &["WhereImpl"], Some(&["LocalTrait"])),
                TypeUseSourceSlot::WherePredicateSubject(0),
                root()
            ),
            ordinary_type_param(
                impl_block(&["crate"], &["WhereImpl"], Some(&["LocalTrait"])),
                "T"
            )
        ),
        v2_resolves_where_composite_subject_nested_type_param => ordinary_relation(
            ordinary_source(
                item(&["crate"], "WhereComposite", ItemKind::Struct),
                TypeUseSourceSlot::WherePredicateSubject(0),
                named(&["T"])
            ),
            ordinary_type_param(item(&["crate"], "WhereComposite", ItemKind::Struct), "T")
        ),
        v2_resolves_where_composite_multi_subject_nested_type_param => ordinary_relation(
            ordinary_source(
                item(&["crate"], "WhereCompositeMulti", ItemKind::Struct),
                TypeUseSourceSlot::WherePredicateSubject(0),
                named(&["T"])
            ),
            ordinary_type_param(item(&["crate"], "WhereCompositeMulti", ItemKind::Struct), "T")
        ),
        v2_resolves_where_composite_multi_second_bound => trait_relation(
            trait_source(
                item(&["crate"], "WhereCompositeMulti", ItemKind::Struct),
                TypeUseSourceSlot::WherePredicateBound {
                    predicate_index: 0,
                    bound_index: 1
                },
                root()
            ),
            trait_item(item(&["crate"], "ExtraTrait", ItemKind::Trait))
        ),
        v2_resolves_where_projection_subject_trait_qualifier => trait_relation(
            trait_source(
                item(&["crate"], "WhereProjection", ItemKind::TypeAlias),
                TypeUseSourceSlot::WherePredicateSubject(0),
                named(&["LocalAssocBound"])
            ),
            trait_item(item(&["crate"], "LocalAssocBound", ItemKind::Trait))
        ),
        v2_resolves_where_projection_multi_subject_trait_qualifier => trait_relation(
            trait_source(
                item(&["crate"], "WhereProjectionMulti", ItemKind::TypeAlias),
                TypeUseSourceSlot::WherePredicateSubject(0),
                named(&["LocalAssocBound"])
            ),
            trait_item(item(&["crate"], "LocalAssocBound", ItemKind::Trait))
        ),
        v2_resolves_where_projection_multi_second_bound => trait_relation(
            trait_source(
                item(&["crate"], "WhereProjectionMulti", ItemKind::TypeAlias),
                TypeUseSourceSlot::WherePredicateBound {
                    predicate_index: 0,
                    bound_index: 1
                },
                root()
            ),
            trait_item(item(&["crate"], "ExtraTrait", ItemKind::Trait))
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
        fixture_nodes_v2_renamed_local_struct_import_param => ordinary_relation(
            ordinary_source(
                item(
                    &["crate", "imports"],
                    "renamed_local_struct_import_param",
                    ItemKind::Function
                ),
                TypeUseSourceSlot::FunctionParam(0),
                root()
            ),
            ordinary_item(item(&["crate", "structs"], "SampleStruct", ItemKind::Struct))
        ),
        fixture_nodes_v2_glob_imported_documented_trait_bound => trait_relation(
            trait_source(
                item(
                    &["crate", "imports"],
                    "glob_imported_documented_trait_bound",
                    ItemKind::Function
                ),
                TypeUseSourceSlot::GenericParamBound {
                    param_index: 0,
                    bound_index: 0
                },
                root()
            ),
            trait_item(item(&["crate", "traits"], "DocumentedTrait", ItemKind::Trait))
        ),
        fixture_nodes_v2_multi_hop_reexport_trait_bound => trait_relation(
            trait_source(
                item(
                    &["crate", "imports"],
                    "multi_hop_reexport_trait_bound",
                    ItemKind::Function
                ),
                TypeUseSourceSlot::GenericParamBound {
                    param_index: 0,
                    bound_index: 0
                },
                root()
            ),
            trait_item(item(&["crate", "traits"], "SimpleTrait", ItemKind::Trait))
        ),
        fixture_nodes_v2_crate_boundary_type_alias_import_param => ordinary_relation(
            ordinary_source(
                item(
                    &["crate", "imports"],
                    "crate_boundary_type_alias_param",
                    ItemKind::Function
                ),
                TypeUseSourceSlot::FunctionParam(0),
                root()
            ),
            ordinary_item(item(&["crate", "type_alias"], "SimpleId", ItemKind::TypeAlias))
        ),
        fixture_nodes_v2_nested_super_super_tuple_struct_import_param => ordinary_relation(
            ordinary_source(
                item(
                    &["crate", "imports", "sub_imports"],
                    "tuple_struct_from_grandparent_import",
                    ItemKind::Function
                ),
                TypeUseSourceSlot::FunctionParam(0),
                root()
            ),
            ordinary_item(item(&["crate", "structs"], "TupleStruct", ItemKind::Struct))
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
        fixture_nodes_v2_type_alias_where_subject_type_param => ordinary_relation(
            ordinary_source(
                item(&["crate", "type_alias"], "ComplexGeneric", ItemKind::TypeAlias),
                TypeUseSourceSlot::WherePredicateSubject(0),
                root()
            ),
            ordinary_type_param(
                item(&["crate", "type_alias"], "ComplexGeneric", ItemKind::TypeAlias),
                "T"
            )
        ),
        fixture_nodes_v2_impl_where_subject_type_param => ordinary_relation(
            ordinary_source(
                impl_block(
                    &["crate", "impls"],
                    &["GenericStruct"],
                    Some(&["SimpleTrait"])
                ),
                TypeUseSourceSlot::WherePredicateSubject(0),
                root()
            ),
            ordinary_type_param(
                impl_block(
                    &["crate", "impls"],
                    &["GenericStruct"],
                    Some(&["SimpleTrait"])
                ),
                "T"
            )
        ),
        fixture_nodes_v2_generic_enum_where_subject_type_param => ordinary_relation(
            ordinary_source(
                item(&["crate", "enums"], "GenericEnum", ItemKind::Enum),
                TypeUseSourceSlot::WherePredicateSubject(0),
                root()
            ),
            ordinary_type_param(item(&["crate", "enums"], "GenericEnum", ItemKind::Enum), "T")
        ),
        fixture_nodes_v2_just_where_clause_subject_type_param => ordinary_relation(
            ordinary_source(
                item(&["crate", "enums"], "JustWhereClause", ItemKind::Enum),
                TypeUseSourceSlot::WherePredicateSubject(0),
                root()
            ),
            ordinary_type_param(item(&["crate", "enums"], "JustWhereClause", ItemKind::Enum), "T")
        ),
    ]
);

type_relations_exact_sources_case!(
    fixture_type_resolution_v2_projection_and_where_sources_are_exact,
    graph: &FIXTURE_TYPE_RESOLUTION_V2.0,
    report: &FIXTURE_TYPE_RESOLUTION_V2_REPORT,
    expected: [
        trait_relation(
            trait_source(
                item(&["crate"], "ProjectedArrayLength", ItemKind::TypeAlias),
                TypeUseSourceSlot::TypeAliasTarget,
                named(&["IntoArrayLength"])
            ),
            trait_item(item(&["crate"], "IntoArrayLength", ItemKind::Trait))
        ),
        trait_relation(
            trait_source(
                item(&["crate"], "LocalAssocBound", ItemKind::Trait),
                TypeUseSourceSlot::AssociatedTypeBound(0),
                root()
            ),
            trait_item(item(&["crate"], "LocalTrait", ItemKind::Trait))
        ),
        ordinary_relation(
            ordinary_source(
                item(&["crate"], "WhereComposite", ItemKind::Struct),
                TypeUseSourceSlot::WherePredicateSubject(0),
                named(&["T"])
            ),
            ordinary_type_param(item(&["crate"], "WhereComposite", ItemKind::Struct), "T")
        ),
        trait_relation(
            trait_source(
                item(&["crate"], "WhereProjection", ItemKind::TypeAlias),
                TypeUseSourceSlot::WherePredicateSubject(0),
                named(&["LocalAssocBound"])
            ),
            trait_item(item(&["crate"], "LocalAssocBound", ItemKind::Trait))
        ),
    ]
);

type_relations_exact_sources_case!(
    fixture_type_resolution_v2_expanded_where_sources_are_exact,
    graph: &FIXTURE_TYPE_RESOLUTION_V2.0,
    report: &FIXTURE_TYPE_RESOLUTION_V2_REPORT,
    expected: [
        ordinary_relation(
            ordinary_source(
                item(&["crate"], "WhereMultiBound", ItemKind::Struct),
                TypeUseSourceSlot::WherePredicateSubject(0),
                root()
            ),
            ordinary_type_param(item(&["crate"], "WhereMultiBound", ItemKind::Struct), "T")
        ),
        trait_relation(
            trait_source(
                item(&["crate"], "WhereMultiBound", ItemKind::Struct),
                TypeUseSourceSlot::WherePredicateBound {
                    predicate_index: 0,
                    bound_index: 0
                },
                root()
            ),
            trait_item(item(&["crate"], "LocalTrait", ItemKind::Trait))
        ),
        trait_relation(
            trait_source(
                item(&["crate"], "WhereMultiBound", ItemKind::Struct),
                TypeUseSourceSlot::WherePredicateBound {
                    predicate_index: 0,
                    bound_index: 1
                },
                root()
            ),
            trait_item(item(&["crate"], "ExtraTrait", ItemKind::Trait))
        ),
        ordinary_relation(
            ordinary_source(
                item(&["crate"], "WhereMultiPredicate", ItemKind::Struct),
                TypeUseSourceSlot::WherePredicateSubject(0),
                root()
            ),
            ordinary_type_param(item(&["crate"], "WhereMultiPredicate", ItemKind::Struct), "T")
        ),
        ordinary_relation(
            ordinary_source(
                item(&["crate"], "WhereMultiPredicate", ItemKind::Struct),
                TypeUseSourceSlot::WherePredicateSubject(1),
                root()
            ),
            ordinary_type_param(item(&["crate"], "WhereMultiPredicate", ItemKind::Struct), "U")
        ),
        trait_relation(
            trait_source(
                item(&["crate"], "WhereMultiPredicate", ItemKind::Struct),
                TypeUseSourceSlot::WherePredicateBound {
                    predicate_index: 0,
                    bound_index: 0
                },
                root()
            ),
            trait_item(item(&["crate"], "LocalTrait", ItemKind::Trait))
        ),
        trait_relation(
            trait_source(
                item(&["crate"], "WhereMultiPredicate", ItemKind::Struct),
                TypeUseSourceSlot::WherePredicateBound {
                    predicate_index: 1,
                    bound_index: 0
                },
                root()
            ),
            trait_item(item(&["crate"], "ExtraTrait", ItemKind::Trait))
        ),
        ordinary_relation(
            ordinary_source(
                item(&["crate"], "WhereRepeatedSubject", ItemKind::Struct),
                TypeUseSourceSlot::WherePredicateSubject(0),
                root()
            ),
            ordinary_type_param(item(&["crate"], "WhereRepeatedSubject", ItemKind::Struct), "T")
        ),
        ordinary_relation(
            ordinary_source(
                item(&["crate"], "WhereRepeatedSubject", ItemKind::Struct),
                TypeUseSourceSlot::WherePredicateSubject(1),
                root()
            ),
            ordinary_type_param(item(&["crate"], "WhereRepeatedSubject", ItemKind::Struct), "T")
        ),
        trait_relation(
            trait_source(
                item(&["crate"], "WhereRepeatedSubject", ItemKind::Struct),
                TypeUseSourceSlot::WherePredicateBound {
                    predicate_index: 0,
                    bound_index: 0
                },
                root()
            ),
            trait_item(item(&["crate"], "LocalTrait", ItemKind::Trait))
        ),
        trait_relation(
            trait_source(
                item(&["crate"], "WhereRepeatedSubject", ItemKind::Struct),
                TypeUseSourceSlot::WherePredicateBound {
                    predicate_index: 1,
                    bound_index: 0
                },
                root()
            ),
            trait_item(item(&["crate"], "ExtraTrait", ItemKind::Trait))
        ),
        ordinary_relation(
            ordinary_source(
                item(&["crate"], "WhereEnum", ItemKind::Enum),
                TypeUseSourceSlot::WherePredicateSubject(0),
                root()
            ),
            ordinary_type_param(item(&["crate"], "WhereEnum", ItemKind::Enum), "T")
        ),
        trait_relation(
            trait_source(
                item(&["crate"], "WhereEnum", ItemKind::Enum),
                TypeUseSourceSlot::WherePredicateBound {
                    predicate_index: 0,
                    bound_index: 0
                },
                root()
            ),
            trait_item(item(&["crate"], "LocalTrait", ItemKind::Trait))
        ),
        ordinary_relation(
            ordinary_source(
                item(&["crate"], "WhereAliasMulti", ItemKind::TypeAlias),
                TypeUseSourceSlot::WherePredicateSubject(0),
                root()
            ),
            ordinary_type_param(item(&["crate"], "WhereAliasMulti", ItemKind::TypeAlias), "T")
        ),
        trait_relation(
            trait_source(
                item(&["crate"], "WhereAliasMulti", ItemKind::TypeAlias),
                TypeUseSourceSlot::WherePredicateBound {
                    predicate_index: 0,
                    bound_index: 0
                },
                root()
            ),
            trait_item(item(&["crate"], "LocalTrait", ItemKind::Trait))
        ),
        trait_relation(
            trait_source(
                item(&["crate"], "WhereAliasMulti", ItemKind::TypeAlias),
                TypeUseSourceSlot::WherePredicateBound {
                    predicate_index: 0,
                    bound_index: 1
                },
                root()
            ),
            trait_item(item(&["crate"], "ExtraTrait", ItemKind::Trait))
        ),
        ordinary_relation(
            ordinary_source(
                impl_block(&["crate"], &["WhereImpl"], Some(&["LocalTrait"])),
                TypeUseSourceSlot::WherePredicateSubject(0),
                root()
            ),
            ordinary_type_param(
                impl_block(&["crate"], &["WhereImpl"], Some(&["LocalTrait"])),
                "T"
            )
        ),
        trait_relation(
            trait_source(
                impl_block(&["crate"], &["WhereImpl"], Some(&["LocalTrait"])),
                TypeUseSourceSlot::WherePredicateBound {
                    predicate_index: 0,
                    bound_index: 0
                },
                root()
            ),
            trait_item(item(&["crate"], "ExtraTrait", ItemKind::Trait))
        ),
        trait_relation(
            trait_source(
                impl_block(&["crate"], &["WhereImpl"], Some(&["LocalTrait"])),
                TypeUseSourceSlot::WherePredicateBound {
                    predicate_index: 0,
                    bound_index: 1
                },
                root()
            ),
            trait_item(item(&["crate"], "AnotherTrait", ItemKind::Trait))
        ),
        ordinary_relation(
            ordinary_source(
                item(&["crate"], "WhereCompositeMulti", ItemKind::Struct),
                TypeUseSourceSlot::WherePredicateSubject(0),
                named(&["T"])
            ),
            ordinary_type_param(item(&["crate"], "WhereCompositeMulti", ItemKind::Struct), "T")
        ),
        trait_relation(
            trait_source(
                item(&["crate"], "WhereCompositeMulti", ItemKind::Struct),
                TypeUseSourceSlot::WherePredicateBound {
                    predicate_index: 0,
                    bound_index: 0
                },
                root()
            ),
            trait_item(item(&["crate"], "LocalTrait", ItemKind::Trait))
        ),
        trait_relation(
            trait_source(
                item(&["crate"], "WhereCompositeMulti", ItemKind::Struct),
                TypeUseSourceSlot::WherePredicateBound {
                    predicate_index: 0,
                    bound_index: 1
                },
                root()
            ),
            trait_item(item(&["crate"], "ExtraTrait", ItemKind::Trait))
        ),
        trait_relation(
            trait_source(
                item(&["crate"], "WhereProjectionMulti", ItemKind::TypeAlias),
                TypeUseSourceSlot::WherePredicateSubject(0),
                named(&["LocalAssocBound"])
            ),
            trait_item(item(&["crate"], "LocalAssocBound", ItemKind::Trait))
        ),
        trait_relation(
            trait_source(
                item(&["crate"], "WhereProjectionMulti", ItemKind::TypeAlias),
                TypeUseSourceSlot::WherePredicateBound {
                    predicate_index: 0,
                    bound_index: 0
                },
                root()
            ),
            trait_item(item(&["crate"], "LocalTrait", ItemKind::Trait))
        ),
        trait_relation(
            trait_source(
                item(&["crate"], "WhereProjectionMulti", ItemKind::TypeAlias),
                TypeUseSourceSlot::WherePredicateBound {
                    predicate_index: 0,
                    bound_index: 1
                },
                root()
            ),
            trait_item(item(&["crate"], "ExtraTrait", ItemKind::Trait))
        ),
    ]
);

type_relations_exact_sources_case!(
    fixture_type_resolution_v2_where_owner_family_sources_are_exact,
    graph: &FIXTURE_TYPE_RESOLUTION_V2.0,
    report: &FIXTURE_TYPE_RESOLUTION_V2_REPORT,
    expected: [
        ordinary_relation(
            ordinary_source(
                item(&["crate"], "where_fn", ItemKind::Function),
                TypeUseSourceSlot::WherePredicateSubject(0),
                root()
            ),
            ordinary_type_param(item(&["crate"], "where_fn", ItemKind::Function), "T")
        ),
        trait_relation(
            trait_source(
                item(&["crate"], "where_fn", ItemKind::Function),
                TypeUseSourceSlot::WherePredicateBound {
                    predicate_index: 0,
                    bound_index: 0
                },
                root()
            ),
            trait_item(item(&["crate"], "LocalTrait", ItemKind::Trait))
        ),
        ordinary_relation(
            ordinary_source(
                method(
                    impl_selector(&["crate"], &["WhereMethod"], None),
                    "where_method"
                ),
                TypeUseSourceSlot::WherePredicateSubject(0),
                root()
            ),
            ordinary_type_param(
                method(
                    impl_selector(&["crate"], &["WhereMethod"], None),
                    "where_method"
                ),
                "T"
            )
        ),
        trait_relation(
            trait_source(
                method(
                    impl_selector(&["crate"], &["WhereMethod"], None),
                    "where_method"
                ),
                TypeUseSourceSlot::WherePredicateBound {
                    predicate_index: 0,
                    bound_index: 0
                },
                root()
            ),
            trait_item(item(&["crate"], "LocalTrait", ItemKind::Trait))
        ),
        ordinary_relation(
            ordinary_source(
                item(&["crate"], "WhereTrait", ItemKind::Trait),
                TypeUseSourceSlot::WherePredicateSubject(0),
                root()
            ),
            ordinary_type_param(item(&["crate"], "WhereTrait", ItemKind::Trait), "T")
        ),
        trait_relation(
            trait_source(
                item(&["crate"], "WhereTrait", ItemKind::Trait),
                TypeUseSourceSlot::WherePredicateBound {
                    predicate_index: 0,
                    bound_index: 0
                },
                root()
            ),
            trait_item(item(&["crate"], "LocalTrait", ItemKind::Trait))
        ),
        ordinary_relation(
            ordinary_source(
                item(&["crate"], "WhereUnion", ItemKind::Union),
                TypeUseSourceSlot::WherePredicateSubject(0),
                root()
            ),
            ordinary_type_param(item(&["crate"], "WhereUnion", ItemKind::Union), "T")
        ),
        trait_relation(
            trait_source(
                item(&["crate"], "WhereUnion", ItemKind::Union),
                TypeUseSourceSlot::WherePredicateBound {
                    predicate_index: 0,
                    bound_index: 0
                },
                root()
            ),
            trait_item(item(&["crate"], "LocalTrait", ItemKind::Trait))
        ),
    ]
);

type_relations_exact_sources_case!(
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

type_relations_exact_sources_case!(
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
