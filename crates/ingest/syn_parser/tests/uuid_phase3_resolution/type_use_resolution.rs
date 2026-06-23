#![cfg(test)]

//! Regression tests for late type-use resolution over the post-tree import graph.
//!
//! These tests currently cover item-backed and generic-param resolutions that are reachable from
//! already-walked type-bearing slots. The single-edge cases assert that one expected target exists;
//! the `*_complete_slot` cases assert the full multiset of semantic outcomes under one source
//! `TypeNode` tree.
//!
//! Known passing shapes:
//!
//! ```rust,ignore
//! fn takes_alias(value: ImportedAlias) {}
//! fn returns_alias() -> ImportedAlias {}
//! impl ImportedTrait for ImportedStruct {}
//! impl<T> GenericTrait<T> for GenericStruct<T> {}
//! trait Child: Parent {}
//! trait GenericChild<T>: GenericParent<T> {}
//! type Alias = OtherAlias;
//! const VALUE: LocalStruct = /* ... */;
//! struct Nested<T> { field: LocalStruct<T> }
//! ```
//!
//! Known pass-but-partial shapes:
//!
//! ```rust,ignore
//! trait Multi: LocalTrait + InternalTrait + std::fmt::Debug {}
//! ```
//!
//! The local trait targets are asserted; external/builtin outcomes such as `std::fmt::Debug` are
//! not yet pinned by exact negative-state cases.
//!
//! Known not-yet-covered roles:
//!
//! ```rust,ignore
//! static VALUE: LocalType = /* ... */;
//! fn method_param(&self, value: LocalType) {}
//! enum E { Variant(LocalType), Struct { field: LocalType } }
//! union U { field: LocalType }
//! ```
//!
//! The walker has branches for these roles, but this file does not yet assert them.
//! Add tests here before changing traversal if these regress: `TypeUseRole` already includes these
//! roles in `crates/ingest/syn_parser/src/resolve/type_resolution.rs:245`, and
//! `TypeUseWalker::collect` already walks enum fields, union fields, consts, and statics at
//! `crates/ingest/syn_parser/src/resolve/type_resolution.rs:1118` and `:1204`.
//!
//! Known not-yet-walked declaration-bound shapes:
//!
//! ```rust,ignore
//! fn f<T: Clone + Send>(value: T) {}
//! struct S<T: std::fmt::Debug> { value: T }
//! enum E<T: Default + Clone> where T: Send { Variant(T) }
//! type Alias<T: std::fmt::Display> = Vec<T>;
//! impl<T: Debug> S<T> {}
//! trait Assoc { type Output: Clone; }
//! ```
//!
//! These bounds are parsed into `GenericParamKind::Type { bounds, .. }`, but the late type-use
//! walker does not yet visit generic parameter bounds as semantic type-use owners.
//! Implementation touchpoints:
//!
//! - Generic parameter bound storage already exists in
//!   `crates/ingest/syn_parser/src/parser/types.rs:57` and
//!   `crates/ingest/syn_parser/src/parser/visitor/state.rs:194`.
//! - Trait-bound `TypeNode`s are already constructed by
//!   `crates/ingest/syn_parser/src/parser/visitor/type_processing.rs:99`.
//! - The missing piece is late traversal: add a type-use role for generic declaration bounds near
//!   `crates/ingest/syn_parser/src/resolve/type_resolution.rs:245`, then have
//!   `TypeUseWalker::collect` walk each owner's `generic_params` around
//!   `crates/ingest/syn_parser/src/resolve/type_resolution.rs:1100`.
//! - Where-clause predicates are not modeled as semantic bound nodes yet; they are only formatted
//!   for names in `crates/ingest/syn_parser/src/parser/visitor/code_visitor.rs:2744`. Supporting
//!   `where T: Bound` needs parser storage before the resolver can walk it.
//! - Associated type bounds such as `trait Assoc { type Output: Clone; }` are blocked earlier:
//!   trait associated const/type parsing is still TODO-only at
//!   `crates/ingest/syn_parser/src/parser/visitor/code_visitor.rs:1926`.
//! - Impl associated types/consts have the same parser-side gap at
//!   `crates/ingest/syn_parser/src/parser/visitor/code_visitor.rs:1715`.
//!
//! Known failing/TDD targets:
//!
//! ```rust,ignore
//! struct UsesFileModule<T> { field: other_mod::OtherFileStruct<T> }
//! ```
//!
//! The nested `T` resolves, but the module-qualified item path through a file module currently
//! resolves as `PathNotFound` in the late resolver. Keep this as a future resolver test rather than
//! weakening exact slot assertions.
//! Implementation touchpoints:
//!
//! - The failing lookup goes through `LateResolver::resolve_provenance` at
//!   `crates/ingest/syn_parser/src/resolve/type_resolution.rs:476`.
//! - Segment lookup is limited to visible `Contains` candidates from the active module in
//!   `LateResolver::scope_candidates` at
//!   `crates/ingest/syn_parser/src/resolve/type_resolution.rs:754`.
//! - Import backlink traversal happens in `LateResolver::resolve_binding_terminals` at
//!   `crates/ingest/syn_parser/src/resolve/type_resolution.rs:809`; file-module declaration to
//!   definition traversal likely needs equivalent treatment for module segments.
//! - Module declaration/definition relations are created during module-tree construction in
//!   `crates/ingest/syn_parser/src/parser/graph/parsed_graph.rs:422` and
//!   `crates/ingest/syn_parser/src/resolve/module_tree.rs:737`; use those backlinks rather than
//!   weakening `PathNotFound` or accepting unresolved results.

