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

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{
    discovery::{TargetKind, TargetSpec},
    error::SynParserError,
    parser::{ParsedCodeGraph, graph::GraphAccess, graph::ParsedGraphError},
};

/// Namespace UUID used for deterministic [`CompilationUnitKey::compilation_unit_id`] (UUID v5).
pub const COMPILATION_UNIT_ID_NAMESPACE: Uuid = uuid::uuid!("a1b2c3d4-e5f6-5a7b-8c9d-0e1f2a3b4c5d");

/// Stable identity for a compilation unit: crate namespace, Cargo target, triple, profile, features.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct CompilationUnitKey {
    pub namespace: Uuid,
    pub target_kind: TargetKind,
    pub target_name: String,
    pub target_root: PathBuf,
    pub target_triple: String,
    pub profile: String,
    pub features: Vec<String>,
}

/// Sort, dedupe, and normalize a feature set for stable keys and hashing.
pub fn normalize_features(features: impl IntoIterator<Item = String>) -> Vec<String> {
    let mut v: Vec<String> = features.into_iter().collect();
    v.sort();
    v.dedup();
    v
}

/// Deterministic UUID derived from the normalized feature list (UUID v5).
pub fn features_hash_uuid(features: &[String]) -> Uuid {
    let joined = features.join("\0");
    Uuid::new_v5(&COMPILATION_UNIT_ID_NAMESPACE, joined.as_bytes())
}

impl CompilationUnitKey {
    /// Returns normalized features (sorted, deduped).
    pub fn normalized_features(&self) -> Vec<String> {
        normalize_features(self.features.iter().cloned())
    }

    /// Hash of the feature list only (for storage and debugging).
    pub fn features_hash(&self) -> Uuid {
        features_hash_uuid(&self.normalized_features())
    }

    /// Deterministic compilation-unit identifier (UUID v5 over the full canonical key).
    pub fn compilation_unit_id(&self) -> Uuid {
        Uuid::new_v5(&COMPILATION_UNIT_ID_NAMESPACE, &self.canonical_bytes())
    }

    fn canonical_bytes(&self) -> Vec<u8> {
        #[derive(serde::Serialize)]
        struct Canon<'a> {
            namespace: Uuid,
            target_kind: &'a str,
            target_name: &'a str,
            target_root: String,
            target_triple: &'a str,
            profile: &'a str,
            features: Vec<String>,
        }
        let kind = match self.target_kind {
            TargetKind::Lib => "lib",
            TargetKind::Bin => "bin",
            TargetKind::Test => "test",
            TargetKind::Example => "example",
            TargetKind::Bench => "bench",
        };
        let canon = Canon {
            namespace: self.namespace,
            target_kind: kind,
            target_name: &self.target_name,
            target_root: self.target_root.to_string_lossy().into_owned(),
            target_triple: &self.target_triple,
            profile: &self.profile,
            features: self.normalized_features(),
        };
        serde_json::to_vec(&canon).expect("canonical CompilationUnitKey serialization")
    }
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
    targets
        .iter()
        .map(|t| {
            compilation_unit_key_from_target(
                namespace,
                t.kind.clone(),
                t.name.clone(),
                t.root.clone(),
                target_triple.clone(),
                profile.clone(),
                features.clone(),
            )
        })
        .collect()
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
    CompilationUnitKey {
        namespace,
        target_kind,
        target_name,
        target_root,
        target_triple,
        profile,
        features: normalize_features(features),
    }
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
        let k1 = CompilationUnitKey {
            namespace: Uuid::nil(),
            target_kind: TargetKind::Lib,
            target_name: "foo".into(),
            target_root: PathBuf::from("/tmp/lib.rs"),
            target_triple: "x86_64-unknown-linux-gnu".into(),
            profile: "dev".into(),
            features: vec!["b".into(), "a".into()],
        };
        let k2 = CompilationUnitKey {
            namespace: Uuid::nil(),
            target_kind: TargetKind::Lib,
            target_name: "foo".into(),
            target_root: PathBuf::from("/tmp/lib.rs"),
            target_triple: "x86_64-unknown-linux-gnu".into(),
            profile: "dev".into(),
            features: vec!["a".into(), "b".into()],
        };
        assert_eq!(k1.compilation_unit_id(), k2.compilation_unit_id());
    }
}
