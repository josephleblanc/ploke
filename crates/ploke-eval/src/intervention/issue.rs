use std::collections::{BTreeMap, BTreeSet};

use ploke_core::tool_types::ToolName;
use serde::{Deserialize, Serialize};

use crate::protocol::protocol_aggregate::ProtocolAggregate;
use crate::record::RunRecord;

pub const INTERVENTION_ISSUE_DETECTION_PROCEDURE: &str = "intervention_issue_detection";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum IssueSelectionBasis {
    ProtocolReviewedIssueCalls,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct IssueEvidence {
    pub reviewed_call_count: usize,
    pub reviewed_issue_call_count: usize,
    pub protocol: IssueProtocolEvidence,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct IssueProtocolEvidence {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub reviewed_call_indices: Vec<usize>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub reviewed_segment_indices: Vec<usize>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub candidate_concerns: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub nearby_segment_labels: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct IssueCase {
    pub selection_basis: IssueSelectionBasis,
    pub target_tool: ToolName,
    pub evidence: IssueEvidence,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IssueDetectionInput {
    pub record: RunRecord,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub protocol_aggregate: Option<ProtocolAggregate>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct IssueDetectionArtifactInput {
    #[serde(default)]
    pub run_id: String,
    #[serde(default)]
    pub subject_id: String,
    #[serde(default)]
    pub total_calls_in_run: usize,
    #[serde(default)]
    pub anchor_segment_count: usize,
    #[serde(default)]
    pub protocol_reviewed_call_count: usize,
    #[serde(default)]
    pub protocol_reviewed_segment_count: usize,
    #[serde(default)]
    pub protocol_artifact_count: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct IssueDetectionOutput {
    pub cases: Vec<IssueCase>,
}

impl IssueDetectionInput {
    pub fn from_record(record: RunRecord, protocol_aggregate: Option<ProtocolAggregate>) -> Self {
        Self {
            record,
            protocol_aggregate,
        }
    }
}

pub fn detect_issue_cases(input: &IssueDetectionInput) -> IssueDetectionOutput {
    let Some(protocol_aggregate) = input.protocol_aggregate.as_ref() else {
        return IssueDetectionOutput { cases: Vec::new() };
    };

    let tool_calls = input.record.tool_calls();
    let mut reduction = BTreeMap::<ToolName, ToolIssueReduction>::new();

    for review in &protocol_aggregate.call_reviews {
        let Some(issue_label) = primary_call_issue(review) else {
            continue;
        };
        let Some(tool_name) = tool_calls
            .get(review.focal_call_index)
            .and_then(|call| parse_tool_name(&call.request.tool))
        else {
            continue;
        };

        let entry = reduction
            .entry(tool_name)
            .or_insert_with(ToolIssueReduction::default);
        entry.reviewed_issue_call_count += 1;
        entry.score += call_review_severity(review);
        entry.reviewed_call_indices.insert(review.focal_call_index);
        if let Some(segment_index) = review.segment_index {
            entry.reviewed_segment_indices.insert(segment_index);
        }
        if let Some(label) = review.segment_label.as_deref() {
            entry.nearby_segment_labels.insert(label.to_string());
        }
        entry.candidate_concerns.insert(issue_label);
        for concern in &review.signals.candidate_concerns {
            entry.candidate_concerns.insert(concern.clone());
        }
    }

    for review in &protocol_aggregate.call_reviews {
        let Some(tool_name) = tool_calls
            .get(review.focal_call_index)
            .and_then(|call| parse_tool_name(&call.request.tool))
        else {
            continue;
        };

        let Some(entry) = reduction.get_mut(&tool_name) else {
            continue;
        };
        entry.reviewed_call_count += 1;
        entry.reviewed_call_indices.insert(review.focal_call_index);
        if let Some(segment_index) = review.segment_index {
            entry.reviewed_segment_indices.insert(segment_index);
        }
        if let Some(label) = review.segment_label.as_deref() {
            entry.nearby_segment_labels.insert(label.to_string());
        }
        for concern in &review.signals.candidate_concerns {
            entry.candidate_concerns.insert(concern.clone());
        }
    }

    let mut ranked = reduction
        .into_iter()
        .filter(|(_, reduction)| reduction.reviewed_issue_call_count > 0)
        .collect::<Vec<_>>();

    ranked.sort_by(|(left_tool, left), (right_tool, right)| {
        right
            .score
            .cmp(&left.score)
            .then_with(|| {
                right
                    .reviewed_issue_call_count
                    .cmp(&left.reviewed_issue_call_count)
            })
            .then_with(|| right.reviewed_call_count.cmp(&left.reviewed_call_count))
            .then_with(|| {
                right
                    .candidate_concerns
                    .len()
                    .cmp(&left.candidate_concerns.len())
            })
            .then_with(|| left_tool.as_str().cmp(right_tool.as_str()))
    });

    let cases = ranked
        .into_iter()
        .map(|(tool, reduction)| IssueCase {
            selection_basis: IssueSelectionBasis::ProtocolReviewedIssueCalls,
            target_tool: tool,
            evidence: IssueEvidence {
                reviewed_call_count: reduction.reviewed_call_count,
                reviewed_issue_call_count: reduction.reviewed_issue_call_count,
                protocol: IssueProtocolEvidence {
                    reviewed_call_indices: reduction.reviewed_call_indices.into_iter().collect(),
                    reviewed_segment_indices: reduction
                        .reviewed_segment_indices
                        .into_iter()
                        .collect(),
                    candidate_concerns: reduction.candidate_concerns.into_iter().collect(),
                    nearby_segment_labels: reduction.nearby_segment_labels.into_iter().collect(),
                },
            },
        })
        .collect();

    IssueDetectionOutput { cases }
}

pub fn select_primary_issue(output: &IssueDetectionOutput) -> Option<IssueCase> {
    output.cases.first().cloned()
}

pub fn issue_detection_artifact_input(input: &IssueDetectionInput) -> IssueDetectionArtifactInput {
    let protocol_reviewed_call_count = input
        .protocol_aggregate
        .as_ref()
        .map(|aggregate| aggregate.coverage.reviewed_call_count)
        .unwrap_or(0);
    let protocol_reviewed_segment_count = input
        .protocol_aggregate
        .as_ref()
        .map(|aggregate| aggregate.coverage.reviewed_segment_count)
        .unwrap_or(0);
    let protocol_artifact_count = input
        .protocol_aggregate
        .as_ref()
        .map(|aggregate| aggregate.coverage.scanned_artifact_count)
        .unwrap_or(0);
    let anchor_segment_count = input
        .protocol_aggregate
        .as_ref()
        .map(|aggregate| aggregate.segmentation.segments.len())
        .unwrap_or(0);

    IssueDetectionArtifactInput {
        run_id: input.record.manifest_id.clone(),
        subject_id: input.record.metadata.benchmark.instance_id.clone(),
        total_calls_in_run: input.record.tool_calls().len(),
        anchor_segment_count,
        protocol_reviewed_call_count,
        protocol_reviewed_segment_count,
        protocol_artifact_count,
    }
}

#[derive(Debug, Default)]
struct ToolIssueReduction {
    reviewed_call_count: usize,
    reviewed_issue_call_count: usize,
    score: u16,
    reviewed_call_indices: BTreeSet<usize>,
    reviewed_segment_indices: BTreeSet<usize>,
    candidate_concerns: BTreeSet<String>,
    nearby_segment_labels: BTreeSet<String>,
}

fn parse_tool_name(value: &str) -> Option<ToolName> {
    ToolName::ALL
        .into_iter()
        .find(|tool| tool.as_str() == value)
}

fn primary_call_issue(
    row: &crate::protocol::protocol_aggregate::ProtocolCallReviewRow,
) -> Option<String> {
    if row.redundancy.verdict == "search_thrash" {
        Some("search_thrash".to_string())
    } else if row.recoverability.verdict == "partial_next_step" {
        Some("partial_next_step".to_string())
    } else if row.overall != "focused_progress" {
        Some(row.overall.clone())
    } else {
        None
    }
}

fn call_review_severity(row: &crate::protocol::protocol_aggregate::ProtocolCallReviewRow) -> u16 {
    if row.redundancy.verdict == "search_thrash" {
        95
    } else if row.recoverability.verdict == "no_clear_recovery" {
        85
    } else if row.recoverability.verdict == "partial_next_step" {
        70
    } else if row.overall == "mixed" || row.overall == "recoverable_misstep" {
        55
    } else {
        25
    }
}
