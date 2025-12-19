use crate::{get_by_id::COMMON_FIELDS_EMBEDDED, Database, DbError};
use ploke_core::io_types::{EmbeddingData, ResolvedEdgeData};
use std::path::Path;

/// Resolve nodes by canonical module path and item name within a specific file at NOW.
/// Returns EmbeddingData rows suitable for IO/snippet operations.
///
/// - relation: the primary relation/table name (e.g., "function", "struct", etc.)
/// - file_path: absolute file path (string-equal match)
/// - module_path: canonical module path segments, including leading "crate", e.g. ["crate","mod","submod"]
/// - item_name: simple item name at the tail of the canonical path
pub fn graph_resolve_exact(
    db: &Database,
    relation: &str,
    file_path: &Path,
    module_path: &[String],
    item_name: &str,
) -> Result<Vec<EmbeddingData>, DbError> {
    // Escape values safely via serde_json string literals
    let file_path_lit = serde_json::to_string(&file_path.to_string_lossy().to_string())
        .unwrap_or_else(|_| "\"\"".to_string());
    let item_name_lit = serde_json::to_string(&item_name).unwrap_or_else(|_| "\"\"".to_string());
    let mod_path_lit = serde_json::to_string(&module_path).unwrap_or_else(|_| "[]".to_string());

    let script = format!(
        r#"
parent_of[child, parent] := *syntax_edge{{source_id: parent, target_id: child, relation_kind: "Contains" @ 'NOW' }}

ancestor[desc, asc] := parent_of[desc, asc]
ancestor[desc, asc] := parent_of[desc, intermediate], ancestor[intermediate, asc]

module_has_file_mod[mid] := *file_mod{{ owner_id: mid @ 'NOW' }}
file_owner_for_module[mod_id, file_owner_id] := module_has_file_mod[mod_id], file_owner_id = mod_id
file_owner_for_module[mod_id, file_owner_id] := ancestor[mod_id, parent], module_has_file_mod[parent], file_owner_id = parent

?[id, name, file_path, file_hash, hash, span, namespace, mod_path] :=
  *{rel}{{ id, name, tracking_hash: hash, span @ 'NOW' }},
  ancestor[id, mod_id],
  *module{{ id: mod_id, path: mod_path, tracking_hash: file_hash @ 'NOW' }},
  file_owner_for_module[mod_id, file_owner_id],
  *file_mod{{ owner_id: file_owner_id, file_path, namespace @ 'NOW' }},
  name == {item_name_lit},
  file_path == {file_path_lit},
  mod_path == {mod_path_lit}
"#,
        rel = relation,
        item_name_lit = item_name_lit,
        file_path_lit = file_path_lit,
        mod_path_lit = mod_path_lit
    );

    let qr = db.raw_query(&script)?;
    // Map ploke_error::Error into DbError::Cozo for now; we can introduce a dedicated error variant later.
    qr.to_embedding_nodes()
        .map_err(|e| DbError::Cozo(e.to_string()))
}

pub fn graph_resolve_edges(
    db: &Database,
    relation: &str,
    file_path: &Path,
    module_path: &[String],
    item_name: &str,
) -> Result<Vec<ResolvedEdgeData>, DbError> {
    // Escape values safely via serde_json string literals
    let file_path_lit = serde_json::to_string(&file_path.to_string_lossy().to_string())
        .unwrap_or_else(|_| "\"\"".to_string());
    let item_name_lit = serde_json::to_string(&item_name).unwrap_or_else(|_| "\"\"".to_string());
    let mod_path_lit = serde_json::to_string(&module_path).unwrap_or_else(|_| "[]".to_string());

    let common_fields_embedded: &str = COMMON_FIELDS_EMBEDDED.as_ref();

    let script = format!(
        r#"
{common_fields_embedded}

parent_of[child, parent] := *syntax_edge{{source_id: parent, target_id: child, relation_kind: "Contains" @ 'NOW' }}

ancestor[desc, asc] := parent_of[desc, asc]
ancestor[desc, asc] := parent_of[desc, intermediate], ancestor[intermediate, asc]

is_file_module[id, file_path] := *module{{id @ 'NOW'}}, *file_mod {{ owner_id: id, file_path @ 'NOW'}}

containing_file[file_path, target_id] := ancestor[target_id, containing_id],
            is_file_module[containing_id, file_path]

module_has_file_mod[mid] := *file_mod{{ owner_id: mid @ 'NOW' }}
file_owner_for_module[mod_id, file_owner_id] := module_has_file_mod[mod_id], file_owner_id = mod_id
file_owner_for_module[mod_id, file_owner_id] := ancestor[mod_id, parent], module_has_file_mod[parent], file_owner_id = parent

resolve_item[id, name, file_path, file_hash, hash, span, namespace, mod_path] :=
  *{rel}{{ id, name, tracking_hash: hash, span @ 'NOW' }},
  ancestor[id, mod_id],
  *module{{ id: mod_id, path: mod_path, tracking_hash: file_hash @ 'NOW' }},
  file_owner_for_module[mod_id, file_owner_id],
  *file_mod{{ owner_id: file_owner_id, file_path, namespace @ 'NOW' }},
  name == {item_name_lit},
  file_path == {file_path_lit},
  mod_path == {mod_path_lit}

new_data[name, canon_path, file_path, to_find_node_id] := 
    *module{{ id: module_node_id, path: canon_path @ 'NOW' }},
    parent_of[node_id, module_node_id],
    has_embedding[node_id, name, hash, span],
    containing_file[file_path, to_find_node_id]

?[target_name, source_name, source_id, target_id, canon_path, file_path, relation_kind] := 
    resolve_item[source_id, source_name, file_path, file_hash, hash, span, namespace, mod_path],
    *syntax_edge{{source_id, target_id, relation_kind @ 'NOW'}},
    new_data[target_name, canon_path, file_path, to_find_node_id]
"#,
        rel = relation,
        item_name_lit = item_name_lit,
        file_path_lit = file_path_lit,
        mod_path_lit = mod_path_lit
    );

    let qr = db.raw_query(&script)?;
    // Map ploke_error::Error into DbError::Cozo for now; we can introduce a dedicated error variant later.
    qr.to_resolved_edges()
        .map_err(|e| DbError::Cozo(e.to_string()))
}
#[cfg(test)]
mod tests {
    use std::ops::Deref;

