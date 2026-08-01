//! Consumer-facing type context expansion wishlist.
//!
//! The tests in this module are not about raw Cozo edge rows. They describe
//! the API shape Ploke's search, context planning, and LLM-facing tools should
//! eventually consume: given a seed code item or type definition, return ranked
//! code-context candidates with a reason that is meaningful to a Rust coding
//! assistant.
//!
//! These tests pin `Database::expand_type_context(...)` as the application-
//! facing retrieval API layered over the lower-level type graph relations.
//!
//! Coverage boundary:
//!
//! - Covered: owner seeds, target seeds, direct and nested users, trait impl
//!   junctions, trait-object bound users, alias expansion, iterator surfaces,
//!   and selected corpus-backed type-context candidates.
//! - Not yet covered: a seed whose relevant relationship is specifically a
//!   where-clause coordinate such as `WherePredicateBound` or
//!   `WherePredicateSubject`.
//! - Consequence: lower-level tests prove where-clause roots and reachability;
//!   this file does not yet prove the application-facing relation label,
//!   ranking, or distance chosen for where-derived candidates.
//! - Recursive/nested type behavior is tested only to the depth available in
//!   the fixture/corpus examples used by the lower-level tests.

use cozo::DataValue;
use ploke_db::{
    Database, DbError, TypeContextCandidate, TypeContextOptions, TypeContextRelation,
    TypeContextSeed,
};
use ploke_test_utils::{
    CORPUS_AXUM_TYPE_GRAPH, CORPUS_CHRONO_TYPE_GRAPH, CORPUS_GENERIC_ARRAY_TYPE_GRAPH,
    CORPUS_MEMCHR_TYPE_GRAPH, CORPUS_SEMVER_TYPE_GRAPH,
};
use uuid::Uuid;

use super::common::{
    enum_id_by_name, field_id_by_owner_index, function_id_by_name, function_id_by_name_in_module,
    setup_typed_backup_db, setup_typed_fixture_db, struct_id_by_name,
    struct_id_by_name_in_file_suffix, struct_id_by_name_in_module, trait_id_by_name_in_module,
    type_alias_row_by_name,
};

#[test]
fn owner_seed_returns_ranked_related_code_with_explanatory_reasons() -> Result<(), DbError> {
    let db = setup_typed_fixture_db("fixture_type_resolution_v2")?;
    let seed_owner = function_id_by_name(&db, "concrete")?;
    let exact_peer = function_id_by_name(&db, "also_concrete")?;
    let nested_peer = function_id_by_name(&db, "takes_vec_of_t")?;
    let direct_target = struct_id_by_name(&db, "T")?;

    let candidates = db.expand_type_context(
        TypeContextSeed::Owner(seed_owner),
        TypeContextOptions::default(),
    )?;

    assert_candidate(
        &candidates,
        exact_peer,
        TypeContextRelation::SameResolvedType,
        "concrete(T) should expand to also_concrete(T) as an exact same-type peer",
    );
    assert_candidate(
        &candidates,
        nested_peer,
        TypeContextRelation::UsesTypeNested,
        "concrete(T) should expand to takes_vec_of_t(Vec<T>) as a nested type peer",
    );
    assert_candidate(
        &candidates,
        direct_target,
        TypeContextRelation::TypeDefinitionImpact,
        "concrete(T) should expand to the resolved T definition as direct type context",
    );
    assert_ranked_before(
        &candidates,
        exact_peer,
        nested_peer,
        "exact root type peers should rank before nested terminal peers",
    );
    Ok(())
}

#[test]
fn type_definition_seed_returns_direct_and_nested_users_ranked_by_type_distance()
-> Result<(), DbError> {
    let db = setup_typed_fixture_db("fixture_type_resolution_v2")?;
    let seed_target = struct_id_by_name(&db, "T")?;
    let direct_user = function_id_by_name(&db, "concrete")?;
    let second_direct_user = function_id_by_name(&db, "also_concrete")?;
    let nested_user = function_id_by_name(&db, "takes_vec_of_t")?;

    let candidates = db.expand_type_context(
        TypeContextSeed::Target(seed_target),
        TypeContextOptions::default(),
    )?;

    assert_candidate(
        &candidates,
        direct_user,
        TypeContextRelation::TypeDefinitionImpact,
        "editing struct T should return concrete(T) as direct impacted context",
    );
    assert_candidate(
        &candidates,
        second_direct_user,
        TypeContextRelation::TypeDefinitionImpact,
        "editing struct T should return also_concrete(T) as direct impacted context",
    );
    assert_candidate(
        &candidates,
        nested_user,
        TypeContextRelation::UsesTypeNested,
        "editing struct T should return takes_vec_of_t(Vec<T>) as nested impacted context",
    );
    assert_ranked_before(
        &candidates,
        direct_user,
        nested_user,
        "direct users of a changed type should rank before nested users",
    );
    Ok(())
}

