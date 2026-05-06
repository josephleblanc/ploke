#![cfg(feature = "typed_type_graph")]

use cozo::{Db, MemStorage};
use ploke_db::{
    Database, DbError, TypeContainmentKind, TypeRelationKind, TypeTargetPath, TypeUseRole,
    TypeUseRoot, to_uuid,
};
use ploke_transform::{schema::create_schema_all, transform::transform_parsed_graph};
use uuid::Uuid;

fn setup_typed_fixture_db(fixture: &'static str) -> Result<Database, DbError> {
    let db = Db::new(MemStorage::default()).expect("in-memory cozo db");
    db.initialize().expect("initialize cozo db");
    create_schema_all(&db).map_err(|err| DbError::QueryExecution(err.to_string()))?;

    let graphs = ploke_test_utils::test_run_phases_and_collect(fixture);
    let mut merged = syn_parser::parser::ParsedCodeGraph::merge_new(graphs)
        .map_err(|err| DbError::QueryExecution(err.to_string()))?;
    let tree = merged
        .build_tree_and_prune()
        .map_err(|err| DbError::QueryExecution(err.to_string()))?;
    transform_parsed_graph(&db, merged, &tree)
        .map_err(|err| DbError::QueryExecution(err.to_string()))?;

    Ok(Database::new(db))
}

fn exactly_one_uuid(db: &Database, script: &str, column: usize) -> Result<Uuid, DbError> {
    let rows = db.raw_query(script)?;
    assert_eq!(
        rows.rows.len(),
        1,
        "expected exactly one row for query:\n{script}\nrows: {:#?}",
        rows.rows
    );
    to_uuid(&rows.rows[0][column])
}

fn function_id_by_name(db: &Database, name: &str) -> Result<Uuid, DbError> {
    exactly_one_uuid(
        db,
        &format!(
            r#"?[id] :=
                *function {{ id, name: "{name}" @ 'NOW' }}"#
        ),
        0,
    )
}

fn function_id_by_name_in_module(
    db: &Database,
    module_path: &[&str],
    name: &str,
) -> Result<Uuid, DbError> {
    let module_path = cozo_string_list(module_path);
    exactly_one_uuid(
        db,
        &format!(
            r#"?[id] :=
                *function {{ id, name: "{name}", module_id @ 'NOW' }},
                *module {{ id: module_id, path: {module_path} @ 'NOW' }}"#
        ),
        0,
    )
}

fn function_param_type_by_name(
    db: &Database,
    name: &str,
    param_index: u32,
) -> Result<Uuid, DbError> {
    let owner_id = function_id_by_name(db, name)?;
    exactly_one_uuid(
        db,
        &format!(
            r#"?[type_id] :=
                *function {{ id: function_id, name: "{name}" @ 'NOW' }},
                *param {{
                    function_id,
                    param_index: {param_index},
                    type_id @ 'NOW'
                }},
                function_id = to_uuid("{owner_id}")"#
        ),
        0,
    )
}

fn type_alias_row_by_name(db: &Database, name: &str) -> Result<(Uuid, Uuid), DbError> {
    let rows = db.raw_query(&format!(
        r#"?[id, type_id] :=
            *type_alias {{ id, name: "{name}", ty_id: type_id @ 'NOW' }}"#
    ))?;
    assert_eq!(
        rows.rows.len(),
        1,
        "expected exactly one type alias named {name}, rows: {:#?}",
        rows.rows
    );
    Ok((to_uuid(&rows.rows[0][0])?, to_uuid(&rows.rows[0][1])?))
}

fn cozo_string_list(items: &[&str]) -> String {
    format!(
        "[{}]",
        items
            .iter()
            .map(|item| format!("\"{item}\""))
            .collect::<Vec<_>>()
            .join(", ")
    )
}

fn trait_id_by_name_in_module(
    db: &Database,
    module_path: &[&str],
    name: &str,
) -> Result<Uuid, DbError> {
    let module_path = cozo_string_list(module_path);
    exactly_one_uuid(
        db,
        &format!(
            r#"?[id] :=
                *module {{ id: module_id, path: {module_path} @ 'NOW' }},
                *syntax_edge {{
                    source_id: module_id,
                    target_id: id,
                    relation_kind: "Contains" @ 'NOW'
                }},
                *trait {{ id, name: "{name}" @ 'NOW' }}"#
        ),
        0,
    )
}

