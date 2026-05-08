use std::{
    fs,
    path::{Path, PathBuf},
};

use chrono::Utc;
use serde::Serialize;

use crate::cli::{InspectOutputFormat, Prototype1MonitorStatusCommand};
use crate::intervention::{Prototype1ContinuationDisposition, load_scheduler_state};
use crate::projection::OperatorProjectionRead;
use crate::spec::PrepareError;

use super::successor;
use super::{
    cli_facing::{MonitorStatusTarget, Prototype1BranchEvaluationReport},
    journal::{JournalEntry, PrototypeJournal, prototype1_transition_journal_path},
};

#[derive(Debug, Clone, Serialize)]
struct Prototype1MonitorStatusReport {
    campaign: String,
    target: TargetField,
    generated_at: String,
    runtime: StatusField,
    parent: ParentField,
    successor: SuccessorField,
    scheduler: SchedulerField,
    children: ChildrenField,
    selection: SelectionField,
    next: String,
    warnings: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
struct TargetField {
    campaign_source: String,
    repo_root_source: String,
}

#[derive(Debug, Clone, Serialize)]
struct StatusField {
    state: String,
    reason: String,
}

#[derive(Debug, Clone, Serialize)]
struct ParentField {
    state: String,
    node_id: Option<String>,
    pid: Option<u32>,
}

#[derive(Debug, Clone, Serialize)]
struct SuccessorField {
    state: String,
    node_id: Option<String>,
    pid: Option<u32>,
    acknowledged: bool,
}

#[derive(Debug, Clone, Serialize)]
struct SchedulerField {
    generation: Option<u32>,
    frontier: Option<usize>,
    completed: Option<usize>,
    failed: Option<usize>,
}

#[derive(Debug, Clone, Serialize)]
struct ChildrenField {
    planned: Option<usize>,
    observed: Option<usize>,
    evaluated: Option<usize>,
    reject: Option<usize>,
    keep: Option<usize>,
    other: Option<usize>,
    source: String,
}

#[derive(Debug, Clone, Serialize)]
struct SelectionField {
    selected_successor: Option<String>,
    decision: Option<String>,
}

#[derive(Debug, Default)]
struct Fold {
    parent: Option<(String, u32)>,
    successor: Option<(String, u32)>,
    successor_acknowledged: bool,
    children_planned: usize,
    children_observed: usize,
    children_evaluated: usize,
    reject: usize,
    keep: usize,
    other: usize,
    selected_successor: Option<String>,
    selection_decision: Option<String>,
}

pub(crate) fn run(
    campaign_id: &str,
    manifest_path: &Path,
    target: &MonitorStatusTarget,
    command: &Prototype1MonitorStatusCommand,
) -> Result<(), PrepareError> {
    let report = build_status(campaign_id, manifest_path, target)?;
    match command.format {
        InspectOutputFormat::Table => print_table(&report),
        InspectOutputFormat::Json => {
            println!(
                "{}",
                serde_json::to_string_pretty(&report).map_err(PrepareError::Serialize)?
            );
        }
    }
    Ok(())
}

fn build_status(
    campaign_id: &str,
    manifest_path: &Path,
    target: &MonitorStatusTarget,
) -> Result<Prototype1MonitorStatusReport, PrepareError> {
    let journal = PrototypeJournal::new(prototype1_transition_journal_path(manifest_path));
    let entries = journal
        .load_entries()
        .map_err(|err| PrepareError::InvalidBatchSelection {
            detail: format!("failed to load transition journal for status projection: {err}"),
        })?;
    let fold = fold_entries(&entries);

    let scheduler =
        load_scheduler_state(manifest_path, OperatorProjectionRead::cli_operator()).ok();
    let scheduler_generation = scheduler
        .as_ref()
        .and_then(|state| state.nodes.iter().map(|node| node.generation).max());
    let scheduler_field = SchedulerField {
        generation: scheduler_generation,
        frontier: scheduler
            .as_ref()
            .map(|state| state.frontier_node_ids.len()),
        completed: scheduler
            .as_ref()
            .map(|state| state.completed_node_ids.len()),
        failed: scheduler.as_ref().map(|state| state.failed_node_ids.len()),
    };

    let parent_alive = fold.parent.as_ref().map(|(_, pid)| pid_alive(*pid));
    let successor_alive = fold.successor.as_ref().map(|(_, pid)| pid_alive(*pid));
    let runtime = compute_runtime(
        parent_alive,
        successor_alive,
        fold.successor_acknowledged,
        scheduler
            .as_ref()
            .and_then(|state| state.last_continuation_decision.as_ref())
            .map(|decision| decision.disposition),
    );

    let selection = SelectionField {
        selected_successor: fold.selected_successor.clone(),
        decision: fold.selection_decision.clone().or_else(|| {
            scheduler
                .as_ref()
                .and_then(|state| state.last_continuation_decision.as_ref())
                .map(|decision| disposition_name(decision.disposition))
        }),
    };

    let children = merge_children_counts(manifest_path, &fold)?;

    let report = Prototype1MonitorStatusReport {
        campaign: campaign_id.to_string(),
        target: TargetField {
            campaign_source: target.campaign_source.to_string(),
            repo_root_source: target.repo_root_source.to_string(),
        },
        generated_at: Utc::now().to_rfc3339(),
        runtime,
        parent: ParentField {
            state: match parent_alive {
                Some(true) => "started".to_string(),
                Some(false) => "retired_dead".to_string(),
                None => "unknown".to_string(),
            },
            node_id: fold.parent.as_ref().map(|(node_id, _)| node_id.clone()),
            pid: fold.parent.as_ref().map(|(_, pid)| *pid),
        },
        successor: SuccessorField {
            state: match successor_alive {
                Some(true) => "alive".to_string(),
                Some(false) => {
                    if fold.successor_acknowledged {
                        "successor_exited".to_string()
                    } else {
                        "dead".to_string()
                    }
                }
                None => {
                    if fold.successor_acknowledged {
                        "acknowledged".to_string()
                    } else {
                        "unknown".to_string()
                    }
                }
            },
            node_id: fold.successor.as_ref().map(|(node_id, _)| node_id.clone()),
            pid: fold.successor.as_ref().map(|(_, pid)| *pid),
            acknowledged: fold.successor_acknowledged,
        },
        scheduler: scheduler_field,
        children,
        selection,
        next: next_hint(parent_alive, successor_alive, fold.successor_acknowledged),
        warnings: target.warnings.clone(),
    };

    Ok(report)
}

fn fold_entries(entries: &[JournalEntry]) -> Fold {
    let mut fold = Fold::default();
    for entry in entries {
        match entry {
            JournalEntry::ParentStarted(parent) => {
                fold.parent = Some((parent.parent_identity.node_id.clone(), parent.pid));
            }
            JournalEntry::SpawnChild(spawn) => {
                fold.children_planned += 1;
                if let Some(pid) = spawn.child_pid {
                    fold.successor = Some((spawn.refs.node_id.clone(), pid));
                }
            }
            JournalEntry::ObserveChild(observe) => {
                if observe.phase == crate::intervention::CommitPhase::Before {
                    fold.children_observed += 1;
                    if let Some(result) = observe.result.as_ref() {
                        fold.children_evaluated += 1;
                        match result {
                            super::journal::ObservedChildResult::Succeeded {
                                overall_disposition,
                                ..
                            } => {
                                let value = format!("{:?}", overall_disposition).to_lowercase();
                                if value == "keep" {
                                    fold.keep += 1;
                                } else if value == "reject" {
                                    fold.reject += 1;
                                } else {
                                    fold.other += 1;
                                }
                            }
                            super::journal::ObservedChildResult::Failed { .. } => {
                                fold.other += 1;
                            }
                        }
                    }
                }
            }
            JournalEntry::SuccessorHandoff(handoff) => {
                fold.successor = Some((handoff.node_id.clone(), handoff.pid));
                fold.successor_acknowledged = true;
            }
            JournalEntry::Successor(record) => match &record.state {
                successor::State::Selected { decision, .. } => {
                    fold.selected_successor = decision.selected_next_branch_id.clone();
                    fold.selection_decision = Some(disposition_name(decision.disposition));
                }
                successor::State::Spawned { pid, .. } | successor::State::Ready { pid, .. } => {
                    fold.successor = Some((record.node_id.clone(), *pid));
                    if matches!(record.state, successor::State::Ready { .. }) {
                        fold.successor_acknowledged = true;
                    }
                }
                successor::State::Completed { .. } => {
                    fold.successor_acknowledged = true;
                }
                successor::State::Checkout { .. }
                | successor::State::TimedOut { .. }
                | successor::State::ExitedBeforeReady { .. } => {}
            },
            JournalEntry::Resource(_)
            | JournalEntry::ChildArtifactCommitted(_)
            | JournalEntry::ActiveCheckoutAdvanced(_)
            | JournalEntry::MaterializeBranch(_)
            | JournalEntry::BuildChild(_)
            | JournalEntry::Child(_)
            | JournalEntry::ChildReady(_) => {}
        }
    }
    fold
}

fn compute_runtime(
    parent_alive: Option<bool>,
    successor_alive: Option<bool>,
    successor_acknowledged: bool,
    continuation: Option<Prototype1ContinuationDisposition>,
) -> StatusField {
    if successor_alive == Some(true) && successor_acknowledged {
        return StatusField {
            state: "handoff".to_string(),
            reason: "successor acknowledged and alive after handoff".to_string(),
        };
    }
    if parent_alive == Some(true) {
        return StatusField {
            state: "running".to_string(),
            reason: "latest parent pid is alive".to_string(),
        };
    }
    if parent_alive == Some(false) && successor_acknowledged {
        if successor_alive == Some(false) {
            return StatusField {
                state: "handoff_dead".to_string(),
                reason: "parent retired and acknowledged successor pid is not alive".to_string(),
            };
        }
        return StatusField {
            state: "handoff".to_string(),
            reason: "parent retired after acknowledged successor handoff".to_string(),
        };
    }
    if let Some(disposition) = continuation {
        if !disposition.allows_successor() {
            return StatusField {
                state: "completed".to_string(),
                reason: format!("scheduler disposition {}", disposition_name(disposition)),
            };
        }
    }
    StatusField {
        state: "unknown".to_string(),
        reason: "missing parent/successor liveness evidence".to_string(),
    }
}

fn next_hint(
    parent_alive: Option<bool>,
    successor_alive: Option<bool>,
    successor_acknowledged: bool,
) -> String {
    match (parent_alive, successor_alive, successor_acknowledged) {
        (Some(true), _, _) => "monitor parent process and scheduler frontier".to_string(),
        (Some(false), Some(true), true) => {
            "handoff in progress; check successor completion and next parent start".to_string()
        }
        (Some(false), Some(false), true) => {
            "successor acknowledged but not alive; inspect transition journal and completion files"
                .to_string()
        }
        _ => "status is partial; inspect scheduler/journal for missing evidence".to_string(),
    }
}

fn print_table(report: &Prototype1MonitorStatusReport) {
    println!("prototype1 status");
    println!("campaign: {}", report.campaign);
    println!(
        "target: campaign_source={} repo_root_source={}",
        report.target.campaign_source, report.target.repo_root_source
    );
    println!("generated_at: {}", report.generated_at);
    println!(
        "runtime: {} ({})",
        report.runtime.state, report.runtime.reason
    );
    println!(
        "parent: {} node={} pid={}",
        report.parent.state,
        opt_str(report.parent.node_id.as_deref()),
        opt_u32(report.parent.pid)
    );
    println!(
        "successor: {} node={} pid={} acknowledged={}",
        report.successor.state,
        opt_str(report.successor.node_id.as_deref()),
        opt_u32(report.successor.pid),
        yes_no(report.successor.acknowledged)
    );
    println!(
        "scheduler: generation={} frontier={} completed={} failed={}",
        opt_u32(report.scheduler.generation),
        opt_usize(report.scheduler.frontier),
        opt_usize(report.scheduler.completed),
        opt_usize(report.scheduler.failed)
    );
    println!(
        "children: planned={} observed={} evaluated={} reject={} keep={} other={} source={}",
        opt_usize(report.children.planned),
        opt_usize(report.children.observed),
        opt_usize(report.children.evaluated),
        opt_usize(report.children.reject),
        opt_usize(report.children.keep),
        opt_usize(report.children.other),
        report.children.source
    );
    println!(
        "selection: successor={} decision={}",
        opt_str(report.selection.selected_successor.as_deref()),
        opt_str(report.selection.decision.as_deref())
    );
    for warning in &report.warnings {
        println!("warning: {warning}");
    }
    println!("next: {}", report.next);
}

fn yes_no(value: bool) -> &'static str {
    if value { "yes" } else { "no" }
}

