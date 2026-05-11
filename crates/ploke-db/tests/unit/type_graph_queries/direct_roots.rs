use ploke_db::{DbError, TypeUseRole, TypeUseRoot};

use super::common::{
    function_id_by_name, function_id_by_name_in_module, function_param_type_by_name,
    function_return_type_by_name_in_module, generic_type_param_id_by_owner_name,
    setup_typed_fixture_db, struct_id_by_name, trait_id_by_name_in_module, type_alias_row_by_name,
};

/// Elementary contract: the DB API should expose direct owner-to-root type-use
/// rows before any structural traversal or terminal resolution is involved.
///
/// Grounding: `uuid_phase3_resolution/type_relations_v2.rs` already asserts
/// that `fixture_type_resolution_v2::concrete` resolves its parameter type to
/// the local `T` struct. This test starts one layer lower in Cozo: the function
/// owner should first expose its parameter slot's root structural type ID.
#[test]
fn function_param_root_type_use_is_queryable() -> Result<(), DbError> {
    let db = setup_typed_fixture_db("fixture_type_resolution_v2")?;
    let owner_id = function_id_by_name(&db, "concrete")?;
    let root_type_id = function_param_type_by_name(&db, "concrete", 0)?;

    let roots = db.type_uses_for_owner(owner_id)?;

    assert!(
        roots.contains(&TypeUseRoot {
            owner_id,
            root_type_id,
            role: TypeUseRole::FunctionParam,
            slot_index: Some(0),
        }),
        "expected function parameter root type use for concrete; roots: {roots:#?}"
    );
    Ok(())
}

/// Direct root contract for function returns.
///
/// Grounding: `fixture_types::apply_op` returns `i32`; regardless of whether
/// that terminal resolves to a local code item, the owner-to-root type-use row
/// should exist for graph traversal and exact root matching.
#[test]
fn function_return_root_type_use_is_queryable() -> Result<(), DbError> {
    let db = setup_typed_fixture_db("fixture_types")?;
    let owner_id = function_id_by_name_in_module(&db, &["crate"], "apply_op")?;
    let root_type_id = function_return_type_by_name_in_module(&db, &["crate"], "apply_op")?;

    let roots = db.type_uses_for_owner(owner_id)?;

    assert!(
        roots.contains(&TypeUseRoot {
            owner_id,
            root_type_id,
            role: TypeUseRole::FunctionReturn,
            slot_index: None,
        }),
        "expected function return root type use for apply_op; roots: {roots:#?}"
    );
    Ok(())
}

/// Direct root contract for type aliases.
///
/// This pins the boundary where a type alias item is connected to the root
/// structural type it names. Deeper argument traversal belongs to
/// `type_contains`, not to this direct owner/root edge.
#[test]
fn type_alias_target_root_type_use_is_queryable() -> Result<(), DbError> {
    let db = setup_typed_fixture_db("fixture_nodes")?;
    let (owner_id, root_type_id) = type_alias_row_by_name(&db, "Mapping")?;

    let roots = db.type_uses_for_owner(owner_id)?;

    assert!(
        roots.contains(&TypeUseRoot {
            owner_id,
            root_type_id,
            role: TypeUseRole::TypeAliasTarget,
            slot_index: None,
        }),
        "expected type alias root type use for Mapping; roots: {roots:#?}"
    );
    Ok(())
}

/// Generic declaration bounds should be queryable both from the containing item
/// and from the precise generic parameter declaration.
#[test]
fn generic_bound_roots_are_queryable_from_item_and_param_owners() -> Result<(), DbError> {
    let db = setup_typed_fixture_db("fixture_type_resolution_v2")?;
    let owner_id = struct_id_by_name(&db, "LocallyBound")?;
    let generic_param_id = generic_type_param_id_by_owner_name(&db, owner_id, "T")?;

    let owner_roots = db.type_uses_for_owner(owner_id)?;
    let generic_param_roots = db.type_uses_for_owner(generic_param_id)?;

    let owner_bound = owner_roots
        .iter()
        .find(|root| root.role == TypeUseRole::GenericBound)
        .copied()
        .unwrap_or_else(|| {
            panic!("expected LocallyBound to expose a GenericBound root; roots: {owner_roots:#?}")
        });

    assert!(
        generic_param_roots.contains(&TypeUseRoot {
            owner_id: generic_param_id,
            root_type_id: owner_bound.root_type_id,
            role: TypeUseRole::GenericParamBound,
            slot_index: Some(0),
        }),
        "expected LocallyBound::T to expose the same bound root as GenericParamBound; roots: {generic_param_roots:#?}"
    );

    Ok(())
}