/// Elementary contract: the DB API should expose direct owner-to-root type-use
/// rows before any structural traversal or terminal resolution is involved.
///
/// Grounding: `uuid_phase3_resolution/type_relations_v2.rs` already asserts
/// that `fixture_type_resolution_v2::concrete` resolves its parameter type to
/// the local `T` struct. This test starts one layer lower in Cozo: the function
/// owner should first expose its parameter slot's root structural type ID.
#[test]
fn type_uses_for_owner_returns_fixture_function_param_root() -> Result<(), DbError> {
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

/// Elementary structural contract: generic arguments of a named type should be
/// queryable through one normalized containment surface, preserving argument
/// order.
///
/// Grounding: `uuid_phase2_partial_graphs/nodes/type_alias.rs` already asserts
/// that `fixture_nodes::type_alias::Mapping` has target type
/// `std::collections::HashMap<K, V>` with two related type nodes in order.
/// This test describes the Cozo-facing shape we want for that same fact.
#[test]
fn direct_type_contains_preserves_mapping_alias_generic_argument_order() -> Result<(), DbError> {
    let db = setup_typed_fixture_db("fixture_nodes")?;
    let (_alias_id, root_type_id) = type_alias_row_by_name(&db, "Mapping")?;

    let edges = db.direct_type_contains(root_type_id)?;
    let argument_positions: Vec<_> = edges
        .iter()
        .filter(|edge| edge.kind == TypeContainmentKind::Argument)
        .map(|edge| edge.position)
        .collect();

    assert_eq!(
        argument_positions,
        vec![Some(0), Some(1)],
        "expected ordered HashMap<K, V> argument containment edges; edges: {edges:#?}"
    );
    Ok(())
}

/// Ambitious resolution contract: direct root resolution should compose owner
/// type-use rows with `type_relation` terminal rows.
///
/// Grounding: the phase3 v2 resolver test already proves the parameter type of
/// `concrete` resolves to the local `T` struct. This DB query should recover
/// the same target through the database graph surface.
#[test]
fn reachable_type_targets_find_direct_param_struct_target() -> Result<(), DbError> {
    let db = setup_typed_fixture_db("fixture_type_resolution_v2")?;
    let owner_id = function_id_by_name(&db, "concrete")?;
    let root_type_id = function_param_type_by_name(&db, "concrete", 0)?;
    let target_id = exactly_one_uuid(
        &db,
        r#"?[id] :=
            *struct { id, name: "T" @ 'NOW' }"#,
        0,
    )?;

    let reachable = db.type_targets_reachable_from_owner(owner_id)?;

    assert!(
        reachable.contains(&TypeTargetPath {
            owner_id,
            root_type_id,
            terminal_type_id: root_type_id,
            target_id,
            relation_kind: TypeRelationKind::Ordinary,
            depth: 0,
        }),
        "expected direct function parameter struct target for concrete; reachable: {reachable:#?}"
    );
    Ok(())
}

/// Ambitious traversal contract: trait-position terminals nested inside
/// ordinary root syntax should still reach their trait definitions.
///
/// Grounding: `fixture_types::draw_object` has parameter type `&dyn Drawable`.
/// The owner root is an ordinary reference type, but the terminal trait bound
/// inside the trait object should resolve to the local `Drawable` trait after
/// walking structural containment.
#[test]
fn reachable_type_targets_walk_reference_trait_object_to_trait_definition() -> Result<(), DbError> {
    let db = setup_typed_fixture_db("fixture_types")?;
    let owner_id = function_id_by_name_in_module(&db, &["crate"], "draw_object")?;
    let target_id = trait_id_by_name_in_module(&db, &["crate"], "Drawable")?;

    let reachable = db.type_targets_reachable_from_owner(owner_id)?;

    assert!(
        reachable.iter().any(|path| {
            path.owner_id == owner_id
                && path.target_id == target_id
                && path.relation_kind == TypeRelationKind::Trait
                && path.depth >= 1
        }),
        "expected draw_object parameter to reach Drawable through &dyn Drawable; reachable: {reachable:#?}"
    );
    Ok(())
}
