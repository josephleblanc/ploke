use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use ploke_protocol::{
    Confidence,
    tool_calls::segment::{IntentLabel, SegmentStatus, SegmentedToolCallSequence},
};
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::protocol_artifacts::{
    ProtocolArtifactIdentityMismatch, StoredProtocolArtifactFile, list_protocol_artifacts,
    validate_protocol_artifact_identity,
};
use crate::run_registry::resolve_protocol_run_identity;
use crate::spec::PrepareError;

const TOOL_CALL_REVIEW: &str = "tool_call_review";
const TOOL_CALL_INTENT_SEGMENTATION: &str = "tool_call_intent_segmentation";
const TOOL_CALL_SEGMENT_REVIEW: &str = "tool_call_segment_review";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProtocolArtifactRef {
    pub path: PathBuf,
    pub created_at_ms: u64,
    pub procedure_name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProtocolRunIdentity {
    pub record_path: PathBuf,
    pub run_dir: PathBuf,
    pub run_id: String,
    pub subject_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProtocolCoverage {
    pub scanned_artifact_count: usize,
    pub artifact_counts: BTreeMap<String, usize>,
    pub total_calls_in_run: usize,
    pub total_segments_in_anchor: usize,
    pub reviewed_call_count: usize,
    pub reviewed_segment_count: usize,
    pub missing_call_indices: Vec<usize>,
    pub missing_segment_indices: Vec<usize>,
    pub skipped_segment_review_count: usize,
    pub segment_anchor_mismatch_count: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProtocolSegmentationCoverage {
    pub total_calls: usize,
    pub labeled_calls: usize,
    pub ambiguous_calls: usize,
    pub labeled_segments: usize,
    pub ambiguous_segments: usize,
    pub uncovered_calls: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProtocolSegmentBasis {
    pub segment_index: usize,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub label: Option<IntentLabel>,
    pub status: SegmentStatus,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub confidence: Option<Confidence>,
    pub rationale: Option<String>,
    pub start_index: usize,
    pub end_index: usize,
    pub turn_span: Vec<usize>,
    pub call_indices: Vec<usize>,
    pub call_count: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProtocolSegmentationAnchor {
    pub artifact: ProtocolArtifactRef,
    pub coverage: ProtocolSegmentationCoverage,
    pub segments: Vec<ProtocolSegmentBasis>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProtocolBranchAssessment {
    pub verdict: String,
    pub confidence: String,
    pub rationale: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ProtocolReviewSignals {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub browse_calls_in_scope: Option<usize>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub candidate_concerns: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub directory_pivots: Option<usize>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub distinct_tool_count: Option<usize>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub edit_calls_in_scope: Option<usize>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub execute_calls_in_scope: Option<usize>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub failed_calls_in_scope: Option<usize>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub labeled_segments_in_source: Option<usize>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ambiguous_segments_in_source: Option<usize>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub read_calls_in_scope: Option<usize>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub repeated_tool_name_count: Option<usize>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scope_turn_count: Option<usize>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub search_calls_in_scope: Option<usize>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub similar_search_neighbors: Option<usize>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub uncovered_calls_in_source: Option<usize>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProtocolCallReviewRow {
    pub artifact: ProtocolArtifactRef,
    pub focal_call_index: usize,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub segment_index: Option<usize>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub segment_label: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub segment_status: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub scope_call_indices: Vec<usize>,
    pub total_calls_in_run: usize,
    pub total_calls_in_scope: usize,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub turn_span: Vec<usize>,
    pub overall: String,
    pub overall_confidence: String,
    pub usefulness: ProtocolBranchAssessment,
    pub redundancy: ProtocolBranchAssessment,
    pub recoverability: ProtocolBranchAssessment,
    pub signals: ProtocolReviewSignals,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProtocolSegmentReviewRow {
    pub artifact: ProtocolArtifactRef,
    pub basis: ProtocolSegmentBasis,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub scope_call_indices: Vec<usize>,
    pub total_calls_in_run: usize,
    pub total_calls_in_scope: usize,
    pub overall: String,
    pub overall_confidence: String,
    pub usefulness: ProtocolBranchAssessment,
    pub redundancy: ProtocolBranchAssessment,
    pub recoverability: ProtocolBranchAssessment,
    pub signals: ProtocolReviewSignals,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProtocolCrosswalkRow {
    pub basis: ProtocolSegmentBasis,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub reviewed_call_indices: Vec<usize>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub segment_review: Option<ProtocolArtifactRef>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProtocolSkippedSegmentReview {
    pub artifact: ProtocolArtifactRef,
    pub segment_index: usize,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub observed_label: Option<String>,
    pub observed_status: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub observed_turn_span: Vec<usize>,
    pub observed_total_calls_in_scope: usize,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub anchor_label: Option<String>,
    pub anchor_status: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub anchor_turn_span: Vec<usize>,
    pub anchor_total_calls_in_scope: usize,
    pub reason: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProtocolDerivedMetrics {
    pub call_review_overall_counts: BTreeMap<String, usize>,
    pub segment_review_overall_counts: BTreeMap<String, usize>,
    pub call_review_confidence_counts: BTreeMap<String, usize>,
    pub segment_review_confidence_counts: BTreeMap<String, usize>,
    pub calls_with_segment_crosswalk: usize,
    pub calls_without_segment_crosswalk: usize,
    pub average_calls_per_anchor_segment: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProtocolAggregate {
    pub run: ProtocolRunIdentity,
    pub coverage: ProtocolCoverage,
    pub segmentation: ProtocolSegmentationAnchor,
    pub call_reviews: Vec<ProtocolCallReviewRow>,
    pub segment_reviews: Vec<ProtocolSegmentReviewRow>,
    pub crosswalk: Vec<ProtocolCrosswalkRow>,
    pub derived_metrics: ProtocolDerivedMetrics,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub skipped_segment_reviews: Vec<ProtocolSkippedSegmentReview>,
}

#[derive(Debug, Error)]
pub enum ProtocolAggregateError {
    #[error(transparent)]
    Source(#[from] PrepareError),
    #[error("no intent segmentation protocol artifact found for run '{record_path}'")]
    MissingAnchor { record_path: PathBuf },
    #[error(
        "protocol artifact '{path}' for procedure '{procedure}' is missing or malformed field '{field}'"
    )]
    InvalidArtifactField {
        path: PathBuf,
        procedure: String,
        field: &'static str,
    },
    #[error(
        "protocol segment review '{path}' does not match anchor basis for segment {segment_index}"
    )]
    SegmentBasisMismatch { path: PathBuf, segment_index: usize },
    #[error(
        "protocol artifact '{path}' for procedure '{procedure}' has {field}='{actual}' but expected '{expected}'"
    )]
    ArtifactIdentityMismatch {
        path: PathBuf,
        procedure: String,
        field: &'static str,
        expected: String,
        actual: String,
    },
    #[error("failed to deserialize protocol artifact '{path}': {source}")]
    DeserializeArtifact {
        path: PathBuf,
        source: serde_json::Error,
    },
}

pub fn load_protocol_aggregate(
    record_path: &Path,
) -> Result<ProtocolAggregate, ProtocolAggregateError> {
    let artifacts = list_protocol_artifacts(record_path)?;
    load_protocol_aggregate_from_artifacts(record_path, artifacts)
}

pub(crate) fn load_protocol_aggregate_from_artifacts(
    record_path: &Path,
    artifacts: Vec<StoredProtocolArtifactFile>,
) -> Result<ProtocolAggregate, ProtocolAggregateError> {
    let resolved_identity = resolve_protocol_run_identity(record_path)?;
    for artifact in &artifacts {
        validate_protocol_artifact_identity(artifact, &resolved_identity)
            .map_err(protocol_artifact_identity_error)?;
    }

    let artifact_counts = count_artifacts(&artifacts);
    let anchor_entry =
        latest_artifact(&artifacts, TOOL_CALL_INTENT_SEGMENTATION).ok_or_else(|| {
            ProtocolAggregateError::MissingAnchor {
                record_path: record_path.to_path_buf(),
            }
        })?;
    let anchor = normalize_anchor(anchor_entry)?;
    let call_index_to_segment_index = build_call_segment_lookup(&anchor.segments);

    let mut accepted_call_reviews: BTreeMap<usize, ProtocolCallReviewRow> = BTreeMap::new();
    let mut accepted_segment_reviews: BTreeMap<usize, ProtocolSegmentReviewRow> = BTreeMap::new();
    let mut skipped_segment_reviews = Vec::new();

    for entry in artifacts
        .iter()
        .filter(|entry| entry.stored.procedure_name == TOOL_CALL_REVIEW)
    {
        match normalize_call_review(entry, &anchor, &call_index_to_segment_index) {
            Ok(row) => insert_latest_call_review(&mut accepted_call_reviews, row),
            Err(ProtocolAggregateError::DeserializeArtifact { .. })
            | Err(ProtocolAggregateError::InvalidArtifactField { .. }) => continue,
            Err(err) => return Err(err),
        }
    }

    for entry in artifacts
        .iter()
        .filter(|entry| entry.stored.procedure_name == TOOL_CALL_SEGMENT_REVIEW)
    {
        match normalize_segment_review(entry, &anchor) {
            Ok(row) => {
                insert_latest_segment_review(&mut accepted_segment_reviews, row);
            }
            Err(ProtocolAggregateError::SegmentBasisMismatch { .. }) => {
                skipped_segment_reviews.push(describe_segment_mismatch(entry, &anchor)?);
            }
            Err(ProtocolAggregateError::DeserializeArtifact { .. })
            | Err(ProtocolAggregateError::InvalidArtifactField { .. }) => continue,
            Err(err) => return Err(err),
        }
    }

    let present_call_indices = accepted_call_reviews.keys().copied().collect::<Vec<_>>();
    let present_segment_indices = accepted_segment_reviews.keys().copied().collect::<Vec<_>>();
    let call_reviews = accepted_call_reviews.into_values().collect::<Vec<_>>();
    let segment_reviews = accepted_segment_reviews.into_values().collect::<Vec<_>>();

    let mut crosswalk = Vec::with_capacity(anchor.segments.len());
    let mut segment_review_lookup: BTreeMap<usize, ProtocolArtifactRef> = BTreeMap::new();
    for row in &segment_reviews {
        segment_review_lookup.insert(row.basis.segment_index, row.artifact.clone());
    }

    for basis in &anchor.segments {
        let reviewed_call_indices = call_reviews
            .iter()
            .filter(|row| row.segment_index == Some(basis.segment_index))
            .map(|row| row.focal_call_index)
            .collect::<Vec<_>>();
        crosswalk.push(ProtocolCrosswalkRow {
            basis: basis.clone(),
            reviewed_call_indices,
            segment_review: segment_review_lookup.get(&basis.segment_index).cloned(),
        });
    }

    let total_calls_in_run = anchor.coverage.total_calls;
    let missing_call_indices = missing_indices(total_calls_in_run, present_call_indices);
    let missing_segment_indices = missing_indices(anchor.segments.len(), present_segment_indices);

    let coverage = ProtocolCoverage {
        scanned_artifact_count: artifacts.len(),
        artifact_counts,
        total_calls_in_run,
        total_segments_in_anchor: anchor.segments.len(),
        reviewed_call_count: call_reviews.len(),
        reviewed_segment_count: segment_reviews.len(),
        missing_call_indices,
        missing_segment_indices,
        skipped_segment_review_count: skipped_segment_reviews.len(),
        segment_anchor_mismatch_count: skipped_segment_reviews.len(),
    };

    let derived_metrics = ProtocolDerivedMetrics {
        call_review_overall_counts: count_string_field(
            call_reviews.iter().map(|row| row.overall.clone()),
        ),
        segment_review_overall_counts: count_string_field(
            segment_reviews.iter().map(|row| row.overall.clone()),
        ),
        call_review_confidence_counts: count_string_field(
            call_reviews
                .iter()
                .map(|row| row.overall_confidence.clone()),
        ),
        segment_review_confidence_counts: count_string_field(
            segment_reviews
                .iter()
                .map(|row| row.overall_confidence.clone()),
        ),
        calls_with_segment_crosswalk: call_reviews
            .iter()
            .filter(|row| row.segment_index.is_some())
            .count(),
        calls_without_segment_crosswalk: call_reviews
            .iter()
            .filter(|row| row.segment_index.is_none())
            .count(),
        average_calls_per_anchor_segment: if anchor.segments.is_empty() {
            0.0
        } else {
            total_calls_in_run as f64 / anchor.segments.len() as f64
        },
    };

    Ok(ProtocolAggregate {
        run: ProtocolRunIdentity {
            record_path: resolved_identity.record_path,
            run_dir: resolved_identity.run_dir,
            run_id: resolved_identity.run_id,
            subject_id: resolved_identity.subject_id,
        },
        coverage,
        segmentation: anchor,
        call_reviews,
        segment_reviews,
        crosswalk,
        derived_metrics,
        skipped_segment_reviews,
    })
}

fn normalize_anchor(
    entry: &StoredProtocolArtifactFile,
) -> Result<ProtocolSegmentationAnchor, ProtocolAggregateError> {
    let output: SegmentedToolCallSequence = from_artifact_output(entry)?;
    let artifact = artifact_ref(entry);
    let coverage = ProtocolSegmentationCoverage {
        total_calls: output.coverage.total_calls,
        labeled_calls: output.coverage.labeled_calls,
        ambiguous_calls: output.coverage.ambiguous_calls,
        labeled_segments: output.coverage.labeled_segments,
        ambiguous_segments: output.coverage.ambiguous_segments,
        uncovered_calls: output.coverage.uncovered_calls,
    };

    let segments = output
        .segments
        .into_iter()
        .map(|segment| {
            let call_count = segment.calls.len();
            let call_indices = segment.calls.into_iter().map(|call| call.index).collect();
            ProtocolSegmentBasis {
                segment_index: segment.segment_index,
                label: segment.label,
                status: segment.status,
                confidence: Some(segment.confidence),
                rationale: Some(segment.rationale),
                start_index: segment.start_index,
                end_index: segment.end_index,
                turn_span: segment
                    .turns
                    .into_iter()
                    .map(|turn| turn as usize)
                    .collect(),
                call_indices,
                call_count,
            }
        })
        .collect();

    Ok(ProtocolSegmentationAnchor {
        artifact,
        coverage,
        segments,
    })
}

fn normalize_call_review(
    entry: &StoredProtocolArtifactFile,
    anchor: &ProtocolSegmentationAnchor,
    call_index_to_segment_index: &BTreeMap<usize, usize>,
) -> Result<ProtocolCallReviewRow, ProtocolAggregateError> {
    let output: RawCallReviewOutput = from_artifact_output(entry)?;
    let artifact = artifact_ref(entry);
    let segment_index = call_index_to_segment_index
        .get(&output.packet.focal_call_index)
        .copied();
    let segment = segment_index.and_then(|segment_index| {
        anchor
            .segments
            .iter()
            .find(|basis| basis.segment_index == segment_index)
    });
    let scope_call_indices = output
        .packet
        .calls
        .iter()
        .map(|call| call.index)
        .collect::<Vec<_>>();

    Ok(ProtocolCallReviewRow {
        artifact,
        focal_call_index: output.packet.focal_call_index,
        segment_index: segment.as_ref().map(|basis| basis.segment_index),
        segment_label: segment.and_then(|basis| basis.label.map(intent_label_name)),
        segment_status: segment
            .as_ref()
            .map(|basis| segment_status_name(basis.status).to_string()),
        scope_call_indices,
        total_calls_in_run: output.packet.total_calls_in_run,
        total_calls_in_scope: output.packet.total_calls_in_scope,
        turn_span: output.packet.turn_span,
        overall: output.overall,
        overall_confidence: output.overall_confidence,
        usefulness: output.usefulness.into(),
        redundancy: output.redundancy.into(),
        recoverability: output.recoverability.into(),
        signals: output.signals.into(),
    })
}

fn normalize_segment_review(
    entry: &StoredProtocolArtifactFile,
    anchor: &ProtocolSegmentationAnchor,
) -> Result<ProtocolSegmentReviewRow, ProtocolAggregateError> {
    let output: RawSegmentReviewOutput = from_artifact_output(entry)?;
    let basis = anchor
        .segments
        .iter()
        .find(|segment| segment.segment_index == output.packet.segment_index)
        .ok_or_else(|| ProtocolAggregateError::SegmentBasisMismatch {
            path: entry.path.clone(),
            segment_index: output.packet.segment_index,
        })?;
    let observed_scope_call_indices = output
        .packet
        .calls
        .iter()
        .map(|call| call.index)
        .collect::<Vec<_>>();

    if output
        .packet
        .segment_label
        .as_ref()
        .is_some_and(|label| intent_label_name_opt(basis.label) != Some(label.as_str()))
        || output
            .packet
            .segment_status
            .as_ref()
            .is_some_and(|status| segment_status_name(basis.status) != status)
        || basis.turn_span != output.packet.turn_span
        || basis.call_count != output.packet.total_calls_in_scope
    {
        return Err(ProtocolAggregateError::SegmentBasisMismatch {
            path: entry.path.clone(),
            segment_index: output.packet.segment_index,
        });
    }

    Ok(ProtocolSegmentReviewRow {
        artifact: artifact_ref(entry),
        basis: basis.clone(),
        scope_call_indices: observed_scope_call_indices,
        total_calls_in_run: output.packet.total_calls_in_run,
        total_calls_in_scope: output.packet.total_calls_in_scope,
        overall: output.overall,
        overall_confidence: output.overall_confidence,
        usefulness: output.usefulness.into(),
        redundancy: output.redundancy.into(),
        recoverability: output.recoverability.into(),
        signals: output.signals.into(),
    })
}

fn describe_segment_mismatch(
    entry: &StoredProtocolArtifactFile,
    anchor: &ProtocolSegmentationAnchor,
) -> Result<ProtocolSkippedSegmentReview, ProtocolAggregateError> {
    let output: RawSegmentReviewOutput = from_artifact_output(entry)?;
    let basis = anchor
        .segments
        .iter()
        .find(|segment| segment.segment_index == output.packet.segment_index)
        .ok_or_else(|| ProtocolAggregateError::SegmentBasisMismatch {
            path: entry.path.clone(),
            segment_index: output.packet.segment_index,
        })?;

    Ok(ProtocolSkippedSegmentReview {
        artifact: artifact_ref(entry),
        segment_index: output.packet.segment_index,
        observed_label: output.packet.segment_label,
        observed_status: output
            .packet
            .segment_status
            .unwrap_or_else(|| "<missing>".to_string()),
        observed_turn_span: output.packet.turn_span,
        observed_total_calls_in_scope: output.packet.total_calls_in_scope,
        anchor_label: basis.label.map(intent_label_name),
        anchor_status: segment_status_name(basis.status).to_string(),
        anchor_turn_span: basis.turn_span.clone(),
        anchor_total_calls_in_scope: basis.call_count,
        reason: "segment review does not match selected anchor basis".to_string(),
    })
}

fn protocol_artifact_identity_error(
    mismatch: ProtocolArtifactIdentityMismatch,
) -> ProtocolAggregateError {
    ProtocolAggregateError::ArtifactIdentityMismatch {
        path: mismatch.path,
        procedure: mismatch.procedure_name,
        field: mismatch.field,
        expected: mismatch.expected,
        actual: mismatch.actual,
    }
}

fn artifact_ref(entry: &StoredProtocolArtifactFile) -> ProtocolArtifactRef {
    ProtocolArtifactRef {
        path: entry.path.clone(),
        created_at_ms: entry.stored.created_at_ms,
        procedure_name: entry.stored.procedure_name.clone(),
    }
}

fn from_artifact_output<T: for<'de> Deserialize<'de>>(
    entry: &StoredProtocolArtifactFile,
) -> Result<T, ProtocolAggregateError> {
    serde_json::from_value(entry.stored.output.clone()).map_err(|source| {
        ProtocolAggregateError::DeserializeArtifact {
            path: entry.path.clone(),
            source,
        }
    })
}

fn latest_artifact<'a>(
    entries: &'a [StoredProtocolArtifactFile],
    procedure_name: &str,
) -> Option<&'a StoredProtocolArtifactFile> {
    entries
        .iter()
        .filter(|entry| entry.stored.procedure_name == procedure_name)
        .max_by(|left, right| {
            (left.stored.created_at_ms, &left.path).cmp(&(right.stored.created_at_ms, &right.path))
        })
}

fn insert_latest_call_review(
    entries: &mut BTreeMap<usize, ProtocolCallReviewRow>,
    value: ProtocolCallReviewRow,
) {
    let index = value.focal_call_index;
    // Today we collapse to one accepted review per focal call. If we later keep
    // multiple protocol artifacts for the same tool-call target, widen this key
    // rather than relaxing identity validation.
    match entries.entry(index) {
        std::collections::btree_map::Entry::Vacant(slot) => {
            slot.insert(value);
        }
        std::collections::btree_map::Entry::Occupied(mut slot) => {
            if is_newer_artifact(&value.artifact, &slot.get().artifact) {
                slot.insert(value);
            }
        }
    }
}

fn insert_latest_segment_review(
    entries: &mut BTreeMap<usize, ProtocolSegmentReviewRow>,
    value: ProtocolSegmentReviewRow,
) {
    let index = value.basis.segment_index;
    match entries.entry(index) {
        std::collections::btree_map::Entry::Vacant(slot) => {
            slot.insert(value);
        }
        std::collections::btree_map::Entry::Occupied(mut slot) => {
            if is_newer_artifact(&value.artifact, &slot.get().artifact) {
                slot.insert(value);
            }
        }
    }
}

fn is_newer_artifact(left: &ProtocolArtifactRef, right: &ProtocolArtifactRef) -> bool {
    (left.created_at_ms, &left.path) > (right.created_at_ms, &right.path)
}

fn build_call_segment_lookup(segments: &[ProtocolSegmentBasis]) -> BTreeMap<usize, usize> {
    let mut lookup = BTreeMap::new();
    for segment in segments {
        for index in &segment.call_indices {
            lookup.insert(*index, segment.segment_index);
        }
    }
    lookup
}

fn missing_indices<I>(total: usize, present: I) -> Vec<usize>
where
    I: IntoIterator<Item = usize>,
{
    let present = present
        .into_iter()
        .collect::<std::collections::BTreeSet<_>>();
    (0..total)
        .filter(|index| !present.contains(index))
        .collect()
}

fn count_artifacts(entries: &[StoredProtocolArtifactFile]) -> BTreeMap<String, usize> {
    let mut counts = BTreeMap::new();
    for entry in entries {
        *counts
            .entry(entry.stored.procedure_name.clone())
            .or_insert(0) += 1;
    }
    counts
}

fn count_string_field(values: impl Iterator<Item = String>) -> BTreeMap<String, usize> {
    let mut counts = BTreeMap::new();
    for value in values {
        *counts.entry(value).or_insert(0) += 1;
    }
    counts
}

#[derive(Debug, Deserialize, Clone)]
struct RawIndexedCall {
    index: usize,
}

#[derive(Debug, Deserialize)]
struct RawCallReviewOutput {
    overall: String,
    overall_confidence: String,
    packet: RawCallReviewPacket,
    recoverability: RawBranchAssessment,
    redundancy: RawBranchAssessment,
    signals: RawSignals,
    usefulness: RawBranchAssessment,
}

#[derive(Debug, Deserialize)]
struct RawSegmentReviewOutput {
    overall: String,
    overall_confidence: String,
    packet: RawSegmentReviewPacket,
    recoverability: RawBranchAssessment,
    redundancy: RawBranchAssessment,
    signals: RawSignals,
    usefulness: RawBranchAssessment,
}

#[allow(dead_code)]
#[derive(Debug, Deserialize)]
struct RawCallReviewPacket {
    calls: Vec<RawIndexedCall>,
    focal_call_index: usize,
    scope_summary: String,
    subject_id: String,
    target_id: String,
    target_kind: String,
    total_calls_in_run: usize,
    total_calls_in_scope: usize,
    turn_span: Vec<usize>,
}

#[allow(dead_code)]
#[derive(Debug, Deserialize)]
struct RawSegmentReviewPacket {
    calls: Vec<RawIndexedCall>,
    scope_summary: String,
    segment_index: usize,
    #[serde(default)]
    segment_label: Option<String>,
    #[serde(default)]
    segment_status: Option<String>,
    subject_id: String,
    target_id: String,
    target_kind: String,
    total_calls_in_run: usize,
    total_calls_in_scope: usize,
    turn_span: Vec<usize>,
}

#[derive(Debug, Deserialize)]
struct RawBranchAssessment {
    confidence: String,
    rationale: String,
    verdict: String,
}

#[derive(Debug, Deserialize)]
struct RawSignals {
    #[serde(default)]
    browse_calls_in_scope: Option<usize>,
    #[serde(default)]
    candidate_concerns: Vec<String>,
    #[serde(default)]
    directory_pivots: Option<usize>,
    #[serde(default)]
    distinct_tool_count: Option<usize>,
    #[serde(default)]
    edit_calls_in_scope: Option<usize>,
    #[serde(default)]
    execute_calls_in_scope: Option<usize>,
    #[serde(default)]
    failed_calls_in_scope: Option<usize>,
    #[serde(default)]
    labeled_segments_in_source: Option<usize>,
    #[serde(default)]
    ambiguous_segments_in_source: Option<usize>,
    #[serde(default)]
    read_calls_in_scope: Option<usize>,
    #[serde(default)]
    repeated_tool_name_count: Option<usize>,
    #[serde(default)]
    scope_turn_count: Option<usize>,
    #[serde(default)]
    search_calls_in_scope: Option<usize>,
    #[serde(default)]
    similar_search_neighbors: Option<usize>,
    #[serde(default)]
    uncovered_calls_in_source: Option<usize>,
}

fn segment_status_name(status: SegmentStatus) -> &'static str {
    match status {
        SegmentStatus::Labeled => "labeled",
        SegmentStatus::Ambiguous => "ambiguous",
    }
}

fn intent_label_name(label: IntentLabel) -> String {
    match label {
        IntentLabel::LocateTarget => "locate_target".to_string(),
        IntentLabel::InspectCandidate => "inspect_candidate".to_string(),
        IntentLabel::RefineSearch => "refine_search".to_string(),
        IntentLabel::ValidateHypothesis => "validate_hypothesis".to_string(),
        IntentLabel::EditAttempt => "edit_attempt".to_string(),
        IntentLabel::Recovery => "recovery".to_string(),
        IntentLabel::Other => "other".to_string(),
    }
}

fn intent_label_name_opt(label: Option<IntentLabel>) -> Option<&'static str> {
    label.map(|label| match label {
        IntentLabel::LocateTarget => "locate_target",
        IntentLabel::InspectCandidate => "inspect_candidate",
        IntentLabel::RefineSearch => "refine_search",
        IntentLabel::ValidateHypothesis => "validate_hypothesis",
        IntentLabel::EditAttempt => "edit_attempt",
        IntentLabel::Recovery => "recovery",
        IntentLabel::Other => "other",
    })
}

impl From<RawBranchAssessment> for ProtocolBranchAssessment {
    fn from(value: RawBranchAssessment) -> Self {
        Self {
            verdict: value.verdict,
            confidence: value.confidence,
            rationale: value.rationale,
        }
    }
}

impl From<RawSignals> for ProtocolReviewSignals {
    fn from(value: RawSignals) -> Self {
        Self {
            browse_calls_in_scope: value.browse_calls_in_scope,
            candidate_concerns: value.candidate_concerns,
            directory_pivots: value.directory_pivots,
            distinct_tool_count: value.distinct_tool_count,
            edit_calls_in_scope: value.edit_calls_in_scope,
            execute_calls_in_scope: value.execute_calls_in_scope,
            failed_calls_in_scope: value.failed_calls_in_scope,
            labeled_segments_in_source: value.labeled_segments_in_source,
            ambiguous_segments_in_source: value.ambiguous_segments_in_source,
            read_calls_in_scope: value.read_calls_in_scope,
            repeated_tool_name_count: value.repeated_tool_name_count,
            scope_turn_count: value.scope_turn_count,
            search_calls_in_scope: value.search_calls_in_scope,
            similar_search_neighbors: value.similar_search_neighbors,
            uncovered_calls_in_source: value.uncovered_calls_in_source,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::inner::core::{RegisteredRunRole, RunIntent, RunStorageRoots};
    use crate::inner::registry::RunRegistration;
    use crate::spec::EvalBudget;
    use serde_json::Value;
    use std::sync::{Mutex, MutexGuard, OnceLock};
    use tempfile::TempDir;

    struct TestRunFixture {
        _env_lock: MutexGuard<'static, ()>,
        _tmp: TempDir,
        record_path: PathBuf,
    }

    fn env_lock() -> &'static Mutex<()> {
        static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
        LOCK.get_or_init(|| Mutex::new(()))
    }

    fn registered_run(run_id: &str, subject_id: &str) -> TestRunFixture {
        let env_lock = env_lock().lock().expect("env lock");
        let tmp = tempfile::tempdir().expect("tmp");
        unsafe {
            std::env::set_var("PLOKE_EVAL_HOME", tmp.path());
        }

        let intent = RunIntent {
            task_id: subject_id.to_string(),
            repo_root: tmp.path().join("repo"),
            storage_roots: RunStorageRoots::new(
                tmp.path().join("registries"),
                tmp.path().join("instances").join(subject_id).join("runs"),
            ),
            base_sha: Some("deadbeef".to_string()),
            budget: EvalBudget::default(),
            model_id: Some("model".to_string()),
            provider_slug: Some("provider".to_string()),
            campaign_id: None,
            batch_id: None,
            run_arm_id: "structured-current-policy".to_string(),
            run_role: RegisteredRunRole::Treatment,
        };
        let registration =
            RunRegistration::register_with_run_id(intent, run_id).expect("registration");
        let record_path = registration.artifacts.record_path.clone();
        registration.persist().expect("persist registration");

        TestRunFixture {
            _env_lock: env_lock,
            _tmp: tmp,
            record_path,
        }
    }

    fn turn_context_json() -> Value {
        serde_json::json!({
            "turn": 1,
            "tool_count": 4,
            "failed_tool_count": 1,
            "patch_proposed": false,
            "patch_applied": false
        })
    }

    fn call_json(
        index: usize,
        turn: u32,
        tool_name: &str,
        tool_kind: &str,
        summary: &str,
    ) -> Value {
        serde_json::json!({
            "index": index,
            "turn": turn,
            "tool_name": tool_name,
            "tool_kind": tool_kind,
            "failed": false,
            "latency_ms": 10,
            "summary": summary,
            "args_preview": "",
            "result_preview": ""
        })
    }

    fn artifact(
        path: &str,
        procedure_name: &str,
        created_at_ms: u64,
        subject_id: &str,
        run_id: &str,
        output: serde_json::Value,
    ) -> StoredProtocolArtifactFile {
        StoredProtocolArtifactFile {
            path: PathBuf::from(path),
            stored: crate::protocol_artifacts::StoredProtocolArtifact {
                schema_version: crate::protocol_artifacts::PROTOCOL_ARTIFACT_SCHEMA_VERSION
                    .to_string(),
                procedure_name: procedure_name.to_string(),
                subject_id: subject_id.to_string(),
                run_id: run_id.to_string(),
                created_at_ms,
                model_id: None,
                provider_slug: None,
                input: serde_json::json!({}),
                output,
                artifact: serde_json::json!({}),
            },
        }
    }

    #[test]
    fn normalizes_latest_unique_rows_and_rejects_basis_mismatch() {
        let run_id = "run-tokio-5583";
        let subject_id = "tokio-rs__tokio-5583";
        let fixture = registered_run(run_id, subject_id);
        let record_path = fixture.record_path.clone();

        let anchor = artifact(
            "/tmp/run/1000_tool_call_intent_segmentation_tokio-rs__tokio-5583.json",
            TOOL_CALL_INTENT_SEGMENTATION,
            1000,
            subject_id,
            run_id,
            serde_json::json!({
                "coverage": {
                    "ambiguous_calls": 0,
                    "ambiguous_segments": 0,
                    "labeled_calls": 4,
                    "labeled_segments": 2,
                    "total_calls": 4,
                    "uncovered_calls": 0
                },
                "segments": [
                    {
                        "calls": [
                            call_json(0, 1, "rg", "search", "find target"),
                            call_json(1, 1, "sed", "read", "inspect file")
                        ],
                        "confidence": "high",
                        "end_index": 1,
                        "label": "locate_target",
                        "rationale": "locate",
                        "segment_index": 0,
                        "start_index": 0,
                        "status": "labeled",
                        "turns": [1]
                    },
                    {
                        "calls": [
                            call_json(2, 1, "rg", "search", "refine search"),
                            call_json(3, 1, "sed", "read", "inspect candidate")
                        ],
                        "confidence": "high",
                        "end_index": 3,
                        "label": "inspect_candidate",
                        "rationale": "inspect",
                        "segment_index": 1,
                        "start_index": 2,
                        "status": "labeled",
                        "turns": [1]
                    }
                ],
                "sequence": {
                    "subject_id": subject_id,
                    "total_turns": 1,
                    "total_calls_in_run": 4,
                    "turns": [turn_context_json()],
                    "calls": [
                        call_json(0, 1, "rg", "search", "find target"),
                        call_json(1, 1, "sed", "read", "inspect file"),
                        call_json(2, 1, "rg", "search", "refine search"),
                        call_json(3, 1, "sed", "read", "inspect candidate")
                    ]
                },
                "signals": {
                    "browse_calls": 1,
                    "directory_pivots": 1,
                    "edit_calls": 0,
                    "execute_calls": 0,
                    "failed_calls": 1,
                    "read_calls": 2,
                    "repeated_search_runs": 1,
                    "search_calls": 2,
                    "search_terms_seen": [],
                    "total_calls": 4,
                    "total_turns": 1
                },
                "overall_rationale": "two labeled segments"
            }),
        );

        let call_review_old = artifact(
            "/tmp/run/2000_tool_call_review_tokio-rs__tokio-5583.json",
            TOOL_CALL_REVIEW,
            2000,
            subject_id,
            run_id,
            serde_json::json!({
                "overall": "mixed",
                "overall_confidence": "medium",
                "packet": {
                    "calls": [{"index": 1}],
                    "focal_call_index": 1,
                    "scope_summary": "focal call 1",
                    "subject_id": subject_id,
                    "target_id": "call:1",
                    "target_kind": "focal_call",
                    "total_calls_in_run": 4,
                    "total_calls_in_scope": 1,
                    "turn_span": [1]
                },
                "recoverability": {
                    "confidence": "high",
                    "rationale": "recover",
                    "verdict": "clear_next_step"
                },
                "redundancy": {
                    "confidence": "high",
                    "rationale": "redundant",
                    "verdict": "distinct"
                },
                "signals": {
                    "browse_calls_in_scope": 0,
                    "candidate_concerns": ["RecoveryOpportunity"],
                    "directory_pivots": 0,
                    "distinct_tool_count": 1,
                    "edit_calls_in_scope": 0,
                    "execute_calls_in_scope": 0,
                    "failed_calls_in_scope": 0,
                    "read_calls_in_scope": 0,
                    "repeated_tool_name_count": 0,
                    "scope_turn_count": 1,
                    "search_calls_in_scope": 0,
                    "similar_search_neighbors": 0
                },
                "usefulness": {
                    "confidence": "medium",
                    "rationale": "useful",
                    "verdict": "helpful_but_non_essential"
                }
            }),
        );

        let call_review_new = artifact(
            "/tmp/run/3000_tool_call_review_tokio-rs__tokio-5583.json",
            TOOL_CALL_REVIEW,
            3000,
            subject_id,
            run_id,
            serde_json::json!({
                "overall": "focused_progress",
                "overall_confidence": "high",
                "packet": {
                    "calls": [{"index": 1}],
                    "focal_call_index": 1,
                    "scope_summary": "focal call 1",
                    "subject_id": subject_id,
                    "target_id": "call:1",
                    "target_kind": "focal_call",
                    "total_calls_in_run": 4,
                    "total_calls_in_scope": 1,
                    "turn_span": [1]
                },
                "recoverability": {
                    "confidence": "high",
                    "rationale": "recover",
                    "verdict": "clear_next_step"
                },
                "redundancy": {
                    "confidence": "high",
                    "rationale": "redundant",
                    "verdict": "distinct"
                },
                "signals": {
                    "browse_calls_in_scope": 0,
                    "candidate_concerns": ["RecoveryOpportunity"],
                    "directory_pivots": 0,
                    "distinct_tool_count": 1,
                    "edit_calls_in_scope": 0,
                    "execute_calls_in_scope": 0,
                    "failed_calls_in_scope": 0,
                    "read_calls_in_scope": 0,
                    "repeated_tool_name_count": 0,
                    "scope_turn_count": 1,
                    "search_calls_in_scope": 0,
                    "similar_search_neighbors": 0
                },
                "usefulness": {
                    "confidence": "high",
                    "rationale": "useful",
                    "verdict": "key_progress"
                }
            }),
        );

        let call_review_two = artifact(
            "/tmp/run/2500_tool_call_review_tokio-rs__tokio-5583.json",
            TOOL_CALL_REVIEW,
            2500,
            subject_id,
            run_id,
            serde_json::json!({
                "overall": "mixed",
                "overall_confidence": "medium",
                "packet": {
                    "calls": [{"index": 2}],
                    "focal_call_index": 2,
                    "scope_summary": "focal call 2",
                    "subject_id": subject_id,
                    "target_id": "call:2",
                    "target_kind": "focal_call",
                    "total_calls_in_run": 4,
                    "total_calls_in_scope": 1,
                    "turn_span": [1]
                },
                "recoverability": {
                    "confidence": "high",
                    "rationale": "recover",
                    "verdict": "clear_next_step"
                },
                "redundancy": {
                    "confidence": "high",
                    "rationale": "redundant",
                    "verdict": "distinct"
                },
                "signals": {
                    "browse_calls_in_scope": 0,
                    "candidate_concerns": [],
                    "directory_pivots": 0,
                    "distinct_tool_count": 1,
                    "edit_calls_in_scope": 0,
                    "execute_calls_in_scope": 0,
                    "failed_calls_in_scope": 0,
                    "read_calls_in_scope": 0,
                    "repeated_tool_name_count": 0,
                    "scope_turn_count": 1,
                    "search_calls_in_scope": 0,
                    "similar_search_neighbors": 0
                },
                "usefulness": {
                    "confidence": "medium",
                    "rationale": "useful",
                    "verdict": "helpful_but_non_essential"
                }
            }),
        );

        let segment_review_ok = artifact(
            "/tmp/run/4000_tool_call_segment_review_tokio-rs__tokio-5583.json",
            TOOL_CALL_SEGMENT_REVIEW,
            4000,
            subject_id,
            run_id,
            serde_json::json!({
                "overall": "focused_progress",
                "overall_confidence": "medium",
                "packet": {
                    "calls": [{"index": 2}, {"index": 3}],
                    "scope_summary": "segment 1",
                    "segment_index": 1,
                    "segment_label": "inspect_candidate",
                    "segment_status": "labeled",
                    "subject_id": subject_id,
                    "target_id": "segment:1",
                    "target_kind": "segment",
                    "total_calls_in_run": 4,
                    "total_calls_in_scope": 2,
                    "turn_span": [1]
                },
                "recoverability": {
                    "confidence": "high",
                    "rationale": "recover",
                    "verdict": "no_recovery_needed"
                },
                "redundancy": {
                    "confidence": "high",
                    "rationale": "redundant",
                    "verdict": "distinct"
                },
                "signals": {
                    "ambiguous_segments_in_source": 0,
                    "browse_calls_in_scope": 0,
                    "candidate_concerns": ["RecoveryOpportunity"],
                    "directory_pivots": 0,
                    "distinct_tool_count": 1,
                    "edit_calls_in_scope": 0,
                    "execute_calls_in_scope": 0,
                    "failed_calls_in_scope": 0,
                    "labeled_segments_in_source": 2,
                    "read_calls_in_scope": 0,
                    "repeated_tool_name_count": 0,
                    "scope_turn_count": 1,
                    "search_calls_in_scope": 2,
                    "similar_search_neighbors": 0,
                    "uncovered_calls_in_source": 0
                },
                "usefulness": {
                    "confidence": "high",
                    "rationale": "useful",
                    "verdict": "key_progress"
                }
            }),
        );

        let segment_review_mismatch = artifact(
            "/tmp/run/5000_tool_call_segment_review_tokio-rs__tokio-5583.json",
            TOOL_CALL_SEGMENT_REVIEW,
            5000,
            subject_id,
            run_id,
            serde_json::json!({
                "overall": "mixed",
                "overall_confidence": "low",
                "packet": {
                    "calls": [{"index": 2}, {"index": 3}],
                    "scope_summary": "segment 1 but drifted",
                    "segment_index": 1,
                    "segment_label": "inspect_candidate_drifted",
                    "segment_status": "labeled",
                    "subject_id": subject_id,
                    "target_id": "segment:1",
                    "target_kind": "segment",
                    "total_calls_in_run": 4,
                    "total_calls_in_scope": 3,
                    "turn_span": [1, 2]
                },
                "recoverability": {
                    "confidence": "low",
                    "rationale": "recover",
                    "verdict": "clear_next_step"
                },
                "redundancy": {
                    "confidence": "low",
                    "rationale": "redundant",
                    "verdict": "distinct"
                },
                "signals": {
                    "ambiguous_segments_in_source": 0,
                    "browse_calls_in_scope": 0,
                    "candidate_concerns": [],
                    "directory_pivots": 0,
                    "distinct_tool_count": 1,
                    "edit_calls_in_scope": 0,
                    "execute_calls_in_scope": 0,
                    "failed_calls_in_scope": 0,
                    "labeled_segments_in_source": 2,
                    "read_calls_in_scope": 0,
                    "repeated_tool_name_count": 0,
                    "scope_turn_count": 1,
                    "search_calls_in_scope": 2,
                    "similar_search_neighbors": 0,
                    "uncovered_calls_in_source": 0
                },
                "usefulness": {
                    "confidence": "low",
                    "rationale": "useful",
                    "verdict": "helpful_but_non_essential"
                }
            }),
        );

        let aggregate = load_protocol_aggregate_from_artifacts(
            &record_path,
            vec![
                anchor,
                call_review_old,
                call_review_new,
                call_review_two,
                segment_review_ok,
                segment_review_mismatch,
            ],
        )
        .expect("aggregate should load");

        assert_eq!(aggregate.run.run_id, run_id);
        assert_eq!(aggregate.run.subject_id, subject_id);
        assert_eq!(aggregate.coverage.total_calls_in_run, 4);
        assert_eq!(aggregate.coverage.reviewed_call_count, 2);
        assert_eq!(aggregate.coverage.reviewed_segment_count, 1);
        assert_eq!(aggregate.coverage.missing_call_indices, vec![0, 3]);
        assert_eq!(aggregate.coverage.missing_segment_indices, vec![0]);
        assert_eq!(aggregate.skipped_segment_reviews.len(), 1);
        assert_eq!(aggregate.call_reviews.len(), 2);
        assert_eq!(aggregate.segment_reviews.len(), 1);
        assert_eq!(aggregate.call_reviews[0].overall, "focused_progress");
        assert_eq!(aggregate.call_reviews[0].focal_call_index, 1);
        assert_eq!(aggregate.call_reviews[0].segment_index, Some(0));
        assert_eq!(aggregate.call_reviews[1].focal_call_index, 2);
        assert_eq!(aggregate.call_reviews[1].segment_index, Some(1));
        assert_eq!(aggregate.segment_reviews[0].basis.segment_index, 1);
        assert_eq!(
            aggregate.segment_reviews[0].basis.label,
            Some(IntentLabel::InspectCandidate)
        );
        assert_eq!(aggregate.crosswalk[1].reviewed_call_indices, vec![2]);
        assert_eq!(
            aggregate.crosswalk[1]
                .segment_review
                .as_ref()
                .unwrap()
                .created_at_ms,
            4000
        );
        assert_eq!(
            aggregate
                .derived_metrics
                .call_review_overall_counts
                .get("mixed"),
            Some(&1)
        );
        assert_eq!(
            aggregate
                .derived_metrics
                .call_review_overall_counts
                .get("focused_progress"),
            Some(&1)
        );
        assert_eq!(aggregate.derived_metrics.calls_with_segment_crosswalk, 2);
    }

    #[test]
    fn loads_ambiguous_anchor_segment_without_label() {
        let run_id = "run-tokio-ambiguous";
        let subject_id = "tokio-rs__tokio-ambiguous";
        let fixture = registered_run(run_id, subject_id);
        let record_path = fixture.record_path.clone();

        let anchor = artifact(
            "/tmp/run/1000_tool_call_intent_segmentation_tokio-rs__tokio-ambiguous.json",
            TOOL_CALL_INTENT_SEGMENTATION,
            1000,
            subject_id,
            run_id,
            serde_json::json!({
                "coverage": {
                    "ambiguous_calls": 2,
                    "ambiguous_segments": 1,
                    "labeled_calls": 0,
                    "labeled_segments": 0,
                    "total_calls": 2,
                    "uncovered_calls": 0
                },
                "segments": [
                    {
                        "calls": [
                            call_json(0, 1, "rg", "search", "search broadly"),
                            call_json(1, 1, "fd", "search", "pivot search")
                        ],
                        "confidence": "medium",
                        "end_index": 1,
                        "rationale": "coherent but hard to classify",
                        "segment_index": 0,
                        "start_index": 0,
                        "status": "ambiguous",
                        "turns": [1]
                    }
                ],
                "sequence": {
                    "subject_id": subject_id,
                    "total_turns": 1,
                    "total_calls_in_run": 2,
                    "turns": [turn_context_json()],
                    "calls": [
                        call_json(0, 1, "rg", "search", "search broadly"),
                        call_json(1, 1, "fd", "search", "pivot search")
                    ]
                },
                "signals": {
                    "browse_calls": 0,
                    "directory_pivots": 0,
                    "edit_calls": 0,
                    "execute_calls": 0,
                    "failed_calls": 0,
                    "read_calls": 0,
                    "repeated_search_runs": 0,
                    "search_calls": 2,
                    "search_terms_seen": [],
                    "total_calls": 2,
                    "total_turns": 1
                },
                "overall_rationale": "one ambiguous segment"
            }),
        );

        let aggregate = load_protocol_aggregate_from_artifacts(&record_path, vec![anchor])
            .expect("aggregate should load ambiguous anchor");

        assert_eq!(aggregate.segmentation.segments.len(), 1);
        assert_eq!(
            aggregate.segmentation.segments[0].status,
            SegmentStatus::Ambiguous
        );
        assert_eq!(aggregate.segmentation.segments[0].label, None);
    }

    #[test]
    fn rejects_mixed_artifacts_with_mismatched_identity() {
        let run_id = "run-tokio-mixed";
        let subject_id = "tokio-rs__tokio-mixed";
        let fixture = registered_run(run_id, subject_id);
        let record_path = fixture.record_path.clone();

        let anchor = artifact(
            "/tmp/run/1000_tool_call_intent_segmentation_tokio-rs__tokio-mixed.json",
            TOOL_CALL_INTENT_SEGMENTATION,
            1000,
            subject_id,
            run_id,
            serde_json::json!({
                "coverage": {
                    "ambiguous_calls": 0,
                    "ambiguous_segments": 0,
                    "labeled_calls": 1,
                    "labeled_segments": 1,
                    "total_calls": 1,
                    "uncovered_calls": 0
                },
                "segments": [
                    {
                        "calls": [call_json(0, 1, "rg", "search", "find target")],
                        "confidence": "high",
                        "end_index": 0,
                        "label": "locate_target",
                        "rationale": "locate",
                        "segment_index": 0,
                        "start_index": 0,
                        "status": "labeled",
                        "turns": [1]
                    }
                ],
                "sequence": {
                    "subject_id": subject_id,
                    "total_turns": 1,
                    "total_calls_in_run": 1,
                    "turns": [turn_context_json()],
                    "calls": [call_json(0, 1, "rg", "search", "find target")]
                },
                "signals": {
                    "browse_calls": 0,
                    "directory_pivots": 0,
                    "edit_calls": 0,
                    "execute_calls": 0,
                    "failed_calls": 0,
                    "read_calls": 0,
                    "repeated_search_runs": 0,
                    "search_calls": 1,
                    "search_terms_seen": [],
                    "total_calls": 1,
                    "total_turns": 1
                },
                "overall_rationale": "single labeled segment"
            }),
        );

        let mismatched_call_review = artifact(
            "/tmp/run/2000_tool_call_review_other.json",
            TOOL_CALL_REVIEW,
            2000,
            "other-subject",
            "run-other",
            serde_json::json!({
                "overall": "mixed",
                "overall_confidence": "medium",
                "packet": {
                    "calls": [{"index": 0}],
                    "focal_call_index": 0,
                    "scope_summary": "focal call 0",
                    "subject_id": "other-subject",
                    "target_id": "call:0",
                    "target_kind": "focal_call",
                    "total_calls_in_run": 1,
                    "total_calls_in_scope": 1,
                    "turn_span": [1]
                },
                "recoverability": {
                    "confidence": "high",
                    "rationale": "recover",
                    "verdict": "clear_next_step"
                },
                "redundancy": {
                    "confidence": "high",
                    "rationale": "redundant",
                    "verdict": "distinct"
                },
                "signals": {
                    "browse_calls_in_scope": 0,
                    "candidate_concerns": [],
                    "directory_pivots": 0,
                    "distinct_tool_count": 1,
                    "edit_calls_in_scope": 0,
                    "execute_calls_in_scope": 0,
                    "failed_calls_in_scope": 0,
                    "read_calls_in_scope": 0,
                    "repeated_tool_name_count": 0,
                    "scope_turn_count": 1,
                    "search_calls_in_scope": 1,
                    "similar_search_neighbors": 0
                },
                "usefulness": {
                    "confidence": "medium",
                    "rationale": "useful",
                    "verdict": "helpful_but_non_essential"
                }
            }),
        );

        let err = load_protocol_aggregate_from_artifacts(
            &record_path,
            vec![anchor, mismatched_call_review],
        )
        .expect_err("mixed identity should fail loudly");

        match err {
            ProtocolAggregateError::ArtifactIdentityMismatch {
                procedure,
                field,
                expected,
                actual,
                ..
            } => {
                assert_eq!(procedure, TOOL_CALL_REVIEW);
                assert_eq!(field, "stored.run_id");
                assert_eq!(expected, run_id);
                assert_eq!(actual, "run-other");
            }
            other => panic!("unexpected error: {other}"),
        }
    }
}