use lazy_static::lazy_static;
use ploke_core::ItemKind;
use syn_parser::parser::ParsedCodeGraph;
use syn_parser::resolve::module_tree::ModuleTree;
use syn_parser::resolve::type_resolution::{
    resolve_type_uses_after_tree, TypeResolutionReport, TypeUseRole,
};

use crate::common::build_tree_for_tests;
use crate::common::type_use_resolution::{
    impl_block, impl_selector, item, method, slot_generic_param, slot_item, struct_field,
    ExpectedTypeUseResolution, ExpectedTypeUseSlot, FieldSelector, TypeUseSourceSlot,
};
use crate::{type_use_resolution_case, type_use_slot_resolution_case};

lazy_static! {
    static ref FIXTURE_TYPES: (ParsedCodeGraph, ModuleTree) = build_tree_for_tests("fixture_types");
    static ref FIXTURE_TYPES_REPORT: TypeResolutionReport =
        resolve_type_uses_after_tree(&FIXTURE_TYPES.0, &FIXTURE_TYPES.1)
            .expect("fixture_types type-use resolution report");
    static ref FIXTURE_NODES: (ParsedCodeGraph, ModuleTree) = build_tree_for_tests("fixture_nodes");
    static ref FIXTURE_NODES_REPORT: TypeResolutionReport =
        resolve_type_uses_after_tree(&FIXTURE_NODES.0, &FIXTURE_NODES.1)
            .expect("fixture_nodes type-use resolution report");
    static ref FIXTURE_CONFLATION: (ParsedCodeGraph, ModuleTree) =
        build_tree_for_tests("fixture_conflation");
    static ref FIXTURE_CONFLATION_REPORT: TypeResolutionReport =
        resolve_type_uses_after_tree(&FIXTURE_CONFLATION.0, &FIXTURE_CONFLATION.1)
            .expect("fixture_conflation type-use resolution report");
}

type_use_resolution_case!(
    fixture_types_imported_point_function_param,
    graph: &FIXTURE_TYPES.0,
    report: &FIXTURE_TYPES_REPORT,
    expected: ExpectedTypeUseResolution {
        owner: item(&["crate", "func", "return_types"], "consumes_point", ItemKind::Function),
        role: TypeUseRole::FunctionParam,
        source: TypeUseSourceSlot::FunctionParam(0),
        target: item(&["crate"], "Point", ItemKind::TypeAlias),
        expect_resolved_type_id: true,
    }
);

type_use_resolution_case!(
    fixture_types_imported_math_operation_function_param,
    graph: &FIXTURE_TYPES.0,
    report: &FIXTURE_TYPES_REPORT,
    expected: ExpectedTypeUseResolution {
        owner: item(&["crate", "func", "return_types"], "math_operation_consumer", ItemKind::Function),
        role: TypeUseRole::FunctionParam,
        source: TypeUseSourceSlot::FunctionParam(0),
        target: item(&["crate"], "MathOperation", ItemKind::TypeAlias),
        expect_resolved_type_id: true,
    }
);

