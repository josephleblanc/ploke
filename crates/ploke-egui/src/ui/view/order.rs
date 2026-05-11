use std::collections::{HashMap, HashSet};

use ploke_records::branch::TreatmentBranchStatus;
use ploke_records::ids::SchedulerNodeId;

use crate::graph::Candidate;

pub(super) fn visual_candidate_order(candidates: Vec<&Candidate>) -> Vec<&Candidate> {
    let known = candidates
        .iter()
        .map(|candidate| candidate.id().clone())
        .collect::<HashSet<_>>();
    let mut children = HashMap::<Option<SchedulerNodeId>, Vec<&Candidate>>::new();

    for candidate in candidates {
        let parent = candidate
            .parent()
            .filter(|parent| known.contains(*parent))
            .cloned();
        children.entry(parent).or_default().push(candidate);
    }

    let mut ordered = Vec::new();
    let mut visited = HashSet::new();
    append_candidate_group(None, &mut children, &mut visited, &mut ordered);

    let mut remaining = children.keys().cloned().collect::<Vec<_>>();
    remaining.sort_by(|left, right| {
        left.as_ref()
            .map(|id| id.as_str())
            .cmp(&right.as_ref().map(|id| id.as_str()))
    });
    for parent in remaining {
        append_candidate_group(parent, &mut children, &mut visited, &mut ordered);
    }

    ordered
}

fn append_candidate_group<'a>(
    parent: Option<SchedulerNodeId>,
    children: &mut HashMap<Option<SchedulerNodeId>, Vec<&'a Candidate>>,
    visited: &mut HashSet<SchedulerNodeId>,
    ordered: &mut Vec<&'a Candidate>,
) {
    let Some(group) = children.remove(&parent) else {
        return;
    };

    for candidate in center_selected(group) {
        if !visited.insert(candidate.id().clone()) {
            continue;
        }
        ordered.push(candidate);
        append_candidate_group(Some(candidate.id().clone()), children, visited, ordered);
    }
}

fn center_selected(mut group: Vec<&Candidate>) -> Vec<&Candidate> {
    group.sort_by(|left, right| candidate_order_key(left).cmp(&candidate_order_key(right)));

    let mut selected = Vec::new();
    let mut other = Vec::new();
    for candidate in group {
        if candidate.status() == TreatmentBranchStatus::Selected {
            selected.push(candidate);
        } else {
            other.push(candidate);
        }
    }

    let right = other.split_off(other.len() / 2);
    other.into_iter().chain(selected).chain(right).collect()
}

fn candidate_order_key(candidate: &Candidate) -> (Option<u64>, &str) {
    (candidate.ruling_epoch(), candidate.id().as_str())
}
