//! Long-horizon type graph contracts.
//!
//! These tests make desired graphRAG capabilities concrete. Each test starts
//! from a search a Rust developer would currently approximate with `rg` and
//! states the structural DB traversal we want instead.

use ploke_db::{Database, DbError, TypeRelationKind};
use ploke_test_utils::{
    CORPUS_CHRONO_TYPE_GRAPH, CORPUS_GENERIC_ARRAY_TYPE_GRAPH, CORPUS_MEMCHR_TYPE_GRAPH,
    CORPUS_SEMVER_TYPE_GRAPH,
};
use uuid::Uuid;

use super::common::{
    assert_owner_reaches_target, enum_id_by_name, exactly_one_uuid, field_id_by_owner_index,
    function_id_by_name, function_id_by_name_in_module, setup_typed_backup_db,
    setup_typed_fixture_db, struct_id_by_name, struct_id_by_name_in_module,
    trait_id_by_name_in_module, type_alias_row_by_name,
};

#[test]
fn exact_root_match_ranks_before_nested_terminal_match() -> Result<(), DbError> {
    let db = setup_typed_fixture_db("fixture_type_resolution_v2")?;
    let exact_owner = function_id_by_name(&db, "concrete")?;
    let nested_owner = function_id_by_name(&db, "takes_vec_of_t")?;
    let target_id = struct_id_by_name(&db, "T")?;

    let exact_paths = db.type_targets_reachable_from_owner(exact_owner)?;
    let nested_paths = db.type_targets_reachable_from_owner(nested_owner)?;

    let exact_depth = min_depth_to_target(&exact_paths, target_id, TypeRelationKind::Ordinary)
        .expect("concrete(T) should reach T directly");
    let nested_depth = min_depth_to_target(&nested_paths, target_id, TypeRelationKind::Ordinary)
        .expect("takes_vec_of_t(Vec<T>) should reach T through containment");

    // Current search habit:
    //   rg -n "T|Vec<T>|takes_vec_of_t|concrete" fixture_type_resolution_v2
    //
    // Desired graphRAG behavior:
    //   A query centered on `T` should find both owners, but `concrete(T)`
    //   should rank closer than `takes_vec_of_t(Vec<T>)` because the former
    //   uses `T` as the root type and the latter reaches it only as a child.
    assert!(
        exact_depth < nested_depth,
        "exact root use should be closer than nested terminal use"
    );
    Ok(())
}

#[test]
fn generic_param_shadow_does_not_collapse_to_same_named_struct() -> Result<(), DbError> {
    let db = setup_typed_fixture_db("fixture_type_resolution_v2")?;
    let owner_id = function_id_by_name(&db, "generic_shadow")?;
    let struct_t_id = struct_id_by_name(&db, "T")?;
    let reachable = db.type_targets_reachable_from_owner(owner_id)?;

    // Current search habit:
    //   rg -n "fn generic_shadow<T>|struct T" fixture_type_resolution_v2
    //
    // Desired graphRAG behavior:
    //   `fn generic_shadow<T>(value: T)` should connect to the function's
    //   generic parameter declaration, not to the crate-level `struct T`.
    //   A retrieval query for concrete `struct T` should not treat the generic
    //   parameter as an equivalent concrete type just because the spelling is
    //   the same.
    assert!(
        !reachable.iter().any(|path| path.target_id == struct_t_id
            && path.relation_kind == TypeRelationKind::Ordinary),
        "generic_shadow<T> should not resolve its parameter to concrete struct T; reachable: {reachable:#?}"
    );
    Ok(())
}

