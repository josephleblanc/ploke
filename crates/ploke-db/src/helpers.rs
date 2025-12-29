use crate::{
    database::to_string,
    get_by_id::COMMON_FIELDS_EMBEDDED,
    result::{get_pos, typed_rows::ResolvedEdgeData},
    Database, DbError, NodeType,
};
use ploke_core::io_types::EmbeddingData;
use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
};

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

ancestor[desc, desc] := *module{{ id: desc @ 'NOW' }}
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

/// This function is intended to be a helper that assists with a tool call to find all the edges
/// leading to or from a target item, as specified by their node kind, file path, module path, and
/// item name.
///
/// Returns all the info needed for either the LLM to learn more about the code item, or for
/// chained tool calls to use the target_id or source_id for other traversal methods.
//
// Because we have the relations in the database as see in `ploke-embed` schema, there are several
// relations we can show here, c.f. ploke/crates/ingest/ploke-transform/src/schema/edges.rs
//
// However, our first pass will be more constrained, since we only want to display the edges that
// lead to or from embedded nodes, which means all the nodes in `NodeType::primary_nodes()`, c.f.
// ploke/crates/ploke-db/src/query/builder.rs
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

ancestor[desc, desc] := *module{{ id: desc @ 'NOW' }}
ancestor[desc, asc] := parent_of[desc, asc]
ancestor[desc, asc] := parent_of[desc, intermediate], ancestor[intermediate, asc]

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

node_with_context[id, name, canon_path, file_path] :=
  parent_of[id, mod_id],
  *module{{ id: mod_id, path: canon_path @ 'NOW' }},
  file_owner_for_module[mod_id, file_owner_id],
  *file_mod{{ owner_id: file_owner_id, file_path @ 'NOW' }},
  has_embedding[id, name, hash, span]

edges_from_focus[source_name, target_name, source_id, target_id, canon_path, file_path, relation_kind] :=
  resolve_item[source_id, source_name, focus_file_path, focus_file_hash, focus_hash, focus_span, focus_namespace, focus_mod_path],
  *syntax_edge{{source_id, target_id, relation_kind @ 'NOW'}},
  node_with_context[target_id, target_name, canon_path, file_path]

edges_to_focus[source_name, target_name, source_id, target_id, canon_path, file_path, relation_kind] :=
  resolve_item[source_id, source_name, focus_file_path, focus_file_hash, focus_hash, focus_span, focus_namespace, focus_mod_path],
  *syntax_edge{{source_id: other_id, target_id: source_id, relation_kind @ 'NOW'}},
  node_with_context[other_id, target_name, canon_path, file_path],
  target_id = other_id

?[source_name, target_name, source_id, target_id, canon_path, file_path, relation_kind] :=
  edges_from_focus[source_name, target_name, source_id, target_id, canon_path, file_path, relation_kind]
?[source_name, target_name, source_id, target_id, canon_path, file_path, relation_kind] :=
  edges_to_focus[source_name, target_name, source_id, target_id, canon_path, file_path, relation_kind]
"#,
        rel = relation,
        item_name_lit = item_name_lit,
        file_path_lit = file_path_lit,
        mod_path_lit = mod_path_lit
    );

    let qr = db.raw_query(&script)?;
    tracing::debug!(
        headers = ?qr.headers,
        rows_len = qr.rows.len(),
        "graph_resolve_edges query result"
    );
    // Map ploke_error::Error into DbError::Cozo for now; we can introduce a dedicated error variant later.
    qr.to_resolved_edges()
        .map_err(|e| DbError::Cozo(e.to_string()))
}

/// Basic metadata needed to invoke `graph_resolve_edges` for a node.
#[derive(Debug, Clone)]
pub struct PrimaryNodeRow {
    pub relation: String,
    pub name: String,
    pub file_path: PathBuf,
    pub module_path: Vec<String>,
}

