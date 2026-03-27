//! Tests for database queries and compilation-unit filters.
#![cfg(feature = "type_bearing_ids")]

use std::collections::{BTreeMap, HashSet};
use std::path::PathBuf;

use cozo::{DataValue, Db, MemStorage, Num, ScriptMutability, UuidWrapper};
use ploke_db::{CodeSnippet, Database, QueryBuilder, to_uuid};
use ploke_error::Error;
use ploke_transform::transform::insert_structural_compilation_unit_slice;
use ploke_transform::schema::primary_nodes::StructNodeSchema;
use syn_parser::compilation_unit::{
    CompilationUnitKey, CompilationUnitTargetKind, StructuralCompilationUnitSlice,
};
use syn_parser::compilation_unit::{compilation_unit_keys_for_targets, default_target_triple};
use syn_parser::compilation_unit::build_structural_compilation_unit_slices;
use uuid::Uuid;

use crate::common::test_helpers;

fn insert_test_struct(db: &Db<MemStorage>, id: Uuid, name: &str) {
    let schema = &StructNodeSchema::SCHEMA;
    let th = Uuid::new_v4();
    let params = BTreeMap::from([
        (schema.id().to_string(), DataValue::Uuid(UuidWrapper(id))),
        (schema.name().to_string(), DataValue::from(name)),
        (
            schema.span().to_string(),
            DataValue::List(vec![
                DataValue::Num(Num::Int(0)),
                DataValue::Num(Num::Int(1)),
            ]),
        ),
        (schema.vis_kind().to_string(), DataValue::from("public")),
        (schema.vis_path().to_string(), DataValue::Null),
        (schema.docstring().to_string(), DataValue::Null),
        (
            schema.tracking_hash().to_string(),
            DataValue::Uuid(UuidWrapper(th)),
        ),
        (schema.cfgs().to_string(), DataValue::Null),
    ]);
    let script = schema.script_put(&params);
    db.run_script(&script, params, ScriptMutability::Mutable)
        .expect("insert struct");
}

fn sample_cu_key(namespace: Uuid, root_label: &str) -> CompilationUnitKey {
    CompilationUnitKey::new(
        namespace,
        CompilationUnitTargetKind::Lib,
        "test".into(),
        PathBuf::from(format!("/tmp/{root_label}/src/lib.rs")),
        "x86_64-unknown-linux-gnu".into(),
        "dev".into(),
        vec![],
    )
}

#[test]
fn test_basic_struct_query_raw() -> Result<(), Error> {
    let db = test_helpers::setup_test_db();
    insert_test_struct(&db, Uuid::new_v4(), "SampleStruct");
    let ploke_db = Database::new(db);

    let result = ploke_db.raw_query(
        r#"?[id, name, vis_kind, docstring] :=
            *struct { id, name, vis_kind, docstring @ 'NOW' },
            name = 'SampleStruct'"#,
    )?;

    let snippets: Vec<CodeSnippet> = result.into_snippets()?;
    assert_eq!(snippets.len(), 1);
    assert_eq!(snippets[0].text, "SampleStruct");

    Ok(())
}

/// After ingest, `compilation_unit_enabled_node` holds the structural mask; the query builder
/// emits the membership conjunct expected for joins (bind primary `id` to `node_id`).
#[test]
fn test_compilation_unit_membership_rows_and_filter_contract() -> Result<(), Error> {
    let db = test_helpers::setup_test_db();
    let ns = Uuid::new_v4();
    let id_in = Uuid::new_v4();
    let id_out = Uuid::new_v4();
    insert_test_struct(&db, id_in, "InCu");
    insert_test_struct(&db, id_out, "OutCu");

    let key = sample_cu_key(ns, "cu1");
    let cu_id = key.compilation_unit_id();
    let slice = StructuralCompilationUnitSlice {
        key,
        cu_id,
        enabled_node_ids: HashSet::from([id_in]),
        enabled_edges: vec![],
        enabled_file_paths: HashSet::new(),
    };
    insert_structural_compilation_unit_slice(&db, &slice).expect("insert slice");

    let ploke_db = Database::new(db);

    let probe = ploke_db
        .raw_query("?[cu_id, node_id] := *compilation_unit_enabled_node { cu_id, node_id }")?;
    assert_eq!(probe.rows.len(), 1);
    assert_eq!(to_uuid(&probe.rows[0][0])?, cu_id);
    assert_eq!(to_uuid(&probe.rows[0][1])?, id_in);

    let clause = QueryBuilder::new()
        .structs()
        .filter_by_compilation_unit(cu_id)
        .filters()
        .join(", ");
    assert_eq!(
        clause,
        format!(
            "*compilation_unit_enabled_node {{ cu_id: '{}', node_id: id }}",
            cu_id
        )
    );

    Ok(())
}

#[test]
fn test_filter_by_compilation_unit_uses_edge_membership_for_syntax_edges() -> Result<(), Error> {
    let cu_id = Uuid::new_v4();
    let clause = QueryBuilder::new()
        .syntax_edges()
        .filter_by_compilation_unit(cu_id)
        .filters()
        .join(", ");

    assert_eq!(
        clause,
        format!(
            "*compilation_unit_enabled_edge {{ cu_id: '{}', source_id, target_id, relation_kind }}",
            cu_id
        )
    );

    Ok(())
}

