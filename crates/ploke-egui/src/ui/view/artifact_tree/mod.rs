//! ArtifactTree projection shape.
//!
//! This module carries the contract-level facts for the default graph view:
//! run-forest nodes (`F`) when available, fallback artifacts (`A`), admitted
//! History patch edges (`P_H`), observed branch applied-patch edges (`P_B`),
//! weak components, roots, orphans, and ruler marks.

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Shape {
    pub(crate) nodes: Nodes,
    pub(crate) edges: Edges,
    pub(crate) components: Components,
    pub(crate) marks: Marks,
}

impl Shape {
    pub(crate) fn new(nodes: Nodes, edges: Edges, components: Components, marks: Marks) -> Self {
        Self {
            nodes,
            edges,
            components,
            marks,
        }
    }

    pub fn nodes(&self) -> Nodes {
        self.nodes
    }

    pub fn edges(&self) -> Edges {
        self.edges
    }

    pub fn components(&self) -> Components {
        self.components
    }

    pub fn marks(&self) -> Marks {
        self.marks
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Nodes {
    pub run_forest: usize,
    pub artifacts: usize,
}

impl Nodes {
    pub(crate) fn new(artifacts: usize) -> Self {
        Self {
            run_forest: 0,
            artifacts,
        }
    }

    pub(crate) fn run_forest(run_forest: usize) -> Self {
        Self {
            run_forest,
            artifacts: 0,
        }
    }

    pub fn total(self) -> usize {
        self.run_forest + self.artifacts
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Edges {
    pub run_forest: usize,
    pub history_patches: usize,
    pub applied_patch_edges: usize,
}

impl Edges {
    pub(crate) fn new(history_patches: usize, applied_patch_edges: usize) -> Self {
        Self {
            run_forest: 0,
            history_patches,
            applied_patch_edges,
        }
    }

    pub(crate) fn run_forest(run_forest: usize) -> Self {
        Self {
            run_forest,
            history_patches: 0,
            applied_patch_edges: 0,
        }
    }

    pub fn total(self) -> usize {
        self.run_forest + self.history_patches + self.applied_patch_edges
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Components {
    pub weak: usize,
    pub roots: usize,
    pub orphan_artifacts: usize,
}

impl Components {
    pub(crate) fn new(weak: usize, roots: usize, orphan_artifacts: usize) -> Self {
        Self {
            weak,
            roots,
            orphan_artifacts,
        }
    }

    pub fn weakly_connected(self, artifact_count: usize) -> bool {
        artifact_count <= 1 || self.weak == 1
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Marks {
    pub ruler_highlights: usize,
}

impl Marks {
    pub(crate) fn new(ruler_highlights: usize) -> Self {
        Self { ruler_highlights }
    }
}
