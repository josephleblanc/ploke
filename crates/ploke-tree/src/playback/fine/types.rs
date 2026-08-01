use ploke_records::playback::{EvidenceStrength, FineOrder, FineStepKind, FineStepRef};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FineHistoryStep {
    pub id: String,
    pub kind: FineStepKind,
    pub evidence: EvidenceStrength,
    pub order: FineOrder,
    pub label: Option<String>,
    pub occurrence_id: Option<String>,
    pub membership_id: Option<String>,
    pub candidate_set_root: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FineRunPlayback {
    steps: Vec<FineHistoryStep>,
}

impl FineRunPlayback {
    pub fn new(steps: Vec<FineHistoryStep>) -> Self {
        Self { steps }
    }

    pub fn iter(&self) -> std::slice::Iter<'_, FineHistoryStep> {
        self.steps.iter()
    }
}

impl IntoIterator for FineRunPlayback {
    type Item = FineHistoryStep;
    type IntoIter = std::vec::IntoIter<FineHistoryStep>;

    fn into_iter(self) -> Self::IntoIter {
        self.steps.into_iter()
    }
}

impl<'a> IntoIterator for &'a FineRunPlayback {
    type Item = &'a FineHistoryStep;
    type IntoIter = std::slice::Iter<'a, FineHistoryStep>;

    fn into_iter(self) -> Self::IntoIter {
        self.steps.iter()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FineHistoryStepRef<'a> {
    pub id: String,
    pub kind: FineStepKind,
    pub evidence: EvidenceStrength,
    pub order: FineOrder,
    pub label: Option<&'a str>,
    pub occurrence_id: Option<&'a str>,
    pub membership_id: Option<&'a str>,
    pub candidate_set_root: Option<&'a str>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FineRunPlaybackRefSteps<'a> {
    steps: Vec<FineHistoryStepRef<'a>>,
    legacy_steps: Vec<FineStepRef<'a>>,
}

impl<'a> FineRunPlaybackRefSteps<'a> {
    pub fn new(steps: Vec<FineHistoryStepRef<'a>>) -> Self {
        let legacy_steps = steps
            .iter()
            .map(|step| FineStepRef {
                id: step.id.clone(),
                kind: step.kind,
                evidence: step.evidence,
                order: step.order,
                label: step.label,
                occurrence_id: step.occurrence_id,
                membership_id: step.membership_id,
            })
            .collect();
        Self {
            steps,
            legacy_steps,
        }
    }

    pub fn iter(&self) -> std::slice::Iter<'_, FineHistoryStepRef<'a>> {
        self.steps.iter()
    }

    pub fn as_slice(&self) -> &[FineStepRef<'a>] {
        self.legacy_steps.as_slice()
    }
}

impl<'a> IntoIterator for FineRunPlaybackRefSteps<'a> {
    type Item = FineHistoryStepRef<'a>;
    type IntoIter = std::vec::IntoIter<FineHistoryStepRef<'a>>;

    fn into_iter(self) -> Self::IntoIter {
        self.steps.into_iter()
    }
}

impl<'a, 'b> IntoIterator for &'b FineRunPlaybackRefSteps<'a> {
    type Item = &'b FineHistoryStepRef<'a>;
    type IntoIter = std::slice::Iter<'b, FineHistoryStepRef<'a>>;

    fn into_iter(self) -> Self::IntoIter {
        self.steps.iter()
    }
}
