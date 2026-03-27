//! Insert compilation-unit identity and structural membership rows.

use std::collections::{BTreeMap, HashSet};
use std::path::PathBuf;

use cozo::{DataValue, Db, MemStorage, Num, ScriptMutability, UuidWrapper};
use ploke_core::CompilationUnitTargetKind;
use syn_parser::compilation_unit::{
    COMPILATION_UNIT_ID_NAMESPACE, CompilationUnitKey, StructuralCompilationUnitSlice,
};
use tracing::instrument;
use uuid::Uuid;

use crate::{
    error::TransformError,
    schema::compilation_unit::{
        CompilationUnitEnabledEdgeSchema, CompilationUnitEnabledFileSchema,
        CompilationUnitEnabledNodeSchema, CompilationUnitMetaSchema, CompilationUnitSchema,
    },
};

/// Persists one compilation unit row plus membership rows for a structural slice.
#[instrument(skip_all, fields(cu_id = %slice.cu_id))]
pub fn insert_structural_compilation_unit_slice(
    db: &Db<MemStorage>,
    slice: &StructuralCompilationUnitSlice,
) -> Result<(), TransformError> {
    insert_compilation_unit_row(db, &slice.key)?;
    insert_enabled_nodes(db, slice.cu_id, &slice.enabled_node_ids)?;
    insert_enabled_edges(db, slice.cu_id, &slice.enabled_edges)?;
    insert_enabled_files(db, slice.cu_id, &slice.enabled_file_paths)?;
    insert_meta(db, slice)?;
    Ok(())
}

fn insert_compilation_unit_row(
    db: &Db<MemStorage>,
    key: &CompilationUnitKey,
) -> Result<(), TransformError> {
    let schema = &CompilationUnitSchema::SCHEMA;
    let features = key.normalized_features();
    let cozo_features: Vec<DataValue> = features
        .iter()
        .map(|s| DataValue::from(s.as_str()))
        .collect();

    let params = BTreeMap::from([
        (
            schema.id().to_string(),
            DataValue::Uuid(UuidWrapper(key.compilation_unit_id())),
        ),
        (
            schema.namespace().to_string(),
            DataValue::Uuid(UuidWrapper(key.namespace)),
        ),
        (
            schema.target_kind().to_string(),
            DataValue::from(match key.target_kind {
                CompilationUnitTargetKind::Lib => "lib",
                CompilationUnitTargetKind::Bin => "bin",
                CompilationUnitTargetKind::Test => "test",
                CompilationUnitTargetKind::Example => "example",
                CompilationUnitTargetKind::Bench => "bench",
            }),
        ),
        (
            schema.target_name().to_string(),
            DataValue::from(key.target_name.as_str()),
        ),
        (
            schema.target_root().to_string(),
            DataValue::from(key.target_root.to_string_lossy().as_ref()),
        ),
        (
            schema.target_triple().to_string(),
            DataValue::from(key.target_triple.as_str()),
        ),
        (
            schema.profile().to_string(),
            DataValue::from(key.profile.as_str()),
        ),
        (
            schema.features().to_string(),
            DataValue::List(cozo_features),
        ),
        (
            schema.features_hash().to_string(),
            DataValue::Uuid(UuidWrapper(key.features_hash())),
        ),
    ]);

    let script = schema.script_put(&params);
    db.run_script(&script, params, ScriptMutability::Mutable)?;
    Ok(())
}

fn insert_enabled_nodes(
    db: &Db<MemStorage>,
    cu_id: Uuid,
    nodes: &HashSet<Uuid>,
) -> Result<(), TransformError> {
    let schema = &CompilationUnitEnabledNodeSchema::SCHEMA;
    for node_id in nodes {
        let params = BTreeMap::from([
            (
                schema.cu_id().to_string(),
                DataValue::Uuid(UuidWrapper(cu_id)),
            ),
            (
                schema.node_id().to_string(),
                DataValue::Uuid(UuidWrapper(*node_id)),
            ),
        ]);
        let script = schema.script_put(&params);
        db.run_script(&script, params, ScriptMutability::Mutable)?;
    }
    Ok(())
}

fn insert_enabled_edges(
    db: &Db<MemStorage>,
    cu_id: Uuid,
    edges: &[syn_parser::compilation_unit::EnabledSyntacticEdge],
) -> Result<(), TransformError> {
    let schema = &CompilationUnitEnabledEdgeSchema::SCHEMA;
    for e in edges {
        let row_id = Uuid::new_v5(
            &COMPILATION_UNIT_ID_NAMESPACE,
            format!(
                "edge:{}:{}:{}:{}",
                cu_id, e.source_id, e.target_id, e.relation_kind
            )
            .as_bytes(),
        );
        let params = BTreeMap::from([
            (
                schema.id().to_string(),
                DataValue::Uuid(UuidWrapper(row_id)),
            ),
            (
                schema.cu_id().to_string(),
                DataValue::Uuid(UuidWrapper(cu_id)),
            ),
            (
                schema.source_id().to_string(),
                DataValue::Uuid(UuidWrapper(e.source_id)),
            ),
            (
                schema.target_id().to_string(),
                DataValue::Uuid(UuidWrapper(e.target_id)),
            ),
            (
                schema.relation_kind().to_string(),
                DataValue::from(e.relation_kind.as_str()),
            ),
        ]);
        let script = schema.script_put(&params);
        db.run_script(&script, params, ScriptMutability::Mutable)?;
    }
    Ok(())
}

