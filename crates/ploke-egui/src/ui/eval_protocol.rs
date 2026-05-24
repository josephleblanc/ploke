use ploke_tree::{
    ClosureEvidence, ProtocolArtifactSummary, RunRecordSummary, graph::EvalProtocolEvidence,
};

pub(crate) struct EvalProtocolDashboard<'g> {
    evidence: EvalProtocolEvidence<'g>,
    availability: EvalProtocolAvailability,
}

impl<'g> EvalProtocolDashboard<'g> {
    pub(crate) fn from_graph(graph: &'g ploke_tree::Graph) -> Self {
        let evidence = graph.eval_protocol_evidence();
        let availability = EvalProtocolAvailability::from_evidence(evidence);

        Self {
            evidence,
            availability,
        }
    }

    pub(crate) fn evidence(&self) -> EvalProtocolEvidence<'g> {
        self.evidence
    }

    pub(crate) fn availability(&self) -> EvalProtocolAvailability {
        self.availability
    }

    pub(crate) fn is_available(&self) -> bool {
        self.evidence.is_available()
    }

    pub(crate) fn closure(&self) -> Option<&'g ClosureEvidence> {
        self.evidence.closure
    }

    pub(crate) fn run_records_summary(&self) -> Option<&'g RunRecordSummary> {
        self.evidence
            .run_records
            .map(|run_records| &run_records.summary)
    }

    pub(crate) fn run_records_file_count(&self) -> Option<usize> {
        self.run_records_summary().map(|summary| summary.file_count)
    }

    pub(crate) fn run_records_parsed_count(&self) -> Option<usize> {
        self.run_records_summary()
            .map(|summary| summary.parsed_count)
    }

    pub(crate) fn run_records_branch_ref_count(&self) -> Option<usize> {
        self.run_records_summary()
            .map(|summary| summary.branch_ref_count)
    }

    pub(crate) fn protocol_summary(&self) -> Option<&'g ProtocolArtifactSummary> {
        self.evidence
            .protocol_artifacts
            .map(|protocol_artifacts| &protocol_artifacts.summary)
    }

    pub(crate) fn protocol_artifacts_file_count(&self) -> Option<usize> {
        self.protocol_summary().map(|summary| summary.file_count)
    }

    pub(crate) fn protocol_artifacts_parsed_count(&self) -> Option<usize> {
        self.protocol_summary().map(|summary| summary.parsed_count)
    }

    pub(crate) fn protocol_artifacts_review_count(&self) -> Option<usize> {
        self.protocol_summary().map(|summary| summary.review_count)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum EvidenceState {
    Available,
    Missing,
    NotApplicable,
}

impl EvidenceState {
    fn from_option<T>(value: Option<&T>) -> Self {
        if value.is_some() {
            Self::Available
        } else {
            Self::Missing
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct EvalProtocolAvailability {
    pub closure: EvidenceState,
    pub run_records: EvidenceState,
    pub protocol_artifacts: EvidenceState,
}

impl EvalProtocolAvailability {
    fn from_evidence(evidence: EvalProtocolEvidence<'_>) -> Self {
        Self {
            closure: EvidenceState::from_option(evidence.closure),
            run_records: EvidenceState::from_option(evidence.run_records),
            protocol_artifacts: EvidenceState::from_option(evidence.protocol_artifacts),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn eval_protocol_dashboard_reports_all_sources_missing_for_empty_graph() {
        let graph = ploke_tree::Graph::default();
        let dashboard = EvalProtocolDashboard::from_graph(&graph);

        assert_eq!(dashboard.availability().closure, EvidenceState::Missing);
        assert_eq!(dashboard.availability().run_records, EvidenceState::Missing);
        assert_eq!(
            dashboard.availability().protocol_artifacts,
            EvidenceState::Missing
        );
        assert!(!dashboard.is_available());
    }

    #[test]
    fn eval_protocol_dashboard_exposes_typed_scalar_missing_values_for_empty_graph() {
        let graph = ploke_tree::Graph::default();
        let dashboard = EvalProtocolDashboard::from_graph(&graph);
        let evidence = dashboard.evidence();

        assert!(evidence.closure.is_none());
        assert!(evidence.run_records.is_none());
        assert!(evidence.protocol_artifacts.is_none());

        assert!(dashboard.closure().is_none());
        assert!(dashboard.run_records_summary().is_none());
        assert_eq!(dashboard.run_records_file_count(), None);
        assert_eq!(dashboard.run_records_parsed_count(), None);
        assert_eq!(dashboard.run_records_branch_ref_count(), None);
        assert!(dashboard.protocol_summary().is_none());
        assert_eq!(dashboard.protocol_artifacts_file_count(), None);
        assert_eq!(dashboard.protocol_artifacts_parsed_count(), None);
        assert_eq!(dashboard.protocol_artifacts_review_count(), None);
    }
}