    use cozo::{DataValue, UuidWrapper};
    use ploke_common::fixtures_crates_dir;
    use ploke_core::ItemKind;
    use ploke_test_utils::{nodes::ParanoidArgs, test_run_phases_and_collect};
    use syn_parser::utils::LogStyle;
    use tracing::info;

    use crate::{
        log_script, DbError,
        multi_embedding::db_ext::{EmbeddingExt, TEST_DB_IMMUTABLE},
        run_script, Database,
    };

    #[test]
    fn resolve_exact_basic() -> Result<(), DbError> {
        let cozo_db =
            ploke_test_utils::setup_db_full_multi_embedding("fixture_nodes")
                .expect("database must be set up correctly");
        let db = Database::new(cozo_db);

        let fixture_root = fixtures_crates_dir().join("fixture_nodes");
        let file_path = fixture_root.join("src/const_static.rs");
        let module_path = vec!["crate".to_string(), "const_static".to_string()];
        let rows = super::graph_resolve_exact(
            &db,
            "const",
            &file_path,
            &module_path,
            "TOP_LEVEL_BOOL",
        )?;

        assert_eq!(rows.len(), 1, "expected a single const result");
        assert_eq!(rows[0].name, "TOP_LEVEL_BOOL");
        assert_eq!(rows[0].file_path, file_path);
        Ok(())
    }

    #[test]
    fn resolve_edges_basic() -> Result<(), DbError> {
        let cozo_db =
            ploke_test_utils::setup_db_full_multi_embedding("fixture_nodes")
                .expect("database must be set up correctly");
        let db = Database::new(cozo_db);

        let fixture_root = fixtures_crates_dir().join("fixture_nodes");
        let file_path = fixture_root.join("src/const_static.rs");
        let module_path = vec!["crate".to_string(), "const_static".to_string()];
        let res = super::graph_resolve_edges(
            &db,
            "const",
            &file_path,
            &module_path,
            "TOP_LEVEL_BOOL",
        );
        tracing::info!(?res);
        let rows = res?;

        // assert_eq!(rows.len(), 1, "expected a single const result");
        assert_eq!(rows[0].source_name, "TOP_LEVEL_BOOL");
        assert_eq!(rows[0].file_path, file_path);
        Ok(())
    }

