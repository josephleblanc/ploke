//! Compilation-unit keys and **structural** slices (module-tree reachability per Cargo target root).
//!
//! Cfg evaluation and dependency-aware slices are planned follow-ups; see
//! `docs/.../compilation-unit-slices-and-db-masks_874f1391.plan.md`.

#[cfg(feature = "cfg_eval")]
mod cfg_filter;
mod collect;

#[cfg(feature = "cfg_eval")]
pub use cfg_filter::filter_structural_slice_by_cfg;
pub use collect::collect_all_node_uuids_in_graph;

use std::{collections::HashSet, path::PathBuf};

use uuid::Uuid;

use crate::{
    discovery::{TargetKind, TargetSpec},
    error::SynParserError,
    parser::{ParsedCodeGraph, graph::GraphAccess, graph::ParsedGraphError},
};

pub use ploke_core::{
    COMPILATION_UNIT_ID_NAMESPACE, CompilationUnitKey, CompilationUnitTargetKind,
    features_hash_uuid, normalize_features,
};

/// User-requested compilation-unit dimensions to cross with discovered targets.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompilationUnitDimensionRequest {
    /// Target triples to materialize.
    pub target_triples: Vec<String>,
    /// Cargo profiles to materialize.
    pub profiles: Vec<String>,
    /// Requested feature sets; each entry is one independent feature set.
    pub feature_sets: Vec<Vec<String>>,
}

impl CompilationUnitDimensionRequest {
    /// Baseline request matching current default behavior.
    pub fn baseline_default() -> Self {
        Self {
            target_triples: vec![default_target_triple()],
            profiles: vec!["dev".to_string()],
            feature_sets: vec![Vec::new()],
        }
    }

    /// Build dimensions from process env vars.
    ///
    /// Supported vars:
    /// - `PLOKE_CU_TARGET_TRIPLES`: comma/space-separated triples
    /// - `PLOKE_CU_PROFILES`: comma/space-separated profiles
    /// - `PLOKE_CU_FEATURE_SETS`: `;`-separated sets, each set comma/space-separated
    pub fn from_env_or_default() -> Self {
        let triples = std::env::var("PLOKE_CU_TARGET_TRIPLES")
            .ok()
            .map(|s| split_tokens(&s))
            .unwrap_or_default();
        let profiles = std::env::var("PLOKE_CU_PROFILES")
            .ok()
            .map(|s| split_tokens(&s))
            .unwrap_or_default();
        let feature_sets = std::env::var("PLOKE_CU_FEATURE_SETS")
            .ok()
            .map(|s| parse_feature_sets(&s))
            .unwrap_or_else(|| {
                vec![
                    std::env::var("PLOKE_CU_FEATURES")
                        .ok()
                        .map(|s| split_tokens(&s))
                        .unwrap_or_default(),
                ]
            });
        Self {
            target_triples: if triples.is_empty() {
                vec![default_target_triple()]
            } else {
                triples
            },
            profiles: if profiles.is_empty() {
                vec!["dev".to_string()]
            } else {
                profiles
            },
            feature_sets: if feature_sets.is_empty() {
                vec![Vec::new()]
            } else {
                feature_sets
            },
        }
        .normalized()
    }

    /// Normalize and dedupe all request dimensions.
    pub fn normalized(mut self) -> Self {
        self.target_triples.sort();
        self.target_triples.dedup();
        self.profiles.sort();
        self.profiles.dedup();
        self.feature_sets = self
            .feature_sets
            .into_iter()
            .map(normalize_features)
            .collect::<Vec<_>>();
        self.feature_sets.sort();
        self.feature_sets.dedup();
        self
    }
}

fn split_tokens(raw: &str) -> Vec<String> {
    raw.split(|c| c == ',' || c == ' ')
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_string)
        .collect()
}

fn parse_feature_sets(raw: &str) -> Vec<Vec<String>> {
    raw.split(';')
        .map(str::trim)
        .filter(|segment| !segment.is_empty())
        .map(split_tokens)
        .collect()
}

