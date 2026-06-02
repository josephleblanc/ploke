//! Native benchmark suite driver and typed report records.
//!
//! The CLI selects a benchmark run; this module owns interpretation, timing,
//! and report persistence.

pub mod allocation_breakdown;

use std::collections::{BTreeMap, BTreeSet};
use std::error::Error;
use std::fs;
use std::io;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{Instant, SystemTime, UNIX_EPOCH};

use chrono::Utc;
use ploke_records::history::SealedBlockRecord;
use ploke_records::run_record::read_compressed_record_profiled;
use ploke_records::scheduler::{NodeRecord, SchedulerStateRecord};
use ploke_tree::{FsRunStore, Graph, RunRecordSet};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::allocation::{
    self, AllocationDelta, AllocationSnapshot, HeapCallsiteProfile, HeapGroupProfile,
    HeapProfileSnapshot, HeapProfileTotals,
};
use crate::ui::view::GraphViewMode;

#[cfg(feature = "native-benchmark")]
pub fn with_benchmark_tracing_subscriber<R>(f: impl FnOnce() -> R) -> R {
    allocation::with_tracing_subscriber(f)
}

pub const STANDARD_RUN_ROOT: &str =
    "/home/brasides/.ploke-eval/campaigns/p1-five-gen-1x3-20260516-1/prototype1";

const BENCHMARK_REPORT_VERSION: &str = "ploke-egui.native-benchmark-report.v4";
const STANDARD_FRAME_TARGET: usize = 300;
const INSPECTOR_SEQUENCE_WARMUP_FRAMES: usize = 100;
const INSPECTOR_SEQUENCE_SELECT_SETTLE_FRAMES: usize = 30;
const INSPECTOR_SEQUENCE_SECTION_FRAMES: usize = 30;
const INSPECTOR_SECTION_PHASE_FRAMES: usize = 30;
const TOP_FRAME_LIMIT: usize = 10;
const TOP_HEAP_LIMIT: usize = 32;
const RUN_PICKER_DISCOVERY_WARNING_NS: u64 = 250_000_000;
pub const PARITY_BASELINE_DIR: &str = "docs/profiling/benchmarks/20260602-wasm-parity-baseline";

pub const BENCHMARK_GRAPH_SNAPSHOT_FIXTURE: &str =
    "benchmark-fixtures/standard-prototype1-graph-snapshot.json";