#[test]
fn child_trait_reaches_supertrait_definition() -> Result<(), DbError> {
    let db = setup_typed_fixture_db("fixture_type_resolution_v2")?;
    let child_trait_id = trait_id_by_name_in_module(&db, &["crate"], "ChildTrait")?;
    let local_trait_id = trait_id_by_name_in_module(&db, &["crate"], "LocalTrait")?;

    // Current search habit:
    //   rg -n "ChildTrait: LocalTrait|trait LocalTrait" fixture_type_resolution_v2
    //
    // Desired graphRAG behavior:
    //   From `ChildTrait`, traverse its trait-position supertrait source to the
    //   `LocalTrait` definition, then outward to implementors and methods that
    //   depend on the inherited bound.
    assert_owner_reaches_target(
        &db,
        child_trait_id,
        local_trait_id,
        TypeRelationKind::Trait,
        0,
        "ChildTrait should reach its LocalTrait supertrait",
    )
}

#[test]
fn trait_impl_reaches_both_self_type_and_trait_definition() -> Result<(), DbError> {
    let db = setup_typed_fixture_db("fixture_type_resolution_v2")?;
    let impl_id = impl_id_with_trait_and_self_type_names(&db, "LocalTrait", "UsesTrait")?;
    let uses_trait_id = struct_id_by_name(&db, "UsesTrait")?;
    let local_trait_id = trait_id_by_name_in_module(&db, &["crate"], "LocalTrait")?;

    // Current search habit:
    //   rg -n "impl LocalTrait for UsesTrait|struct UsesTrait|trait LocalTrait"
    //
    // Desired graphRAG behavior:
    //   The impl owner is a junction. It should traverse through `ImplSelf` to
    //   `UsesTrait` and through `ImplTrait` to `LocalTrait`, so a change to
    //   either endpoint can find the connecting impl block.
    assert_owner_reaches_target(
        &db,
        impl_id,
        uses_trait_id,
        TypeRelationKind::Ordinary,
        0,
        "impl LocalTrait for UsesTrait should reach UsesTrait through ImplSelf",
    )?;
    assert_owner_reaches_target(
        &db,
        impl_id,
        local_trait_id,
        TypeRelationKind::Trait,
        0,
        "impl LocalTrait for UsesTrait should reach LocalTrait through ImplTrait",
    )
}

#[test]
fn alias_dependency_can_distinguish_alias_root_from_expanded_target() -> Result<(), DbError> {
    let db = setup_typed_backup_db(&CORPUS_CHRONO_TYPE_GRAPH)?;
    let (alias_owner_id, alias_root_type_id) = type_alias_row_by_name(&db, "MappedLocalTime")?;
    let local_result_id = enum_id_by_name(&db, "LocalResult")?;

    let roots = db.type_uses_for_owner(alias_owner_id)?;
    let reachable = db.type_targets_reachable_from_owner(alias_owner_id)?;

    // Current search habit:
    //   rg -n "MappedLocalTime|LocalResult|TimeZone" chrono/src
    //
    // Desired graphRAG behavior:
    //   `MappedLocalTime<T> = LocalResult<T>` should support two different
    //   questions:
    //
    //   1. "Who mentions the alias by name?" uses the root type identity.
    //   2. "Who is semantically tied to LocalResult?" follows expansion and
    //      terminal resolution to the enum definition.
    assert!(
        roots
            .iter()
            .any(|root| root.root_type_id == alias_root_type_id),
        "MappedLocalTime should expose its alias root type use; roots: {roots:#?}"
    );
    assert!(
        reachable
            .iter()
            .any(|path| path.target_id == local_result_id
                && path.relation_kind == TypeRelationKind::Ordinary),
        "MappedLocalTime should reach LocalResult through alias target resolution; reachable: {reachable:#?}"
    );
    Ok(())
}

#[test]
fn semver_vec_field_clusters_with_nested_comparator_users() -> Result<(), DbError> {
    let db = setup_typed_backup_db(&CORPUS_SEMVER_TYPE_GRAPH)?;
    let version_req_id = struct_id_by_name_in_module(&db, &["crate"], "VersionReq")?;
    let comparators_field_id = field_id_by_owner_index(&db, version_req_id, 0)?;
    let comparator_id = struct_id_by_name(&db, "Comparator")?;

    // Current search habit:
    //   rg -n "comparators|Vec<Comparator>|Comparator|Op" semver/src
    //
    // Desired graphRAG behavior:
    //   `VersionReq.comparators: Vec<Comparator>` should reach `Comparator`
    //   through type containment and then cluster with comparator parsing,
    //   display, serde, and matching code even when those users do not mention
    //   the `comparators` field.
    assert_owner_reaches_target(
        &db,
        comparators_field_id,
        comparator_id,
        TypeRelationKind::Ordinary,
        1,
        "VersionReq.comparators should reach Comparator through Vec's argument",
    )
}