/// One enabled syntactic edge, matching the `syntax_edge` key shape for joins.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct EnabledSyntacticEdge {
    pub source_id: Uuid,
    pub target_id: Uuid,
    pub relation_kind: String,
}

/// Structural membership for one compilation unit after module-tree prune for the target root.
#[derive(Debug, Clone)]
pub struct StructuralCompilationUnitSlice {
    pub key: CompilationUnitKey,
    pub cu_id: Uuid,
    pub enabled_node_ids: HashSet<Uuid>,
    pub enabled_edges: Vec<EnabledSyntacticEdge>,
    pub enabled_file_paths: HashSet<PathBuf>,
}

/// Build a structural slice: partition → merge for target root → build tree → prune → collect.
pub fn build_structural_compilation_unit_slice(
    parsed_graphs: Vec<ParsedCodeGraph>,
    key: &CompilationUnitKey,
) -> Result<StructuralCompilationUnitSlice, SynParserError> {
    if parsed_graphs.is_empty() {
        return Err(SynParserError::MergeRequiresInput);
    }

    let partition = ParsedCodeGraph::partition_by_selected_roots(parsed_graphs)?;
    if !partition
        .selected_root_paths
        .iter()
        .any(|p| p == &key.target_root)
    {
        return Err(SynParserError::ParsedGraphError(
            ParsedGraphError::RootFileNotFound(key.target_root.clone()),
        ));
    }

    let mut merged = partition.merge_for_root(&key.target_root)?;
    if merged.crate_namespace != key.namespace {
        return Err(SynParserError::InternalState(format!(
            "CompilationUnitKey namespace {} does not match merged graph namespace {}",
            key.namespace, merged.crate_namespace
        )));
    }

    let _tree = merged
        .build_tree_and_prune_for_root_path(&key.target_root)
        .map_err(|e| {
            SynParserError::InternalState(format!(
                "Failed to build module tree for compilation unit: {e}"
            ))
        })?;

    let enabled_node_ids = collect_all_node_uuids_in_graph(&merged);
    let enabled_edges = collect_enabled_edges(&merged, &enabled_node_ids);
    let enabled_file_paths = collect_enabled_file_paths(&merged);

    Ok(StructuralCompilationUnitSlice {
        cu_id: key.compilation_unit_id(),
        key: key.clone(),
        enabled_node_ids,
        enabled_edges,
        enabled_file_paths,
    })
}

fn collect_enabled_edges(
    graph: &ParsedCodeGraph,
    nodes: &HashSet<Uuid>,
) -> Vec<EnabledSyntacticEdge> {
    graph
        .relations()
        .iter()
        .filter_map(|rel| {
            let s = rel.source().uuid();
            let t = rel.target().uuid();
            if nodes.contains(&s) && nodes.contains(&t) {
                Some(EnabledSyntacticEdge {
                    source_id: s,
                    target_id: t,
                    relation_kind: rel.kind_str().to_string(),
                })
            } else {
                None
            }
        })
        .collect()
}

fn collect_enabled_file_paths(graph: &ParsedCodeGraph) -> HashSet<PathBuf> {
    let mut out = HashSet::new();
    for m in graph.modules() {
        if let Some(p) = m.file_path() {
            out.insert(p.to_path_buf());
        }
    }
    out
}

/// One [`CompilationUnitKey`] per discovered Cargo target, sharing triple/profile/features.
pub fn compilation_unit_keys_for_targets(
    namespace: Uuid,
    targets: &[TargetSpec],
    target_triple: String,
    profile: String,
    features: Vec<String>,
) -> Vec<CompilationUnitKey> {
    enumerate_compilation_unit_keys(
        namespace,
        targets,
        &CompilationUnitDimensionRequest {
            target_triples: vec![target_triple],
            profiles: vec![profile],
            feature_sets: vec![features],
        },
    )
}