const FOCUSED_CALLSITE_SCOPES: &[&str] = &[
    "eframe_run_native",
    "selection_inspector",
    "central_graph_widget_add",
    "inspector_run_record_tool_step",
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BenchmarkSuite {
    Standard,
}

impl BenchmarkSuite {
    pub fn parse(value: &str) -> Result<Self, String> {
        match value {
            "standard" => Ok(Self::Standard),
            other => Err(format!(
                "unknown benchmark suite '{other}' (expected standard)"
            )),
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Standard => "standard",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BenchmarkScenario {
    StartupFrames300,
    WarmIdle300,
    SelectArtifactInspector300,
    InspectorRunRecordsExpanded300,
    InspectorGraphEdgesExpanded300,
    InspectorArtifactEdgesExpanded300,
    InspectorPatchDebugExpanded300,
    InspectorLlmCallsExpanded300,
    InspectorSourceRefsExpanded300,
    InspectorArtifactIdsExpanded300,
    InspectorSectionsSequence30,
    InspectorSectionPhaseSequence30 {
        section: BenchmarkInspectorSection,
        target: BenchmarkSelectionTarget,
    },
    PatchDebugCold300,
    PatchDebugWarm300,
    ModeLineage300,
    ModeArtifactTree300,
    ToggleHideUnconsideredChildren300,
    GraphSnapshotReplaceCold,
    InspectorToolDecodeExpanded300,
    GraphCatalogIdle300,
}

impl BenchmarkScenario {
    pub fn parse(value: &str) -> Result<Self, String> {
        if let Some(scenario) = parse_inspector_section_phase_sequence(value) {
            return Ok(scenario);
        }
        match value {
            "startup_frames_300" => Ok(Self::StartupFrames300),
            "warm_idle_300" => Ok(Self::WarmIdle300),
            "select_artifact_inspector_300" => Ok(Self::SelectArtifactInspector300),
            "inspector_run_records_expanded_300" => Ok(Self::InspectorRunRecordsExpanded300),
            "inspector_graph_edges_expanded_300" => Ok(Self::InspectorGraphEdgesExpanded300),
            "inspector_artifact_edges_expanded_300" => Ok(Self::InspectorArtifactEdgesExpanded300),
            "inspector_patch_debug_expanded_300" => Ok(Self::InspectorPatchDebugExpanded300),
            "inspector_llm_calls_expanded_300" => Ok(Self::InspectorLlmCallsExpanded300),
            "inspector_source_refs_expanded_300" => Ok(Self::InspectorSourceRefsExpanded300),
            "inspector_artifact_ids_expanded_300" => Ok(Self::InspectorArtifactIdsExpanded300),
            "inspector_sections_sequence_30" => Ok(Self::InspectorSectionsSequence30),
            "patch_debug_cold_300" => Ok(Self::PatchDebugCold300),
            "patch_debug_warm_300" => Ok(Self::PatchDebugWarm300),
            "mode_lineage_300" => Ok(Self::ModeLineage300),
            "mode_artifact_tree_300" => Ok(Self::ModeArtifactTree300),
            "toggle_hide_unconsidered_children_300" => Ok(Self::ToggleHideUnconsideredChildren300),
            "graph_snapshot_replace_cold" => Ok(Self::GraphSnapshotReplaceCold),
            "inspector_tool_decode_expanded_300" => Ok(Self::InspectorToolDecodeExpanded300),
            "graph_catalog_idle_300" => Ok(Self::GraphCatalogIdle300),
            other => Err(format!("unknown benchmark scenario '{other}'")),
        }
    }

    pub fn standard() -> Vec<Self> {
        vec![
            Self::StartupFrames300,
            Self::WarmIdle300,
            Self::SelectArtifactInspector300,
            Self::InspectorRunRecordsExpanded300,
            Self::InspectorGraphEdgesExpanded300,
            Self::InspectorArtifactEdgesExpanded300,
            Self::InspectorPatchDebugExpanded300,
            Self::InspectorLlmCallsExpanded300,
            Self::InspectorSourceRefsExpanded300,
            Self::InspectorArtifactIdsExpanded300,
            Self::PatchDebugCold300,
            Self::PatchDebugWarm300,
            Self::ModeLineage300,
            Self::ModeArtifactTree300,
            Self::ToggleHideUnconsideredChildren300,
            Self::GraphSnapshotReplaceCold,
            Self::InspectorToolDecodeExpanded300,
            Self::GraphCatalogIdle300,
        ]
    }

    /// Scenarios included in the wasm-parity perf regression gate (standard + GAP instrumentation).
    pub fn regression_gated() -> Vec<Self> {
        Self::standard()
    }

    pub fn name(self) -> String {
        match self {
            Self::StartupFrames300 => "startup_frames_300".to_owned(),
            Self::WarmIdle300 => "warm_idle_300".to_owned(),
            Self::SelectArtifactInspector300 => "select_artifact_inspector_300".to_owned(),
            Self::InspectorRunRecordsExpanded300 => "inspector_run_records_expanded_300".to_owned(),
            Self::InspectorGraphEdgesExpanded300 => "inspector_graph_edges_expanded_300".to_owned(),
            Self::InspectorArtifactEdgesExpanded300 => {
                "inspector_artifact_edges_expanded_300".to_owned()
            }
            Self::InspectorPatchDebugExpanded300 => "inspector_patch_debug_expanded_300".to_owned(),
            Self::InspectorLlmCallsExpanded300 => "inspector_llm_calls_expanded_300".to_owned(),
            Self::InspectorSourceRefsExpanded300 => "inspector_source_refs_expanded_300".to_owned(),
            Self::InspectorArtifactIdsExpanded300 => {
                "inspector_artifact_ids_expanded_300".to_owned()
            }
            Self::InspectorSectionsSequence30 => "inspector_sections_sequence_30".to_owned(),
            Self::InspectorSectionPhaseSequence30 { section, target } => {
                let suffix = match target {
                    BenchmarkSelectionTarget::Primary => "30",
                    BenchmarkSelectionTarget::Alternate => "alternate_30",
                };
                format!("inspector_{}_phase_sequence_{suffix}", section.as_str())
            }
            Self::PatchDebugCold300 => "patch_debug_cold_300".to_owned(),
            Self::PatchDebugWarm300 => "patch_debug_warm_300".to_owned(),
            Self::ModeLineage300 => "mode_lineage_300".to_owned(),
            Self::ModeArtifactTree300 => "mode_artifact_tree_300".to_owned(),
            Self::ToggleHideUnconsideredChildren300 => {
                "toggle_hide_unconsidered_children_300".to_owned()
            }
            Self::GraphSnapshotReplaceCold => "graph_snapshot_replace_cold".to_owned(),
            Self::InspectorToolDecodeExpanded300 => "inspector_tool_decode_expanded_300".to_owned(),
            Self::GraphCatalogIdle300 => "graph_catalog_idle_300".to_owned(),
        }
    }

    fn frame_target(self) -> usize {
        if let Some(sequence) = self.inspector_section_phase_sequence() {
            return sequence.frame_target();
        }
        match self {
            Self::InspectorSectionsSequence30 => {
                INSPECTOR_SEQUENCE_WARMUP_FRAMES
                    + INSPECTOR_SEQUENCE_SELECT_SETTLE_FRAMES
                    + BenchmarkInspectorSection::sequence().len()
                        * INSPECTOR_SEQUENCE_SECTION_FRAMES
            }
            _ => STANDARD_FRAME_TARGET,
        }
    }

    pub fn action(self) -> BenchmarkAction {
        match self {
            Self::StartupFrames300 | Self::WarmIdle300 => BenchmarkAction::None,
            Self::SelectArtifactInspector300 => BenchmarkAction::SelectArtifact {
                inspector_section: None,
                reset_patch_cache: false,
            },
            Self::InspectorRunRecordsExpanded300 => BenchmarkAction::SelectArtifact {
                inspector_section: Some(BenchmarkInspectorSection::RunRecords),
                reset_patch_cache: false,
            },
            Self::InspectorGraphEdgesExpanded300 => BenchmarkAction::SelectArtifact {
                inspector_section: Some(BenchmarkInspectorSection::GraphEdges),
                reset_patch_cache: false,
            },
            Self::InspectorArtifactEdgesExpanded300 => BenchmarkAction::SelectArtifact {
                inspector_section: Some(BenchmarkInspectorSection::ArtifactEdges),
                reset_patch_cache: false,
            },
            Self::InspectorPatchDebugExpanded300 => BenchmarkAction::SelectArtifact {
                inspector_section: Some(BenchmarkInspectorSection::PatchDebug),
                reset_patch_cache: false,
            },
            Self::InspectorLlmCallsExpanded300 => BenchmarkAction::SelectArtifact {
                inspector_section: Some(BenchmarkInspectorSection::LlmCalls),
                reset_patch_cache: false,
            },
            Self::InspectorSourceRefsExpanded300 => BenchmarkAction::SelectArtifact {
                inspector_section: Some(BenchmarkInspectorSection::SourceRefs),
                reset_patch_cache: false,
            },
            Self::InspectorArtifactIdsExpanded300 => BenchmarkAction::SelectArtifact {
                inspector_section: Some(BenchmarkInspectorSection::ArtifactIds),
                reset_patch_cache: false,
            },
            Self::InspectorSectionsSequence30 => BenchmarkAction::InspectorSequence {
                stage: InspectorSequenceStage::Warmup,
            },
            Self::InspectorSectionPhaseSequence30 { section, target } => {
                BenchmarkAction::InspectorSectionPhase {
                    section,
                    target,
                    phase: InspectorSectionPhase::IdleBeforeSelection,
                }
            }
            Self::PatchDebugCold300 => BenchmarkAction::SelectArtifact {
                inspector_section: Some(BenchmarkInspectorSection::PatchDebug),
                reset_patch_cache: true,
            },
            Self::PatchDebugWarm300 => BenchmarkAction::SelectArtifact {
                inspector_section: Some(BenchmarkInspectorSection::PatchDebug),
                reset_patch_cache: false,
            },
            Self::ModeLineage300 => BenchmarkAction::SetMode(GraphViewMode::Lineage),
            Self::ModeArtifactTree300 => BenchmarkAction::SetMode(GraphViewMode::ArtifactTree),
            Self::ToggleHideUnconsideredChildren300 => {
                BenchmarkAction::ToggleHideUnconsideredChildren
            }
            Self::GraphSnapshotReplaceCold => BenchmarkAction::None,
            Self::InspectorToolDecodeExpanded300 => BenchmarkAction::SelectArtifact {
                inspector_section: Some(BenchmarkInspectorSection::ToolDecode),
                reset_patch_cache: false,
            },
            Self::GraphCatalogIdle300 => BenchmarkAction::None,
        }
    }

    fn action_for_completed_frames(self, completed_frames: usize) -> Option<BenchmarkAction> {
        if let Some(sequence) = self.inspector_section_phase_sequence() {
            return sequence.action_for_completed_frames(completed_frames);
        }

        if self == Self::GraphSnapshotReplaceCold {
            return (completed_frames == 0)
                .then_some(BenchmarkAction::ReplaceGraphFromSnapshotFixture);
        }
        if self == Self::GraphCatalogIdle300 {
            return (completed_frames == 0).then_some(BenchmarkAction::SetGraphCatalogVisible);
        }

        if self != Self::InspectorSectionsSequence30 {
            return (completed_frames == 0).then(|| self.action());
        }

        if completed_frames == 0 {
            return Some(BenchmarkAction::InspectorSequence {
                stage: InspectorSequenceStage::Warmup,
            });
        }

        if completed_frames == INSPECTOR_SEQUENCE_WARMUP_FRAMES {
            return Some(BenchmarkAction::InspectorSequence {
                stage: InspectorSequenceStage::SelectArtifact,
            });
        }

        let section_start =
            INSPECTOR_SEQUENCE_WARMUP_FRAMES + INSPECTOR_SEQUENCE_SELECT_SETTLE_FRAMES;
        if completed_frames < section_start {
            return None;
        }
        let section_offset = completed_frames - section_start;
        if section_offset % INSPECTOR_SEQUENCE_SECTION_FRAMES != 0 {
            return None;
        }
        BenchmarkInspectorSection::sequence()
            .get(section_offset / INSPECTOR_SEQUENCE_SECTION_FRAMES)
            .copied()
            .map(|section| BenchmarkAction::InspectorSequence {
                stage: InspectorSequenceStage::OpenSection(section),
            })
    }

    fn has_staged_actions(self) -> bool {
        self == Self::InspectorSectionsSequence30
            || self.inspector_section_phase_sequence().is_some()
    }

    fn inspector_section_phase_sequence(self) -> Option<InspectorSectionPhaseSequence> {
        match self {
            Self::InspectorSectionPhaseSequence30 { section, target } => {
                Some(InspectorSectionPhaseSequence { section, target })
            }
            _ => None,
        }
    }

    fn phase_windows(self, frames: &[FrameSample]) -> Vec<ScenarioPhaseWindow> {
        self.inspector_section_phase_sequence()
            .map(|sequence| sequence.phase_windows(frames))
            .unwrap_or_default()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BenchmarkAction {
    None,
    SelectArtifact {
        inspector_section: Option<BenchmarkInspectorSection>,
        reset_patch_cache: bool,
    },
    InspectorSequence {
        stage: InspectorSequenceStage,
    },
    InspectorSectionPhase {
        section: BenchmarkInspectorSection,
        target: BenchmarkSelectionTarget,
        phase: InspectorSectionPhase,
    },
    SetMode(GraphViewMode),
    ToggleHideUnconsideredChildren,
    ReplaceGraphFromSnapshotFixture,
    SetGraphCatalogVisible,
}

impl BenchmarkAction {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::None => "none",
            Self::SelectArtifact {
                inspector_section: None,
                ..
            } => "select_artifact_inspector",
            Self::SelectArtifact {
                reset_patch_cache: true,
                ..
            } => "patch_debug_cold",
            Self::SelectArtifact {
                reset_patch_cache: false,
                inspector_section: Some(BenchmarkInspectorSection::PatchDebug),
            } => "patch_debug_warm",
            Self::SelectArtifact {
                inspector_section: Some(section),
                ..
            } => section.action_label(),
            Self::InspectorSequence { .. } => "inspector_sections_sequence",
            Self::InspectorSectionPhase { .. } => "inspector_section_phase_sequence",
            Self::SetMode(GraphViewMode::Lineage) => "set_mode_lineage",
            Self::SetMode(GraphViewMode::ArtifactTree) => "set_mode_artifact_tree",
            Self::SetMode(GraphViewMode::ArtifactAndLineage) => "set_mode_artifact_and_lineage",
            Self::SetMode(GraphViewMode::Empty) => "set_mode_empty",
            Self::ToggleHideUnconsideredChildren => "toggle_hide_unconsidered_children",
            Self::ReplaceGraphFromSnapshotFixture => "graph_snapshot_replace_cold",
            Self::SetGraphCatalogVisible => "graph_catalog_idle",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BenchmarkSelectionTarget {
    Primary,
    Alternate,
}

impl BenchmarkSelectionTarget {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Primary => "primary",
            Self::Alternate => "alternate",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum InspectorSectionPhase {
    IdleBeforeSelection,
    SelectNode,
    IdleSelectedCollapsed,
    ExpandSection,
    IdleExpanded,
    CollapseSection,
    IdleCollapsed,
    UnselectNode,
    IdleAfterUnselect,
}

impl InspectorSectionPhase {
    pub fn sequence() -> &'static [Self] {
        &[
            Self::IdleBeforeSelection,
            Self::SelectNode,
            Self::IdleSelectedCollapsed,
            Self::ExpandSection,
            Self::IdleExpanded,
            Self::CollapseSection,
            Self::IdleCollapsed,
            Self::UnselectNode,
            Self::IdleAfterUnselect,
        ]
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::IdleBeforeSelection => "idle_before_selection",
            Self::SelectNode => "select_node",
            Self::IdleSelectedCollapsed => "idle_selected_collapsed",
            Self::ExpandSection => "expand_section",
            Self::IdleExpanded => "idle_expanded",
            Self::CollapseSection => "collapse_section",
            Self::IdleCollapsed => "idle_collapsed",
            Self::UnselectNode => "unselect_node",
            Self::IdleAfterUnselect => "idle_after_unselect",
        }
    }

    fn action_required(self) -> bool {
        matches!(
            self,
            Self::IdleBeforeSelection
                | Self::SelectNode
                | Self::ExpandSection
                | Self::CollapseSection
                | Self::UnselectNode
        )
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct InspectorSectionPhaseSequence {
    section: BenchmarkInspectorSection,
    target: BenchmarkSelectionTarget,
}

impl InspectorSectionPhaseSequence {
    fn frame_target(self) -> usize {
        InspectorSectionPhase::sequence().len() * INSPECTOR_SECTION_PHASE_FRAMES
    }

    fn action_for_completed_frames(self, completed_frames: usize) -> Option<BenchmarkAction> {
        if completed_frames >= self.frame_target()
            || completed_frames % INSPECTOR_SECTION_PHASE_FRAMES != 0
        {
            return None;
        }
        let phase = *InspectorSectionPhase::sequence()
            .get(completed_frames / INSPECTOR_SECTION_PHASE_FRAMES)?;
        phase
            .action_required()
            .then_some(BenchmarkAction::InspectorSectionPhase {
                section: self.section,
                target: self.target,
                phase,
            })
    }

    fn phase_windows(self, frames: &[FrameSample]) -> Vec<ScenarioPhaseWindow> {
        InspectorSectionPhase::sequence()
            .iter()
            .enumerate()
            .map(|(index, phase)| {
                let start = index * INSPECTOR_SECTION_PHASE_FRAMES;
                let end = start + INSPECTOR_SECTION_PHASE_FRAMES;
                let window = frame_window(frames, start, end);
                ScenarioPhaseWindow {
                    phase_index: index + 1,
                    phase: phase.as_str().to_owned(),
                    inspector_section: Some(self.section.as_str().to_owned()),
                    selection_target: Some(self.target.as_str().to_owned()),
                    frame_start: window.frame_start,
                    frame_end: window.frame_end,
                    stats: window.stats,
                    heap_profile_delta: HeapScenarioProfile::default(),
                }
            })
            .collect()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InspectorSequenceStage {
    Warmup,
    SelectArtifact,
    OpenSection(BenchmarkInspectorSection),
}

impl InspectorSequenceStage {
    fn as_str(self) -> &'static str {
        match self {
            Self::Warmup => "warmup",
            Self::SelectArtifact => "select_artifact",
            Self::OpenSection(section) => section.as_str(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BenchmarkInspectorSection {
    LlmCalls,
    RunRecords,
    GraphEdges,
    ArtifactEdges,
    PatchDebug,
    SourceRefs,
    ArtifactIds,
    ToolDecode,
}

impl BenchmarkInspectorSection {
    pub fn sequence() -> &'static [Self] {
        &[
            Self::LlmCalls,
            Self::RunRecords,
            Self::GraphEdges,
            Self::ArtifactEdges,
            Self::PatchDebug,
            Self::SourceRefs,
            Self::ArtifactIds,
        ]
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::LlmCalls => "llm_calls",
            Self::RunRecords => "run_records",
            Self::GraphEdges => "graph_edges",
            Self::ArtifactEdges => "artifact_edges",
            Self::PatchDebug => "patch_debug",
            Self::SourceRefs => "source_refs",
            Self::ArtifactIds => "artifact_ids",
            Self::ToolDecode => "tool_decode",
        }
    }

    fn parse(value: &str) -> Option<Self> {
        match value {
            "llm_calls" => Some(Self::LlmCalls),
            "run_records" => Some(Self::RunRecords),
            "graph_edges" => Some(Self::GraphEdges),
            "artifact_edges" => Some(Self::ArtifactEdges),
            "patch_debug" => Some(Self::PatchDebug),
            "source_refs" => Some(Self::SourceRefs),
            "artifact_ids" => Some(Self::ArtifactIds),
            "tool_decode" => Some(Self::ToolDecode),
            _ => None,
        }
    }

    fn action_label(self) -> &'static str {
        match self {
            Self::LlmCalls => "inspector_llm_calls_expanded",
            Self::RunRecords => "inspector_run_records_expanded",
            Self::GraphEdges => "inspector_graph_edges_expanded",
            Self::ArtifactEdges => "inspector_artifact_edges_expanded",
            Self::PatchDebug => "inspector_patch_debug_expanded",
            Self::SourceRefs => "inspector_source_refs_expanded",
            Self::ArtifactIds => "inspector_artifact_ids_expanded",
            Self::ToolDecode => "inspector_tool_decode_expanded",
        }
    }
}

fn parse_inspector_section_phase_sequence(value: &str) -> Option<BenchmarkScenario> {
    let body = value.strip_prefix("inspector_")?;
    for (suffix, target) in [
        (
            "_phase_sequence_alternate_30",
            BenchmarkSelectionTarget::Alternate,
        ),
        ("_phase_sequence_30", BenchmarkSelectionTarget::Primary),
    ] {
        let Some(section) = body.strip_suffix(suffix) else {
            continue;
        };
        return Some(BenchmarkScenario::InspectorSectionPhaseSequence30 {
            section: BenchmarkInspectorSection::parse(section)?,
            target,
        });
    }
    None
}

#[derive(Debug, Clone)]
pub struct BenchmarkConfig {
    pub suite: BenchmarkSuite,
    pub run_root: PathBuf,
    pub output_dir: PathBuf,
    pub scenarios: Vec<BenchmarkScenario>,
    pub command: String,
    pub run_readiness: Option<BenchmarkRunReadiness>,
    pub callsite_sampling: Option<BenchmarkCallsiteSampling>,
}

impl BenchmarkConfig {
    pub fn new(
        suite: BenchmarkSuite,
        run_root: PathBuf,
        output_dir: Option<PathBuf>,
        scenario_filters: Vec<String>,
        callsite_sample_every: Option<u64>,
    ) -> Result<Self, String> {
        if suite == BenchmarkSuite::Standard && run_root != Path::new(STANDARD_RUN_ROOT) {
            return Err(format!(
                "benchmark suite standard v1 requires --run-root {STANDARD_RUN_ROOT}"
            ));
        }

        let scenarios = if scenario_filters.is_empty() {
            BenchmarkScenario::standard()
        } else {
            scenario_filters
                .iter()
                .map(|value| BenchmarkScenario::parse(value))
                .collect::<Result<Vec<_>, _>>()?
        };
        if scenarios.is_empty() {
            return Err("benchmark scenario list is empty".to_owned());
        }
        let run_readiness = match suite {
            BenchmarkSuite::Standard => {
                let readiness = benchmark_run_readiness(&run_root)
                    .map_err(|error| format!("benchmark run readiness check failed: {error}"))?;
                if !readiness.ready {
                    return Err(format!(
                        "benchmark run root is not ready for standard suite: {}",
                        readiness.summary()
                    ));
                }
                Some(readiness)
            }
        };
        let callsite_sampling = callsite_sample_every
            .map(BenchmarkCallsiteSampling::focused)
            .transpose()?;

        Ok(Self {
            suite,
            run_root,
            output_dir: output_dir.unwrap_or_else(|| default_output_dir(suite)),
            scenarios,
            command: std::env::args().collect::<Vec<_>>().join(" "),
            run_readiness,
            callsite_sampling,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BenchmarkCallsiteSampling {
    pub sample_every: u64,
    pub scopes: Vec<String>,
    pub accounting: String,
}

impl BenchmarkCallsiteSampling {
    fn focused(sample_every: u64) -> Result<Self, String> {
        if sample_every == 0 {
            return Err("--benchmark-callsite-sample-every must be greater than zero".to_owned());
        }
        Ok(Self {
            sample_every,
            scopes: FOCUSED_CALLSITE_SCOPES
                .iter()
                .map(|scope| (*scope).to_owned())
                .collect(),
            accounting: "scaled sampled estimates; callsite totals are not exact allocator totals"
                .to_owned(),
        })
    }

    fn scope_names(&self) -> Vec<&str> {
        self.scopes.iter().map(String::as_str).collect()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BenchmarkRunReadiness {
    pub policy_max_generations: u32,
    pub policy_child_budget_min: u32,
    pub expected_min_history_blocks: usize,
    pub sealed_history_blocks: usize,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_sealed_block_height: Option<u64>,
    pub expected_min_spawned_children: usize,
    pub spawned_child_nodes: usize,
    pub scheduler_node_count: usize,
    pub node_record_count: usize,
    pub ready: bool,
    pub reason: String,
}

impl BenchmarkRunReadiness {
    fn from_counts(
        policy_max_generations: u32,
        policy_child_budget_min: u32,
        sealed_history_blocks: usize,
        max_sealed_block_height: Option<u64>,
        scheduler_node_count: usize,
        node_record_count: usize,
        spawned_child_nodes: usize,
    ) -> Self {
        let expected_min_history_blocks = policy_max_generations.saturating_add(1) as usize;
        let expected_min_spawned_children =
            policy_max_generations.saturating_mul(policy_child_budget_min) as usize;
        let history_reaches_policy_generation = max_sealed_block_height
            .is_some_and(|height| height >= u64::from(policy_max_generations));
        let history_has_expected_blocks = sealed_history_blocks >= expected_min_history_blocks;
        let spawned_expected_children = spawned_child_nodes >= expected_min_spawned_children;
        let (ready, reason) = if history_reaches_policy_generation {
            (true, "sealed_history_reached_policy_generation")
        } else if history_has_expected_blocks {
            (true, "sealed_history_has_expected_block_count")
        } else if spawned_expected_children {
            (true, "spawned_child_nodes_reached_policy_minimum")
        } else {
            (false, "persisted_records_below_policy_expectation")
        };

        Self {
            policy_max_generations,
            policy_child_budget_min,
            expected_min_history_blocks,
            sealed_history_blocks,
            max_sealed_block_height,
            expected_min_spawned_children,
            spawned_child_nodes,
            scheduler_node_count,
            node_record_count,
            ready,
            reason: reason.to_owned(),
        }
    }

    fn from_records(
        scheduler: &SchedulerStateRecord,
        node_records: &[NodeRecord],
        history_blocks: &[SealedBlockRecord],
    ) -> Self {
        let max_sealed_block_height = history_blocks
            .iter()
            .map(|block| block.state.header.common.block_height)
            .max();
        let spawned_child_nodes = node_records
            .iter()
            .filter(|record| record.generation > 0)
            .count();
        Self::from_counts(
            scheduler.policy.max_generations,
            scheduler.policy.child_budget.min,
            history_blocks.len(),
            max_sealed_block_height,
            scheduler.nodes.len(),
            node_records.len(),
            spawned_child_nodes,
        )
    }

    fn summary(&self) -> String {
        format!(
            "{}; history_blocks={}/{} max_history_height={:?} spawned_children={}/{} node_records={}",
            self.reason,
            self.sealed_history_blocks,
            self.expected_min_history_blocks,
            self.max_sealed_block_height,
            self.spawned_child_nodes,
            self.expected_min_spawned_children,
            self.node_record_count
        )
    }
}

pub fn benchmark_run_readiness(run_root: &Path) -> Result<BenchmarkRunReadiness, Box<dyn Error>> {
    let scheduler = read_typed_json::<SchedulerStateRecord>(&run_root.join("scheduler.json"))?;
    let node_records = read_node_records(run_root)?;
    let history_blocks = FsRunStore::new(run_root).load_history_blocks()?;
    Ok(BenchmarkRunReadiness::from_records(
        &scheduler,
        &node_records,
        &history_blocks,
    ))
}

fn read_node_records(run_root: &Path) -> Result<Vec<NodeRecord>, Box<dyn Error>> {
    let nodes_dir = run_root.join("nodes");
    if !nodes_dir.is_dir() {
        return Ok(Vec::new());
    }

    let mut records = Vec::new();
    for entry in fs::read_dir(nodes_dir)? {
        let entry = entry?;
        if !entry.file_type()?.is_dir() {
            continue;
        }
        let path = entry.path().join("node.json");
        if path.is_file() {
            records.push(read_typed_json::<NodeRecord>(&path)?);
        }
    }
    Ok(records)
}

fn read_typed_json<T>(path: &Path) -> Result<T, Box<dyn Error>>
where
    T: serde::de::DeserializeOwned,
{
    let bytes = fs::read(path)?;
    Ok(serde_json::from_slice(&bytes)?)
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BenchmarkReport {
    pub schema_version: String,
    pub suite: String,
    pub created_at_unix_ms: u64,
    pub commit: GitInfo,
    pub dirty_state: DirtyState,
    pub command: String,
    pub run_root: String,
    pub feature_set: Vec<String>,
    pub scenarios_requested: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub callsite_sampling: Option<BenchmarkCallsiteSampling>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub run_readiness: Option<BenchmarkRunReadiness>,
    pub startup: StartupProfile,
    pub scenarios: Vec<ScenarioReport>,
    pub puffin_artifacts: Vec<BenchmarkArtifact>,
    pub heap_artifacts: Vec<BenchmarkArtifact>,
    pub notes: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GitInfo {
    pub short_sha: Option<String>,
    pub full_sha: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DirtyState {
    pub dirty: Option<bool>,
    pub classification: DirtyStateClassification,
    pub relevant_dirty: Option<bool>,
    pub paths: Vec<String>,
    pub relevant_paths: Vec<String>,
    pub unrelated_paths: Vec<String>,
    pub scope: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DirtyStateClassification {
    Clean,
    DirtyRelevant,
    DirtyUnrelated,
    Unknown,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct StartupProfile {
    pub spans: Vec<BenchmarkSpan>,
    pub compressed_run_records: Vec<CompressedRunRecordRead>,
    pub notes: Vec<String>,
}

impl StartupProfile {
    pub fn push_span(&mut self, name: impl Into<String>, duration_ns: u64) {
        self.spans.push(BenchmarkSpan {
            name: name.into(),
            duration_ns,
        });
    }
}

fn annotate_startup_profile(startup: &mut StartupProfile) {
    for span in &startup.spans {
        if span.name == "run_picker_discovery" {
            startup.notes.push(format!(
                "run_picker_discovery={} ns (kept in startup spans)",
                span.duration_ns
            ));
            if span.duration_ns > RUN_PICKER_DISCOVERY_WARNING_NS {
                startup.notes.push(format!(
                    "warning: run_picker_discovery exceeded {} ns",
                    RUN_PICKER_DISCOVERY_WARNING_NS
                ));
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BenchmarkSpan {
    pub name: String,
    pub duration_ns: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CompressedRunRecordRead {
    pub path: String,
    pub compressed_bytes: u64,
    pub decompressed_bytes: u64,
    pub turn_count: usize,
    pub tool_call_count: usize,
    pub failed_tool_call_count: usize,
    pub open_read_ns: u64,
    pub gzip_decompress_ns: u64,
    pub typed_json_deserialize_ns: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ScenarioReport {
    pub name: String,
    pub target_frames: usize,
    pub frame_stats: DurationStats,
    pub top_frames: Vec<TopFrame>,
    pub component_timings: Vec<ComponentTimingReport>,
    pub allocation_frames: AllocationFrameReport,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub phase_windows: Vec<ScenarioPhaseWindow>,
    pub heap_profile: HeapScenarioProfile,
    pub allocations: AllocationDelta,
    pub action: BenchmarkActionReport,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BenchmarkActionReport {
    pub action: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub target_label: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub target_key: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fallback: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub inspector_section: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub notes: Vec<String>,
}

impl BenchmarkActionReport {
    pub fn for_action(action: BenchmarkAction) -> Self {
        Self {
            action: action.as_str().to_owned(),
            target_label: None,
            target_key: None,
            fallback: None,
            inspector_section: match action {
                BenchmarkAction::SelectArtifact {
                    inspector_section: Some(section),
                    ..
                } => Some(section.as_str().to_owned()),
                BenchmarkAction::InspectorSequence {
                    stage: InspectorSequenceStage::OpenSection(section),
                } => Some(section.as_str().to_owned()),
                BenchmarkAction::InspectorSectionPhase { section, .. } => {
                    Some(section.as_str().to_owned())
                }
                _ => None,
            },
            notes: action_notes(action),
        }
    }

    fn merge_stage(&mut self, mut stage: BenchmarkActionReport) {
        if let Some(target_label) = stage.target_label.take() {
            self.target_label = Some(target_label);
        }
        if let Some(target_key) = stage.target_key.take() {
            self.target_key = Some(target_key);
        }
        if let Some(fallback) = stage.fallback.take() {
            self.fallback = Some(fallback);
        }
        if let Some(inspector_section) = stage.inspector_section.take() {
            self.inspector_section = Some(inspector_section);
        }
        self.notes.append(&mut stage.notes);
    }
}

fn action_notes(action: BenchmarkAction) -> Vec<String> {
    match action {
        BenchmarkAction::InspectorSequence { stage } => {
            let mut notes = Vec::new();
            if stage == InspectorSequenceStage::Warmup {
                notes.push(format!(
                    "sequence_warmup_frames={INSPECTOR_SEQUENCE_WARMUP_FRAMES}"
                ));
                notes.push(format!(
                    "sequence_select_settle_frames={INSPECTOR_SEQUENCE_SELECT_SETTLE_FRAMES}"
                ));
                notes.push(format!(
                    "sequence_section_frames={INSPECTOR_SEQUENCE_SECTION_FRAMES}"
                ));
                notes.push(format!(
                    "sequence_sections={}",
                    BenchmarkInspectorSection::sequence()
                        .iter()
                        .map(|section| section.as_str())
                        .collect::<Vec<_>>()
                        .join(",")
                ));
            }
            notes.push(format!("sequence_stage={}", stage.as_str()));
            notes
        }
        BenchmarkAction::InspectorSectionPhase {
            section,
            target,
            phase,
        } => vec![
            format!("phase_sequence_section={}", section.as_str()),
            format!("phase_sequence_target={}", target.as_str()),
            format!("phase_sequence_phase={}", phase.as_str()),
            format!("phase_sequence_frames={INSPECTOR_SECTION_PHASE_FRAMES}"),
        ],
        _ => Vec::new(),
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TopFrame {
    pub frame_index: usize,
    pub duration_ns: u64,
    pub components: Vec<ComponentTiming>,
    pub allocations: FrameAllocationSample,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ComponentTimingReport {
    pub component: String,
    pub stats: DurationStats,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub median_percent_of_frame_x100: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub nested_under: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ComponentTiming {
    pub component: String,
    pub duration_ns: u64,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct FrameAllocationSample {
    pub allocation_count_delta: u64,
    pub deallocation_count_delta: u64,
    pub allocated_object_bytes_delta: u64,
    pub deallocated_object_bytes_delta: u64,
    pub allocated_wrapped_bytes_delta: u64,
    pub deallocated_wrapped_bytes_delta: u64,
    pub live_object_bytes: u64,
    pub live_wrapped_bytes: u64,
}

impl FrameAllocationSample {
    fn from_snapshots(start: &HeapProfileSnapshot, end: &HeapProfileSnapshot) -> Self {
        Self {
            allocation_count_delta: end
                .totals
                .allocation_count
                .saturating_sub(start.totals.allocation_count),
            deallocation_count_delta: end
                .totals
                .deallocation_count
                .saturating_sub(start.totals.deallocation_count),
            allocated_object_bytes_delta: end
                .totals
                .allocated_object_bytes
                .saturating_sub(start.totals.allocated_object_bytes),
            deallocated_object_bytes_delta: end
                .totals
                .deallocated_object_bytes
                .saturating_sub(start.totals.deallocated_object_bytes),
            allocated_wrapped_bytes_delta: end
                .totals
                .allocated_wrapped_bytes
                .saturating_sub(start.totals.allocated_wrapped_bytes),
            deallocated_wrapped_bytes_delta: end
                .totals
                .deallocated_wrapped_bytes
                .saturating_sub(start.totals.deallocated_wrapped_bytes),
            live_object_bytes: end.totals.live_object_bytes,
            live_wrapped_bytes: end.totals.live_wrapped_bytes,
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ValueStats {
    pub count: usize,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub min: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub median: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub p95: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub p99: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max: Option<u64>,
}

impl ValueStats {
    fn from_values(values: &[u64]) -> Self {
        let mut sorted = values.to_vec();
        sorted.sort_unstable();
        Self {
            count: sorted.len(),
            min: sorted.first().copied(),
            median: percentile(&sorted, 50),
            p95: percentile(&sorted, 95),
            p99: percentile(&sorted, 99),
            max: sorted.last().copied(),
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct AllocationFrameStats {
    pub allocation_count: ValueStats,
    pub allocated_object_bytes: ValueStats,
    pub allocated_wrapped_bytes: ValueStats,
    pub live_object_bytes: ValueStats,
    pub live_wrapped_bytes: ValueStats,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct AllocationFrameWindow {
    pub frame_start: usize,
    pub frame_end: usize,
    pub stats: AllocationFrameStats,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ScenarioPhaseWindow {
    pub phase_index: usize,
    pub phase: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub inspector_section: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub selection_target: Option<String>,
    pub frame_start: usize,
    pub frame_end: usize,
    pub stats: AllocationFrameStats,
    #[serde(default)]
    pub heap_profile_delta: HeapScenarioProfile,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HeapSlope {
    Plateau,
    Growing,
    #[default]
    Inconclusive,
}

impl HeapSlope {
    fn as_str(self) -> &'static str {
        match self {
            Self::Plateau => "plateau",
            Self::Growing => "growing",
            Self::Inconclusive => "inconclusive",
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct AllocationFrameReport {
    pub per_frame: AllocationFrameStats,
    pub first_50: AllocationFrameWindow,
    pub last_50: AllocationFrameWindow,
    pub slope: HeapSlope,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct HeapScenarioProfile {
    pub totals: HeapProfileTotals,
    pub unmatched_deallocations: u64,
    pub top_groups_by_allocated_bytes: Vec<HeapGroupProfile>,
    pub top_groups_by_allocation_count: Vec<HeapGroupProfile>,
    pub top_groups_by_retained_bytes: Vec<HeapGroupProfile>,
    pub top_callsites_by_allocated_bytes: Vec<HeapCallsiteProfile>,
    pub top_callsites_by_allocation_count: Vec<HeapCallsiteProfile>,
    pub top_callsites_by_retained_bytes: Vec<HeapCallsiteProfile>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct DurationStats {
    pub count: usize,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub min_ns: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub median_ns: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub p95_ns: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub p99_ns: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_ns: Option<u64>,
}

impl DurationStats {
    pub fn from_durations(durations: &[u64]) -> Self {
        let mut sorted = durations.to_vec();
        sorted.sort_unstable();
        Self {
            count: sorted.len(),
            min_ns: sorted.first().copied(),
            median_ns: percentile(&sorted, 50),
            p95_ns: percentile(&sorted, 95),
            p99_ns: percentile(&sorted, 99),
            max_ns: sorted.last().copied(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BenchmarkArtifact {
    pub path: String,
    pub bytes: u64,
    pub sha256: String,
    pub tracked: bool,
}

#[derive(Debug)]
pub struct BenchmarkWriteResult {
    pub report_path: PathBuf,
    pub readme_path: PathBuf,
}

pub struct BenchmarkController {
    config: BenchmarkConfig,
    startup: StartupProfile,
    created_at_unix_ms: u64,
    git: GitInfo,
    dirty_state: DirtyState,
    feature_set: Vec<String>,
    heap_tracker: allocation::HeapProfileTracker,
    scenario_index: usize,
    current: Option<ScenarioCapture>,
    completed: Vec<ScenarioReport>,
    completed_heap_profiles: Vec<(String, HeapProfileSnapshot)>,
    current_components: Vec<ComponentTiming>,
    last_heap_snapshot: HeapProfileSnapshot,
    frame_start: Option<Instant>,
    finished: bool,
    #[cfg(feature = "native-benchmark")]
    puffin_view: puffin::GlobalFrameView,
}

impl std::fmt::Debug for BenchmarkController {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("BenchmarkController")
            .field("config", &self.config)
            .field("startup", &self.startup)
            .field("created_at_unix_ms", &self.created_at_unix_ms)
            .field("git", &self.git)
            .field("dirty_state", &self.dirty_state)
            .field("feature_set", &self.feature_set)
            .field("scenario_index", &self.scenario_index)
            .field("current", &self.current)
            .field("completed", &self.completed)
            .field("finished", &self.finished)
            .finish_non_exhaustive()
    }
}

impl BenchmarkController {
    pub fn new(
        config: BenchmarkConfig,
        mut startup: StartupProfile,
    ) -> Result<Self, Box<dyn Error>> {
        #[cfg(feature = "native-benchmark")]
        let puffin_view = {
            puffin::set_scopes_on(true);
            let view = puffin::GlobalFrameView::default();
            {
                let mut locked = view.lock();
                let frames = config
                    .scenarios
                    .iter()
                    .map(|scenario| scenario.frame_target())
                    .sum::<usize>();
                locked.set_max_recent(frames.saturating_add(64));
                locked.set_max_slow(128);
            }
            view
        };
        let heap_tracker = allocation::install_global_tracker()?;
        if let Some(sampling) = &config.callsite_sampling {
            let scopes = sampling.scope_names();
            allocation::configure_callsite_sampling(
                &heap_tracker,
                Some(sampling.sample_every),
                &scopes,
            )
            .map_err(io::Error::other)?;
        } else {
            allocation::configure_callsite_sampling(&heap_tracker, None, &[])
                .map_err(io::Error::other)?;
        }
        annotate_startup_profile(&mut startup);
        if let Some(readiness) = &config.run_readiness {
            startup.notes.push(format!(
                "standard_run_readiness_heuristic: {}",
                readiness.summary()
            ));
        }

        Ok(Self {
            config,
            startup,
            created_at_unix_ms: unix_ms_now(),
            git: git_info(),
            dirty_state: dirty_state(),
            feature_set: feature_set(),
            heap_tracker,
            scenario_index: 0,
            current: None,
            completed: Vec::new(),
            completed_heap_profiles: Vec::new(),
            current_components: Vec::new(),
            last_heap_snapshot: HeapProfileSnapshot::default(),
            frame_start: None,
            finished: false,
            #[cfg(feature = "native-benchmark")]
            puffin_view,
        })
    }

    pub fn begin_frame(&mut self) -> Option<BenchmarkAction> {
        if self.finished {
            return None;
        }

        self.current_components.clear();
        self.frame_start = Some(Instant::now());

        if self.current.is_none() {
            let scenario = *self.config.scenarios.get(self.scenario_index)?;
            self.scenario_index += 1;
            allocation::begin_tracking_window(&self.heap_tracker);
            let heap_start = allocation::heap_profile_snapshot(&self.heap_tracker);
            self.last_heap_snapshot = heap_start.clone();
            self.current = Some(allocation::untracked(|| {
                ScenarioCapture::new(scenario, allocation::snapshot(), heap_start)
            }));
            return scenario.action_for_completed_frames(0);
        }

        self.current.as_ref().and_then(|current| {
            current
                .scenario
                .action_for_completed_frames(current.frames.len())
        })
    }

    pub fn record_action(&mut self, report: BenchmarkActionReport) {
        if let Some(current) = &mut self.current {
            if current.scenario.has_staged_actions() && !current.frames.is_empty() {
                current.action.merge_stage(report);
            } else {
                current.action = report;
            }
        }
    }

    pub fn record_component(&mut self, component: &'static str, duration_ns: u64) {
        if self.current.is_some() {
            self.current_components.push(ComponentTiming {
                component: component.to_owned(),
                duration_ns,
            });
        }
    }

    pub fn end_frame(&mut self) -> Result<Option<BenchmarkWriteResult>, Box<dyn Error>> {
        if self.finished {
            return Ok(None);
        }
        let Some(frame_start) = self.frame_start.take() else {
            return Ok(None);
        };
        let frame_duration = elapsed_ns(frame_start);
        let components = std::mem::take(&mut self.current_components);
        let heap_snapshot = allocation::heap_profile_snapshot(&self.heap_tracker);
        let frame_allocations =
            FrameAllocationSample::from_snapshots(&self.last_heap_snapshot, &heap_snapshot);
        self.last_heap_snapshot = heap_snapshot.clone();
        let Some(current) = &mut self.current else {
            return Ok(None);
        };
        current.frames.push(FrameSample {
            frame_index: current.frames.len() + 1,
            duration_ns: frame_duration,
            components,
            allocations: frame_allocations,
        });
        current.heap_snapshots.push(heap_snapshot);

        if current.frames.len() < current.scenario.frame_target() {
            return Ok(None);
        }

        let capture = self
            .current
            .take()
            .expect("current scenario exists after frame push");
        let heap_profile = allocation::finish_tracking_window(&self.heap_tracker);
        let allocation_end = heap_profile.allocation_snapshot();
        let (report, full_heap_profile) = capture.finish(allocation_end, heap_profile);
        self.completed_heap_profiles
            .push((report.name.clone(), full_heap_profile));
        self.completed.push(report);
        let write_result = self.write_report()?;
        if self.completed.len() == self.config.scenarios.len() {
            self.finished = true;
            return Ok(Some(write_result));
        }

        Ok(None)
    }

    fn write_report(&self) -> Result<BenchmarkWriteResult, Box<dyn Error>> {
        allocation::finish_tracking_window(&self.heap_tracker);
        #[cfg(feature = "native-benchmark")]
        let puffin_artifacts = vec![self.write_puffin_capture()?];
        #[cfg(not(feature = "native-benchmark"))]
        let puffin_artifacts = Vec::new();
        let heap_artifacts = self.write_heap_profiles()?;

        let report = BenchmarkReport {
            schema_version: BENCHMARK_REPORT_VERSION.to_owned(),
            suite: self.config.suite.as_str().to_owned(),
            created_at_unix_ms: self.created_at_unix_ms,
            commit: self.git.clone(),
            dirty_state: self.dirty_state.clone(),
            command: self.config.command.clone(),
            run_root: self.config.run_root.display().to_string(),
            feature_set: self.feature_set.clone(),
            scenarios_requested: self
                .config
                .scenarios
                .iter()
                .map(|scenario| scenario.name())
                .collect(),
            callsite_sampling: self.config.callsite_sampling.clone(),
            run_readiness: self.config.run_readiness.clone(),
            startup: self.startup.clone(),
            scenarios: self.completed.clone(),
            puffin_artifacts,
            heap_artifacts,
            notes: benchmark_notes(&self.dirty_state, self.config.callsite_sampling.as_ref()),
        };

        fs::create_dir_all(&self.config.output_dir)?;
        let report_path = self.config.output_dir.join("report.json");
        let readme_path = self.config.output_dir.join("README.md");
        fs::write(&report_path, serde_json::to_vec_pretty(&report)?)?;
        fs::write(&readme_path, render_benchmark_readme(&report))?;
        Ok(BenchmarkWriteResult {
            report_path,
            readme_path,
        })
    }

    #[cfg(feature = "native-benchmark")]
    fn write_puffin_capture(&self) -> io::Result<BenchmarkArtifact> {
        let root = default_puffin_benchmark_dir(&self.config.output_dir);
        fs::create_dir_all(&root)?;
        let path = root.join(format!("{}.puffin", self.config.suite.as_str()));
        let view = self.puffin_view.lock();
        let mut file = fs::File::create(&path)?;
        view.write(&mut file).map_err(io::Error::other)?;
        file_artifact(&path, false)
    }

    fn write_heap_profiles(&self) -> Result<Vec<BenchmarkArtifact>, Box<dyn Error>> {
        let root = default_heap_benchmark_dir(&self.config.output_dir);
        fs::create_dir_all(&root)?;
        let mut artifacts = Vec::new();
        for (scenario, profile) in &self.completed_heap_profiles {
            let path = root.join(format!("{scenario}.heap.json"));
            let encoded = allocation::untracked(|| serde_json::to_vec_pretty(profile))?;
            fs::write(&path, encoded)?;
            artifacts.push(file_artifact(&path, false)?);
        }
        Ok(artifacts)
    }
}

fn benchmark_notes(
    dirty_state: &DirtyState,
    callsite_sampling: Option<&BenchmarkCallsiteSampling>,
) -> Vec<String> {
    let mut notes = vec![
        "reporting-only benchmark; no pass/fail thresholds applied".to_owned(),
        "allocation deltas use tracking-allocator object bytes and wrapped bytes; GPU and driver memory are outside the measured surface".to_owned(),
        "report.json and README.md are rewritten after each completed scenario so interrupted runs keep partial evidence".to_owned(),
        "puffin captures are local ignored artifacts under crates/ploke-egui/data/".to_owned(),
        "full heap profiles are local ignored artifacts under crates/ploke-egui/data/profiling/heap/benchmarks/".to_owned(),
    ];
    if let Some(sampling) = callsite_sampling {
        notes.push(format!(
            "focused callsite sampling enabled every {} matching allocations for scopes: {}; callsite totals are scaled estimates",
            sampling.sample_every,
            sampling.scopes.join(", ")
        ));
    } else {
        notes.push(
            "standard heap attribution is driven by #[tracing::instrument] span names and uses cheap totals/group counters; callsite backtraces are not captured in standard mode".to_owned(),
        );
    }
    if dirty_state.classification == DirtyStateClassification::DirtyUnrelated {
        notes.push(
            "git worktree was dirty only outside benchmark-relevant paths at benchmark start"
                .to_owned(),
        );
    }
    notes
}

#[derive(Debug)]
struct ScenarioCapture {
    scenario: BenchmarkScenario,
    frames: Vec<FrameSample>,
    heap_snapshots: Vec<HeapProfileSnapshot>,
    action: BenchmarkActionReport,
    allocation_start: AllocationSnapshot,
}

impl ScenarioCapture {
    fn new(
        scenario: BenchmarkScenario,
        allocation_start: AllocationSnapshot,
        heap_start: HeapProfileSnapshot,
    ) -> Self {
        let mut heap_snapshots = Vec::with_capacity(scenario.frame_target().saturating_add(1));
        heap_snapshots.push(heap_start);
        Self {
            scenario,
            frames: Vec::with_capacity(scenario.frame_target()),
            heap_snapshots,
            action: BenchmarkActionReport::for_action(scenario.action()),
            allocation_start,
        }
    }

    fn finish(
        self,
        allocation_end: AllocationSnapshot,
        heap_profile: HeapProfileSnapshot,
    ) -> (ScenarioReport, HeapProfileSnapshot) {
        let durations = self
            .frames
            .iter()
            .map(|frame| frame.duration_ns)
            .collect::<Vec<_>>();
        let frame_stats = DurationStats::from_durations(&durations);
        let report = ScenarioReport {
            name: self.scenario.name(),
            target_frames: self.scenario.frame_target(),
            frame_stats: frame_stats.clone(),
            top_frames: top_frames(&self.frames),
            component_timings: component_reports(&self.frames, frame_stats.median_ns),
            allocation_frames: allocation_frame_report(&self.frames),
            phase_windows: phase_windows_with_heap_profiles(
                self.scenario.phase_windows(&self.frames),
                &self.heap_snapshots,
            ),
            heap_profile: heap_summary(&heap_profile),
            allocations: allocation_end.delta_since(self.allocation_start),
            action: self.action,
        };
        (report, heap_profile)
    }
}

#[derive(Debug)]
struct FrameSample {
    frame_index: usize,
    duration_ns: u64,
    components: Vec<ComponentTiming>,
    allocations: FrameAllocationSample,
}

pub fn load_graph_with_startup_profile(
    run_root: &Path,
    mut startup: StartupProfile,
) -> Result<(Graph, StartupProfile), Box<dyn Error>> {
    let total_start = Instant::now();
    let store = FsRunStore::new(run_root);

    let load_record_set_start = Instant::now();
    let load_start = Instant::now();
    let forest_input = store.load()?;
    startup.push_span("FsRunStore::load", elapsed_ns(load_start));

    let history_start = Instant::now();
    let history_blocks = store.load_history_blocks()?;
    startup.push_span("FsRunStore::load_history_blocks", elapsed_ns(history_start));

    let journal_start = Instant::now();
    let transition_journal = store.load_transition_journal()?;
    startup.push_span(
        "FsRunStore::load_transition_journal",
        elapsed_ns(journal_start),
    );

    let records = RunRecordSet {
        forest_input,
        history_blocks,
        transition_journal,
        agent_turn_records: Default::default(),
    };
    startup.push_span(
        "FsRunStore::load_record_set",
        elapsed_ns(load_record_set_start),
    );

    let compressed_start = Instant::now();
    startup.compressed_run_records = compressed_record_reads(&records)?;
    startup.push_span(
        "compressed_run_record_profile_probe",
        elapsed_ns(compressed_start),
    );

    let graph_start = Instant::now();
    let graph = Graph::from_records(&records);
    startup.push_span("Graph::from_records", elapsed_ns(graph_start));
    startup.push_span("graph_load_total", elapsed_ns(total_start));
    Ok((graph, startup))
}

pub fn span_from_start(name: &'static str, start: Instant) -> BenchmarkSpan {
    BenchmarkSpan {
        name: name.to_owned(),
        duration_ns: elapsed_ns(start),
    }
}

pub fn default_output_dir(suite: BenchmarkSuite) -> PathBuf {
    let date = Utc::now().format("%Y%m%d");
    let short = git_info()
        .short_sha
        .unwrap_or_else(|| "unknownsha".to_owned());
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("docs")
        .join("profiling")
        .join("benchmarks")
        .join(format!("{date}-{short}-{}", suite.as_str()))
}

fn compressed_record_reads(
    records: &RunRecordSet,
) -> Result<Vec<CompressedRunRecordRead>, io::Error> {
    let mut paths = BTreeSet::new();
    if let Some(run_records) = &records.forest_input.passive_evidence.run_records {
        for refs in run_records.refs_by_branch.values() {
            for record_ref in refs {
                paths.insert(record_ref.record_path.clone());
            }
        }
    }

    paths
        .iter()
        .map(|path| {
            let profiled = read_compressed_record_profiled(path)?;
            Ok(CompressedRunRecordRead {
                path: path.display().to_string(),
                compressed_bytes: profiled.profile.compressed_bytes,
                decompressed_bytes: profiled.profile.decompressed_bytes,
                turn_count: profiled.record.turn_count(),
                tool_call_count: profiled.record.tool_call_count(),
                failed_tool_call_count: profiled.record.failed_tool_call_count(),
                open_read_ns: profiled.profile.open_read_ns,
                gzip_decompress_ns: profiled.profile.decompress_ns,
                typed_json_deserialize_ns: profiled.profile.deserialize_ns,
            })
        })
        .collect()
}

fn top_frames(frames: &[FrameSample]) -> Vec<TopFrame> {
    let mut frames = frames
        .iter()
        .map(|frame| TopFrame {
            frame_index: frame.frame_index,
            duration_ns: frame.duration_ns,
            components: frame.components.clone(),
            allocations: frame.allocations,
        })
        .collect::<Vec<_>>();
    frames.sort_by(|left, right| {
        right
            .duration_ns
            .cmp(&left.duration_ns)
            .then_with(|| left.frame_index.cmp(&right.frame_index))
    });
    frames.truncate(TOP_FRAME_LIMIT);
    frames
}

fn component_reports(
    frames: &[FrameSample],
    median_frame_ns: Option<u64>,
) -> Vec<ComponentTimingReport> {
    let mut durations: BTreeMap<String, Vec<u64>> = BTreeMap::new();
    for frame in frames {
        for component in &frame.components {
            durations
                .entry(component.component.clone())
                .or_default()
                .push(component.duration_ns);
        }
    }
    durations
        .into_iter()
        .map(|(component, durations)| {
            let stats = DurationStats::from_durations(&durations);
            let median_percent_of_frame_x100 =
                stats
                    .median_ns
                    .zip(median_frame_ns)
                    .and_then(|(component_ns, frame_ns)| {
                        (frame_ns > 0).then_some(component_ns.saturating_mul(10_000) / frame_ns)
                    });
            let nested_under = component_parent(component.as_str()).map(str::to_owned);
            ComponentTimingReport {
                component,
                stats,
                median_percent_of_frame_x100,
                nested_under,
            }
        })
        .collect()
}

fn component_parent(component: &str) -> Option<&'static str> {
    match component {
        "diagnostics" | "graph_catalog" => Some("run_navigation"),
        "inspector_tool_decode" => Some("central_graph"),
        _ => None,
    }
}

pub fn parity_baseline_report_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join(PARITY_BASELINE_DIR)
        .join("report.json")
}

pub fn benchmark_graph_snapshot_fixture_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(BENCHMARK_GRAPH_SNAPSHOT_FIXTURE)
}

pub fn load_benchmark_report(path: &Path) -> Result<BenchmarkReport, Box<dyn Error>> {
    let bytes = fs::read(path)?;
    Ok(serde_json::from_slice(&bytes)?)
}

#[derive(Debug)]
pub struct BenchmarkRegressionFailure {
    pub scenario: String,
    pub field: String,
    pub baseline: String,
    pub actual: String,
}

impl std::fmt::Display for BenchmarkRegressionFailure {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            formatter,
            "scenario={} field={} baseline={} actual={}",
            self.scenario, self.field, self.baseline, self.actual
        )
    }
}

pub fn compare_benchmark_reports(
    baseline: &BenchmarkReport,
    actual: &BenchmarkReport,
) -> Result<(), Vec<BenchmarkRegressionFailure>> {
    let mut failures = Vec::new();
    for scenario_name in BenchmarkScenario::regression_gated()
        .iter()
        .map(|scenario| scenario.name())
    {
        let Some(baseline_scenario) = baseline
            .scenarios
            .iter()
            .find(|report| report.name == scenario_name)
        else {
            failures.push(BenchmarkRegressionFailure {
                scenario: scenario_name.clone(),
                field: "scenario.missing_in_baseline".to_owned(),
                baseline: "present".to_owned(),
                actual: "missing".to_owned(),
            });
            continue;
        };
        let Some(actual_scenario) = actual
            .scenarios
            .iter()
            .find(|report| report.name == scenario_name)
        else {
            failures.push(BenchmarkRegressionFailure {
                scenario: scenario_name.clone(),
                field: "scenario.missing_in_actual".to_owned(),
                baseline: "present".to_owned(),
                actual: "missing".to_owned(),
            });
            continue;
        };
        compare_scenario_reports(baseline_scenario, actual_scenario, &mut failures);
    }
    if failures.is_empty() {
        Ok(())
    } else {
        Err(failures)
    }
}

fn compare_scenario_reports(
    baseline: &ScenarioReport,
    actual: &ScenarioReport,
    failures: &mut Vec<BenchmarkRegressionFailure>,
) {
    let scenario = baseline.name.clone();
    compare_duration_stat(
        failures,
        &scenario,
        "frame_stats.median_ns",
        baseline.frame_stats.median_ns,
        actual.frame_stats.median_ns,
    );
    compare_duration_stat(
        failures,
        &scenario,
        "frame_stats.p95_ns",
        baseline.frame_stats.p95_ns,
        actual.frame_stats.p95_ns,
    );
    compare_value_stat(
        failures,
        &scenario,
        "allocation_frames.per_frame.allocation_count.median",
        baseline.allocation_frames.per_frame.allocation_count.median,
        actual.allocation_frames.per_frame.allocation_count.median,
    );
    compare_value_stat(
        failures,
        &scenario,
        "allocation_frames.per_frame.allocation_count.p95",
        baseline.allocation_frames.per_frame.allocation_count.p95,
        actual.allocation_frames.per_frame.allocation_count.p95,
    );
    compare_value_stat(
        failures,
        &scenario,
        "allocation_frames.per_frame.allocated_object_bytes.median",
        baseline
            .allocation_frames
            .per_frame
            .allocated_object_bytes
            .median,
        actual
            .allocation_frames
            .per_frame
            .allocated_object_bytes
            .median,
    );
    compare_value_stat(
        failures,
        &scenario,
        "allocation_frames.per_frame.allocated_object_bytes.p95",
        baseline
            .allocation_frames
            .per_frame
            .allocated_object_bytes
            .p95,
        actual
            .allocation_frames
            .per_frame
            .allocated_object_bytes
            .p95,
    );
    compare_value_stat(
        failures,
        &scenario,
        "allocation_frames.per_frame.allocated_wrapped_bytes.median",
        baseline
            .allocation_frames
            .per_frame
            .allocated_wrapped_bytes
            .median,
        actual
            .allocation_frames
            .per_frame
            .allocated_wrapped_bytes
            .median,
    );
    compare_value_stat(
        failures,
        &scenario,
        "allocation_frames.per_frame.allocated_wrapped_bytes.p95",
        baseline
            .allocation_frames
            .per_frame
            .allocated_wrapped_bytes
            .p95,
        actual
            .allocation_frames
            .per_frame
            .allocated_wrapped_bytes
            .p95,
    );
    compare_heap_slope(failures, &scenario, baseline, actual);
    for baseline_component in &baseline.component_timings {
        let Some(actual_component) = actual
            .component_timings
            .iter()
            .find(|component| component.component == baseline_component.component)
        else {
            failures.push(BenchmarkRegressionFailure {
                scenario: scenario.clone(),
                field: format!("component_timings.{}.missing", baseline_component.component),
                baseline: "present".to_owned(),
                actual: "missing".to_owned(),
            });
            continue;
        };
        compare_duration_stat(
            failures,
            &scenario,
            &format!(
                "component_timings.{}.median_ns",
                baseline_component.component
            ),
            baseline_component.stats.median_ns,
            actual_component.stats.median_ns,
        );
        compare_duration_stat(
            failures,
            &scenario,
            &format!("component_timings.{}.p95_ns", baseline_component.component),
            baseline_component.stats.p95_ns,
            actual_component.stats.p95_ns,
        );
        if let (Some(baseline_share), Some(actual_share)) = (
            baseline_component.median_percent_of_frame_x100,
            actual_component.median_percent_of_frame_x100,
        ) {
            if actual_share > baseline_share {
                failures.push(BenchmarkRegressionFailure {
                    scenario: scenario.clone(),
                    field: format!(
                        "component_timings.{}.median_percent_of_frame_x100",
                        baseline_component.component
                    ),
                    baseline: baseline_share.to_string(),
                    actual: actual_share.to_string(),
                });
            }
        }
    }
}

fn compare_duration_stat(
    failures: &mut Vec<BenchmarkRegressionFailure>,
    scenario: &str,
    field: &str,
    baseline: Option<u64>,
    actual: Option<u64>,
) {
    if actual > baseline {
        failures.push(BenchmarkRegressionFailure {
            scenario: scenario.to_owned(),
            field: field.to_owned(),
            baseline: format_optional_u64(baseline),
            actual: format_optional_u64(actual),
        });
    }
}

fn compare_value_stat(
    failures: &mut Vec<BenchmarkRegressionFailure>,
    scenario: &str,
    field: &str,
    baseline: Option<u64>,
    actual: Option<u64>,
) {
    if actual > baseline {
        failures.push(BenchmarkRegressionFailure {
            scenario: scenario.to_owned(),
            field: field.to_owned(),
            baseline: format_optional_u64(baseline),
            actual: format_optional_u64(actual),
        });
    }
}

fn compare_heap_slope(
    failures: &mut Vec<BenchmarkRegressionFailure>,
    scenario: &str,
    baseline: &ScenarioReport,
    actual: &ScenarioReport,
) {
    if heap_slope_rank(actual.allocation_frames.slope)
        > heap_slope_rank(baseline.allocation_frames.slope)
    {
        failures.push(BenchmarkRegressionFailure {
            scenario: scenario.to_owned(),
            field: "allocation_frames.slope".to_owned(),
            baseline: baseline.allocation_frames.slope.as_str().to_owned(),
            actual: actual.allocation_frames.slope.as_str().to_owned(),
        });
        return;
    }
    if baseline.allocation_frames.slope == HeapSlope::Growing
        && actual.allocation_frames.slope == HeapSlope::Growing
    {
        compare_value_stat(
            failures,
            scenario,
            "allocation_frames.last_50.allocated_wrapped_bytes.median",
            baseline
                .allocation_frames
                .last_50
                .stats
                .allocated_wrapped_bytes
                .median,
            actual
                .allocation_frames
                .last_50
                .stats
                .allocated_wrapped_bytes
                .median,
        );
        compare_value_stat(
            failures,
            scenario,
            "allocation_frames.last_50.allocated_wrapped_bytes.p95",
            baseline
                .allocation_frames
                .last_50
                .stats
                .allocated_wrapped_bytes
                .p95,
            actual
                .allocation_frames
                .last_50
                .stats
                .allocated_wrapped_bytes
                .p95,
        );
    }
}

fn heap_slope_rank(slope: HeapSlope) -> u8 {
    match slope {
        HeapSlope::Plateau => 0,
        HeapSlope::Inconclusive => 1,
        HeapSlope::Growing => 2,
    }
}

fn format_optional_u64(value: Option<u64>) -> String {
    value
        .map(|value| value.to_string())
        .unwrap_or_else(|| "None".to_owned())
}

fn allocation_frame_report(frames: &[FrameSample]) -> AllocationFrameReport {
    let first_50 = frame_window(frames, 0, 50);
    let last_start = frames.len().saturating_sub(50);
    let last_50 = frame_window(frames, last_start, frames.len());
    AllocationFrameReport {
        per_frame: allocation_frame_stats(frames),
        first_50: first_50.clone(),
        last_50: last_50.clone(),
        slope: classify_heap_slope(&first_50, &last_50),
    }
}

fn phase_windows_with_heap_profiles(
    mut windows: Vec<ScenarioPhaseWindow>,
    heap_snapshots: &[HeapProfileSnapshot],
) -> Vec<ScenarioPhaseWindow> {
    for window in &mut windows {
        let start_index = window.frame_start.saturating_sub(1);
        let end_index = window.frame_end;
        if let (Some(start), Some(end)) = (
            heap_snapshots.get(start_index),
            heap_snapshots.get(end_index),
        ) {
            window.heap_profile_delta = heap_summary(&end.delta_since(start));
        }
    }
    windows
}

fn frame_window(frames: &[FrameSample], start: usize, end: usize) -> AllocationFrameWindow {
    let end = end.min(frames.len());
    let start = start.min(end);
    AllocationFrameWindow {
        frame_start: start.saturating_add(1),
        frame_end: end,
        stats: allocation_frame_stats(&frames[start..end]),
    }
}

fn allocation_frame_stats(frames: &[FrameSample]) -> AllocationFrameStats {
    AllocationFrameStats {
        allocation_count: ValueStats::from_values(
            &frames
                .iter()
                .map(|frame| frame.allocations.allocation_count_delta)
                .collect::<Vec<_>>(),
        ),
        allocated_object_bytes: ValueStats::from_values(
            &frames
                .iter()
                .map(|frame| frame.allocations.allocated_object_bytes_delta)
                .collect::<Vec<_>>(),
        ),
        allocated_wrapped_bytes: ValueStats::from_values(
            &frames
                .iter()
                .map(|frame| frame.allocations.allocated_wrapped_bytes_delta)
                .collect::<Vec<_>>(),
        ),
        live_object_bytes: ValueStats::from_values(
            &frames
                .iter()
                .map(|frame| frame.allocations.live_object_bytes)
                .collect::<Vec<_>>(),
        ),
        live_wrapped_bytes: ValueStats::from_values(
            &frames
                .iter()
                .map(|frame| frame.allocations.live_wrapped_bytes)
                .collect::<Vec<_>>(),
        ),
    }
}

fn classify_heap_slope(first: &AllocationFrameWindow, last: &AllocationFrameWindow) -> HeapSlope {
    let Some(first_live) = first.stats.live_wrapped_bytes.median else {
        return HeapSlope::Inconclusive;
    };
    let Some(last_live) = last.stats.live_wrapped_bytes.median else {
        return HeapSlope::Inconclusive;
    };
    if first.stats.live_wrapped_bytes.count < 50 || last.stats.live_wrapped_bytes.count < 50 {
        return HeapSlope::Inconclusive;
    }
    let threshold = (first_live / 10).max(1_048_576);
    if last_live > first_live.saturating_add(threshold) {
        HeapSlope::Growing
    } else {
        HeapSlope::Plateau
    }
}

fn heap_summary(snapshot: &HeapProfileSnapshot) -> HeapScenarioProfile {
    HeapScenarioProfile {
        totals: snapshot.totals,
        unmatched_deallocations: snapshot.unmatched_deallocations,
        top_groups_by_allocated_bytes: snapshot.top_groups_by_allocated_bytes(TOP_HEAP_LIMIT),
        top_groups_by_allocation_count: snapshot.top_groups_by_allocation_count(TOP_HEAP_LIMIT),
        top_groups_by_retained_bytes: snapshot.top_groups_by_retained_bytes(TOP_HEAP_LIMIT),
        top_callsites_by_allocated_bytes: snapshot.top_callsites_by_allocated_bytes(TOP_HEAP_LIMIT),
        top_callsites_by_allocation_count: snapshot
            .top_callsites_by_allocation_count(TOP_HEAP_LIMIT),
        top_callsites_by_retained_bytes: snapshot.top_callsites_by_retained_bytes(TOP_HEAP_LIMIT),
    }
}

fn percentile(sorted: &[u64], percentile: usize) -> Option<u64> {
    if sorted.is_empty() {
        return None;
    }
    let index = ((sorted.len() * percentile).div_ceil(100)).saturating_sub(1);
    sorted.get(index.min(sorted.len() - 1)).copied()
}

fn render_benchmark_readme(report: &BenchmarkReport) -> String {
    let mut text = String::new();
    text.push_str("# ploke-egui Native Benchmark\n\n");
    text.push_str(&format!("suite: `{}`\n\n", report.suite));
    text.push_str(&format!("run_root: `{}`\n\n", report.run_root));
    if let Some(short) = &report.commit.short_sha {
        text.push_str(&format!("commit: `{short}`\n\n"));
    }
    text.push_str(&format!(
        "dirty_state: `{}`\n\n",
        report.dirty_state.classification.as_str()
    ));
    if !report.dirty_state.relevant_paths.is_empty() {
        text.push_str("benchmark-relevant dirty paths:\n");
        for path in &report.dirty_state.relevant_paths {
            text.push_str(&format!("- `{path}`\n"));
        }
        text.push('\n');
    }
    if !report.dirty_state.unrelated_paths.is_empty() {
        text.push_str("unrelated dirty paths:\n");
        for path in &report.dirty_state.unrelated_paths {
            text.push_str(&format!("- `{path}`\n"));
        }
        text.push('\n');
    }
    if let Some(sampling) = &report.callsite_sampling {
        text.push_str("## Callsite Sampling\n\n");
        text.push_str(&format!(
            "- sample every: `{}` matching allocations\n",
            sampling.sample_every
        ));
        text.push_str(&format!("- accounting: `{}`\n", sampling.accounting));
        text.push_str("- scopes:\n");
        for scope in &sampling.scopes {
            text.push_str(&format!("  - `{scope}`\n"));
        }
        text.push('\n');
    }
    if let Some(readiness) = &report.run_readiness {
        text.push_str("## Run Readiness\n\n");
        text.push_str(&format!("- ready: `{}`\n", readiness.ready));
        text.push_str(&format!("- reason: `{}`\n", readiness.reason));
        text.push_str(&format!(
            "- history blocks: `{}` / `{}` expected, max height `{:?}`\n",
            readiness.sealed_history_blocks,
            readiness.expected_min_history_blocks,
            readiness.max_sealed_block_height
        ));
        text.push_str(&format!(
            "- spawned children: `{}` / `{}` expected minimum\n",
            readiness.spawned_child_nodes, readiness.expected_min_spawned_children
        ));
        text.push('\n');
    }
    if !report.startup.spans.is_empty() {
        text.push_str("## Startup\n\n");
        for span in &report.startup.spans {
            text.push_str(&format!("- `{}`: {} ns\n", span.name, span.duration_ns));
        }
        if !report.startup.notes.is_empty() {
            for note in &report.startup.notes {
                text.push_str(&format!("- note: {note}\n"));
            }
        }
        text.push('\n');
    }
    text.push_str("## Scenarios\n\n");
    for scenario in &report.scenarios {
        text.push_str(&format!(
            "- `{}`: frames={}, median={} ns, p95={} ns, p99={} ns, max={} ns, heap_slope={}\n",
            scenario.name,
            scenario.frame_stats.count,
            render_optional_ns(scenario.frame_stats.median_ns),
            render_optional_ns(scenario.frame_stats.p95_ns),
            render_optional_ns(scenario.frame_stats.p99_ns),
            render_optional_ns(scenario.frame_stats.max_ns),
            scenario.allocation_frames.slope.as_str()
        ));
        for component in &scenario.component_timings {
            if let Some(percent) = component.median_percent_of_frame_x100 {
                let nested = component
                    .nested_under
                    .as_ref()
                    .map(|parent| format!(", nested_under={parent}"))
                    .unwrap_or_default();
                text.push_str(&format!(
                    "  - component `{}`: median={} ns, median_frame_share={}%{}\n",
                    component.component,
                    render_optional_ns(component.stats.median_ns),
                    render_percent_x100(percent),
                    nested
                ));
            }
        }
        for phase in &scenario.phase_windows {
            text.push_str(&format!(
                "  - phase {} `{}`: frames={}..{}, median_allocs={}, median_object_bytes={}, median_wrapped_bytes={}, median_live_object_bytes={}\n",
                phase.phase_index,
                phase.phase,
                phase.frame_start,
                phase.frame_end,
                render_optional_value(phase.stats.allocation_count.median),
                render_optional_value(phase.stats.allocated_object_bytes.median),
                render_optional_value(phase.stats.allocated_wrapped_bytes.median),
                render_optional_value(phase.stats.live_object_bytes.median),
            ));
        }
        if let Some(callsite) = scenario
            .heap_profile
            .top_callsites_by_allocated_bytes
            .first()
        {
            text.push_str(&format!(
                "  - top heap callsite by allocated bytes: `{}` ({} wrapped bytes)\n",
                callsite.callsite.symbol, callsite.totals.allocated_wrapped_bytes
            ));
        }
    }
    text.push_str("\n## Local Puffin Captures\n\n");
    if report.puffin_artifacts.is_empty() {
        text.push_str("- none recorded\n");
    } else {
        for artifact in &report.puffin_artifacts {
            text.push_str(&format!(
                "- `{}`: {} bytes, sha256 `{}`\n",
                artifact.path, artifact.bytes, artifact.sha256
            ));
        }
    }
    text.push_str("\n## Local Heap Profiles\n\n");
    if report.heap_artifacts.is_empty() {
        text.push_str("- none recorded\n");
    } else {
        for artifact in &report.heap_artifacts {
            text.push_str(&format!(
                "- `{}`: {} bytes, sha256 `{}`\n",
                artifact.path, artifact.bytes, artifact.sha256
            ));
        }
    }
    text.push_str("\nSee `report.json` for typed timings and compact allocation summaries.\n");
    text
}

fn render_optional_ns(value: Option<u64>) -> String {
    value
        .map(|value| value.to_string())
        .unwrap_or_else(|| "n/a".to_owned())
}

fn render_optional_value(value: Option<u64>) -> String {
    value
        .map(|value| value.to_string())
        .unwrap_or_else(|| "n/a".to_owned())
}

fn render_percent_x100(value: u64) -> String {
    format!("{}.{:02}", value / 100, value % 100)
}

fn unix_ms_now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis().try_into().unwrap_or(u64::MAX))
        .unwrap_or(0)
}

fn elapsed_ns(start: Instant) -> u64 {
    start.elapsed().as_nanos().try_into().unwrap_or(u64::MAX)
}

fn git_info() -> GitInfo {
    GitInfo {
        short_sha: git_output(["rev-parse", "--short=12", "HEAD"]),
        full_sha: git_output(["rev-parse", "HEAD"]),
    }
}

fn dirty_state() -> DirtyState {
    let Some(status) = git_output(["status", "--short"]) else {
        return DirtyState {
            dirty: None,
            classification: DirtyStateClassification::Unknown,
            relevant_dirty: None,
            paths: Vec::new(),
            relevant_paths: Vec::new(),
            unrelated_paths: Vec::new(),
            scope: "git status --short at benchmark start".to_owned(),
        };
    };
    let paths = git_status_paths(&status);
    let relevant_paths: Vec<_> = paths
        .iter()
        .filter(|path| is_benchmark_relevant_dirty_path(path))
        .cloned()
        .collect();
    let unrelated_paths: Vec<_> = paths
        .iter()
        .filter(|path| !is_benchmark_relevant_dirty_path(path))
        .cloned()
        .collect();
    let dirty = !paths.is_empty();
    let relevant_dirty = !relevant_paths.is_empty();
    let classification = match (dirty, relevant_dirty) {
        (false, _) => DirtyStateClassification::Clean,
        (true, true) => DirtyStateClassification::DirtyRelevant,
        (true, false) => DirtyStateClassification::DirtyUnrelated,
    };

    DirtyState {
        dirty: Some(dirty),
        classification,
        relevant_dirty: Some(relevant_dirty),
        paths,
        relevant_paths,
        unrelated_paths,
        scope: "git status --short at benchmark start; benchmark-relevant paths are Cargo manifests, ploke-egui inputs except generated benchmark reports, ploke-tree, and ploke-records".to_owned(),
    }
}

impl DirtyStateClassification {
    fn as_str(self) -> &'static str {
        match self {
            Self::Clean => "clean",
            Self::DirtyRelevant => "dirty_relevant",
            Self::DirtyUnrelated => "dirty_unrelated",
            Self::Unknown => "unknown",
        }
    }
}

fn git_status_paths(status: &str) -> Vec<String> {
    status
        .lines()
        .filter_map(git_status_path)
        .map(str::to_owned)
        .collect()
}

fn git_status_path(line: &str) -> Option<&str> {
    let path = if matches!(line.as_bytes().get(2), Some(b' ')) {
        line.get(3..)?
    } else {
        line.split_once(' ').map(|(_, path)| path.trim())?
    };
    let path = path.split(" -> ").last().unwrap_or(path).trim();
    (!path.is_empty()).then_some(path)
}

fn is_benchmark_relevant_dirty_path(path: &str) -> bool {
    if path.starts_with("crates/ploke-egui/docs/profiling/benchmarks/") {
        return false;
    }
    path == "Cargo.lock"
        || path == "Cargo.toml"
        || path.starts_with("crates/ploke-egui/")
        || path.starts_with("crates/ploke-tree/")
        || path.starts_with("crates/ploke-records/")
}

fn git_output<const N: usize>(args: [&str; N]) -> Option<String> {
    let output = Command::new("git")
        .args(args)
        .current_dir(workspace_root())
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    Some(String::from_utf8_lossy(&output.stdout).trim().to_owned())
}

fn feature_set() -> Vec<String> {
    let mut features = Vec::new();
    if cfg!(feature = "dev") {
        features.push("dev".to_owned());
    }
    if cfg!(feature = "native-benchmark") {
        features.push("native-benchmark".to_owned());
    }
    if cfg!(feature = "profile-with-puffin") {
        features.push("profile-with-puffin".to_owned());
    }
    features
}

#[cfg(feature = "native-benchmark")]
fn default_puffin_benchmark_dir(output_dir: &Path) -> PathBuf {
    let name = output_dir
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_else(|| "custom-output".to_owned());
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("data")
        .join("profiling")
        .join("puffin")
        .join("benchmarks")
        .join(name)
}

fn default_heap_benchmark_dir(output_dir: &Path) -> PathBuf {
    let name = output_dir
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_else(|| "custom-output".to_owned());
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("data")
        .join("profiling")
        .join("heap")
        .join("benchmarks")
        .join(name)
}

fn file_artifact(path: &Path, tracked: bool) -> io::Result<BenchmarkArtifact> {
    let metadata = fs::metadata(path)?;
    Ok(BenchmarkArtifact {
        path: path.display().to_string(),
        bytes: metadata.len(),
        sha256: sha256_file(path)?,
        tracked,
    })
}

fn sha256_file(path: &Path) -> io::Result<String> {
    let mut file = fs::File::open(path)?;
    let mut hasher = Sha256::new();
    let mut buffer = [0_u8; 64 * 1024];
    loop {
        let read = file.read(&mut buffer)?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }
    Ok(format!("{:x}", hasher.finalize()))
}

fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .map(Path::to_path_buf)
        .unwrap_or_else(|| PathBuf::from(env!("CARGO_MANIFEST_DIR")))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn benchmark_report_json_roundtrips() {
        let report = BenchmarkReport {
            schema_version: BENCHMARK_REPORT_VERSION.to_owned(),
            suite: "standard".to_owned(),
            created_at_unix_ms: 1,
            commit: GitInfo {
                short_sha: Some("abc123".to_owned()),
                full_sha: Some("abc123def".to_owned()),
            },
            dirty_state: DirtyState {
                dirty: Some(false),
                classification: DirtyStateClassification::Clean,
                relevant_dirty: Some(false),
                paths: Vec::new(),
                relevant_paths: Vec::new(),
                unrelated_paths: Vec::new(),
                scope: "fixture".to_owned(),
            },
            command: "ploke-egui --benchmark-suite standard".to_owned(),
            run_root: STANDARD_RUN_ROOT.to_owned(),
            feature_set: vec!["dev".to_owned(), "native-benchmark".to_owned()],
            scenarios_requested: vec!["startup_frames_300".to_owned()],
            callsite_sampling: Some(BenchmarkCallsiteSampling::focused(32).expect("sampling")),
            run_readiness: Some(BenchmarkRunReadiness::from_counts(
                5,
                1,
                6,
                Some(5),
                1,
                19,
                18,
            )),
            startup: StartupProfile {
                spans: vec![BenchmarkSpan {
                    name: "Graph::from_records".to_owned(),
                    duration_ns: 42,
                }],
                compressed_run_records: Vec::new(),
                notes: Vec::new(),
            },
            scenarios: vec![ScenarioReport {
                name: "startup_frames_300".to_owned(),
                target_frames: 300,
                frame_stats: DurationStats::from_durations(&[1, 2, 3]),
                top_frames: vec![TopFrame {
                    frame_index: 3,
                    duration_ns: 3,
                    components: vec![ComponentTiming {
                        component: "central_graph".to_owned(),
                        duration_ns: 2,
                    }],
                    allocations: FrameAllocationSample::default(),
                }],
                component_timings: vec![ComponentTimingReport {
                    component: "central_graph".to_owned(),
                    stats: DurationStats::from_durations(&[2]),
                    median_percent_of_frame_x100: Some(6_666),
                    nested_under: None,
                }],
                allocation_frames: AllocationFrameReport::default(),
                phase_windows: Vec::new(),
                heap_profile: HeapScenarioProfile::default(),
                allocations: AllocationSnapshot {
                    enabled: true,
                    allocation_count: 10,
                    deallocation_count: 4,
                    allocated_bytes: 100,
                    deallocated_bytes: 40,
                }
                .delta_since(AllocationSnapshot {
                    enabled: true,
                    allocation_count: 1,
                    deallocation_count: 1,
                    allocated_bytes: 10,
                    deallocated_bytes: 10,
                }),
                action: BenchmarkActionReport::for_action(BenchmarkAction::None),
            }],
            puffin_artifacts: vec![BenchmarkArtifact {
                path: "crates/ploke-egui/data/profiling/puffin/benchmarks/x/standard.puffin"
                    .to_owned(),
                bytes: 12,
                sha256: "00".to_owned(),
                tracked: false,
            }],
            heap_artifacts: vec![BenchmarkArtifact {
                path: "crates/ploke-egui/data/profiling/heap/benchmarks/x/startup_frames_300.heap.json"
                    .to_owned(),
                bytes: 12,
                sha256: "00".to_owned(),
                tracked: false,
            }],
            notes: vec!["fixture".to_owned()],
        };

        let encoded = serde_json::to_string_pretty(&report).expect("serialize report");
        let decoded: BenchmarkReport = serde_json::from_str(&encoded).expect("deserialize report");
        assert_eq!(decoded, report);
    }

    #[test]
    fn benchmark_duration_stats_percentiles_and_top_frames() {
        let stats = DurationStats::from_durations(&[10, 20, 30, 40, 50]);
        assert_eq!(stats.min_ns, Some(10));
        assert_eq!(stats.median_ns, Some(30));
        assert_eq!(stats.p95_ns, Some(50));
        assert_eq!(stats.p99_ns, Some(50));
        assert_eq!(stats.max_ns, Some(50));

        let frames = vec![
            FrameSample {
                frame_index: 1,
                duration_ns: 10,
                components: Vec::new(),
                allocations: FrameAllocationSample::default(),
            },
            FrameSample {
                frame_index: 2,
                duration_ns: 50,
                components: Vec::new(),
                allocations: FrameAllocationSample::default(),
            },
            FrameSample {
                frame_index: 3,
                duration_ns: 30,
                components: Vec::new(),
                allocations: FrameAllocationSample::default(),
            },
        ];
        let top = top_frames(&frames);
        assert_eq!(top[0].frame_index, 2);
        assert_eq!(top[1].frame_index, 3);
        assert_eq!(top[2].frame_index, 1);
    }

    #[test]
    fn benchmark_heap_slope_classifies_plateau_growing_and_inconclusive() {
        let plateau = allocation_windows_for_slope(1_000_000, 1_010_000, 50);
        assert_eq!(
            classify_heap_slope(&plateau.0, &plateau.1),
            HeapSlope::Plateau
        );

        let growing = allocation_windows_for_slope(1_000_000, 3_000_000, 50);
        assert_eq!(
            classify_heap_slope(&growing.0, &growing.1),
            HeapSlope::Growing
        );

        let inconclusive = allocation_windows_for_slope(1_000_000, 3_000_000, 20);
        assert_eq!(
            classify_heap_slope(&inconclusive.0, &inconclusive.1),
            HeapSlope::Inconclusive
        );
    }

    #[test]
    fn inspector_section_sequence_uses_short_staged_windows() {
        let scenario = BenchmarkScenario::InspectorSectionsSequence30;
        assert_eq!(scenario.name(), "inspector_sections_sequence_30");
        assert_eq!(
            scenario.frame_target(),
            INSPECTOR_SEQUENCE_WARMUP_FRAMES
                + INSPECTOR_SEQUENCE_SELECT_SETTLE_FRAMES
                + BenchmarkInspectorSection::sequence().len() * INSPECTOR_SEQUENCE_SECTION_FRAMES
        );
        assert_eq!(
            scenario.action_for_completed_frames(0),
            Some(BenchmarkAction::InspectorSequence {
                stage: InspectorSequenceStage::Warmup
            })
        );
        assert_eq!(
            scenario.action_for_completed_frames(INSPECTOR_SEQUENCE_WARMUP_FRAMES),
            Some(BenchmarkAction::InspectorSequence {
                stage: InspectorSequenceStage::SelectArtifact
            })
        );
        assert_eq!(
            scenario.action_for_completed_frames(
                INSPECTOR_SEQUENCE_WARMUP_FRAMES + INSPECTOR_SEQUENCE_SELECT_SETTLE_FRAMES
            ),
            Some(BenchmarkAction::InspectorSequence {
                stage: InspectorSequenceStage::OpenSection(BenchmarkInspectorSection::LlmCalls)
            })
        );
        assert_eq!(
            scenario.action_for_completed_frames(
                INSPECTOR_SEQUENCE_WARMUP_FRAMES
                    + INSPECTOR_SEQUENCE_SELECT_SETTLE_FRAMES
                    + INSPECTOR_SEQUENCE_SECTION_FRAMES
            ),
            Some(BenchmarkAction::InspectorSequence {
                stage: InspectorSequenceStage::OpenSection(BenchmarkInspectorSection::RunRecords)
            })
        );
        assert_eq!(scenario.action_for_completed_frames(101), None);
    }

    #[test]
    fn inspector_section_phase_sequence_tracks_nine_thirty_frame_windows() {
        let scenario = BenchmarkScenario::InspectorSectionPhaseSequence30 {
            section: BenchmarkInspectorSection::PatchDebug,
            target: BenchmarkSelectionTarget::Alternate,
        };
        assert_eq!(
            scenario.name(),
            "inspector_patch_debug_phase_sequence_alternate_30"
        );
        assert_eq!(
            BenchmarkScenario::parse("inspector_patch_debug_phase_sequence_alternate_30"),
            Ok(scenario)
        );
        assert_eq!(
            BenchmarkScenario::parse("inspector_llm_calls_phase_sequence_30"),
            Ok(BenchmarkScenario::InspectorSectionPhaseSequence30 {
                section: BenchmarkInspectorSection::LlmCalls,
                target: BenchmarkSelectionTarget::Primary,
            })
        );
        assert_eq!(
            scenario.frame_target(),
            InspectorSectionPhase::sequence().len() * INSPECTOR_SECTION_PHASE_FRAMES
        );
        assert_eq!(
            scenario.action_for_completed_frames(0),
            Some(BenchmarkAction::InspectorSectionPhase {
                section: BenchmarkInspectorSection::PatchDebug,
                target: BenchmarkSelectionTarget::Alternate,
                phase: InspectorSectionPhase::IdleBeforeSelection
            })
        );
        assert_eq!(
            scenario.action_for_completed_frames(INSPECTOR_SECTION_PHASE_FRAMES),
            Some(BenchmarkAction::InspectorSectionPhase {
                section: BenchmarkInspectorSection::PatchDebug,
                target: BenchmarkSelectionTarget::Alternate,
                phase: InspectorSectionPhase::SelectNode
            })
        );
        assert_eq!(
            scenario.action_for_completed_frames(INSPECTOR_SECTION_PHASE_FRAMES * 2),
            None
        );
        assert_eq!(
            scenario.action_for_completed_frames(INSPECTOR_SECTION_PHASE_FRAMES * 3),
            Some(BenchmarkAction::InspectorSectionPhase {
                section: BenchmarkInspectorSection::PatchDebug,
                target: BenchmarkSelectionTarget::Alternate,
                phase: InspectorSectionPhase::ExpandSection
            })
        );
        assert_eq!(
            scenario.action_for_completed_frames(INSPECTOR_SECTION_PHASE_FRAMES * 5),
            Some(BenchmarkAction::InspectorSectionPhase {
                section: BenchmarkInspectorSection::PatchDebug,
                target: BenchmarkSelectionTarget::Alternate,
                phase: InspectorSectionPhase::CollapseSection
            })
        );
        assert_eq!(
            scenario.action_for_completed_frames(INSPECTOR_SECTION_PHASE_FRAMES * 7),
            Some(BenchmarkAction::InspectorSectionPhase {
                section: BenchmarkInspectorSection::PatchDebug,
                target: BenchmarkSelectionTarget::Alternate,
                phase: InspectorSectionPhase::UnselectNode
            })
        );
        assert_eq!(
            scenario.action_for_completed_frames(INSPECTOR_SECTION_PHASE_FRAMES * 9),
            None
        );

        let frames = frame_samples_with_live_bytes(42, scenario.frame_target());
        let windows = scenario.phase_windows(&frames);
        assert_eq!(windows.len(), 9);
        assert_eq!(windows[0].phase_index, 1);
        assert_eq!(windows[0].phase, "idle_before_selection");
        assert_eq!(windows[0].frame_start, 1);
        assert_eq!(windows[0].frame_end, 30);
        assert_eq!(windows[8].phase_index, 9);
        assert_eq!(windows[8].phase, "idle_after_unselect");
        assert_eq!(windows[8].frame_start, 241);
        assert_eq!(windows[8].frame_end, 270);
        assert_eq!(
            windows[8].selection_target.as_deref(),
            Some(BenchmarkSelectionTarget::Alternate.as_str())
        );
    }

    #[test]
    fn inspector_section_phase_windows_include_heap_profile_delta() {
        let scenario = BenchmarkScenario::InspectorSectionPhaseSequence30 {
            section: BenchmarkInspectorSection::RunRecords,
            target: BenchmarkSelectionTarget::Primary,
        };
        let frames = frame_samples_with_live_bytes(42, scenario.frame_target());
        let mut heap_snapshots = vec![HeapProfileSnapshot::default(); scenario.frame_target() + 1];
        heap_snapshots[0] = heap_profile_snapshot_with_groups(&[]);
        heap_snapshots[30] = heap_profile_snapshot_with_groups(&[("root", 7, 700)]);
        heap_snapshots[60] =
            heap_profile_snapshot_with_groups(&[("root", 7, 700), ("selection_inspector", 4, 200)]);

        let windows =
            phase_windows_with_heap_profiles(scenario.phase_windows(&frames), &heap_snapshots);

        assert_eq!(windows[0].heap_profile_delta.totals.allocation_count, 7);
        assert_eq!(
            windows[0].heap_profile_delta.top_groups_by_allocation_count[0]
                .name
                .as_deref(),
            Some("root")
        );
        assert_eq!(windows[1].heap_profile_delta.totals.allocation_count, 4);
        assert_eq!(
            windows[1].heap_profile_delta.top_groups_by_allocated_bytes[0]
                .name
                .as_deref(),
            Some("selection_inspector")
        );
    }

    #[test]
    fn inspector_section_phase_sequence_scenarios_cover_collapsible_sections() {
        for section in BenchmarkInspectorSection::sequence() {
            for target in [
                BenchmarkSelectionTarget::Primary,
                BenchmarkSelectionTarget::Alternate,
            ] {
                let scenario = BenchmarkScenario::InspectorSectionPhaseSequence30 {
                    section: *section,
                    target,
                };
                assert_eq!(BenchmarkScenario::parse(&scenario.name()), Ok(scenario));
                assert_eq!(
                    scenario.frame_target(),
                    InspectorSectionPhase::sequence().len() * INSPECTOR_SECTION_PHASE_FRAMES
                );
            }
        }
    }

    #[test]
    fn benchmark_run_readiness_accepts_history_or_spawned_child_evidence() {
        let history_ready = BenchmarkRunReadiness::from_counts(5, 1, 6, Some(5), 1, 1, 0);
        assert!(history_ready.ready);
        assert_eq!(
            history_ready.reason,
            "sealed_history_reached_policy_generation"
        );

        let child_ready = BenchmarkRunReadiness::from_counts(5, 1, 1, Some(0), 1, 6, 5);
        assert!(child_ready.ready);
        assert_eq!(
            child_ready.reason,
            "spawned_child_nodes_reached_policy_minimum"
        );

        let not_ready = BenchmarkRunReadiness::from_counts(5, 1, 1, Some(0), 1, 3, 2);
        assert!(!not_ready.ready);
        assert_eq!(
            not_ready.reason,
            "persisted_records_below_policy_expectation"
        );
    }

    #[test]
    fn benchmark_standard_run_readiness_matches_persisted_records() {
        let run_root = Path::new(STANDARD_RUN_ROOT);
        if !run_root.join("scheduler.json").is_file() {
            return;
        }

        let readiness = benchmark_run_readiness(run_root).expect("standard run readiness");
        assert!(readiness.ready, "{readiness:?}");
        assert_eq!(readiness.policy_max_generations, 5);
        assert_eq!(readiness.sealed_history_blocks, 6);
        assert_eq!(readiness.max_sealed_block_height, Some(5));
        assert!(readiness.spawned_child_nodes >= readiness.expected_min_spawned_children);
    }

    fn allocation_windows_for_slope(
        first_live: u64,
        last_live: u64,
        count: usize,
    ) -> (AllocationFrameWindow, AllocationFrameWindow) {
        let first_frames = frame_samples_with_live_bytes(first_live, count);
        let last_frames = frame_samples_with_live_bytes(last_live, count);
        (
            frame_window(&first_frames, 0, first_frames.len()),
            frame_window(&last_frames, 0, last_frames.len()),
        )
    }

    fn frame_samples_with_live_bytes(live_wrapped_bytes: u64, count: usize) -> Vec<FrameSample> {
        (0..count)
            .map(|index| FrameSample {
                frame_index: index + 1,
                duration_ns: 1,
                components: Vec::new(),
                allocations: FrameAllocationSample {
                    live_wrapped_bytes,
                    live_object_bytes: live_wrapped_bytes,
                    ..FrameAllocationSample::default()
                },
            })
            .collect()
    }

    fn heap_profile_snapshot_with_groups(groups: &[(&str, u64, u64)]) -> HeapProfileSnapshot {
        let mut totals = HeapProfileTotals::default();
        let groups = groups
            .iter()
            .enumerate()
            .map(
                |(index, (name, allocation_count, allocated_object_bytes))| {
                    let group_totals = HeapProfileTotals {
                        allocation_count: *allocation_count,
                        allocated_object_bytes: *allocated_object_bytes,
                        allocated_wrapped_bytes: *allocated_object_bytes,
                        live_object_bytes: *allocated_object_bytes,
                        live_wrapped_bytes: *allocated_object_bytes,
                        ..HeapProfileTotals::default()
                    };
                    totals.allocation_count += group_totals.allocation_count;
                    totals.allocated_object_bytes += group_totals.allocated_object_bytes;
                    totals.allocated_wrapped_bytes += group_totals.allocated_wrapped_bytes;
                    totals.live_object_bytes += group_totals.live_object_bytes;
                    totals.live_wrapped_bytes += group_totals.live_wrapped_bytes;
                    HeapGroupProfile {
                        group_id: index + 1,
                        name: Some((*name).to_owned()),
                        totals: group_totals,
                    }
                },
            )
            .collect::<Vec<_>>();
        HeapProfileSnapshot {
            enabled: true,
            totals,
            groups,
            callsites: Vec::new(),
            unmatched_deallocations: 0,
        }
    }

    #[test]
    fn dirty_state_path_classification_keeps_docs_unrelated() {
        let paths = git_status_paths(
            "M docs/workflow/evalnomicon/drafts/eval/report.md\n?? crates/ploke-egui/tmp.txt\n?? crates/ploke-egui/docs/profiling/benchmarks/20260517-abc-standard/README.md\n",
        );

        assert_eq!(
            paths,
            vec![
                "docs/workflow/evalnomicon/drafts/eval/report.md",
                "crates/ploke-egui/tmp.txt",
                "crates/ploke-egui/docs/profiling/benchmarks/20260517-abc-standard/README.md",
            ]
        );
        assert!(!is_benchmark_relevant_dirty_path(&paths[0]));
        assert!(is_benchmark_relevant_dirty_path(&paths[1]));
        assert!(!is_benchmark_relevant_dirty_path(&paths[2]));
    }

    #[test]
    fn compare_benchmark_reports_rejects_frame_time_regression() {
        let baseline = ScenarioReport {
            name: "warm_idle_300".to_owned(),
            target_frames: 300,
            frame_stats: DurationStats {
                count: 300,
                median_ns: Some(1_000_000),
                p95_ns: Some(1_100_000),
                ..DurationStats::default()
            },
            top_frames: Vec::new(),
            component_timings: Vec::new(),
            allocation_frames: AllocationFrameReport::default(),
            phase_windows: Vec::new(),
            heap_profile: HeapScenarioProfile::default(),
            allocations: AllocationDelta::default(),
            action: BenchmarkActionReport::for_action(BenchmarkAction::None),
        };
        let mut actual = baseline.clone();
        actual.frame_stats.median_ns = Some(1_000_001);
        let baseline_report = BenchmarkReport {
            schema_version: BENCHMARK_REPORT_VERSION.to_owned(),
            suite: "standard".to_owned(),
            created_at_unix_ms: 0,
            commit: GitInfo {
                short_sha: None,
                full_sha: None,
            },
            dirty_state: DirtyState {
                dirty: Some(false),
                classification: DirtyStateClassification::Clean,
                relevant_dirty: Some(false),
                paths: Vec::new(),
                relevant_paths: Vec::new(),
                unrelated_paths: Vec::new(),
                scope: "fixture".to_owned(),
            },
            command: "fixture".to_owned(),
            run_root: STANDARD_RUN_ROOT.to_owned(),
            feature_set: Vec::new(),
            scenarios_requested: vec!["warm_idle_300".to_owned()],
            callsite_sampling: None,
            run_readiness: None,
            startup: StartupProfile::default(),
            scenarios: vec![baseline],
            puffin_artifacts: Vec::new(),
            heap_artifacts: Vec::new(),
            notes: Vec::new(),
        };
        let mut actual_report = baseline_report.clone();
        actual_report.scenarios = vec![actual];
        let failures =
            compare_benchmark_reports(&baseline_report, &actual_report).expect_err("regression");
        assert!(failures.iter().any(|failure| {
            failure.field == "frame_stats.median_ns" && failure.scenario == "warm_idle_300"
        }));
    }

    #[test]
    fn benchmark_regression() {
        let baseline_path = parity_baseline_report_path();
        assert!(
            baseline_path.is_file(),
            "missing parity baseline at {} (run Phase 0 baseline capture first)",
            baseline_path.display()
        );
        let baseline = load_benchmark_report(&baseline_path).expect("load baseline");
        for scenario in BenchmarkScenario::regression_gated() {
            let name = scenario.name();
            assert!(
                baseline.scenarios.iter().any(|report| report.name == name),
                "baseline missing gated scenario {name}"
            );
        }
        assert!(
            benchmark_graph_snapshot_fixture_path().is_file(),
            "missing benchmark graph snapshot fixture at {}",
            benchmark_graph_snapshot_fixture_path().display()
        );
        compare_benchmark_reports(&baseline, &baseline).expect("compare harness sanity");
    }

    /// Full zero-tolerance compare vs committed baseline. Run only after refreshing baseline
    /// on the same machine/profile; back-to-back harness runs can differ slightly without code changes.
    #[test]
    #[ignore = "orchestrator gate: cargo test -p ploke-egui --features dev,native-benchmark benchmark_regression_against_baseline -- --ignored --nocapture"]
    fn benchmark_regression_against_baseline() {
        let run_root = Path::new(STANDARD_RUN_ROOT);
        if !run_root.join("scheduler.json").is_file() {
            eprintln!(
                "benchmark_regression_against_baseline skipped: STANDARD_RUN_ROOT missing at {}",
                run_root.display()
            );
            return;
        }
        let readiness = benchmark_run_readiness(run_root).expect("readiness probe");
        assert!(
            readiness.ready,
            "standard run root not ready: {}",
            readiness.summary()
        );

        let baseline_path = parity_baseline_report_path();
        let baseline = load_benchmark_report(&baseline_path).expect("load baseline");

        let output_dir = benchmark_test_output_dir().join("regression-compare");
        let _ = fs::remove_dir_all(&output_dir);
        let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        let workspace_root = manifest_dir
            .parent()
            .and_then(|path| path.parent())
            .expect("workspace root");
        let status = Command::new(env!("CARGO"))
            .current_dir(workspace_root)
            .args([
                "run",
                "-p",
                "ploke-egui",
                "--features",
                "dev,native-benchmark",
                "--",
                "--run-root",
                STANDARD_RUN_ROOT,
                "--benchmark-suite",
                "standard",
                "--benchmark-output",
            ])
            .arg(output_dir.to_string_lossy().as_ref())
            .status()
            .expect("spawn benchmark run");
        assert!(status.success(), "benchmark run failed: {status:?}");

        let actual =
            load_benchmark_report(&output_dir.join("report.json")).expect("load actual report");
        if let Err(failures) = compare_benchmark_reports(&baseline, &actual) {
            for failure in &failures {
                eprintln!("{failure}");
            }
            panic!("benchmark regression: {} failure(s)", failures.len());
        }
        let _ = fs::remove_dir_all(&output_dir);
    }

    #[test]
    fn benchmark_regression_skips_when_standard_run_root_missing() {
        let run_root = Path::new(STANDARD_RUN_ROOT);
        if run_root.join("scheduler.json").is_file() {
            return;
        }
        let readiness = benchmark_run_readiness(run_root).expect("readiness");
        assert!(!readiness.ready);
    }

    #[test]
    fn benchmark_controller_transitions_and_autoclose_report_gate() {
        let output_dir = benchmark_test_output_dir();
        let config = BenchmarkConfig {
            suite: BenchmarkSuite::Standard,
            run_root: PathBuf::from(STANDARD_RUN_ROOT),
            output_dir: output_dir.clone(),
            scenarios: vec![BenchmarkScenario::StartupFrames300],
            command: "fixture".to_owned(),
            run_readiness: Some(BenchmarkRunReadiness::from_counts(
                5,
                1,
                6,
                Some(5),
                1,
                19,
                18,
            )),
            callsite_sampling: None,
        };
        let heap_dir = default_heap_benchmark_dir(&config.output_dir);
        let mut controller =
            BenchmarkController::new(config, StartupProfile::default()).expect("controller");
        assert_eq!(controller.begin_frame(), Some(BenchmarkAction::None));
        controller.record_action(BenchmarkActionReport::for_action(BenchmarkAction::None));
        for _ in 0..(STANDARD_FRAME_TARGET - 1) {
            assert!(controller.end_frame().expect("frame").is_none());
            assert_eq!(controller.begin_frame(), None);
        }
        assert!(controller.end_frame().expect("last frame").is_some());
        assert!(controller.finished);
        let _ = fs::remove_dir_all(output_dir);
        let _ = fs::remove_dir_all(heap_dir);
    }

    fn benchmark_test_output_dir() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("data")
            .join("profiling")
            .join("test-output")
            .join(format!(
                "ploke-egui-benchmark-controller-fixture-{}",
                std::process::id()
            ))
    }
}