#[test]
fn trait_seed_returns_impl_junctions_and_self_types() -> Result<(), DbError> {
    let db = setup_typed_fixture_db("fixture_type_resolution_v2")?;
    let trait_id = trait_id_by_name_in_module(&db, &["crate"], "LocalTrait")?;
    let impl_id = impl_id_with_trait_and_self_type_names(&db, "LocalTrait", "UsesTrait")?;
    let self_type_id = struct_id_by_name(&db, "UsesTrait")?;

    let candidates = db.expand_type_context(
        TypeContextSeed::Target(trait_id),
        TypeContextOptions::default(),
    )?;

    assert_candidate(
        &candidates,
        impl_id,
        TypeContextRelation::ImplOfTrait,
        "changing LocalTrait should surface impl LocalTrait for UsesTrait",
    );
    assert_candidate(
        &candidates,
        self_type_id,
        TypeContextRelation::ImplSelfType,
        "changing LocalTrait should surface the implementing UsesTrait type",
    );
    assert_ranked_before(
        &candidates,
        impl_id,
        self_type_id,
        "the impl junction should rank before the self type reached through that impl",
    );
    Ok(())
}

#[test]
fn trait_bound_seed_returns_bound_owners_and_trait_definition_context() -> Result<(), DbError> {
    let db = setup_typed_fixture_db("fixture_types")?;
    let trait_id = trait_id_by_name_in_module(&db, &["crate"], "Drawable")?;
    let draw_object_id = function_id_by_name_in_module(&db, &["crate"], "draw_object")?;

    let candidates = db.expand_type_context(
        TypeContextSeed::Target(trait_id),
        TypeContextOptions::default(),
    )?;

    assert_candidate(
        &candidates,
        draw_object_id,
        TypeContextRelation::TraitBound,
        "Drawable should pull in &dyn Drawable users as trait-bound context",
    );
    Ok(())
}

#[test]
fn alias_seed_preserves_alias_identity_and_expands_to_semantic_target() -> Result<(), DbError> {
    let db = setup_typed_backup_db(&CORPUS_CHRONO_TYPE_GRAPH)?;
    let (alias_id, _root_type_id) = type_alias_row_by_name(&db, "MappedLocalTime")?;
    let local_result_id = enum_id_by_name(&db, "LocalResult")?;

    let candidates = db.expand_type_context(
        TypeContextSeed::Owner(alias_id),
        TypeContextOptions::default(),
    )?;

    assert_candidate(
        &candidates,
        local_result_id,
        TypeContextRelation::AliasExpansion,
        "MappedLocalTime should expand to its LocalResult semantic target",
    );
    Ok(())
}

#[test]
fn semver_version_req_seed_returns_matching_and_comparator_context() -> Result<(), DbError> {
    let db = setup_typed_backup_db(&CORPUS_SEMVER_TYPE_GRAPH)?;
    let version_req_id = struct_id_by_name_in_module(&db, &["crate"], "VersionReq")?;
    let matches_req_id = function_id_by_name_in_module(&db, &["crate", "eval"], "matches_req")?;
    let comparators_field_id = field_id_by_owner_index(&db, version_req_id, 0)?;
    let comparator_id = struct_id_by_name(&db, "Comparator")?;

    let candidates = db.expand_type_context(
        TypeContextSeed::Target(version_req_id),
        TypeContextOptions::default(),
    )?;

    assert_candidate(
        &candidates,
        matches_req_id,
        TypeContextRelation::TypeDefinitionImpact,
        "VersionReq should pull in matching/evaluation code that consumes it",
    );
    assert_candidate(
        &candidates,
        comparators_field_id,
        TypeContextRelation::UsesTypeNested,
        "VersionReq should pull in its comparators field as a structural type neighbor",
    );
    assert_candidate(
        &candidates,
        comparator_id,
        TypeContextRelation::UsesTypeNested,
        "VersionReq should pull in Comparator through Vec<Comparator>",
    );
    Ok(())
}

#[test]
fn axum_struct_seed_includes_nested_trait_object_field_targets_with_rag_distance()
-> Result<(), DbError> {
    let db = setup_typed_backup_db(&CORPUS_AXUM_TYPE_GRAPH)?;
    let boxed_into_route_id =
        struct_id_by_name_in_file_suffix(&db, "axum/src/boxed.rs", "BoxedIntoRoute")?;
    let erased_into_route_id =
        trait_id_by_name_in_file_suffix(&db, "axum/src/boxed.rs", "ErasedIntoRoute")?;

    let candidates = db.expand_type_context(
        TypeContextSeed::Target(boxed_into_route_id),
        TypeContextOptions {
            max_distance: 4,
            ..TypeContextOptions::default()
        },
    )?;

    assert_candidate(
        &candidates,
        erased_into_route_id,
        TypeContextRelation::UsesTypeNested,
        "BoxedIntoRoute should expose its nested Box<dyn ErasedIntoRoute<...>> field target within the RAG distance budget",
    );
    Ok(())
}