type_use_resolution_case!(
    fixture_types_imported_math_operation_function_return,
    graph: &FIXTURE_TYPES.0,
    report: &FIXTURE_TYPES_REPORT,
    expected: ExpectedTypeUseResolution {
        owner: item(&["crate", "func", "return_types"], "math_operation_producer", ItemKind::Function),
        role: TypeUseRole::FunctionReturn,
        source: TypeUseSourceSlot::FunctionReturn,
        target: item(&["crate"], "MathOperation", ItemKind::TypeAlias),
        expect_resolved_type_id: true,
    }
);

type_use_resolution_case!(
    fixture_types_nested_imported_point_function_param,
    graph: &FIXTURE_TYPES.0,
    report: &FIXTURE_TYPES_REPORT,
    expected: ExpectedTypeUseResolution {
        owner: item(
            &["crate", "func", "return_types", "restricted_duplicate"],
            "consumes_point",
            ItemKind::Function
        ),
        role: TypeUseRole::FunctionParam,
        source: TypeUseSourceSlot::FunctionParam(0),
        target: item(&["crate"], "Point", ItemKind::TypeAlias),
        expect_resolved_type_id: true,
    }
);

type_use_resolution_case!(
    fixture_types_nested_imported_math_operation_function_return,
    graph: &FIXTURE_TYPES.0,
    report: &FIXTURE_TYPES_REPORT,
    expected: ExpectedTypeUseResolution {
        owner: item(
            &["crate", "func", "return_types", "restricted_duplicate"],
            "math_operation_producer",
            ItemKind::Function
        ),
        role: TypeUseRole::FunctionReturn,
        source: TypeUseSourceSlot::FunctionReturn,
        target: item(&["crate"], "MathOperation", ItemKind::TypeAlias),
        expect_resolved_type_id: true,
    }
);

type_use_resolution_case!(
    fixture_nodes_inner_imported_simple_struct_impl_self,
    graph: &FIXTURE_NODES.0,
    report: &FIXTURE_NODES_REPORT,
    expected: ExpectedTypeUseResolution {
        owner: impl_block(&["crate", "impls", "inner"], &["SimpleStruct"], None),
        role: TypeUseRole::ImplSelf,
        source: TypeUseSourceSlot::ImplSelf,
        target: item(&["crate", "impls"], "SimpleStruct", ItemKind::Struct),
        expect_resolved_type_id: true,
    }
);

type_use_resolution_case!(
    fixture_nodes_inner_imported_simple_trait_impl_trait,
    graph: &FIXTURE_NODES.0,
    report: &FIXTURE_NODES_REPORT,
    expected: ExpectedTypeUseResolution {
        owner: impl_block(
            &["crate", "impls", "inner"],
            &["InnerStruct"],
            Some(&["SimpleTrait"])
        ),
        role: TypeUseRole::ImplTrait,
        source: TypeUseSourceSlot::ImplTrait,
        target: item(&["crate", "impls"], "SimpleTrait", ItemKind::Trait),
        expect_resolved_type_id: true,
    }
);

type_use_resolution_case!(
    fixture_nodes_simple_struct_new_self_return,
    graph: &FIXTURE_NODES.0,
    report: &FIXTURE_NODES_REPORT,
    expected: ExpectedTypeUseResolution {
        owner: method(
            impl_selector(&["crate", "impls"], &["SimpleStruct"], None),
            "new"
        ),
        role: TypeUseRole::MethodReturn,
        source: TypeUseSourceSlot::MethodReturn,
        target: item(&["crate", "impls"], "SimpleStruct", ItemKind::Struct),
        expect_resolved_type_id: true,
    }
);

type_use_resolution_case!(
    fixture_nodes_local_simple_trait_impl_self,
    graph: &FIXTURE_NODES.0,
    report: &FIXTURE_NODES_REPORT,
    expected: ExpectedTypeUseResolution {
        owner: impl_block(&["crate", "impls"], &["SimpleStruct"], Some(&["SimpleTrait"])),
        role: TypeUseRole::ImplSelf,
        source: TypeUseSourceSlot::ImplSelf,
        target: item(&["crate", "impls"], "SimpleStruct", ItemKind::Struct),
        expect_resolved_type_id: true,
    }
);

