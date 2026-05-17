use super::{Confidence, Domain, DomainFinding, DomainName, Verdict};
use crate::mbe;
use crate::successor_selection::evidence::SelectionInput;

#[derive(Debug, Clone, Copy)]
pub(crate) struct OracleDomain;

impl Domain for OracleDomain {
    fn name(&self) -> DomainName {
        DomainName::Oracle
    }

    fn evaluate(&self, input: &SelectionInput) -> DomainFinding {
        let configured = input.comparisons.len();
        let evaluated = input
            .comparisons
            .iter()
            .filter(|comparison| comparison.oracle_evaluation.is_some())
            .count();
        let resolved = input
            .comparisons
            .iter()
            .filter(|comparison| {
                comparison
                    .oracle_evaluation
                    .as_ref()
                    .is_some_and(|evaluation| evaluation.evidence.verdict == mbe::Verdict::Resolved)
            })
            .count();

        let verdict = if evaluated == 0 {
            Verdict::Inconclusive
        } else if resolved > 0 {
            Verdict::Better
        } else {
            Verdict::Worse
        };
        let confidence = if configured == 0 || evaluated == 0 {
            Confidence::Low
        } else if evaluated == configured {
            Confidence::High
        } else {
            Confidence::Medium
        };

        DomainFinding {
            domain: self.name(),
            verdict,
            confidence,
            metrics: Vec::new(),
            evidence_refs: input
                .comparisons
                .iter()
                .filter_map(|comparison| {
                    comparison.oracle_evaluation.as_ref().map(|evaluation| {
                        format!(
                            "mbe:{}:{}",
                            evaluation.evidence.report_id, evaluation.evidence.instance_id
                        )
                    })
                })
                .collect(),
            rationale: vec![
                format!("oracle_configured_instances={configured}"),
                format!("oracle_evaluated_instances={evaluated}"),
                format!("oracle_resolved_instances={resolved}"),
            ],
        }
    }
}