#[test]
fn test_filter_by_compilation_units_or_clause_contract() -> Result<(), Error> {
    let db = test_helpers::setup_test_db();
    let ns = Uuid::new_v4();
    let id_a = Uuid::new_v4();
    let id_b = Uuid::new_v4();
    insert_test_struct(&db, id_a, "Alpha");
    insert_test_struct(&db, id_b, "Beta");

    let key_a = sample_cu_key(ns, "orca");
    let key_b = sample_cu_key(ns, "orb");
    let cu_a = key_a.compilation_unit_id();
    let cu_b = key_b.compilation_unit_id();

    insert_structural_compilation_unit_slice(
        &db,
        &StructuralCompilationUnitSlice {
            key: key_a,
            cu_id: cu_a,
            enabled_node_ids: HashSet::from([id_a]),
            enabled_edges: vec![],
            enabled_file_paths: HashSet::new(),
        },
    )
    .expect("insert a");
    insert_structural_compilation_unit_slice(
        &db,
        &StructuralCompilationUnitSlice {
            key: key_b,
            cu_id: cu_b,
            enabled_node_ids: HashSet::from([id_b]),
            enabled_edges: vec![],
            enabled_file_paths: HashSet::new(),
        },
    )
    .expect("insert b");

    let or_clause = QueryBuilder::new()
        .structs()
        .filter_by_compilation_units(&[cu_a, cu_b])
        .filters()
        .join(", ");

    let a = format!(
        "*compilation_unit_enabled_node {{ cu_id: '{}', node_id: id }}",
        cu_a
    );
    let b = format!(
        "*compilation_unit_enabled_node {{ cu_id: '{}', node_id: id }}",
        cu_b
    );
    assert_eq!(or_clause, format!("({a} or {b})"));

    Ok(())
}

#[test]
fn test_filter_by_compilation_units_or_clause_for_syntax_edges() -> Result<(), Error> {
    let cu_a = Uuid::new_v4();
    let cu_b = Uuid::new_v4();
    let or_clause = QueryBuilder::new()
        .syntax_edges()
        .filter_by_compilation_units(&[cu_a, cu_b])
        .filters()
        .join(", ");

    let a = format!(
        "*compilation_unit_enabled_edge {{ cu_id: '{}', source_id, target_id, relation_kind }}",
        cu_a
    );
    let b = format!(
        "*compilation_unit_enabled_edge {{ cu_id: '{}', source_id, target_id, relation_kind }}",
        cu_b
    );
    assert_eq!(or_clause, format!("({a} or {b})"));

    Ok(())
}

#[test]
fn test_fixture_based_membership_rows_diverge_per_target_compilation_unit() -> Result<(), Error> {
    let db = test_helpers::setup_test_db();
    let parsed_graphs = ploke_test_utils::test_run_phases_and_collect("fixture_multi_target_cu");
    let ctx = parsed_graphs
        .iter()
        .find_map(|g| g.crate_context.as_ref())
        .expect("crate context");
    let keys = compilation_unit_keys_for_targets(
        ctx.namespace,
        &ctx.targets,
        default_target_triple(),
        "dev".to_string(),
        vec![],
    );
    let lib_key = keys
        .iter()
        .find(|k| k.target_kind == CompilationUnitTargetKind::Lib)
        .expect("lib key")
        .clone();
    let bin_key = keys
        .iter()
        .find(|k| k.target_kind == CompilationUnitTargetKind::Bin)
        .expect("bin key")
        .clone();

    let slices = build_structural_compilation_unit_slices(parsed_graphs.clone(), &keys)
        .expect("build slices");
    for slice in &slices {
        insert_structural_compilation_unit_slice(&db, slice).expect("insert slice");
    }
    let ploke_db = Database::new(db);
    let rows = ploke_db
        .raw_query("?[cu_id, node_id] := *compilation_unit_enabled_node { cu_id, node_id }")?;
    let mut by_cu: std::collections::HashMap<Uuid, HashSet<Uuid>> = std::collections::HashMap::new();
    for row in rows.rows {
        let cu_id = to_uuid(&row[0])?;
        let node_id = to_uuid(&row[1])?;
        by_cu.entry(cu_id).or_default().insert(node_id);
    }

    let lib_nodes = by_cu
        .get(&lib_key.compilation_unit_id())
        .expect("lib cu membership");
    let bin_nodes = by_cu
        .get(&bin_key.compilation_unit_id())
        .expect("bin cu membership");
    assert!(!lib_nodes.is_empty());
    assert!(!bin_nodes.is_empty());
    assert_ne!(lib_nodes, bin_nodes);

    let lib_clause = QueryBuilder::new()
        .structs()
        .filter_by_compilation_unit(lib_key.compilation_unit_id())
        .filters()
        .join(", ");
    assert_eq!(
        lib_clause,
        format!(
            "*compilation_unit_enabled_node {{ cu_id: '{}', node_id: id }}",
            lib_key.compilation_unit_id()
        )
    );

    Ok(())
}
