#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) enum EvalProtocolRenderMode {
    #[default]
    Full,
    CallReviewScanOnly,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) struct InspectorOpenState {
    force_open: Option<InspectorPanelSection>,
    force_exclusive: bool,
}

impl InspectorOpenState {
    #[cfg(all(
        not(target_arch = "wasm32"),
        feature = "dev",
        feature = "native-benchmark"
    ))]
    pub(crate) fn benchmark(
        section: Option<crate::benchmark::BenchmarkInspectorSection>,
        exclusive: bool,
    ) -> Self {
        Self {
            force_open: section.map(InspectorPanelSection::from_benchmark),
            force_exclusive: exclusive,
        }
    }

    pub(crate) fn open(self, section: InspectorPanelSection) -> Option<bool> {
        if self.force_exclusive {
            Some(self.force_open == Some(section))
        } else {
            (self.force_open == Some(section)).then_some(true)
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum InspectorPanelSection {
    Identity,
    #[serde(alias = "RunReview")]
    EvalProtocol,
    Roles,
    PatchGeneration,
    LlmCalls,
    Patches,
    RunRecords,
    GraphEdges,
    ArtifactEdges,
    SourceRefs,
    ArtifactIds,
    Technical,
    PatchDebug,
    CandidateComparison,
    LineageAuthority,
}

impl InspectorPanelSection {
    pub(crate) fn title(&self) -> &'static str {
        match self {
            Self::Identity => "Identity",
            Self::EvalProtocol => "Eval & Protocol",
            Self::Roles => "Roles",
            Self::PatchGeneration => "Patch Generation",
            Self::LlmCalls => "LLM Calls",
            Self::RunRecords => "Run Records",
            Self::GraphEdges => "Graph Edges",
            Self::ArtifactEdges => "Artifact Edges",
            Self::Patches => "Patches",
            Self::SourceRefs => "Source Refs",
            Self::ArtifactIds => "Artifact IDs",
            Self::Technical => "Technical",
            Self::PatchDebug => "Patch Debug",
            Self::CandidateComparison => "Candidate Comparison",
            Self::LineageAuthority => "Lineage Authority",
        }
    }
    #[cfg(all(
        not(target_arch = "wasm32"),
        feature = "dev",
        feature = "native-benchmark"
    ))]
    fn from_benchmark(section: crate::benchmark::BenchmarkInspectorSection) -> Self {
        match section {
            crate::benchmark::BenchmarkInspectorSection::LlmCalls => Self::LlmCalls,
            crate::benchmark::BenchmarkInspectorSection::RunRecords => Self::RunRecords,
            crate::benchmark::BenchmarkInspectorSection::GraphEdges => Self::GraphEdges,
            crate::benchmark::BenchmarkInspectorSection::ArtifactEdges => Self::ArtifactEdges,
            crate::benchmark::BenchmarkInspectorSection::PatchDebug => Self::PatchDebug,
            crate::benchmark::BenchmarkInspectorSection::SourceRefs => Self::SourceRefs,
            crate::benchmark::BenchmarkInspectorSection::ArtifactIds => Self::ArtifactIds,
        }
    }
}
