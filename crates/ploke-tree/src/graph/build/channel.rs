use crate::ChannelEvidence;
use crate::graph::{EvidenceKind, EvidenceLocator, EvidenceSubject};

use super::Builder;

impl Builder {
    pub(super) fn ingest_channel_summary(&mut self, evidence: &ChannelEvidence) {
        self.attach_located_evidence(
            EvidenceSubject::ChannelSummary {
                file_count: evidence.file_count,
                line_count: evidence.line_count,
                parsed_count: evidence.parsed_count,
                parse_error_count: evidence.parse_error_count,
            },
            EvidenceKind::ChannelSummary,
            vec![EvidenceLocator::LoadedSummary {
                name: "channel_envelopes",
            }],
        );
    }
}
