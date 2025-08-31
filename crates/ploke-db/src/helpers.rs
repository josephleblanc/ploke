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

?[id, name, file_path, file_hash, hash, span, namespace, mod_path] :=
  *{rel}{{ id, name, tracking_hash: hash, span @ 'NOW' }},
  ancestor[id, mod_id],
  *module{{ id: mod_id, path: mod_path, tracking_hash: file_hash @ 'NOW' }},
  *file_mod{{ owner_id: mod_id, file_path, namespace @ 'NOW' }},
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

?[id, name, file_path, file_hash, hash, span, namespace, mod_path] :=
  *{rel}{{ id, name, tracking_hash: hash, span @ 'NOW' }},
  ancestor[id, mod_id],
  *module{{ id: mod_id, path: mod_path, tracking_hash: file_hash @ 'NOW' }},
  *file_mod{{ owner_id: mod_id, file_path, namespace @ 'NOW' }},
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::get_by_id::{GetNodeInfo, NodePaths};
    use crate::utils::test_utils::TEST_DB_NODES;
    use crate::{database::{to_string, to_uuid}, result::get_pos, Database, NodeType, QueryResult};
    use std::fs;
    use std::path::PathBuf;

    #[test]
    fn test_resolve_nodes_by_canon_in_file_via_paths_from_id() {
        // Acquire test DB (fixture_nodes) loaded by the test framework
        let db_arc = TEST_DB_NODES
            .clone()
            .expect("problem loading fixture_nodes from cold start");
        let db = db_arc
            .lock()
            .expect("problem getting lock on test db for fixture_nodes");

        // Gather a small set of embedded nodes as our seed
        let qr = db.get_common_nodes().expect("get_common_nodes");
        let id_index: usize = get_pos(&qr.headers, "id").expect("id header");
        let name_index: usize = get_pos(&qr.headers, "name").expect("name header");
        // Limit to a handful for speed
        let sample_rows = qr.rows.into_iter().take(8).collect::<Vec<_>>();

        // Prepare artifact file
        let out_dir = PathBuf::from("tests/ai_temp_data");
        let _ = fs::create_dir_all(&out_dir);
        let out_path = out_dir.join("test-output.txt");
        let mut log = String::new();

        for row in sample_rows {
            let id = to_uuid(&row[id_index]).expect("id uuid");
            let name = to_string(&row[name_index]).expect("name str");
            // Fetch canon + file via id-anchored path resolver
            let paths_rows = db.paths_from_id(id).expect("paths_from_id");
            let node_paths: NodePaths = paths_rows.try_into().expect("NodePaths");
            log.push_str(&format!(
                "Checking id={} name={}\n  canon={}\n  file={}\n",
                id,
                name,
                node_paths.canon,
                node_paths.file
            ));

            // Split canon into module path and item name
            let canon = node_paths.canon;
            let last_sep = canon.rfind("::").expect("canon has module path");
            let module_str = &canon[..last_sep];
            let item_name = &canon[last_sep + 2..];
            let module_vec: Vec<String> = module_str
                .split("::")
                .map(|s| s.to_string())
                .collect();

            // Try strict helper for each primary relation; expect exactly one to match by id
            let mut matched = false;
            for ty in NodeType::primary_nodes() {
                let rel = ty.relation_str();
                let res = resolve_nodes_by_canon_in_file(
                    &db,
                    rel,
                    PathBuf::from(&node_paths.file).as_path(),
                    &module_vec,
                    item_name,
                );
                match res {
                    Ok(v) if !v.is_empty() => {
                        let any_id = v.iter().any(|ed| ed.id == id);
                        log.push_str(&format!(
                            "  relation={} returned {} row(s); contains id match = {}\n",
                            rel,
                            v.len(),
                            any_id
                        ));
                        if any_id {
                            matched = true;
                            break;
                        }
                    }
                    Ok(_) => {
                        log.push_str(&format!("  relation={} returned 0 rows\n", rel));
                    }
                    Err(e) => {
                        log.push_str(&format!("  relation={} error: {}\n", rel, e));
                    }
                }
            }

            // If strict matching failed, run relaxed diagnostics to surface module-only candidates
            if !matched {
                log.push_str("  strict match failed; running relaxed module-only diagnostics\n");
                for ty in NodeType::primary_nodes() {
                    let rel = ty.relation_str();
                    match resolve_nodes_by_canon(&db, rel, &module_vec, item_name) {
                        Ok(cands) => {
                            let files: Vec<String> = cands
                                .iter()
                                .map(|ed| ed.file_path.to_string_lossy().to_string())
                                .collect();
                            let any_id = cands.iter().any(|ed| ed.id == id);
                            log.push_str(&format!(
                                "  relaxed relation={} candidates={} any_id_match={} files={:?}\n",
                                rel,
                                cands.len(),
                                any_id,
                                files
                            ));
                        }
                        Err(e) => {
                            log.push_str(&format!(
                                "  relaxed relation={} error: {}\n",
                                rel, e
                            ));
                        }
                    }
                }
            }

            // Persist progress before assertion so artifacts exist on failure
            let out_dir = PathBuf::from("tests/ai_temp_data");
            let _ = fs::create_dir_all(&out_dir);
            let out_path = out_dir.join("test-output.txt");
            let _ = fs::write(&out_path, &log);
            assert!(matched, "resolve_nodes_by_canon_in_file failed to resolve id={} canon={} file={}", id, canon, node_paths.file);
        }
        fs::write(out_path, log).expect("write artifact");
    }
}