fn opt_str(value: Option<&str>) -> &str {
    value.unwrap_or("unknown")
}

fn opt_u32(value: Option<u32>) -> String {
    value
        .map(|raw| raw.to_string())
        .unwrap_or_else(|| "unknown".to_string())
}

fn opt_usize(value: Option<usize>) -> String {
    value
        .map(|raw| raw.to_string())
        .unwrap_or_else(|| "unknown".to_string())
}

fn merge_children_counts(manifest_path: &Path, fold: &Fold) -> Result<ChildrenField, PrepareError> {
    let prototype_root = manifest_path
        .parent()
        .unwrap_or_else(|| Path::new("."))
        .join("prototype1");
    let reports = load_evaluations(&prototype_root.join("evaluations"))?;
    if reports.is_empty() {
        return Ok(ChildrenField {
            planned: Some(fold.children_planned),
            observed: Some(fold.children_observed),
            evaluated: Some(fold.children_evaluated),
            reject: Some(fold.reject),
            keep: Some(fold.keep),
            other: Some(fold.other),
            source: "journal".to_string(),
        });
    }

    let mut reject = 0usize;
    let mut keep = 0usize;
    let mut other = 0usize;
    for report in &reports {
        match serde_name(&report.overall_disposition).as_str() {
            "reject" => reject += 1,
            "keep" => keep += 1,
            _ => other += 1,
        }
    }

    Ok(ChildrenField {
        planned: Some(fold.children_planned),
        observed: Some(fold.children_observed),
        evaluated: Some(reports.len()),
        reject: Some(reject),
        keep: Some(keep),
        other: Some(other),
        source: "evaluation_reports".to_string(),
    })
}

