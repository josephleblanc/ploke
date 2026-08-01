use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use super::{ComparedRunEvidence, MetricScore, ScoreDiagnostic, ScoreState};
use crate::protocol::protocol_aggregate::ProtocolArtifactRef;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub(crate) struct ScoreProfileComponent {
    pub(crate) component_id: String,
    pub(crate) procedure_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct ScoreComponent {
    pub(crate) component_id: String,
    pub(crate) procedure_id: String,
    pub(crate) state: ScoreState,
    pub(crate) score: Option<i64>,
    pub(crate) metrics: Vec<MetricScore>,
    pub(crate) provenance: ScoreComponentProvenance,
    pub(crate) diagnostics: Vec<ScoreDiagnostic>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct ScoreComponentProvenance {
    pub(crate) baseline_registration_path: Option<PathBuf>,
    pub(crate) treatment_registration_path: Option<PathBuf>,
    pub(crate) baseline_record_path: Option<PathBuf>,
    pub(crate) treatment_record_path: Option<PathBuf>,
    pub(crate) baseline_run_id: Option<String>,
    pub(crate) treatment_run_id: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub(crate) baseline_protocol_artifacts: Vec<ProtocolArtifactRef>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub(crate) treatment_protocol_artifacts: Vec<ProtocolArtifactRef>,
}

impl ScoreComponentProvenance {
    pub(super) fn from_compared(compared: &ComparedRunEvidence) -> Self {
        Self {
            baseline_registration_path: compared.baseline_registration_path.clone(),
            treatment_registration_path: compared.treatment_registration_path.clone(),
            baseline_record_path: compared.baseline_record_path.clone(),
            treatment_record_path: compared.treatment_record_path.clone(),
            baseline_run_id: compared.baseline_run.as_ref().map(|run| run.run_id.clone()),
            treatment_run_id: compared
                .treatment_run
                .as_ref()
                .map(|run| run.run_id.clone()),
            baseline_protocol_artifacts: Vec::new(),
            treatment_protocol_artifacts: Vec::new(),
        }
    }
}
