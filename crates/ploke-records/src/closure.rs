//! Passive Prototype 1 closure-state DTOs.
//!
//! These records mirror `closure-state.json`, the reduced campaign snapshot
//! written by `ploke-eval`. The snapshot is useful for locating baseline eval
//! and protocol evidence, but it is not History authority and does not replace
//! the compressed run records or protocol artifact files it references.

use std::collections::BTreeMap;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::record::{Record, RecordFamily, RecordFormat};

pub const CLOSURE_STATE_SCHEMA_VERSION: &str = "closure-state.v1";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ClosureStateRecord {
    pub schema_version: String,
    pub campaign_id: String,
    pub updated_at: String,
    pub config: ClosureConfigRecord,
    pub registry: RegistryClosureSummaryRecord,
    pub eval: EvalClosureSummaryRecord,
    pub protocol: ProtocolClosureSummaryRecord,
    #[serde(default)]
    pub instances: Vec<ClosureInstanceRowRecord>,
}

impl Record for ClosureStateRecord {
    const FAMILY: RecordFamily = RecordFamily::ClosureState;
    const SCHEMA: &'static str = CLOSURE_STATE_SCHEMA_VERSION;
    const FORMAT: RecordFormat = RecordFormat::Json;
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ClosureConfigRecord {
    pub benchmark_family: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub provider_slug: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub registry_path: Option<PathBuf>,
    #[serde(default)]
    pub dataset_sources: Vec<ClosureDatasetSourceRecord>,
    #[serde(default)]
    pub required_procedures: Vec<String>,
    #[serde(alias = "runs_root")]
    pub instances_root: PathBuf,
    pub batches_root: PathBuf,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub framework: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ClosureDatasetSourceRecord {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub key: Option<String>,
    pub path: PathBuf,
    pub label: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
pub enum ClosureClassRecord {
    Complete,
    Failed,
    Missing,
    Ineligible,
    Incompatible,
    Partial,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
pub enum RegistryInstanceStatusRecord {
    Mapped,
    Missing,
    Ambiguous,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RegistryClosureSummaryRecord {
    pub expected_total: usize,
    pub mapped_total: usize,
    pub missing_total: usize,
    pub ambiguous_total: usize,
    pub status: ClosureClassRecord,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct EvalClosureSummaryRecord {
    pub expected_total: usize,
    pub complete_total: usize,
    pub failed_total: usize,
    pub missing_total: usize,
    pub partial_total: usize,
    pub in_progress_total: usize,
    pub status: ClosureClassRecord,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_transition_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ProtocolClosureSummaryRecord {
    pub expected_total: usize,
    pub full_total: usize,
    pub partial_total: usize,
    pub failed_total: usize,
    pub missing_total: usize,
    pub incompatible_total: usize,
    pub ineligible_total: usize,
    pub in_progress_total: usize,
    pub status: ClosureClassRecord,
    #[serde(default)]
    pub required_procedures: Vec<String>,
    #[serde(default)]
    pub status_by_procedure: BTreeMap<String, ProcedureClosureSummaryRecord>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_transition_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ProcedureClosureSummaryRecord {
    pub expected_total: usize,
    pub complete_total: usize,
    pub failed_total: usize,
    pub missing_total: usize,
    pub incompatible_total: usize,
    pub partial_total: usize,
    pub ineligible_total: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ClosureInstanceRowRecord {
    pub instance_id: String,
    pub dataset_label: String,
    pub repo_family: String,
    pub registry_status: RegistryInstanceStatusRecord,
    pub eval_status: ClosureClassRecord,
    pub protocol_status: ClosureClassRecord,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub eval_failure: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub protocol_failure: Option<String>,
    pub artifacts: ClosureArtifactRefsRecord,
    #[serde(default)]
    pub protocol_procedures: BTreeMap<String, ClosureClassRecord>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub protocol_counts: Option<ClosureProtocolCountsRecord>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_event_at: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct ClosureArtifactRefsRecord {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub registration_path: Option<PathBuf>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub run_manifest: Option<PathBuf>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub run_root: Option<PathBuf>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub record_path: Option<PathBuf>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub execution_log: Option<PathBuf>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub indexing_status: Option<PathBuf>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parse_failure: Option<PathBuf>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub snapshot_status: Option<PathBuf>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub msb_submission: Option<PathBuf>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub protocol_artifacts_dir: Option<PathBuf>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub protocol_anchor: Option<PathBuf>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub batch_failure_sources: Vec<PathBuf>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ClosureProtocolCountsRecord {
    pub total_calls: usize,
    pub reviewed_calls: usize,
    pub total_segments: usize,
    pub usable_segments: usize,
    pub mismatched_segments: usize,
    pub missing_segments: usize,
}
