#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GraphWarning {
    pub kind: GraphWarningKind,
    pub detail: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GraphWarningKind {
    DuplicateBlockHash,
    DuplicateEntryId,
    DuplicateLineageBlockHeight,
    BlockEntryCountMismatch,
    DuplicateCandidateMembershipId,
    CandidateSetMembershipCountMismatch,
    CandidateSetMembershipMissingForPayload,
    CandidateSetMembershipAmbiguousForPayload,
    SelectedMembershipMissing,
}
