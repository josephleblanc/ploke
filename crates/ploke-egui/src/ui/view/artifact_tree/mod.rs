//! ArtifactTree projection shape.
//!
//! This module carries the contract-level facts for the default graph view:
//! artifact nodes (`A`), admitted History successor edges (`P_H`),
//! parent-produced child edges (`P_C`), plus relation inventory for History
//! opened-from context (`P_O`) and raw patch provenance (`P_B`), weak
//! components, roots, orphans, and ruler marks.

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

    pub fn total(self) -> usize {
        self.run_forest + self.artifacts
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Edges {
    pub run_forest: usize,
    pub history_patches: usize,
    pub produced_child_edges: usize,
    pub opened_from_edges: usize,
    pub applied_patch_edges: usize,
}

impl Edges {
    pub(crate) fn new(
        history_patches: usize,
        produced_child_edges: usize,
        opened_from_edges: usize,
        applied_patch_edges: usize,
    ) -> Self {
        Self {
            run_forest: 0,
            history_patches,
            produced_child_edges,
            opened_from_edges,
            applied_patch_edges,
        }
    }

    pub fn total(self) -> usize {
        self.run_forest
            + self.history_patches
            + self.produced_child_edges
            + self.opened_from_edges
            + self.applied_patch_edges
    }

    pub fn visible_primary_total(self) -> usize {
        self.run_forest + self.history_patches + self.produced_child_edges
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