#[test]
fn memchr_iterator_seed_returns_constructor_and_iterator_surface() -> Result<(), DbError> {
    let db = setup_typed_backup_db(&CORPUS_MEMCHR_TYPE_GRAPH)?;
    let memchr_id = struct_id_by_name(&db, "Memchr")?;
    let constructor_id = function_id_by_name_in_module(&db, &["crate", "memchr"], "memchr_iter")?;

    let candidates = db.expand_type_context(
        TypeContextSeed::Target(memchr_id),
        TypeContextOptions::default(),
    )?;

    assert_candidate(
        &candidates,
        constructor_id,
        TypeContextRelation::TypeDefinitionImpact,
        "Memchr should pull in the memchr_iter constructor returning it",
    );
    assert_has_relation(
        &candidates,
        TypeContextRelation::IteratorSurface,
        "Memchr should eventually expand to Iterator/DoubleEndedIterator impl surface",
    );
    Ok(())
}

#[test]
fn generic_array_alias_seed_returns_storage_type_and_const_generic_context() -> Result<(), DbError>
{
    let db = setup_typed_backup_db(&CORPUS_GENERIC_ARRAY_TYPE_GRAPH)?;
    let (alias_id, _root_type_id) = type_alias_row_by_name(&db, "ConstGenericArray")?;
    let generic_array_id = struct_id_by_name(&db, "GenericArray")?;

    let candidates = db.expand_type_context(
        TypeContextSeed::Owner(alias_id),
        TypeContextOptions::default(),
    )?;

    assert_candidate(
        &candidates,
        generic_array_id,
        TypeContextRelation::ConstGenericAlias,
        "ConstGenericArray should pull in GenericArray as its storage type",
    );
    Ok(())
}

fn assert_candidate(
    candidates: &[TypeContextCandidate],
    node_id: Uuid,
    relation: TypeContextRelation,
    message: &str,
) {
    assert!(
        candidates
            .iter()
            .any(|candidate| candidate.node_id == node_id && candidate.relation == relation),
        "{message}; candidates: {candidates:#?}"
    );
}

fn assert_ranked_before(
    candidates: &[TypeContextCandidate],
    earlier_node_id: Uuid,
    later_node_id: Uuid,
    message: &str,
) {
    let earlier = candidate_distance(candidates, earlier_node_id)
        .unwrap_or_else(|| panic!("missing earlier candidate {earlier_node_id}; {message}"));
    let later = candidate_distance(candidates, later_node_id)
        .unwrap_or_else(|| panic!("missing later candidate {later_node_id}; {message}"));
    assert!(
        earlier < later,
        "{message}; earlier distance: {earlier}; later distance: {later}; candidates: {candidates:#?}"
    );
}

fn assert_has_relation(
    candidates: &[TypeContextCandidate],
    relation: TypeContextRelation,
    message: &str,
) {
    assert!(
        candidates
            .iter()
            .any(|candidate| candidate.relation == relation),
        "{message}; candidates: {candidates:#?}"
    );
}

fn candidate_distance(candidates: &[TypeContextCandidate], node_id: Uuid) -> Option<u32> {
    candidates
        .iter()
        .filter(|candidate| candidate.node_id == node_id)
        .map(|candidate| candidate.distance)
        .min()
}

fn trait_id_by_name_in_file_suffix(
    db: &Database,
    file_suffix: &str,
    trait_name: &str,
) -> Result<Uuid, DbError> {
    let rows = db.raw_query(&format!(
        r#"?[trait_id, file_path] :=
            *trait {{ id: trait_id, name: "{trait_name}" @ 'NOW' }},
            *syntax_edge {{
                source_id: module_id,
                target_id: trait_id,
                relation_kind: "Contains" @ 'NOW'
            }},
            *file_mod {{ owner_id: module_id, file_path @ 'NOW' }}"#
    ))?;
    let matching: Vec<_> = rows
        .rows
        .iter()
        .filter_map(|row| {
            let file_path = match &row[1] {
                DataValue::Str(path) => path.as_str(),
                _ => return None,
            };
            file_path.ends_with(file_suffix).then(|| row[0].clone())
        })
        .collect();
    assert_eq!(
        matching.len(),
        1,
        "expected exactly one trait named {trait_name} in file suffix {file_suffix}; rows: {:#?}",
        rows.rows
    );
    ploke_db::to_uuid(&matching[0])
}

fn impl_id_with_trait_and_self_type_names(
    db: &Database,
    trait_name: &str,
    self_type_name: &str,
) -> Result<Uuid, DbError> {
    super::common::exactly_one_uuid(
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