    #[test]
    fn graph_resolve_exact_matches_fixture_nodes_struct() -> Result<(), DbError> {
        let cozo_db =
            ploke_test_utils::setup_db_full_multi_embedding("fixture_nodes")
                .expect("database must be set up correctly");
        let db = Database::new(cozo_db);

        let fixture_root = fixtures_crates_dir().join("fixture_nodes");
        let file_path = fixture_root.join("src/structs.rs");
        let module_path = vec!["crate".to_string(), "structs".to_string()];
        let rows = super::graph_resolve_exact(
            &db,
            "struct",
            &file_path,
            &module_path,
            "SampleStruct",
        )?;

        assert_eq!(rows.len(), 1, "expected a single struct result");
        let row = &rows[0];
        assert_eq!(row.name, "SampleStruct");
        assert_eq!(row.file_path, file_path);
        assert!(
            row.start_byte < row.end_byte,
            "byte span should be non-empty"
        );

        // Regenerate the expected ID using the same fixture metadata used by the paranoid tests.
        let parsed = test_run_phases_and_collect("fixture_nodes");
        let args = ParanoidArgs {
            fixture: "fixture_nodes",
            relative_file_path: "src/structs.rs",
            expected_path: &["crate", "structs"],
            ident: "SampleStruct",
            item_kind: ItemKind::Struct,
            expected_cfg: None,
        };
        let test_info = args
            .generate_pid(&parsed)
            .map_err(|e| DbError::Cozo(e.to_string()))?;
        let expected_uuid = match test_info.test_pid().to_cozo_uuid() {
            DataValue::Uuid(UuidWrapper(uuid)) => uuid,
            other => panic!("unexpected id variant {other:?}"),
        };
        assert_eq!(
            row.id, expected_uuid,
            "resolved UUID should match the parser-generated ID"
        );

        Ok(())
    }

    #[test]
    fn resolve_nodes_by_canon_falls_back_when_paths_differ() -> Result<(), DbError> {
        let cozo_db =
            ploke_test_utils::setup_db_full_multi_embedding("fixture_nodes")
                .expect("database must be set up correctly");
        let db = Database::new(cozo_db);

        let fixtures_root = fixtures_crates_dir();
        let canonical_path = fixtures_root
            .join("fixture_nodes")
            .join("src/imports.rs");
        let different_path = fixtures_root
            .join("fixture_nodes_copy")
            .join("src/imports.rs");
        let module_path = vec!["crate".to_string(), "imports".to_string()];
        let item_name = "use_imported_items";

        let strict_hit = super::graph_resolve_exact(
            &db,
            "function",
            &canonical_path,
            &module_path,
            item_name,
        )?;
        assert_eq!(strict_hit.len(), 1, "expected strict resolver to match");

        let strict_miss = super::graph_resolve_exact(
            &db,
            "function",
            &different_path,
            &module_path,
            item_name,
        )?;
        assert!(
            strict_miss.is_empty(),
            "path-sensitive resolver should refuse nodes from other absolute paths"
        );

        let canonical_rows =
            super::resolve_nodes_by_canon(&db, "function", &module_path, item_name)?;
        assert_eq!(canonical_rows.len(), 1, "canonical lookup should find the node");
        assert_eq!(
            canonical_rows[0].id, strict_hit[0].id,
            "canonical lookup should return the same node id"
        );
        assert_eq!(
            canonical_rows[0].file_path, canonical_path,
            "file path should reflect the stored canonical location"
        );

        Ok(())
    }
}

/// Resolve nodes by canonical module path and item name (no file_path equality).
/// This relaxed resolver is intended for diagnostics and as a fallback when
/// absolute file paths differ across environments. It mirrors the projection of
/// `graph_resolve_exact` but omits the `file_path == ...` filter.
pub fn resolve_nodes_by_canon(
    db: &Database,
    relation: &str,
    module_path: &[String],
    item_name: &str,
) -> Result<Vec<EmbeddingData>, DbError> {
    let item_name_lit = serde_json::to_string(&item_name).unwrap_or_else(|_| "\"\"".to_string());
    let mod_path_lit = serde_json::to_string(&module_path).unwrap_or_else(|_| "[]".to_string());

    let script = format!(
        r#"
parent_of[child, parent] := *syntax_edge{{source_id: parent, target_id: child, relation_kind: "Contains" @ 'NOW' }}

ancestor[desc, asc] := parent_of[desc, asc]
ancestor[desc, asc] := parent_of[desc, intermediate], ancestor[intermediate, asc]

module_has_file_mod[mid] := *file_mod{{ owner_id: mid @ 'NOW' }}
file_owner_for_module[mod_id, file_owner_id] := module_has_file_mod[mod_id], file_owner_id = mod_id
file_owner_for_module[mod_id, file_owner_id] := ancestor[mod_id, parent], module_has_file_mod[parent], file_owner_id = parent

?[id, name, file_path, file_hash, hash, span, namespace, mod_path] :=
  *{rel}{{ id, name, tracking_hash: hash, span @ 'NOW' }},
  ancestor[id, mod_id],
  *module{{ id: mod_id, path: mod_path, tracking_hash: file_hash @ 'NOW' }},
  file_owner_for_module[mod_id, file_owner_id],
  *file_mod{{ owner_id: file_owner_id, file_path, namespace @ 'NOW' }},
  name == {item_name_lit},
  mod_path == {mod_path_lit}
"#,
        rel = relation,
        item_name_lit = item_name_lit,
        mod_path_lit = mod_path_lit
    );

    let qr = db.raw_query(&script)?;
    qr.to_embedding_nodes()
        .map_err(|e| DbError::Cozo(e.to_string()))
}