type_use_resolution_case!(
    fixture_nodes_local_simple_trait_impl_trait,
    graph: &FIXTURE_NODES.0,
    report: &FIXTURE_NODES_REPORT,
    expected: ExpectedTypeUseResolution {
        owner: impl_block(&["crate", "impls"], &["SimpleStruct"], Some(&["SimpleTrait"])),
        role: TypeUseRole::ImplTrait,
        source: TypeUseSourceSlot::ImplTrait,
        target: item(&["crate", "impls"], "SimpleTrait", ItemKind::Trait),
        expect_resolved_type_id: true,
    }
);

type_use_resolution_case!(
    fixture_nodes_simple_trait_supertrait,
    graph: &FIXTURE_NODES.0,
    report: &FIXTURE_NODES_REPORT,
    expected: ExpectedTypeUseResolution {
        owner: item(&["crate", "traits"], "SuperTrait", ItemKind::Trait),
        role: TypeUseRole::TraitSuper,
        source: TypeUseSourceSlot::TraitSuper(0),
        target: item(&["crate", "traits"], "SimpleTrait", ItemKind::Trait),
        expect_resolved_type_id: true,
    }
);

type_use_resolution_case!(
    fixture_nodes_multi_supertrait_simple_trait,
    graph: &FIXTURE_NODES.0,
    report: &FIXTURE_NODES_REPORT,
    expected: ExpectedTypeUseResolution {
        owner: item(&["crate", "traits"], "MultiSuperTrait", ItemKind::Trait),
        role: TypeUseRole::TraitSuper,
        source: TypeUseSourceSlot::TraitSuper(0),
        target: item(&["crate", "traits"], "SimpleTrait", ItemKind::Trait),
        expect_resolved_type_id: true,
    }
);

type_use_resolution_case!(
    fixture_nodes_multi_supertrait_internal_trait,
    graph: &FIXTURE_NODES.0,
    report: &FIXTURE_NODES_REPORT,
    expected: ExpectedTypeUseResolution {
        owner: item(&["crate", "traits"], "MultiSuperTrait", ItemKind::Trait),
        role: TypeUseRole::TraitSuper,
        source: TypeUseSourceSlot::TraitSuper(1),
        target: item(&["crate", "traits"], "InternalTrait", ItemKind::Trait),
        expect_resolved_type_id: true,
    }
);

type_use_resolution_case!(
    fixture_nodes_generic_trait_supertrait,
    graph: &FIXTURE_NODES.0,
    report: &FIXTURE_NODES_REPORT,
    expected: ExpectedTypeUseResolution {
        owner: item(&["crate", "traits"], "GenericSuperTrait", ItemKind::Trait),
        role: TypeUseRole::TraitSuper,
        source: TypeUseSourceSlot::TraitSuper(0),
        target: item(&["crate", "traits"], "GenericTrait", ItemKind::Trait),
        expect_resolved_type_id: true,
    }
);

type_use_slot_resolution_case!(
    fixture_nodes_generic_trait_supertrait_complete_slot,
    graph: &FIXTURE_NODES.0,
    report: &FIXTURE_NODES_REPORT,
    expected: ExpectedTypeUseSlot {
        owner: item(&["crate", "traits"], "GenericSuperTrait", ItemKind::Trait),
        role: TypeUseRole::TraitSuper,
        source: TypeUseSourceSlot::TraitSuper(0),
        resolutions: &[
            slot_item(item(&["crate", "traits"], "GenericTrait", ItemKind::Trait)),
            slot_generic_param("T"),
        ],
    }
);

type_use_resolution_case!(
    fixture_nodes_type_alias_to_type_alias,
    graph: &FIXTURE_NODES.0,
    report: &FIXTURE_NODES_REPORT,
    expected: ExpectedTypeUseResolution {
        owner: item(&["crate", "type_alias"], "IdAlias", ItemKind::TypeAlias),
        role: TypeUseRole::TypeAliasTarget,
        source: TypeUseSourceSlot::TypeAliasTarget,
        target: item(&["crate", "type_alias"], "SimpleId", ItemKind::TypeAlias),
        expect_resolved_type_id: true,
    }
);

type_use_resolution_case!(
    fixture_nodes_inner_outer_point_type_alias_target,
    graph: &FIXTURE_NODES.0,
    report: &FIXTURE_NODES_REPORT,
    expected: ExpectedTypeUseResolution {
        owner: item(&["crate", "type_alias", "inner"], "OuterPoint", ItemKind::TypeAlias),
        role: TypeUseRole::TypeAliasTarget,
        source: TypeUseSourceSlot::TypeAliasTarget,
        target: item(&["crate", "type_alias"], "Point", ItemKind::TypeAlias),
        expect_resolved_type_id: true,
    }
);

