use crate::{Database, DbError};
use ploke_core::io_types::EmbeddingData;
use std::path::Path;

/// Resolve nodes by canonical module path and item name within a specific file at NOW.
/// Returns EmbeddingData rows suitable for IO/snippet operations.
///
/// - relation: the primary relation/table name (e.g., "function", "struct", etc.)
/// - file_path: absolute file path (string-equal match)
/// - module_path: canonical module path segments, including leading "crate", e.g. ["crate","mod","submod"]
/// - item_name: simple item name at the tail of the canonical path
pub fn resolve_nodes_by_canon_in_file(
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

/// Resolve nodes by canonical module path and item name (no file_path equality).
/// This relaxed resolver is intended for diagnostics and as a fallback when
/// absolute file paths differ across environments. It mirrors the projection of
/// `resolve_nodes_by_canon_in_file` but omits the `file_path == ...` filter.
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