/// Enumerate all primary nodes that have embeddings, capturing enough context to call
/// `graph_resolve_edges`.
pub fn list_primary_nodes(db: &Database) -> Result<Vec<PrimaryNodeRow>, DbError> {
    let common_fields_embedded: &str = COMMON_FIELDS_EMBEDDED.as_ref();
    let relation_rules = NodeType::primary_nodes()
        .iter()
        .map(|nt| {
            let rel = nt.relation_str();
            format!(
                r#"
rel_{rel}[relation, name, file_path, mod_path] :=
  has_embedding[id, name, hash, span],
  ancestor[id, mod_id],
  *module{{ id: mod_id, path: mod_path, tracking_hash: file_hash @ 'NOW' }},
  file_owner_for_module[mod_id, file_owner_id],
  *file_mod{{ owner_id: file_owner_id, file_path @ 'NOW' }},
  relation = "{rel}"
"#,
                rel = rel
            )
        })
        .collect::<Vec<_>>()
        .join("\n");

    let unions = NodeType::primary_nodes()
        .iter()
        .map(|nt| {
            let rel = nt.relation_str();
            format!(
                "?[relation, name, file_path, mod_path] := rel_{rel}[relation, name, file_path, mod_path]"
            )
        })
        .collect::<Vec<_>>()
        .join("\n");

    let script = format!(
        r#"
{common_fields_embedded}

parent_of[child, parent] := *syntax_edge{{source_id: parent, target_id: child, relation_kind: "Contains" @ 'NOW' }}

ancestor[desc, desc] := *module{{ id: desc @ 'NOW' }}
ancestor[desc, asc] := parent_of[desc, asc]
ancestor[desc, asc] := parent_of[desc, intermediate], ancestor[intermediate, asc]

module_has_file_mod[mid] := *file_mod{{ owner_id: mid @ 'NOW' }}
file_owner_for_module[mod_id, file_owner_id] := module_has_file_mod[mod_id], file_owner_id = mod_id
file_owner_for_module[mod_id, file_owner_id] := ancestor[mod_id, parent], module_has_file_mod[parent], file_owner_id = parent

{relation_rules}

{unions}
"#,
        common_fields_embedded = common_fields_embedded,
        relation_rules = relation_rules,
        unions = unions
    );

    let qr = db.raw_query(&script)?;
    let relation_idx = get_pos(&qr.headers, "relation")?;
    let name_idx = get_pos(&qr.headers, "name")?;
    let file_idx = get_pos(&qr.headers, "file_path")?;
    let mod_idx = get_pos(&qr.headers, "mod_path")?;

    let rows = qr
        .rows
        .into_iter()
        .map(|row| {
            let relation = to_string(&row[relation_idx])?;
            let name = to_string(&row[name_idx])?;
            let file_path = PathBuf::from(to_string(&row[file_idx])?);
            let module_path = row[mod_idx]
                .get_slice()
                .unwrap_or(&[])
                .iter()
                .filter_map(|v| v.get_str().map(str::to_owned))
                .collect::<Vec<_>>();
            Ok(PrimaryNodeRow {
                relation,
                name,
                file_path,
                module_path,
            })
        })
        .collect::<Result<Vec<_>, DbError>>()?;

    Ok(rows)
}

/// Run `graph_resolve_edges` for every primary node, returning successes and failures for inspection.
pub fn resolve_edges_for_all_primary(
    db: &Database,
) -> Vec<(PrimaryNodeRow, Result<Vec<ResolvedEdgeData>, DbError>)> {
    match list_primary_nodes(db) {
        Ok(nodes) => nodes
            .into_iter()
            .map(|row| {
                let res = graph_resolve_edges(
                    db,
                    &row.relation,
                    row.file_path.as_path(),
                    &row.module_path,
                    &row.name,
                );
                (row, res)
            })
            .collect(),
        Err(err) => vec![(
            PrimaryNodeRow {
                relation: "<error>".into(),
                name: "<error>".into(),
                file_path: PathBuf::new(),
                module_path: Vec::new(),
            },
            Err(err),
        )],
    }
}