fn load_evaluations(dir: &Path) -> Result<Vec<Prototype1BranchEvaluationReport>, PrepareError> {
    if !dir.exists() {
        return Ok(Vec::new());
    }

    let mut reports = Vec::new();
    for entry in fs::read_dir(dir).map_err(|source| PrepareError::ReadManifest {
        path: dir.to_path_buf(),
        source,
    })? {
        let entry = entry.map_err(|source| PrepareError::ReadManifest {
            path: dir.to_path_buf(),
            source,
        })?;
        let path = entry.path();
        if path.extension().and_then(|extension| extension.to_str()) != Some("json") {
            continue;
        }
        let text = fs::read_to_string(&path).map_err(|source| PrepareError::ReadManifest {
            path: path.clone(),
            source,
        })?;
        reports.push(serde_json::from_str(&text).map_err(|source| {
            PrepareError::ParseManifest {
                path: path.clone(),
                source,
            }
        })?);
    }
    Ok(reports)
}

fn serde_name<T: Serialize>(value: &T) -> String {
    crate::cli::serde_name(value)
}

fn disposition_name(disposition: Prototype1ContinuationDisposition) -> String {
    match disposition {
        Prototype1ContinuationDisposition::ContinueReady => "continue_ready",
        Prototype1ContinuationDisposition::ContinueExploreFromRejected => {
            "continue_explore_from_rejected"
        }
        Prototype1ContinuationDisposition::StopMaxGenerations => "stop_max_generations",
        Prototype1ContinuationDisposition::StopMaxTotalNodes => "stop_max_total_nodes",
        Prototype1ContinuationDisposition::StopNoSelectedBranch => "stop_no_selected_branch",
        Prototype1ContinuationDisposition::StopOnFirstKeepSatisfied => {
            "stop_on_first_keep_satisfied"
        }
        Prototype1ContinuationDisposition::StopSelectedBranchRejected => {
            "stop_selected_branch_rejected"
        }
    }
    .to_string()
}

fn pid_alive(pid: u32) -> bool {
    std::path::Path::new("/proc").join(pid.to_string()).exists()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn runtime_is_handoff_dead_when_parent_retired_and_acknowledged_successor_not_alive() {
        let runtime = compute_runtime(Some(false), Some(false), true, None);
        assert_eq!(runtime.state, "handoff_dead");
    }

    #[test]
    fn runtime_is_handoff_when_parent_retired_and_acknowledged_successor_alive() {
        let runtime = compute_runtime(Some(false), Some(true), true, None);
        assert_eq!(runtime.state, "handoff");
    }
}
