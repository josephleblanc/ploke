use ploke_db::{DbError, TypeUseRole, TypeUseRoot};

use super::common::{
    function_id_by_name, function_id_by_name_in_module, function_param_type_by_name,
    function_return_type_by_name_in_module, setup_typed_fixture_db, type_alias_row_by_name,
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
