//! Small in-crate graph fixture for exercising the UI before record loading exists.

use ploke_records::branch::TreatmentBranchStatus;
use ploke_records::ids::{ArtifactId, PatchId};

use crate::graph::{Artifact, EdgeId, EvidenceId, Graph};

pub fn sample_graph() -> Graph {
    let mut graph = Graph::new();

    let a1 = ArtifactId("artifact:A1".to_string());
    let a2 = ArtifactId("artifact:A2".to_string());
    let a3 = ArtifactId("artifact:A3".to_string());
    let a4 = ArtifactId("artifact:A4".to_string());
    let a5 = ArtifactId("artifact:A5".to_string());
    let a6 = ArtifactId("artifact:A6".to_string());

    for artifact in [&a1, &a2, &a3, &a4, &a5, &a6] {
        graph.insert_artifact(Artifact::new(artifact.clone()));
    }

    graph
        .insert_patch_edge(
            EdgeId::new("edge:A1-A2"),
            a1.clone(),
            a2,
            PatchId("patch:P1".to_string()),
            EvidenceId::new("evidence:edge:A1-A2"),
            TreatmentBranchStatus::Synthesized,
        )
        .expect("sample graph edge endpoints exist");
    graph
        .insert_patch_edge(
            EdgeId::new("edge:A1-A3"),
            a1.clone(),
            a3.clone(),
            PatchId("patch:P2".to_string()),
            EvidenceId::new("evidence:edge:A1-A3"),
            TreatmentBranchStatus::Selected,
        )
        .expect("sample graph edge endpoints exist");
    graph
        .insert_patch_edge(
            EdgeId::new("edge:A1-A4"),
            a1,
            a4,
            PatchId("patch:P3".to_string()),
            EvidenceId::new("evidence:edge:A1-A4"),
            TreatmentBranchStatus::Dropped,
        )
        .expect("sample graph edge endpoints exist");
    graph
        .insert_patch_edge(
            EdgeId::new("edge:A3-A5"),
            a3.clone(),
            a5,
            PatchId("patch:P4".to_string()),
            EvidenceId::new("evidence:edge:A3-A5"),
            TreatmentBranchStatus::Synthesized,
        )
        .expect("sample graph edge endpoints exist");
    graph
        .insert_patch_edge(
            EdgeId::new("edge:A3-A6"),
            a3,
            a6,
            PatchId("patch:P5".to_string()),
            EvidenceId::new("evidence:edge:A3-A6"),
            TreatmentBranchStatus::Dropped,
        )
        .expect("sample graph edge endpoints exist");

    graph
}
