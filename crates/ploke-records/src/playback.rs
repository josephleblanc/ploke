use std::marker::PhantomData;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
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

impl PlaybackGranularity for Coarse {
    type Step = CoarseStep;
    type StepRef<'a> = CoarseStepRef<'a>;
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
    use super::{CoarseStep, CoarseStepRef, EvidenceStrength, RunPlayback, RunPlaybackRef};

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
}
