//! Union crate ingest: one merged graph + transform, then structural CU masks per target.

use cozo::{Db, MemStorage};
use syn_parser::compilation_unit::{
    build_structural_compilation_unit_slice, compilation_unit_keys_for_targets,
    default_target_triple,
};
use syn_parser::ParsedCodeGraph;

use crate::error::TransformError;
use crate::transform::transform_parsed_graph;

use super::insert_structural_compilation_unit_slice;

/// Merge all Cargo target roots, [`ParsedCodeGraph::build_tree_and_prune`], [`transform_parsed_graph`]
/// once, then persist one structural [`syn_parser::compilation_unit::StructuralCompilationUnitSlice`]
/// per discovered target (same triple/profile/features baseline).
pub fn transform_union_crate_and_structural_masks(
    db: &Db<MemStorage>,
    parsed_graphs: Vec<ParsedCodeGraph>,
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

    let keys = compilation_unit_keys_for_targets(
        namespace,
        &targets,
        default_target_triple(),
        "dev".to_string(),
        std::env::var("PLOKE_CU_FEATURES")
            .ok()
            .map(|s| {
                s.split(|c| c == ',' || c == ' ')
                    .filter(|t| !t.is_empty())
                    .map(str::to_string)
                    .collect()
            })
            .unwrap_or_default(),
    );
    for key in &keys {
        let slice = build_structural_compilation_unit_slice(parsed_for_masks.clone(), key)
            .map_err(|e| TransformError::Transformation(e.to_string()))?;
        insert_structural_compilation_unit_slice(db, &slice)?;
    }
    Ok(())
}
