//! Cozo relations for compilation-unit identity and structural masks.

use super::*;

use crate::define_schema;

define_schema!(CompilationUnitSchema {
    "compilation_unit",
    id: "Uuid",
    namespace: "Uuid",
    target_kind: "String",
    target_name: "String",
    target_root: "String",
    target_triple: "String",
    profile: "String",
    features: "[String]",
    features_hash: "Uuid",
});

define_schema!(CompilationUnitEnabledNodeSchema {
    "compilation_unit_enabled_node",
    cu_id: "Uuid",
    node_id: "Uuid",
});

define_schema!(CompilationUnitEnabledEdgeSchema {
    "compilation_unit_enabled_edge",
    id: "Uuid",
    cu_id: "Uuid",
    source_id: "Uuid",
    target_id: "Uuid",
    relation_kind: "String",
});

define_schema!(CompilationUnitEnabledFileSchema {
    "compilation_unit_enabled_file",
    id: "Uuid",
    cu_id: "Uuid",
    file_path: "String",
});

define_schema!(CompilationUnitMetaSchema {
    "compilation_unit_meta",
    cu_id: "Uuid",
    enabled_node_count: "Int",
    enabled_edge_count: "Int",
    enabled_file_count: "Int",
});
