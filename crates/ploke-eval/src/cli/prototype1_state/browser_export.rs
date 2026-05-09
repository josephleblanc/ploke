use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use ploke_records::journal::JournalEntry;
use ploke_tree::FsRunStore;
use ploke_tree_browser::NodeBranchInfo;

use crate::spec::PrepareError;

/// Export an enriched fine-history browser model as JSON.
pub(crate) fn export_browser_model(
    campaign_id: &str,
    manifest_path: &Path,
    output_path: Option<&Path>,
) -> Result<(), PrepareError> {
    let run_root = prototype_root(manifest_path);
    let store = FsRunStore::new(run_root.clone());

    let blocks = store
        .load_history_blocks()
        .map_err(|source| PrepareError::DatabaseSetup {
            phase: "load sealed history blocks",
            detail: source.to_string(),
        })?;

    let mut model = ploke_tree_browser::fine_history_browser_model_from_blocks(&blocks);

    // Load evaluations.
    let eval_evidence =
        store
            .load_evaluation_evidence()
            .map_err(|source| PrepareError::DatabaseSetup {
                phase: "load evaluation evidence",
                detail: source.to_string(),
            })?;

    // Build node_id → branch info from the transition journal.
    let node_branches = build_node_branch_map(&run_root);

    // Enrich the model.
    let eval_index = eval_evidence
        .as_ref()
        .map(|ev| &ev.index)
        .cloned()
        .unwrap_or_default();
    ploke_tree_browser::enrich_fine_browser_model(&mut model, &eval_index, &node_branches, None);

    // Build run summary.
    let node_count = count_nodes(&run_root);
    let max_generation = blocks
        .iter()
        .map(|b| b.state.header.common.block_height)
        .max()
        .unwrap_or(0);
    let journal_entry_count = count_journal_lines(&run_root);
    model.run_summary = Some(ploke_tree_browser::build_run_summary(
        campaign_id.to_owned(),
        node_count,
        max_generation,
        blocks.len(),
        eval_evidence.as_ref().map(|ev| &ev.summary),
        journal_entry_count,
    ));

    let json =
        serde_json::to_string_pretty(&model).map_err(|source| PrepareError::Serialize(source))?;

    match output_path {
        Some(path) => {
            std::fs::write(path, &json).map_err(|source| PrepareError::WriteManifest {
                path: path.to_path_buf(),
                source,
            })?;
            eprintln!("browser model written to {}", path.display());
        }
        None => {
            println!("{json}");
        }
    }

    Ok(())
}

/// Build a node_id → NodeBranchInfo map from the transition journal's
/// `MaterializeBranch` entries.
fn build_node_branch_map(run_root: &Path) -> BTreeMap<String, NodeBranchInfo> {
    let path = run_root.join("transition-journal.jsonl");
    if !path.is_file() {
        return BTreeMap::new();
    }

    let text = match std::fs::read_to_string(&path) {
        Ok(t) => t,
        Err(_) => return BTreeMap::new(),
    };

    let mut map = BTreeMap::new();
    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let entry: JournalEntry = match serde_json::from_str(line) {
            Ok(e) => e,
            Err(_) => continue,
        };
        if let JournalEntry::MaterializeBranch(record) = entry {
            let node_id = record.refs.node_id.clone();
            map.insert(
                node_id,
                NodeBranchInfo {
                    branch_id: record.refs.branch_id.clone(),
                    candidate_id: record.refs.candidate_id.clone(),
                    target_relpath: record.paths.target_relpath.to_string_lossy().into_owned(),
                    source_state_id: record.refs.source_state_id.clone(),
                    source_content_hash: Some(record.hashes.source.0.clone()),
                    proposed_content_hash: Some(record.hashes.proposed.0.clone()),
                    patch_id: None,
                },
            );
        }
    }

    map
}

fn count_nodes(run_root: &Path) -> usize {
    let nodes_dir = run_root.join("nodes");
    if !nodes_dir.is_dir() {
        return 0;
    }
    std::fs::read_dir(nodes_dir)
        .map(|entries| entries.filter_map(|e| e.ok()).count())
        .unwrap_or(0)
}

fn count_journal_lines(run_root: &Path) -> usize {
    let path = run_root.join("transition-journal.jsonl");
    if !path.is_file() {
        return 0;
    }
    std::fs::read_to_string(&path)
        .map(|text| text.lines().filter(|l| !l.trim().is_empty()).count())
        .unwrap_or(0)
}

fn prototype_root(manifest_path: &Path) -> PathBuf {
    manifest_path
        .parent()
        .unwrap_or_else(|| Path::new("."))
        .join("prototype1")
}
