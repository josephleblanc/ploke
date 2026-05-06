use ploke_db::{DbError, TypeRelationKind};

use super::common::{function_id_by_name, setup_typed_fixture_db, struct_id_by_name};

/// Retrieval contract: code owners that share an exact root type should rank
/// closer than owners that only share a nested terminal type.
///
/// This is the graphRAG-oriented owner-to-owner traversal layered on top of:
///
/// owner -> root type use -> type containment closure -> resolved target
/// <- type containment closure <- root type use <- related owner
#[test]
fn related_owners_rank_exact_root_match_before_nested_terminal_match() -> Result<(), DbError> {
    let db = setup_typed_fixture_db("fixture_type_resolution_v2")?;
    let origin_owner_id = function_id_by_name(&db, "concrete")?;
    let exact_owner_id = function_id_by_name(&db, "also_concrete")?;
    let nested_owner_id = function_id_by_name(&db, "takes_vec_of_t")?;
    let target_id = struct_id_by_name(&db, "T")?;

    let related = db.type_related_owners(origin_owner_id)?;

    let exact = related
        .iter()
        .find(|path| {
            path.related_owner_id == exact_owner_id
                && path.shared_target_id == target_id
                && path.relation_kind == TypeRelationKind::Ordinary
        })
        .expect("concrete(T) should be related to also_concrete(T) through exact T root");
    let nested = related
        .iter()
        .find(|path| {
            path.related_owner_id == nested_owner_id
                && path.shared_target_id == target_id
                && path.relation_kind == TypeRelationKind::Ordinary
        })
        .expect("concrete(T) should be related to takes_vec_of_t(Vec<T>) through nested T");

    assert_eq!(exact.origin_depth, 0);
    assert_eq!(exact.related_depth, 0);
    assert_eq!(exact.distance, 0);
    assert_eq!(nested.origin_depth, 0);
    assert!(
        nested.related_depth > 0,
        "Vec<T> should reach T through positive-depth containment"
    );
    assert!(
        exact.distance < nested.distance,
        "exact root relation should rank ahead of nested terminal relation; related: {related:#?}"
    );
    assert!(
        related
            .iter()
            .all(|path| path.related_owner_id != origin_owner_id),
        "related owner traversal should not return the origin owner itself"
    );
    Ok(())
}

/// Retrieval contract: starting from an edited type definition should find code
/// owners that use that type, with direct root users ranked before owners that
/// mention it only inside a composite type.
#[test]
fn target_centered_owner_query_ranks_direct_users_before_nested_users() -> Result<(), DbError> {
    let db = setup_typed_fixture_db("fixture_type_resolution_v2")?;
    let direct_owner_id = function_id_by_name(&db, "concrete")?;
    let second_direct_owner_id = function_id_by_name(&db, "also_concrete")?;
    let nested_owner_id = function_id_by_name(&db, "takes_vec_of_t")?;
    let target_id = struct_id_by_name(&db, "T")?;

    let owners = db.type_owners_for_target(target_id)?;

    let direct = owners
        .iter()
        .find(|path| {
            path.owner_id == direct_owner_id && path.relation_kind == TypeRelationKind::Ordinary
        })
        .expect("struct T should find concrete(T) as a direct user");
    let second_direct = owners
        .iter()
        .find(|path| {
            path.owner_id == second_direct_owner_id
                && path.relation_kind == TypeRelationKind::Ordinary
        })
        .expect("struct T should find also_concrete(T) as a direct user");
    let nested = owners
        .iter()
        .find(|path| {
            path.owner_id == nested_owner_id && path.relation_kind == TypeRelationKind::Ordinary
        })
        .expect("struct T should find takes_vec_of_t(Vec<T>) as a nested user");

    assert_eq!(direct.depth, 0);
    assert_eq!(second_direct.depth, 0);
    assert!(
        nested.depth > 0,
        "Vec<T> owner should reach T through positive-depth containment"
    );
    assert!(
        direct.depth < nested.depth && second_direct.depth < nested.depth,
        "direct users of T should rank ahead of nested users; owners: {owners:#?}"
    );
    Ok(())
}
