use std::collections::BTreeMap;
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use ploke_records::journal::JournalEntry;
use ploke_tree::FsRunStore;
use ploke_tree_browser::{NodeBranchInfo, ProtocolSnapshot};

use crate::protocol_artifacts::list_protocol_artifacts;
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
    let eval_index = eval_evidence
        .as_ref()
        .map(|ev| &ev.index)
        .cloned()
        .unwrap_or_default();

    // Build node_id → branch info from the transition journal.
    let node_branches = build_node_branch_map(&run_root);
    let protocol_snapshots = build_protocol_snapshots_by_branch(&eval_index)?;

    // Enrich the model.
    ploke_tree_browser::enrich_fine_browser_model(
        &mut model,
        &eval_index,
        &node_branches,
        Some(&protocol_snapshots),
    );

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

fn build_protocol_snapshots_by_branch(
    evaluations: &BTreeMap<String, ploke_records::evaluation::Artifact>,
) -> Result<BTreeMap<String, ProtocolSnapshot>, PrepareError> {
    let mut snapshots = BTreeMap::new();
    for (branch_id, evaluation) in evaluations {
        let Some(record_path) = evaluation
            .compared_instances
            .iter()
            .find_map(|comparison| comparison.treatment_record_path.as_deref())
        else {
            continue;
        };
        let artifacts =
            list_protocol_artifacts(record_path).map_err(|source| PrepareError::DatabaseSetup {
                phase: "load protocol artifacts for browser model",
                detail: source.to_string(),
            })?;
        if artifacts.is_empty() {
            continue;
        }

        let mut snapshot = ProtocolSnapshot {
            intent_segmentation_count: 0,
            tool_call_review_count: 0,
            segment_review_count: 0,
            issue_detection_count: 0,
            synthesis_count: 0,
            model_id: None,
            provider_slug: None,
        };
        for artifact in artifacts {
            match artifact.stored.procedure_name.as_str() {
                "tool_call_intent_segmentation" => snapshot.intent_segmentation_count += 1,
                "tool_call_review" => snapshot.tool_call_review_count += 1,
                "tool_call_segment_review" => snapshot.segment_review_count += 1,
                "intervention_issue_detection" => snapshot.issue_detection_count += 1,
                "intervention_synthesis" => snapshot.synthesis_count += 1,
                _ => {}
            }
            if snapshot.model_id.is_none() {
                snapshot.model_id = artifact.stored.model_id.clone();
                snapshot.provider_slug = artifact.stored.provider_slug.clone();
            }
        }
        snapshots.insert(branch_id.clone(), snapshot);
    }

    Ok(snapshots)
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
    let mut ambiguous_nodes = BTreeSet::new();
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
            if ambiguous_nodes.contains(&node_id) {
                continue;
            }
            let info = NodeBranchInfo {
                branch_id: record.refs.branch_id.clone(),
                candidate_id: record.refs.candidate_id.clone(),
                target_relpath: record.paths.target_relpath.to_string_lossy().into_owned(),
                source_state_id: record.refs.source_state_id.clone(),
                source_content_hash: Some(record.hashes.source.0.clone()),
                proposed_content_hash: Some(record.hashes.proposed.0.clone()),
                patch_id: None,
            };
            if let Some(existing) = map.get(&node_id) {
                if same_node_branch_info(existing, &info) {
                    continue;
                }
                map.remove(&node_id);
                ambiguous_nodes.insert(node_id);
            } else {
                map.insert(node_id, info);
            }
        }
    }

    map
}

fn same_node_branch_info(left: &NodeBranchInfo, right: &NodeBranchInfo) -> bool {
    left.branch_id == right.branch_id
        && left.candidate_id == right.candidate_id
        && left.target_relpath == right.target_relpath
        && left.source_state_id == right.source_state_id
        && left.source_content_hash == right.source_content_hash
        && left.proposed_content_hash == right.proposed_content_hash
        && left.patch_id == right.patch_id
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