fn insert_enabled_files(
    db: &Db<MemStorage>,
    cu_id: Uuid,
    paths: &HashSet<PathBuf>,
) -> Result<(), TransformError> {
    let schema = &CompilationUnitEnabledFileSchema::SCHEMA;
    for p in paths {
        let s = p.to_string_lossy();
        let row_id = Uuid::new_v5(
            &COMPILATION_UNIT_ID_NAMESPACE,
            format!("file:{cu_id}:{s}").as_bytes(),
        );
        let params = BTreeMap::from([
            (
                schema.id().to_string(),
                DataValue::Uuid(UuidWrapper(row_id)),
            ),
            (
                schema.cu_id().to_string(),
                DataValue::Uuid(UuidWrapper(cu_id)),
            ),
            (schema.file_path().to_string(), DataValue::from(s.as_ref())),
        ]);
        let script = schema.script_put(&params);
        db.run_script(&script, params, ScriptMutability::Mutable)?;
    }
    Ok(())
}

fn insert_meta(
    db: &Db<MemStorage>,
    slice: &StructuralCompilationUnitSlice,
) -> Result<(), TransformError> {
    let schema = &CompilationUnitMetaSchema::SCHEMA;
    let params = BTreeMap::from([
        (
            schema.cu_id().to_string(),
            DataValue::Uuid(UuidWrapper(slice.cu_id)),
        ),
        (
            schema.enabled_node_count().to_string(),
            DataValue::Num(Num::Int(slice.enabled_node_ids.len() as i64)),
        ),
        (
            schema.enabled_edge_count().to_string(),
            DataValue::Num(Num::Int(slice.enabled_edges.len() as i64)),
        ),
        (
            schema.enabled_file_count().to_string(),
            DataValue::Num(Num::Int(slice.enabled_file_paths.len() as i64)),
        ),
    ]);
    let script = schema.script_put(&params);
    db.run_script(&script, params, ScriptMutability::Mutable)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use cozo::{Db, MemStorage};
    use ploke_test_utils::test_run_phases_and_collect;
    use syn_parser::{
        compilation_unit::{
            build_structural_compilation_unit_slice, compilation_unit_key_from_target,
            default_target_triple,
        },
        discovery::TargetKind,
        parser::ParsedCodeGraph,
    };

    use crate::{
        error::TransformError, schema::create_schema_all, transform::transform_parsed_graph,
    };

    use super::insert_structural_compilation_unit_slice;

    #[test]
    fn structural_compilation_unit_round_trip() -> Result<(), TransformError> {
        let db = Db::new(MemStorage::default()).expect("db");
        db.initialize().expect("init");
        create_schema_all(&db)?;

        let parsed_graphs = test_run_phases_and_collect("fixture_nodes");
        let partition =
            ParsedCodeGraph::partition_by_selected_roots(parsed_graphs.clone()).expect("partition");
        let root = partition
            .select_default_root_path()
            .expect("root")
            .to_path_buf();
        let ctx = parsed_graphs
            .iter()
            .find_map(|g| g.crate_context.as_ref())
            .expect("crate context");

        let key = compilation_unit_key_from_target(
            ctx.namespace,
            TargetKind::Lib,
            ctx.name.clone(),
            root,
            default_target_triple(),
            "dev".to_string(),
            vec![],
        );

        let slice =
            build_structural_compilation_unit_slice(parsed_graphs, &key).expect("structural slice");

        let mut merged = partition.merge_for_root(&key.target_root).expect("merge");
        let tree = merged
            .build_tree_and_prune_for_root_path(&key.target_root)
            .expect("tree");

        transform_parsed_graph(&db, merged, &tree)?;
        insert_structural_compilation_unit_slice(&db, &slice)?;

        Ok(())
    }

    #[test]
    fn cfg_filter_feature_gated_item_respects_compilation_unit_key() {
        use syn_parser::compilation_unit::filter_structural_slice_by_cfg;
        use syn_parser::parser::graph::{GraphAccess, GraphNode};
        use syn_parser::parser::visitor::ActiveCfg;

        let parsed_graphs = test_run_phases_and_collect("fixture_cfg_cu");
        let partition =
            ParsedCodeGraph::partition_by_selected_roots(parsed_graphs.clone()).expect("partition");
        let root = partition
            .select_default_root_path()
            .expect("root")
            .to_path_buf();
        let ctx = parsed_graphs
            .iter()
            .find_map(|g| g.crate_context.as_ref())
            .expect("ctx");
        let key_with = compilation_unit_key_from_target(
            ctx.namespace,
            TargetKind::Lib,
            ctx.name.clone(),
            root.clone(),
            default_target_triple(),
            "dev".to_string(),
            vec!["foo".to_string()],
        );
        let key_without = compilation_unit_key_from_target(
            ctx.namespace,
            TargetKind::Lib,
            ctx.name.clone(),
            root,
            default_target_triple(),
            "dev".to_string(),
            vec![],
        );
        let slice = build_structural_compilation_unit_slice(parsed_graphs.clone(), &key_with)
            .expect("slice");
        let merged = partition
            .merge_for_root(&key_with.target_root)
            .expect("merge");
        let active_with = ActiveCfg::from_compilation_unit_key(&key_with);
        let active_without = ActiveCfg::from_compilation_unit_key(&key_without);
        let filtered_with = filter_structural_slice_by_cfg(&merged, &slice, &active_with);
        let filtered_without = filter_structural_slice_by_cfg(&merged, &slice, &active_without);

        let only_foo_id = merged
            .functions()
            .iter()
            .find_map(|f| (f.name == "only_when_foo").then_some(f.any_id().uuid()));
        if let Some(id) = only_foo_id {
            assert!(filtered_with.enabled_node_ids.contains(&id));
            assert!(!filtered_without.enabled_node_ids.contains(&id));
        }
        assert!(filtered_with.enabled_node_ids.len() >= filtered_without.enabled_node_ids.len());
    }
}
