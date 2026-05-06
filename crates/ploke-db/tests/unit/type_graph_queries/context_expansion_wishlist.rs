//! Consumer-facing type context expansion wishlist.
//!
//! The tests in this module are not about raw Cozo edge rows. They describe
//! the API shape Ploke's search, context planning, and LLM-facing tools should
//! eventually consume: given a seed code item or type definition, return ranked
//! code-context candidates with a reason that is meaningful to a Rust coding
//! assistant.
//!
//! Each test is ignored until `Database::expand_type_context(...)` or an
//! equivalent application-facing retrieval API exists.

use ploke_db::{Database, DbError};
use ploke_test_utils::{
    CORPUS_CHRONO_TYPE_GRAPH, CORPUS_GENERIC_ARRAY_TYPE_GRAPH, CORPUS_MEMCHR_TYPE_GRAPH,
    CORPUS_SEMVER_TYPE_GRAPH,
};
use uuid::Uuid;

use super::common::{
    enum_id_by_name, field_id_by_owner_index, function_id_by_name, function_id_by_name_in_module,
    setup_typed_backup_db, setup_typed_fixture_db, struct_id_by_name, struct_id_by_name_in_module,
    trait_id_by_name_in_module, type_alias_row_by_name,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TypeContextSeed {
    Owner(Uuid),
    Target(Uuid),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TypeContextRelation {
    SameResolvedType,
    UsesTypeNested,
    TypeDefinitionImpact,
    ImplOfTrait,
    ImplSelfType,
    AliasExpansion,
    TraitBound,
    IteratorSurface,
    ConstGenericAlias,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct TypeContextCandidate {
    node_id: Uuid,
    relation: TypeContextRelation,
    distance: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct TypeContextOptions {
    include_nested: bool,
    include_impls: bool,
    include_traits: bool,
    max_distance: u32,
}

impl Default for TypeContextOptions {
    fn default() -> Self {
        Self {
            include_nested: true,
            include_impls: true,
            include_traits: true,
            max_distance: 8,
        }
    }
}

#[test]
#[ignore = "wishlist: app-facing type context expansion API is not implemented yet"]
fn owner_seed_returns_ranked_related_code_with_explanatory_reasons() -> Result<(), DbError> {
    let db = setup_typed_fixture_db("fixture_type_resolution_v2")?;
    let seed_owner = function_id_by_name(&db, "concrete")?;
    let exact_peer = function_id_by_name(&db, "also_concrete")?;
    let nested_peer = function_id_by_name(&db, "takes_vec_of_t")?;

    let candidates = expand_type_context(
        &db,
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
    assert_ranked_before(
        &candidates,
        exact_peer,
        nested_peer,
        "exact root type peers should rank before nested terminal peers",
    );
    Ok(())
}

#[test]
#[ignore = "wishlist: app-facing type context expansion API is not implemented yet"]
fn type_definition_seed_returns_direct_and_nested_users_ranked_by_type_distance()
-> Result<(), DbError> {
    let db = setup_typed_fixture_db("fixture_type_resolution_v2")?;
    let seed_target = struct_id_by_name(&db, "T")?;
    let direct_user = function_id_by_name(&db, "concrete")?;
    let second_direct_user = function_id_by_name(&db, "also_concrete")?;
    let nested_user = function_id_by_name(&db, "takes_vec_of_t")?;

    let candidates = expand_type_context(
        &db,
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
#[ignore = "wishlist: app-facing type context expansion API is not implemented yet"]
fn trait_seed_returns_impl_junctions_and_self_types() -> Result<(), DbError> {
    let db = setup_typed_fixture_db("fixture_type_resolution_v2")?;
    let trait_id = trait_id_by_name_in_module(&db, &["crate"], "LocalTrait")?;
    let impl_id = impl_id_with_trait_and_self_type_names(&db, "LocalTrait", "UsesTrait")?;
    let self_type_id = struct_id_by_name(&db, "UsesTrait")?;

    let candidates = expand_type_context(
        &db,
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
#[ignore = "wishlist: app-facing type context expansion API is not implemented yet"]
fn trait_bound_seed_returns_bound_owners_and_trait_definition_context() -> Result<(), DbError> {
    let db = setup_typed_fixture_db("fixture_types")?;
    let trait_id = trait_id_by_name_in_module(&db, &["crate"], "Drawable")?;
    let draw_object_id = function_id_by_name_in_module(&db, &["crate"], "draw_object")?;

    let candidates = expand_type_context(
        &db,
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
#[ignore = "wishlist: app-facing type context expansion API is not implemented yet"]
fn alias_seed_preserves_alias_identity_and_expands_to_semantic_target() -> Result<(), DbError> {
    let db = setup_typed_backup_db(&CORPUS_CHRONO_TYPE_GRAPH)?;
    let (alias_id, _root_type_id) = type_alias_row_by_name(&db, "MappedLocalTime")?;
    let local_result_id = enum_id_by_name(&db, "LocalResult")?;

    let candidates = expand_type_context(
        &db,
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
#[ignore = "wishlist: app-facing type context expansion API is not implemented yet"]
fn semver_version_req_seed_returns_matching_and_comparator_context() -> Result<(), DbError> {
    let db = setup_typed_backup_db(&CORPUS_SEMVER_TYPE_GRAPH)?;
    let version_req_id = struct_id_by_name_in_module(&db, &["crate"], "VersionReq")?;
    let matches_req_id = function_id_by_name_in_module(&db, &["crate", "eval"], "matches_req")?;
    let comparators_field_id = field_id_by_owner_index(&db, version_req_id, 0)?;
    let comparator_id = struct_id_by_name(&db, "Comparator")?;

    let candidates = expand_type_context(
        &db,
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
#[ignore = "wishlist: app-facing type context expansion API is not implemented yet"]
fn memchr_iterator_seed_returns_constructor_and_iterator_surface() -> Result<(), DbError> {
    let db = setup_typed_backup_db(&CORPUS_MEMCHR_TYPE_GRAPH)?;
    let memchr_id = struct_id_by_name(&db, "Memchr")?;
    let constructor_id = function_id_by_name_in_module(&db, &["crate", "memchr"], "memchr_iter")?;

    let candidates = expand_type_context(
        &db,
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
#[ignore = "wishlist: app-facing type context expansion API is not implemented yet"]
fn generic_array_alias_seed_returns_storage_type_and_const_generic_context() -> Result<(), DbError>
{
    let db = setup_typed_backup_db(&CORPUS_GENERIC_ARRAY_TYPE_GRAPH)?;
    let (alias_id, _root_type_id) = type_alias_row_by_name(&db, "ConstGenericArray")?;
    let generic_array_id = struct_id_by_name(&db, "GenericArray")?;

    let candidates = expand_type_context(
        &db,
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

fn expand_type_context(
    _db: &Database,
    _seed: TypeContextSeed,
    _options: TypeContextOptions,
) -> Result<Vec<TypeContextCandidate>, DbError> {
    todo!("implement app-facing type context expansion over type graph primitives")
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
