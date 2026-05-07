#![allow(dead_code)] // Phase 1 boundary; live adapters are wired in later phases.

use thiserror::Error;

use super::{graph, surface};

pub(crate) trait Harness {
    type Graph: graph::View;
    type Proposal;
    type Applied;
    type Run;
    type Error;

    fn graph(&self) -> &Self::Graph;
    fn propose(&self, input: Input<'_>) -> Result<(Self::Proposal, Self::Run), Self::Error>;
    fn apply_checked(
        &self,
        proposal: Self::Proposal,
        check: surface::Check,
    ) -> Result<Self::Applied, Self::Error>;
}

/// Patch-shaped Artifact transition evidence produced after a surface check.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ArtifactDelta {
    base: surface::Ref,
    after: surface::Ref,
    touches: Vec<surface::Touch>,
}

impl ArtifactDelta {
    fn from_check(check: surface::Check) -> Self {
        let (base, after, touches) = check.into_parts();
        Self {
            base,
            after,
            touches,
        }
    }

    pub(crate) fn base(&self) -> &surface::Ref {
        &self.base
    }

    pub(crate) fn after(&self) -> &surface::Ref {
        &self.after
    }

    pub(crate) fn touches(&self) -> &[surface::Touch] {
        &self.touches
    }
}

pub(crate) struct Input<'a> {
    pub(crate) proposal: &'a str,
    pub(crate) run: &'a str,
    pub(crate) base: &'a surface::Ref,
    pub(crate) after: surface::Ref,
    pub(crate) touches: Vec<surface::Touch>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Proposal {
    id: String,
    base: surface::Ref,
    after: surface::Ref,
    touches: Vec<surface::Touch>,
}

impl Proposal {
    pub(crate) fn draft(&self) -> surface::Draft<'_> {
        surface::Draft {
            proposal: &self.id,
            base: &self.base,
            after: &self.after,
            touches: &self.touches,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Run {
    id: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Applied {
    delta: ArtifactDelta,
}

impl Applied {
    pub(crate) fn delta(&self) -> &ArtifactDelta {
        &self.delta
    }
}

#[derive(Debug, Clone)]
pub(crate) struct Mock<G> {
    graph: G,
}

impl<G> Mock<G> {
    pub(crate) fn new(graph: G) -> Self {
        Self { graph }
    }
}

#[derive(Debug, Error, PartialEq, Eq)]
pub(crate) enum Error {
    #[error("checked surface does not match proposal")]
    CheckMismatch,
}

impl<G> Harness for Mock<G>
where
    G: graph::View,
{
    type Graph = G;
    type Proposal = Proposal;
    type Applied = Applied;
    type Run = Run;
    type Error = Error;

    fn graph(&self) -> &Self::Graph {
        &self.graph
    }

    fn propose(&self, input: Input<'_>) -> Result<(Self::Proposal, Self::Run), Self::Error> {
        Ok((
            Proposal {
                id: input.proposal.to_string(),
                base: input.base.clone(),
                after: input.after,
                touches: input.touches,
            },
            Run {
                id: input.run.to_string(),
            },
        ))
    }

    fn apply_checked(
        &self,
        proposal: Self::Proposal,
        check: surface::Check,
    ) -> Result<Self::Applied, Self::Error> {
        if !check.matches(
            &proposal.id,
            &proposal.base,
            &proposal.after,
            &proposal.touches,
        ) {
            return Err(Error::CheckMismatch);
        }
        Ok(Applied {
            delta: ArtifactDelta::from_check(check),
        })
    }
}