/// Associated type bounds should expose a direct trait-position root from the
/// containing trait. Precise associated type item ownership is a separate
/// future surface; this pins the current GraphRAG-friendly owner path.
#[test]
fn associated_type_bound_root_is_queryable_from_containing_trait() -> Result<(), DbError> {
    let db = setup_typed_fixture_db("fixture_type_resolution_v2")?;
    let owner_id = trait_id_by_name_in_module(&db, &["crate"], "LocalAssocBound")?;

    let roots = db.type_uses_for_owner(owner_id)?;

    assert!(
        roots.iter().any(|root| {
            root.owner_id == owner_id
                && root.role == TypeUseRole::AssociatedTypeBound
                && root.slot_index == Some(0)
        }),
        "expected LocalAssocBound to expose an AssociatedTypeBound root; roots: {roots:#?}"
    );

    Ok(())
}

/// Where predicates should keep the bounded subject separate from the trait
/// bounds. Direct generic-param subjects additionally expose a derived
/// generic-param-owned bound root.
#[test]
fn where_predicate_roots_are_queryable_from_item_and_param_owners() -> Result<(), DbError> {
    let db = setup_typed_fixture_db("fixture_type_resolution_v2")?;
    let owner_id = struct_id_by_name(&db, "WhereLocal")?;
    let generic_param_id = generic_type_param_id_by_owner_name(&db, owner_id, "T")?;

    let owner_roots = db.type_uses_for_owner(owner_id)?;
    let generic_param_roots = db.type_uses_for_owner(generic_param_id)?;

    assert!(
        owner_roots.iter().any(|root| {
            root.owner_id == owner_id
                && root.role == TypeUseRole::WherePredicateSubject
                && root.slot_index == Some(0)
        }),
        "expected WhereLocal to expose a WherePredicateSubject root; roots: {owner_roots:#?}"
    );

    let owner_bound = owner_roots
        .iter()
        .find(|root| {
            root.owner_id == owner_id
                && root.role == TypeUseRole::WherePredicateBound
                && root.slot_index == Some(0)
        })
        .copied()
        .unwrap_or_else(|| {
            panic!(
                "expected WhereLocal to expose a WherePredicateBound root; roots: {owner_roots:#?}"
            )
        });

    assert!(
        generic_param_roots.contains(&TypeUseRoot {
            owner_id: generic_param_id,
            root_type_id: owner_bound.root_type_id,
            role: TypeUseRole::WhereGenericParamBound,
            slot_index: Some(0),
        }),
        "expected WhereLocal::T to expose the same bound root as WhereGenericParamBound; roots: {generic_param_roots:#?}"
    );

    Ok(())
}

/// Composite where-predicate subjects are queryable as subjects, but should not
/// be collapsed into a direct generic-param-owned bound root.
#[test]
fn composite_where_predicate_subject_is_not_a_generic_param_bound() -> Result<(), DbError> {
    let db = setup_typed_fixture_db("fixture_type_resolution_v2")?;
    let owner_id = struct_id_by_name(&db, "WhereComposite")?;
    let generic_param_id = generic_type_param_id_by_owner_name(&db, owner_id, "T")?;

    let owner_roots = db.type_uses_for_owner(owner_id)?;
    let generic_param_roots = db.type_uses_for_owner(generic_param_id)?;

    assert!(
        owner_roots.iter().any(|root| {
            root.owner_id == owner_id
                && root.role == TypeUseRole::WherePredicateSubject
                && root.slot_index == Some(0)
        }),
        "expected WhereComposite to expose a WherePredicateSubject root; roots: {owner_roots:#?}"
    );
    assert!(
        owner_roots.iter().any(|root| {
            root.owner_id == owner_id
                && root.role == TypeUseRole::WherePredicateBound
                && root.slot_index == Some(0)
        }),
        "expected WhereComposite to expose a WherePredicateBound root; roots: {owner_roots:#?}"
    );
    assert!(
        generic_param_roots
            .iter()
            .all(|root| root.role != TypeUseRole::WhereGenericParamBound),
        "composite subject Vec<T> should not be collapsed into a WhereGenericParamBound root; roots: {generic_param_roots:#?}"
    );

    Ok(())
}