/// Count syntax edges of a given relation_kind.
pub fn count_edges_by_kind(db: &Database, relation_kind: &str) -> Result<usize, DbError> {
    let rel_lit = serde_json::to_string(&relation_kind).unwrap_or_else(|_| "\"\"".to_string());
    let script = format!(
        r#"
?[count(relation_kind)] :=
    *syntax_edge{{relation_kind @ 'NOW'}},
    relation_kind == {rel_lit}
"#
    );
    let qr = db
        .run_script(&script, BTreeMap::new(), cozo::ScriptMutability::Immutable)
        .map_err(|e| DbError::Cozo(e.to_string()))?;
    Database::into_usize(qr)
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

#[cfg(test)]
mod tests {
    use std::{collections::VecDeque, ops::Deref};

    use cozo::{DataValue, UuidWrapper};
    use itertools::Itertools;
    use ploke_common::fixtures_crates_dir;
    use ploke_core::ItemKind;
    use ploke_test_utils::{nodes::ParanoidArgs, test_run_phases_and_collect};
    use std::collections::HashMap;
    use syn_parser::{parser::relations::SyntacticRelation, utils::LogStyle};
    use tracing::{info, level_filters::LevelFilter, Level};
    use tracing_subscriber::{layer::SubscriberExt as _, util::SubscriberInitExt};

    use super::PrimaryNodeRow;
    use crate::{
        helpers::count_edges_by_kind,
        log_script,
        multi_embedding::{
            db_ext::{EmbeddingExt, TEST_DB_IMMUTABLE},
            hnsw_ext::init_tracing_once,
        },
        result::typed_rows::ResolvedEdgeData,
        run_script, Database, DbError,
    };

    #[test]
    fn resolve_exact_basic() -> Result<(), DbError> {
        let cozo_db = ploke_test_utils::setup_db_full_multi_embedding("fixture_nodes")
            .expect("database must be set up correctly");
        let db = Database::new(cozo_db);

        let fixture_root = fixtures_crates_dir().join("fixture_nodes");
        let file_path = fixture_root.join("src/const_static.rs");
        let module_path = vec!["crate".to_string(), "const_static".to_string()];
        let rows =
            super::graph_resolve_exact(&db, "const", &file_path, &module_path, "TOP_LEVEL_BOOL")?;

        assert_eq!(rows.len(), 1, "expected a single const result");
        assert_eq!(rows[0].name, "TOP_LEVEL_BOOL");
        assert_eq!(rows[0].file_path, file_path);
        Ok(())
    }

    #[test]
    #[ignore = "possibly bugged, revisit soon 2025-12-29"]
    fn resolve_edges_basic() -> Result<(), DbError> {
        let cozo_db = ploke_test_utils::setup_db_full_multi_embedding("fixture_nodes")
            .expect("database must be set up correctly");
        let db = Database::new(cozo_db);

        let fixture_root = fixtures_crates_dir().join("fixture_nodes");
        let file_path = fixture_root.join("src/const_static.rs");
        let module_path = vec!["crate".to_string(), "const_static".to_string()];
        let rows =
            super::graph_resolve_edges(&db, "const", &file_path, &module_path, "TOP_LEVEL_BOOL")?;

        assert!(
            !rows.is_empty(),
            "graph_resolve_edges should return at least one edge"
        );
        assert_eq!(rows[0].source_name, "TOP_LEVEL_BOOL");
        assert_eq!(rows[0].file_path, file_path);
        Ok(())
    }

    #[test]
    fn resolve_edges_module() -> Result<(), DbError> {
        let cozo_db = ploke_test_utils::setup_db_full_multi_embedding("fixture_nodes")
            .expect("database must be set up correctly");
        let db = Database::new(cozo_db);

        let fixture_root = fixtures_crates_dir().join("fixture_nodes");
        let file_path = fixture_root.join("src/const_static.rs");
        let module_path = vec!["crate".to_string(), "const_static".to_string()];
        let resolved =
            super::graph_resolve_exact(&db, "module", &file_path, &module_path, "const_static")?;
        assert!(
            !resolved.is_empty(),
            "expected to resolve the module node for const_static.rs"
        );
        let rows =
            super::graph_resolve_edges(&db, "module", &file_path, &module_path, "const_static")?;

        assert!(
            !rows.is_empty(),
            "graph_resolve_edges should return at least one edge"
        );
        assert_eq!(rows[0].source_name, "const_static");
        assert_eq!(rows[0].file_path, file_path);
        Ok(())
    }

    #[test]
    fn resolve_edges_all_primary_smoke() -> Result<(), DbError> {
        let cozo_db = ploke_test_utils::setup_db_full_multi_embedding("fixture_nodes")
            .expect("database must be set up correctly");
        let db = Database::new(cozo_db);

        let results = super::resolve_edges_for_all_primary(&db);
        let total = results.len();
        let successes = results.iter().filter(|(_, r)| r.is_ok()).count();
        let successes_with_edges = results
            .iter()
            .filter(|(_, r)| matches!(r, Ok(v) if !v.is_empty()))
            .count();
        let one_edge_count = results
            .iter()
            .filter(|(_, r)| matches!(r, Ok(v) if v.len() == 1))
            .count();

        println!(
            "resolve_edges_for_all_primary: total={}, ok={}, ok_with_edges={}, one_edge_nodes={}",
            total, successes, successes_with_edges, one_edge_count
        );

        // Print the top 10 nodes by edge count (with a small preview of targets).
        let mut ok_results: Vec<(&PrimaryNodeRow, &Vec<ResolvedEdgeData>)> = results
            .iter()
            .filter_map(|(row, res)| res.as_ref().ok().map(|edges| (row, edges)))
            .filter(|(_, edges)| !edges.is_empty())
            .collect();
        ok_results.sort_by(|a, b| b.1.len().cmp(&a.1.len()));

        for (row, edges) in ok_results.into_iter().take(10) {
            let sample_targets = edges
                .iter()
                .take(5)
                .map(|e| format!("{} ({})", e.target_name, e.relation_kind))
                .join(", ");
            let canon = edges
                .get(0)
                .map(|e| e.canon_path.0.clone())
                .unwrap_or_else(|| "<unknown>".to_string());
            println!(
                "{} {} [{}] -> {} edges: [{}]",
                row.relation,
                row.name,
                canon,
                edges.len(),
                sample_targets
            );
        }

        // Relation-kind histogram across all resolved edges.
        let mut rel_counts: HashMap<&str, usize> = HashMap::new();
        for (_, res) in &results {
            if let Ok(edges) = res {
                for e in edges {
                    *rel_counts.entry(e.relation_kind.as_str()).or_default() += 1;
                }
            }
        }
        let mut rels_sorted: Vec<_> = rel_counts.into_iter().collect();
        rels_sorted.sort_by(|a, b| b.1.cmp(&a.1));
        let rels_preview = rels_sorted
            .iter()
            .take(5)
            .map(|(k, v)| format!("{k}: {v}"))
            .join(", ");
        println!("relation counts (top): {rels_preview}");

        assert!(total > 0, "should find at least one primary node");
        assert!(
            successes > 0,
            "at least one node should resolve successfully"
        );
        assert!(
            successes_with_edges > 0,
            "at least one node should have edges"
        );
        Ok(())
    }

    #[test]
    fn test_tracing_prints() {
        init_tracing_once("test-edges", Level::INFO);
        tracing::info!("info");
        tracing::warn!("warn");
        tracing::debug!("debug");
        tracing::trace!("trace");
        tracing::error!("error");
        tracing::info!(target: "test-edges", "info");
        tracing::warn!(target: "test-edges", "warn");
        tracing::debug!(target: "test-edges", "debug");
        tracing::trace!(target: "test-edges", "trace");
        tracing::error!(target: "test-edges", "error");
        tracing::info!(target: "not-target", "info");
        tracing::warn!(target: "not-target", "warn");
        tracing::debug!(target: "not-target", "debug");
        tracing::trace!(target: "not-target", "trace");
        tracing::error!(target: "not-target", "error");
    }

    /// Run the full edge sweep against the workspace `ploke-db` crate to see a more realistic edge profile.
    #[test]
    #[ignore = "A long-running demonstration that introspects edges for ploke-db, takes about a minute"]
    fn resolve_edges_all_primary_ploke_db() -> Result<(), DbError> {
        let x = ploke_test_utils::init_test_tracing_with_target("test-edges", Level::INFO);
        let cozo_db = ploke_test_utils::setup_db_full_crate("ploke-db")
            .expect("workspace crate database must be set up correctly");
        let db = Database::new(cozo_db);

        tracing::info!(target: "test-edges", "starting test");

        let results = super::resolve_edges_for_all_primary(&db);
        let total = results.len();
        let successes = results.iter().filter(|(_, r)| r.is_ok()).count();
        let successes_with_edges = results
            .iter()
            .filter(|(_, r)| matches!(r, Ok(v) if !v.is_empty()))
            .count();
        let one_edge_count = results
            .iter()
            .filter(|(_, r)| matches!(r, Ok(v) if v.len() == 1))
            .count();

        tracing::info!(
            "[ploke-db] resolve_edges_for_all_primary: total={}, ok={}, ok_with_edges={}, one_edge_nodes={}",
            total, successes, successes_with_edges, one_edge_count
        );

        let mut ok_results: Vec<(&PrimaryNodeRow, &Vec<ResolvedEdgeData>)> = results
            .iter()
            .filter_map(|(row, res)| res.as_ref().ok().map(|edges| (row, edges)))
            .filter(|(_, edges)| !edges.is_empty())
            .collect();
        ok_results.sort_by(|a, b| b.1.len().cmp(&a.1.len()));

        for (row, edges) in ok_results.iter().take(10) {
            let sample_targets = edges
                .iter()
                .take(5)
                .map(|e| format!("{} ({})", e.target_name, e.relation_kind))
                .join(", ");
            let canon = edges
                .get(0)
                .map(|e| e.canon_path.0.clone())
                .unwrap_or_else(|| "<unknown>".to_string());
            println!(
                "[ploke-db] {} {} [{}] -> {} edges: [{}]",
                row.relation,
                row.name,
                canon,
                edges.len(),
                sample_targets
            );
        }

        let mut rel_counts: HashMap<&str, usize> = HashMap::new();
        for (_, res) in &results {
            if let Ok(edges) = res {
                for e in edges {
                    *rel_counts.entry(e.relation_kind.as_str()).or_default() += 1;
                }
            }
        }
        let mut rels_sorted: Vec<_> = rel_counts.into_iter().collect();
        rels_sorted.sort_by(|a, b| b.1.cmp(&a.1));
        let rels_preview = rels_sorted
            .iter()
            .take(10)
            .map(|(k, v)| format!("{k}: {v}"))
            .join(", ");
        println!("[ploke-db] relation counts (top): {rels_preview}");

        assert!(total > 0, "should find at least one primary node");
        assert!(
            successes > 0,
            "at least one node should resolve successfully"
        );
        assert!(
            successes_with_edges > 0,
            "at least one node should have edges"
        );
        let all_edge_kinds = [
            "Contains",
            "ResolvesToDefinition",
            "CustomPath",
            "Sibling",
            "ModuleImports",
            "ReExports",
            "ImportedBy",
            "StructField",
            "UnionField",
            "VariantField",
            "EnumVariant",
            "ImplAssociatedItem",
            "TraitAssociatedItem",
        ];
        let mut all_counts: VecDeque<(&'static str, usize)> = VecDeque::new();
        for kind in all_edge_kinds {
            let count = count_edges_by_kind(&db, kind)?;
            all_counts.push_front((kind, count));
        }
        let sorted = all_counts.iter().sorted_by(|a, b| Ord::cmp(&b.1, &a.1));
        let printable = sorted
            .map(|(kind, count)| format!("{kind: <20} | {count}"))
            .join("\n");
        tracing::info!(target: "test-edges", "Edges sorted by count:\n{}", printable);
        Ok(())
    }

    #[test]
    fn count_edges_by_kind_contains_fixture_nodes() -> Result<(), DbError> {
        let cozo_db = ploke_test_utils::setup_db_full_multi_embedding("fixture_nodes")
            .expect("database must be set up correctly");
        let db = Database::new(cozo_db);
        let contains_count = super::count_edges_by_kind(&db, "Contains")?;
        assert!(contains_count > 0, "fixture should have Contains edges");
        Ok(())
    }

    #[test]
    fn graph_resolve_exact_matches_fixture_nodes_struct() -> Result<(), DbError> {
        let cozo_db = ploke_test_utils::setup_db_full_multi_embedding("fixture_nodes")
            .expect("database must be set up correctly");
        let db = Database::new(cozo_db);

        let fixture_root = fixtures_crates_dir().join("fixture_nodes");
        let file_path = fixture_root.join("src/structs.rs");
        let module_path = vec!["crate".to_string(), "structs".to_string()];
        let rows =
            super::graph_resolve_exact(&db, "struct", &file_path, &module_path, "SampleStruct")?;

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
        let cozo_db = ploke_test_utils::setup_db_full_multi_embedding("fixture_nodes")
            .expect("database must be set up correctly");
        let db = Database::new(cozo_db);

        let fixtures_root = fixtures_crates_dir();
        let canonical_path = fixtures_root.join("fixture_nodes").join("src/imports.rs");
        let different_path = fixtures_root
            .join("fixture_nodes_copy")
            .join("src/imports.rs");
        let module_path = vec!["crate".to_string(), "imports".to_string()];
        let item_name = "use_imported_items";

        let strict_hit =
            super::graph_resolve_exact(&db, "function", &canonical_path, &module_path, item_name)?;
        assert_eq!(strict_hit.len(), 1, "expected strict resolver to match");

        let strict_miss =
            super::graph_resolve_exact(&db, "function", &different_path, &module_path, item_name)?;
        assert!(
            strict_miss.is_empty(),
            "path-sensitive resolver should refuse nodes from other absolute paths"
        );

        let canonical_rows =
            super::resolve_nodes_by_canon(&db, "function", &module_path, item_name)?;
        assert_eq!(
            canonical_rows.len(),
            1,
            "canonical lookup should find the node"
        );
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