type_use_resolution_case!(
    fixture_nodes_use_inner_type_alias_target,
    graph: &FIXTURE_NODES.0,
    report: &FIXTURE_NODES_REPORT,
    expected: ExpectedTypeUseResolution {
        owner: item(&["crate", "type_alias"], "UseInner", ItemKind::TypeAlias),
        role: TypeUseRole::TypeAliasTarget,
        source: TypeUseSourceSlot::TypeAliasTarget,
        target: item(
            &["crate", "type_alias", "inner"],
            "InnerPublic",
            ItemKind::TypeAlias
        ),
        expect_resolved_type_id: true,
    }
);

type_use_resolution_case!(
    fixture_nodes_const_struct_type,
    graph: &FIXTURE_NODES.0,
    report: &FIXTURE_NODES_REPORT,
    expected: ExpectedTypeUseResolution {
        owner: item(&["crate", "const_static"], "STRUCT_CONST", ItemKind::Const),
        role: TypeUseRole::Const,
        source: TypeUseSourceSlot::ConstType,
        target: item(&["crate", "const_static"], "SimpleStruct", ItemKind::Struct),
        expect_resolved_type_id: true,
    }
);

type_use_resolution_case!(
    fixture_nodes_const_alias_type,
    graph: &FIXTURE_NODES.0,
    report: &FIXTURE_NODES_REPORT,
    expected: ExpectedTypeUseResolution {
        owner: item(&["crate", "const_static"], "ALIASED_CONST", ItemKind::Const),
        role: TypeUseRole::Const,
        source: TypeUseSourceSlot::ConstType,
        target: item(&["crate", "const_static"], "MyInt", ItemKind::TypeAlias),
        expect_resolved_type_id: true,
    }
);

type_use_resolution_case!(
    fixture_conflation_imported_top_level_trait_impl_trait,
    graph: &FIXTURE_CONFLATION.0,
    report: &FIXTURE_CONFLATION_REPORT,
    expected: ExpectedTypeUseResolution {
        owner: impl_block(
            &["crate", "inner_mod"],
            &["InnerStruct"],
            Some(&["TopLevelTrait"])
        ),
        role: TypeUseRole::ImplTrait,
        source: TypeUseSourceSlot::ImplTrait,
        target: item(&["crate"], "TopLevelTrait", ItemKind::Trait),
        expect_resolved_type_id: true,
    }
);

type_use_slot_resolution_case!(
    fixture_conflation_imported_top_level_trait_impl_trait_complete_slot,
    graph: &FIXTURE_CONFLATION.0,
    report: &FIXTURE_CONFLATION_REPORT,
    expected: ExpectedTypeUseSlot {
        owner: impl_block(
            &["crate", "inner_mod"],
            &["InnerStruct"],
            Some(&["TopLevelTrait"])
        ),
        role: TypeUseRole::ImplTrait,
        source: TypeUseSourceSlot::ImplTrait,
        resolutions: &[
            slot_item(item(&["crate"], "TopLevelTrait", ItemKind::Trait)),
            slot_generic_param("T"),
        ],
    }
);

type_use_resolution_case!(
    fixture_conflation_inner_struct_self_return,
    graph: &FIXTURE_CONFLATION.0,
    report: &FIXTURE_CONFLATION_REPORT,
    expected: ExpectedTypeUseResolution {
        owner: method(
            impl_selector(&["crate", "inner_mod"], &["InnerStruct"], None),
            "inner_method"
        ),
        role: TypeUseRole::MethodReturn,
        source: TypeUseSourceSlot::MethodReturn,
        target: item(&["crate", "inner_mod"], "InnerStruct", ItemKind::Struct),
        expect_resolved_type_id: true,
    }
);

type_use_slot_resolution_case!(
    fixture_conflation_nested_generic_field_complete_slot,
    graph: &FIXTURE_CONFLATION.0,
    report: &FIXTURE_CONFLATION_REPORT,
    expected: ExpectedTypeUseSlot {
        owner: struct_field(&["crate"], "NestedGeneric", FieldSelector::Index(0)),
        role: TypeUseRole::Field,
        source: TypeUseSourceSlot::FieldType,
        resolutions: &[
            slot_item(item(&["crate"], "TopLevelStruct", ItemKind::Struct)),
            slot_generic_param("T"),
        ],
    }
);
