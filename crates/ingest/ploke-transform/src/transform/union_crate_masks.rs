//! Union crate ingest: one merged graph + transform, then structural CU masks per target.

use cozo::{Db, MemStorage};
use ploke_core::CompilationUnitKey;
use syn_parser::ParsedCodeGraph;
use syn_parser::compilation_unit::{
    CompilationUnitDimensionRequest, build_structural_compilation_unit_slices,
    compilation_unit_keys_for_targets, default_target_triple,
};
use syn_parser::parser::visitor::ActiveCfg;

use crate::error::TransformError;
use crate::transform::transform_parsed_graph;

use super::insert_structural_compilation_unit_slice;

/// Merge all Cargo target roots, [`ParsedCodeGraph::build_tree_and_prune`], [`transform_parsed_graph`]
/// once, then persist one structural [`syn_parser::compilation_unit::StructuralCompilationUnitSlice`]
/// per discovered target (same triple/profile/features baseline).
pub fn transform_union_crate_and_structural_masks(
    db: &Db<MemStorage>,
    parsed_graphs: Vec<ParsedCodeGraph>,
    compilation_units: Option<Vec<CompilationUnitKey>>,
) -> Result<(), TransformError> {
    if parsed_graphs.is_empty() {
        return Err(TransformError::Transformation(
            "transform_union_crate_and_structural_masks: empty parsed_graphs".into(),
        ));
    }
    let (namespace, targets) = parsed_graphs
        .iter()
        .find_map(|g| g.crate_context.as_ref())
        .map(|ctx| (ctx.namespace, ctx.targets.clone()))
        .ok_or_else(|| {
            TransformError::Transformation(
                "transform_union_crate_and_structural_masks: missing crate context".into(),
            )
        })?;

    let parsed_for_masks = parsed_graphs.clone();
    let (merged, tree) = ParsedCodeGraph::merge_union_graph_and_prune_tree(parsed_graphs)
        .map_err(|e| TransformError::Transformation(e.to_string()))?;
    transform_parsed_graph(db, merged, &tree)?;

    let keys = compilation_units.unwrap_or_else(|| {
        let dims = CompilationUnitDimensionRequest::from_env_or_default();
        if dims.target_triples.len() == 1
            && dims.profiles.len() == 1
            && dims.feature_sets.len() == 1
        {
            // Keep existing fast path shape for a single dimension tuple.
            compilation_unit_keys_for_targets(
                namespace,
                &targets,
                dims.target_triples
                    .first()
                    .cloned()
                    .unwrap_or_else(default_target_triple),
                dims.profiles
                    .first()
                    .cloned()
                    .unwrap_or_else(|| "dev".to_string()),
                dims.feature_sets.first().cloned().unwrap_or_default(),
            )
        } else {
            syn_parser::compilation_unit::enumerate_compilation_unit_keys(
                namespace, &targets, &dims,
            )
        }
    });
    let slices = build_structural_compilation_unit_slices(parsed_for_masks.clone(), &keys)
        .map_err(|e| TransformError::Transformation(e.to_string()))?;
    let partition = ParsedCodeGraph::partition_by_selected_roots(parsed_for_masks)
        .map_err(|e| TransformError::Transformation(e.to_string()))?;
    for slice in &slices {
        let merged_for_slice = partition
            .merge_for_root(&slice.key.target_root)
            .map_err(|e| TransformError::Transformation(e.to_string()))?;
        let active_cfg = ActiveCfg::from_compilation_unit_key(&slice.key);
        let cfg_refined =
            syn_parser::compilation_unit::filter_structural_slice_by_cfg(
                &merged_for_slice,
                slice,
                &active_cfg,
            );
        insert_structural_compilation_unit_slice(db, &cfg_refined)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use cozo::{DataValue, Db, MemStorage, ScriptMutability};
    use ploke_test_utils::test_run_phases_and_collect;
    use syn_parser::compilation_unit::{
        build_structural_compilation_unit_slices, compilation_unit_key_from_target,
        default_target_triple, filter_structural_slice_by_cfg,
    };
    use syn_parser::discovery::TargetKind;
    use syn_parser::parser::ParsedCodeGraph;
    use syn_parser::parser::visitor::ActiveCfg;

    use crate::schema::create_schema_all;

    use super::transform_union_crate_and_structural_masks;

    #[test]
    fn persists_cfg_refined_masks_per_compilation_unit() {
        let db = Db::new(MemStorage::default()).expect("db");
        db.initialize().expect("init");
        create_schema_all(&db).expect("schema");

        let parsed_graphs = test_run_phases_and_collect("fixture_cfg_cu");
        let partition = ParsedCodeGraph::partition_by_selected_roots(parsed_graphs.clone())
            .expect("partition");
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
        let keys = vec![key_with.clone(), key_without.clone()];

        transform_union_crate_and_structural_masks(&db, parsed_graphs.clone(), Some(keys.clone()))
            .expect("transform");

        let structural = build_structural_compilation_unit_slices(parsed_graphs.clone(), &keys)
            .expect("structural slices");
        let expected_counts = structural
            .iter()
            .map(|slice| {
                let partition =
                    ParsedCodeGraph::partition_by_selected_roots(parsed_graphs.clone())
                        .expect("partition");
                let merged = partition
                    .merge_for_root(&slice.key.target_root)
                    .expect("merged");
                let refined = filter_structural_slice_by_cfg(
                    &merged,
                    slice,
                    &ActiveCfg::from_compilation_unit_key(&slice.key),
                );
                (slice.cu_id, refined.enabled_node_ids.len() as i64)
            })
            .collect::<HashMap<_, _>>();

        let rows = db
            .run_script(
                "?[cu_id, node_id] := *compilation_unit_enabled_node{cu_id, node_id}",
                Default::default(),
                ScriptMutability::Immutable,
            )
            .expect("query");
        let mut observed_counts: HashMap<uuid::Uuid, i64> = HashMap::new();
        for row in rows.rows {
            let cu_id = match &row[0] {
                cozo::DataValue::Uuid(v) => v.0,
                other => panic!("expected uuid cu_id, got {other:?}"),
            };
            match &row[1] {
                DataValue::Uuid(_) => {
                    *observed_counts.entry(cu_id).or_insert(0) += 1;
                }
                other => panic!("expected uuid node_id, got {other:?}"),
            };
        }

        assert_eq!(observed_counts, expected_counts);
        assert!(expected_counts.contains_key(&key_with.compilation_unit_id()));
        assert!(expected_counts.contains_key(&key_without.compilation_unit_id()));
    }
}