#[test]
fn memchr_iterator_constructor_clusters_with_iterator_impls() -> Result<(), DbError> {
    let db = setup_typed_backup_db(&CORPUS_MEMCHR_TYPE_GRAPH)?;
    let owner_id = function_id_by_name_in_module(&db, &["crate", "memchr"], "memchr_iter")?;
    let memchr_id = struct_id_by_name(&db, "Memchr")?;

    // Current search habit:
    //   rg -n "memchr_iter|struct Memchr|impl Iterator for Memchr|DoubleEndedIterator"
    //
    // Desired graphRAG behavior:
    //   `memchr_iter(...) -> Memchr<'h>` should reach the iterator struct and
    //   then continue through self-type matching to inherent and trait impls on
    //   `Memchr`, so examples of consuming the iterator are close to the
    //   constructor.
    assert_owner_reaches_target(
        &db,
        owner_id,
        memchr_id,
        TypeRelationKind::Ordinary,
        0,
        "memchr_iter should reach the Memchr iterator return type",
    )
}

#[test]
fn generic_array_alias_clusters_with_const_generic_storage_type() -> Result<(), DbError> {
    let db = setup_typed_backup_db(&CORPUS_GENERIC_ARRAY_TYPE_GRAPH)?;
    let (alias_owner_id, _root_type_id) = type_alias_row_by_name(&db, "ConstGenericArray")?;
    let generic_array_id = struct_id_by_name(&db, "GenericArray")?;

    // Current search habit:
    //   rg -n "ConstGenericArray|GenericArray|ConstArrayLength|ArrayLength"
    //
    // Desired graphRAG behavior:
    //   The DB should connect the const-generic compatibility alias to the
    //   concrete storage type even before it can prove all valid `N` choices or
    //   solve every bound. Later generic-constraint solving can refine this
    //   cluster, but the basic alias -> storage-type edge should already exist.
    assert_owner_reaches_target(
        &db,
        alias_owner_id,
        generic_array_id,
        TypeRelationKind::Ordinary,
        0,
        "ConstGenericArray should reach GenericArray",
    )
}

fn min_depth_to_target(
    paths: &[ploke_db::TypeTargetPath],
    target_id: Uuid,
    relation_kind: TypeRelationKind,
) -> Option<u32> {
    paths
        .iter()
        .filter(|path| path.target_id == target_id && path.relation_kind == relation_kind)
        .map(|path| path.depth)
        .min()
}

fn impl_id_with_trait_and_self_type_names(
    db: &Database,
    trait_name: &str,
    self_type_name: &str,
) -> Result<Uuid, DbError> {
    exactly_one_uuid(
        db,
        &format!(
            r#"?[impl_id] :=
                *impl {{ id: impl_id @ 'NOW' }},
                *type_use {{
                    owner_id: impl_id,
                    root_type_id: self_type_id,
                    role: "ImplSelf" @ 'NOW'
                }},
                *type_relation {{
                    source_id: self_type_id,
                    target_id: self_target_id,
                    relation_kind: "Ordinary" @ 'NOW'
                }},
                *struct {{ id: self_target_id, name: "{self_type_name}" @ 'NOW' }},
                *type_use {{
                    owner_id: impl_id,
                    root_type_id: trait_type_id,
                    role: "ImplTrait" @ 'NOW'
                }},
                *type_relation {{
                    source_id: trait_type_id,
                    target_id: trait_target_id,
                    relation_kind: "Trait" @ 'NOW'
                }},
                *trait {{ id: trait_target_id, name: "{trait_name}" @ 'NOW' }}"#
        ),
        0,
    )
}