/// Enumerate all compilation-unit keys from discovered targets and requested dimensions.
pub fn enumerate_compilation_unit_keys(
    namespace: Uuid,
    targets: &[TargetSpec],
    dimensions: &CompilationUnitDimensionRequest,
) -> Vec<CompilationUnitKey> {
    let dims = dimensions.clone().normalized();
    let mut keys = Vec::new();
    for target in targets {
        for triple in &dims.target_triples {
            for profile in &dims.profiles {
                for features in &dims.feature_sets {
                    keys.push(compilation_unit_key_from_target(
                        namespace,
                        target.kind.clone(),
                        target.name.clone(),
                        target.root.clone(),
                        triple.clone(),
                        profile.clone(),
                        features.clone(),
                    ));
                }
            }
        }
    }
    keys
}

/// Convenience: build a [`CompilationUnitKey`] from discovery + environment defaults.
pub fn compilation_unit_key_from_target(
    namespace: Uuid,
    target_kind: TargetKind,
    target_name: String,
    target_root: PathBuf,
    target_triple: String,
    profile: String,
    features: Vec<String>,
) -> CompilationUnitKey {
    let target_kind = match target_kind {
        TargetKind::Lib => CompilationUnitTargetKind::Lib,
        TargetKind::Bin => CompilationUnitTargetKind::Bin,
        TargetKind::Test => CompilationUnitTargetKind::Test,
        TargetKind::Example => CompilationUnitTargetKind::Example,
        TargetKind::Bench => CompilationUnitTargetKind::Bench,
    };
    CompilationUnitKey::new(
        namespace,
        target_kind,
        target_name,
        target_root,
        target_triple,
        profile,
        normalize_features(features),
    )
}

/// Default target triple for the host (matches `cargo` / `rustc` `TARGET` when set).
pub fn default_target_triple() -> String {
    std::env::var("TARGET").unwrap_or_else(|_| "x86_64-unknown-linux-gnu".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalize_features_sorts_and_dedupes() {
        let v = normalize_features(["b".into(), "a".into(), "a".into()]);
        assert_eq!(v, vec!["a", "b"]);
    }

    #[test]
    fn compilation_unit_id_is_deterministic() {
        let k1 = CompilationUnitKey::new(
            Uuid::nil(),
            CompilationUnitTargetKind::Lib,
            "foo".into(),
            PathBuf::from("/tmp/lib.rs"),
            "x86_64-unknown-linux-gnu".into(),
            "dev".into(),
            vec!["b".into(), "a".into()],
        );
        let k2 = CompilationUnitKey::new(
            Uuid::nil(),
            CompilationUnitTargetKind::Lib,
            "foo".into(),
            PathBuf::from("/tmp/lib.rs"),
            "x86_64-unknown-linux-gnu".into(),
            "dev".into(),
            vec!["a".into(), "b".into()],
        );
        assert_eq!(k1.compilation_unit_id(), k2.compilation_unit_id());
    }

    #[test]
    fn enumerate_compilation_unit_keys_cross_products_dimensions() {
        let targets = vec![TargetSpec {
            kind: TargetKind::Lib,
            name: "demo".to_string(),
            root: PathBuf::from("src/lib.rs"),
        }];
        let dims = CompilationUnitDimensionRequest {
            target_triples: vec![
                "x86_64-unknown-linux-gnu".into(),
                "aarch64-apple-darwin".into(),
            ],
            profiles: vec!["dev".into(), "release".into()],
            feature_sets: vec![vec![], vec!["serde".into()]],
        };
        let keys = enumerate_compilation_unit_keys(Uuid::nil(), &targets, &dims);
        assert_eq!(keys.len(), 8);
    }

    #[test]
    fn feature_set_env_supports_multiple_sets() {
        let sets = parse_feature_sets("serde,simd; tokio ;");
        assert_eq!(
            sets,
            vec![
                vec!["serde".to_string(), "simd".to_string()],
                vec!["tokio".to_string()]
            ]
        );
    }
}
