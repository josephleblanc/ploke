#![allow(dead_code)] // Phase 1 boundary; live adapters are wired in later phases.

use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::path::PathBuf;

use thiserror::Error;

use super::{ArtifactDelta, surface};

pub(crate) trait View {
    type Artifact;
    type Projection;
    type Rule;
    type Bounds;
    type Query;
    type Target;
    type Hit;
    type Span;
    type Delta;
    type Error;

    fn project(&self, artifact: &Self::Artifact) -> Result<Self::Projection, Self::Error>;

    fn bounds(
        &self,
        projection: &Self::Projection,
        rules: &[Self::Rule],
    ) -> Result<Self::Bounds, Self::Error>;

    fn search(
        &self,
        projection: &Self::Projection,
        bounds: &Self::Bounds,
        query: &Self::Query,
    ) -> Result<Vec<Self::Hit>, Self::Error>;

    fn resolve(
        &self,
        projection: &Self::Projection,
        bounds: &Self::Bounds,
        target: &Self::Target,
    ) -> Result<Self::Span, Self::Error>;

    fn delta(
        &self,
        before: &Self::Projection,
        after: &Self::Projection,
        patch: &ArtifactDelta,
    ) -> Result<Self::Delta, Self::Error>;
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(crate) struct Target {
    path: PathBuf,
    name: String,
}

impl Target {
    pub(crate) fn new(path: impl Into<PathBuf>, name: impl Into<String>) -> Self {
        Self {
            path: path.into(),
            name: name.into(),
        }
    }

    pub(crate) fn path(&self) -> &PathBuf {
        &self.path
    }

