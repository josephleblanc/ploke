use std::marker::PhantomData;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EvidenceStrength {
    Projection,
    TypedRecord,
    AdmittedHistory,
    SealedHistory,
}

pub trait PlaybackGranularity {
    type Step;
    type StepRef<'a>
    where
        Self: 'a;
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct Coarse;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct Fine;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct CoarseStep {
    pub id: String,
    pub evidence: EvidenceStrength,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct CoarseStepRef<'a> {
    pub id: &'a str,
    pub evidence: EvidenceStrength,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FineStepKind {
    ParentStarted,
    EvaluationRecorded,
    CandidateConsidered,
    SuccessorSelected,
    HistoryEntryAdmitted,
    HistoryBlockSealed,
    SuccessorReadyAck,
    SuccessorCompletion,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct FineOrder {
    pub block_height: u64,
    pub phase_rank: u16,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub entry_index: Option<usize>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub candidate_index: Option<usize>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct FineStep {
    pub id: String,
    pub kind: FineStepKind,
    pub evidence: EvidenceStrength,
    pub order: FineOrder,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub occurrence_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub membership_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct FineStepRef<'a> {
    pub id: String,
    pub kind: FineStepKind,
    pub evidence: EvidenceStrength,
    pub order: FineOrder,
    pub label: Option<&'a str>,
    pub occurrence_id: Option<&'a str>,
    pub membership_id: Option<&'a str>,
}

impl PlaybackGranularity for Coarse {
    type Step = CoarseStep;
    type StepRef<'a> = CoarseStepRef<'a>;
}

impl PlaybackGranularity for Fine {
    type Step = FineStep;
    type StepRef<'a> = FineStepRef<'a>;
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RunPlayback<G = Coarse>
where
    G: PlaybackGranularity,
{
    steps: Vec<G::Step>,
    _granularity: PhantomData<G>,
}

impl<G> RunPlayback<G>
where
    G: PlaybackGranularity,
{
    pub fn new(steps: Vec<G::Step>) -> Self {
        Self {
            steps,
            _granularity: PhantomData,
        }
    }

    pub fn iter(&self) -> std::slice::Iter<'_, G::Step> {
        self.steps.iter()
    }
}

impl<'a, G> IntoIterator for &'a RunPlayback<G>
where
    G: PlaybackGranularity,
{
    type Item = &'a G::Step;
    type IntoIter = std::slice::Iter<'a, G::Step>;

    fn into_iter(self) -> Self::IntoIter {
        self.steps.iter()
    }
}

impl<G> IntoIterator for RunPlayback<G>
where
    G: PlaybackGranularity,
{
    type Item = G::Step;
    type IntoIter = std::vec::IntoIter<G::Step>;

    fn into_iter(self) -> Self::IntoIter {
        self.steps.into_iter()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RunPlaybackRef<'a, G = Coarse>
where
    G: PlaybackGranularity + 'a,
{
    steps: &'a [G::StepRef<'a>],
    _granularity: PhantomData<G>,
}

impl<'a, G> RunPlaybackRef<'a, G>
where
    G: PlaybackGranularity + 'a,
{
    pub fn new(steps: &'a [G::StepRef<'a>]) -> Self {
        Self {
            steps,
            _granularity: PhantomData,
        }
    }

    pub fn iter(&self) -> std::slice::Iter<'_, G::StepRef<'a>> {
        self.steps.iter()
    }
}

impl<'a, G> IntoIterator for RunPlaybackRef<'a, G>
where
    G: PlaybackGranularity + 'a,
{
    type Item = &'a G::StepRef<'a>;
    type IntoIter = std::slice::Iter<'a, G::StepRef<'a>>;

    fn into_iter(self) -> Self::IntoIter {
        self.steps.iter()
    }
}

impl<'a, G> IntoIterator for &'a RunPlaybackRef<'a, G>
where
    G: PlaybackGranularity + 'a,
{
    type Item = &'a G::StepRef<'a>;
    type IntoIter = std::slice::Iter<'a, G::StepRef<'a>>;

    fn into_iter(self) -> Self::IntoIter {
        self.steps.iter()
    }
}

#[cfg(test)]
mod tests {
    use super::{
        CoarseStep, CoarseStepRef, EvidenceStrength, FineOrder, FineStep, FineStepKind,
        FineStepRef, RunPlayback, RunPlaybackRef,
    };

    #[test]
    fn run_playback_owned_iter_and_into_iter() {
        let playback = RunPlayback::<super::Coarse>::new(vec![
            CoarseStep {
                id: "a".to_string(),
                evidence: EvidenceStrength::TypedRecord,
            },
            CoarseStep {
                id: "b".to_string(),
                evidence: EvidenceStrength::SealedHistory,
            },
        ]);

        let ids: Vec<&str> = playback.iter().map(|step| step.id.as_str()).collect();
        assert_eq!(ids, vec!["a", "b"]);

        let owned_ids: Vec<String> = playback.into_iter().map(|step| step.id).collect();
        assert_eq!(owned_ids, vec!["a".to_string(), "b".to_string()]);
    }

    #[test]
    fn run_playback_ref_iter_and_into_iter() {
        let steps = [
            CoarseStepRef {
                id: "a",
                evidence: EvidenceStrength::Projection,
            },
            CoarseStepRef {
                id: "b",
                evidence: EvidenceStrength::AdmittedHistory,
            },
        ];
        let playback = RunPlaybackRef::<super::Coarse>::new(&steps);

        let ids: Vec<&str> = playback.iter().map(|step| step.id).collect();
        assert_eq!(ids, vec!["a", "b"]);

        let by_value_ids: Vec<&str> = playback.into_iter().map(|step| step.id).collect();
        assert_eq!(by_value_ids, vec!["a", "b"]);

        let by_ref_ids: Vec<&str> = (&playback).into_iter().map(|step| step.id).collect();
        assert_eq!(by_ref_ids, vec!["a", "b"]);
    }

    #[test]
    fn run_playback_fine_iterates_ordered_steps() {
        let order = FineOrder {
            block_height: 0,
            phase_rank: 30,
            entry_index: Some(0),
            candidate_index: None,
        };
        let playback = RunPlayback::<super::Fine>::new(vec![FineStep {
            id: "entry:a".to_string(),
            kind: FineStepKind::HistoryEntryAdmitted,
            evidence: EvidenceStrength::AdmittedHistory,
            order,
            label: Some("entry admitted".to_string()),
            occurrence_id: None,
            membership_id: None,
        }]);

        let step = playback.iter().next().expect("fine step");
        assert_eq!(step.id, "entry:a");
        assert_eq!(step.kind, FineStepKind::HistoryEntryAdmitted);
        assert_eq!(step.order, order);
    }

    #[test]
    fn run_playback_ref_fine_iterates_borrowed_steps() {
        let order = FineOrder {
            block_height: 1,
            phase_rank: 60,
            entry_index: None,
            candidate_index: None,
        };
        let steps = [FineStepRef {
            id: "block:hash-1".to_owned(),
            kind: FineStepKind::HistoryBlockSealed,
            evidence: EvidenceStrength::SealedHistory,
            order,
            label: Some("sealed"),
            occurrence_id: None,
            membership_id: None,
        }];
        let playback = RunPlaybackRef::<super::Fine>::new(&steps);

        let step = playback.iter().next().expect("fine ref step");
        assert_eq!(step.id, "block:hash-1");
        assert_eq!(step.kind, FineStepKind::HistoryBlockSealed);
        assert_eq!(step.order, order);
    }
}