    pub(crate) fn name(&self) -> &str {
        &self.name
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct Span {
    target: Target,
    path: PathBuf,
    start: usize,
    end: usize,
    hash: surface::Hash,
}

impl Span {
    pub(crate) fn new(
        target: Target,
        path: impl Into<PathBuf>,
        start: usize,
        end: usize,
        hash: surface::Hash,
    ) -> Self {
        Self {
            target,
            path: path.into(),
            start,
            end,
            hash,
        }
    }

    pub(crate) fn target(&self) -> &Target {
        &self.target
    }

    pub(crate) fn path(&self) -> &PathBuf {
        &self.path
    }

    pub(crate) fn start(&self) -> usize {
        self.start
    }

    pub(crate) fn end(&self) -> usize {
        self.end
    }

    pub(crate) fn hash(&self) -> &surface::Hash {
        &self.hash
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Node {
    target: Target,
    path: PathBuf,
    start: usize,
    end: usize,
}

impl Node {
    pub(crate) fn new(target: Target, path: impl Into<PathBuf>, start: usize, end: usize) -> Self {
        Self {
            target,
            path: path.into(),
            start,
            end,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Projection {
    artifact: surface::Ref,
    spans: BTreeMap<Target, Span>,
    edges: BTreeMap<Target, BTreeSet<Target>>,
}

impl Projection {
    pub(crate) fn artifact(&self) -> &surface::Ref {
        &self.artifact
    }

    pub(crate) fn span(&self, target: &Target) -> Option<&Span> {
        self.spans.get(target)
    }

    pub(crate) fn spans(&self) -> impl Iterator<Item = &Span> {
        self.spans.values()
    }

    pub(crate) fn edges(&self) -> impl Iterator<Item = (&Target, &BTreeSet<Target>)> {
        self.edges.iter()
    }

    fn descendants(&self, target: &Target) -> BTreeSet<Target> {
        let mut found = BTreeSet::new();
        let mut queue = VecDeque::new();
        if let Some(children) = self.edges.get(target) {
            queue.extend(children.iter().cloned());
        }
        while let Some(next) = queue.pop_front() {
            if found.insert(next.clone()) {
                if let Some(children) = self.edges.get(&next) {
                    queue.extend(children.iter().cloned());
                }
            }
        }
        found
    }

    fn ancestors(&self, target: &Target) -> BTreeSet<Target> {
        let mut found = BTreeSet::new();
        let mut queue = VecDeque::from([target.clone()]);
        while let Some(next) = queue.pop_front() {
            for (parent, children) in &self.edges {
                if children.contains(&next) && found.insert(parent.clone()) {
                    queue.push_back(parent.clone());
                }
            }
        }
        found
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Rule {
    Include(Target),
    Exclude(Target),
    Ancestors(Target),
    Descendants(Target),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Bounds {
    artifact: surface::Ref,
    spans: BTreeMap<Target, Span>,
}

impl Bounds {
    pub(crate) fn artifact(&self) -> &surface::Ref {
        &self.artifact
    }

    pub(crate) fn targets(&self) -> BTreeSet<&Target> {
        self.spans.keys().collect()
    }

    pub(crate) fn span(&self, target: &Target) -> Option<&Span> {
        self.spans.get(target)
    }

    pub(crate) fn spans(&self) -> impl Iterator<Item = &Span> {
        self.spans.values()
    }

    pub(crate) fn contains(&self, span: &Span) -> bool {
        self.spans
            .get(span.target())
            .is_some_and(|bounded| bounded == span)
    }

    pub(crate) fn covers(&self, other: &Bounds) -> bool {
        self.artifact == other.artifact
            && other
                .spans
                .iter()
                .all(|(target, span)| self.spans.get(target) == Some(span))
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Query {
    text: String,
}

impl Query {
    pub(crate) fn new(text: impl Into<String>) -> Self {
        Self { text: text.into() }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Hit {
    target: Target,
    span: Span,
}

impl Hit {
    pub(crate) fn target(&self) -> &Target {
        &self.target
    }

    pub(crate) fn span(&self) -> &Span {
        &self.span
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Delta {
    base: surface::Ref,
    after: surface::Ref,
    touched: BTreeSet<Target>,
}

impl Delta {
    pub(crate) fn touched(&self) -> &BTreeSet<Target> {
        &self.touched
    }
}

#[derive(Debug, Error, PartialEq, Eq)]
pub(crate) enum Error {
    #[error("projection artifact did not match bounds artifact")]
    ArtifactMismatch,
    #[error("unknown graph target: {0:?}")]
    UnknownTarget(Target),
    #[error("target is outside graph bounds: {0:?}")]
    Outside(Target),
    #[error("graph node has no material file hash: {0}")]
    MissingFile(String),
    #[error("patch does not apply to graph projections")]
    DeltaMismatch,
}

#[derive(Debug, Clone)]
pub(crate) struct Mock {
    nodes: Vec<Node>,
    edges: BTreeMap<Target, BTreeSet<Target>>,
}

impl Mock {
    pub(crate) fn new(nodes: Vec<Node>, edges: impl IntoIterator<Item = (Target, Target)>) -> Self {
        let mut graph = BTreeMap::<Target, BTreeSet<Target>>::new();
        for (parent, child) in edges {
            graph.entry(parent).or_default().insert(child);
        }
        Self {
            nodes,
            edges: graph,
        }
    }
}

impl View for Mock {
    type Artifact = surface::Artifact;
    type Projection = Projection;
    type Rule = Rule;
    type Bounds = Bounds;
    type Query = Query;
    type Target = Target;
    type Hit = Hit;
    type Span = Span;
    type Delta = Delta;
    type Error = Error;

    fn project(&self, artifact: &Self::Artifact) -> Result<Self::Projection, Self::Error> {
        let mut spans = BTreeMap::new();
        for node in &self.nodes {
            let hash = artifact
                .file_hash(&node.path)
                .cloned()
                .ok_or_else(|| Error::MissingFile(node.path.display().to_string()))?;
            spans.insert(
                node.target.clone(),
                Span::new(
                    node.target.clone(),
                    node.path.clone(),
                    node.start,
                    node.end,
                    hash,
                ),
            );
        }
        Ok(Projection {
            artifact: artifact.reference().clone(),
            spans,
            edges: self.edges.clone(),
        })
    }

    fn bounds(
        &self,
        projection: &Self::Projection,
        rules: &[Self::Rule],
    ) -> Result<Self::Bounds, Self::Error> {
        let mut targets = BTreeSet::new();
        for rule in rules {
            match rule {
                Rule::Include(target) => {
                    if projection.span(target).is_none() {
                        return Err(Error::UnknownTarget(target.clone()));
                    }
                    targets.insert(target.clone());
                }
                Rule::Exclude(target) => {
                    targets.remove(target);
                }
                Rule::Ancestors(target) => {
                    if projection.span(target).is_none() {
                        return Err(Error::UnknownTarget(target.clone()));
                    }
                    targets.extend(projection.ancestors(target));
                }
                Rule::Descendants(target) => {
                    if projection.span(target).is_none() {
                        return Err(Error::UnknownTarget(target.clone()));
                    }
                    targets.extend(projection.descendants(target));
                }
            }
        }

        let mut spans = BTreeMap::new();
        for target in targets {
            let span = projection
                .span(&target)
                .ok_or_else(|| Error::UnknownTarget(target.clone()))?;
            spans.insert(target, span.clone());
        }
        Ok(Bounds {
            artifact: projection.artifact.clone(),
            spans,
        })
    }

    fn search(
        &self,
        projection: &Self::Projection,
        bounds: &Self::Bounds,
        query: &Self::Query,
    ) -> Result<Vec<Self::Hit>, Self::Error> {
        if projection.artifact != bounds.artifact {
            return Err(Error::ArtifactMismatch);
        }
        Ok(bounds
            .spans
            .iter()
            .filter(|(target, _)| target.name().contains(&query.text))
            .map(|(target, span)| Hit {
                target: target.clone(),
                span: span.clone(),
            })
            .collect())
    }

    fn resolve(
        &self,
        projection: &Self::Projection,
        bounds: &Self::Bounds,
        target: &Self::Target,
    ) -> Result<Self::Span, Self::Error> {
        if projection.artifact != bounds.artifact {
            return Err(Error::ArtifactMismatch);
        }
        let span = projection
            .span(target)
            .ok_or_else(|| Error::UnknownTarget(target.clone()))?;
        if !bounds.contains(span) {
            return Err(Error::Outside(target.clone()));
        }
        Ok(span.clone())
    }

    fn delta(
        &self,
        before: &Self::Projection,
        after: &Self::Projection,
        patch: &ArtifactDelta,
    ) -> Result<Self::Delta, Self::Error> {
        if before.artifact() != patch.base() || after.artifact() != patch.after() {
            return Err(Error::DeltaMismatch);
        }
        Ok(Delta {
            base: patch.base().clone(),
            after: patch.after().clone(),
            touched: patch
                .touches()
                .iter()
                .map(|touch| touch.span().target().clone())
                .collect(),
        })
    }
}
