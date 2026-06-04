//! Heap attribution used by native benchmark reports.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

ploke_egui_macros::profile_scope_catalog! {
    pub(crate) mod scope {
        ROOT = "root",
        ROOT_UI_THREAD = "root_ui_thread",
        ROOT_OTHER_THREAD = "root_other_thread",
        EFRAME_RUN_NATIVE = "eframe_run_native",
        FRAME_UPDATE = "frame_update",
        BENCHMARK_FRAME_BEGIN = "benchmark_frame_begin",
        BENCHMARK_APPLY_ACTION = "benchmark_apply_action",
        FRAME_PREPARE_SELECTION = "frame_prepare_selection",
        EMIT_DIAGNOSTICS = "emit_diagnostics",
        PUFFIN_CAPTURE = "puffin_capture",
        BENCHMARK_FRAME_END = "benchmark_frame_end",
        BENCHMARK_FINISH_FRAME = "benchmark_finish_frame",
        BENCHMARK_END_FRAME = "benchmark_end_frame",
        TOP_STRIP = "top_strip",
        RUN_NAVIGATION = "run_navigation",
        DIAGNOSTICS = "diagnostics",
        SELECTION_INSPECTOR = "selection_inspector",
        CENTRAL_GRAPH = "central_graph",
        EGUI_PANEL_TOP_STRIP_LAYOUT = "egui_panel_top_strip_layout",
        EGUI_PANEL_RUN_NAVIGATION_LAYOUT = "egui_panel_run_navigation_layout",
        EGUI_PANEL_SELECTION_INSPECTOR_LAYOUT = "egui_panel_selection_inspector_layout",
        EGUI_PANEL_TIMELINE_LAYOUT = "egui_panel_timeline_layout",
        EGUI_PANEL_CENTRAL_LAYOUT = "egui_panel_central_layout",
        INSPECTOR_SCROLL_AREA_LAYOUT = "inspector_scroll_area_layout",
        INSPECTOR_COLLAPSING_HEADER_LAYOUT = "inspector_collapsing_header_layout",
        EVAL_PROTOCOL = "eval_protocol",
        EVAL_PROTOCOL_VISUAL_SUMMARY = "eval_protocol_visual_summary",
        EVAL_PROTOCOL_PROTOCOL_DRILLDOWNS = "eval_protocol_protocol_drilldowns",
        EVAL_PROTOCOL_CALL_REVIEW_SCAN = "eval_protocol_call_review_scan",
        EVAL_PROTOCOL_CALL_REVIEW_SPOTLIGHT = "eval_protocol_call_review_spotlight",
        EVAL_PROTOCOL_RUN_SYNTHESIS_SPOTLIGHT = "eval_protocol_run_synthesis_spotlight",
        EVAL_PROTOCOL_CALL_REVIEW_ROW = "eval_protocol_call_review_row",
        CENTRAL_GRAPH_LAYOUT_STATE_RESTORE = "central_graph_layout_state_restore",
        CENTRAL_GRAPH_NAVIGATION_PREPARE = "central_graph_navigation_prepare",
        CENTRAL_GRAPH_WIDGET_BUILD = "central_graph_widget_build",
        CENTRAL_GRAPH_WIDGET_ADD = "central_graph_widget_add",
        CENTRAL_GRAPH_DIAGNOSTICS_UPDATE = "central_graph_diagnostics_update",
        EGUI_GRAPHS_NODE_UPDATE = "egui_graphs_node_update",
        EGUI_GRAPHS_NODE_SHAPE_LAYOUT = "egui_graphs_node_shape_layout",
        EGUI_GRAPHS_EDGE_UPDATE = "egui_graphs_edge_update",
        EGUI_GRAPHS_EDGE_SHAPE_LAYOUT = "egui_graphs_edge_shape_layout",
        EGUI_GRAPHS_EDGE_CURVE_LAYOUT = "egui_graphs_edge_curve_layout",
        EGUI_GRAPHS_EDGE_LABEL_LAYOUT = "egui_graphs_edge_label_layout",
        TIMELINE = "timeline",
        GRAPH_PROJECTION_CACHE_REFRESH = "graph_projection_cache_refresh",
        EGUI_TEXT_FONT_LAYOUT = "egui_text_font_layout",
        INSPECTOR_PARENT_CREATE_LLM_CALLS = "inspector_parent_create_llm_calls",
        INSPECTOR_PARENT_CREATE_SOURCE_STATUS = "inspector_parent_create_source_status",
        INSPECTOR_RUN_RECORDS = "inspector_run_records",
        INSPECTOR_RUN_RECORDS_RESOLVE_SLOT = "inspector_run_records_resolve_slot",
        INSPECTOR_RUN_RECORDS_ROW = "inspector_run_records_row",
        INSPECTOR_RUN_RECORDS_WIDGET_ROW = "inspector_run_records_widget_row",
        INSPECTOR_RUN_RECORDS_TEXT_GALLEY = "inspector_run_records_text_galley",
        INSPECTOR_RUN_RECORDS_TEXT_CACHE_LOOKUP = "inspector_run_records_text_cache_lookup",
        INSPECTOR_RUN_RECORDS_TEXT_CACHE_HIT = "inspector_run_records_text_cache_hit",
        INSPECTOR_RUN_RECORDS_TEXT_LAYOUT_OWNED_STRING = "inspector_run_records_text_layout_owned_string",
        INSPECTOR_RUN_RECORDS_TEXT_EGUI_LAYOUT = "inspector_run_records_text_egui_layout",
        INSPECTOR_RUN_RECORDS_TEXT_CACHE_STORE = "inspector_run_records_text_cache_store",
        INSPECTOR_RUN_RECORDS_LABEL_WIDGET = "inspector_run_records_label_widget",
        INSPECTOR_RUN_RECORDS_ID_GALLEY = "inspector_run_records_id_galley",
        INSPECTOR_RUN_RECORDS_ID_CACHE_LOOKUP = "inspector_run_records_id_cache_lookup",
        INSPECTOR_RUN_RECORDS_ID_CACHE_HIT = "inspector_run_records_id_cache_hit",
        INSPECTOR_RUN_RECORDS_ID_LABEL_PREP = "inspector_run_records_id_label_prep",
        INSPECTOR_RUN_RECORDS_ID_EGUI_LAYOUT = "inspector_run_records_id_egui_layout",
        INSPECTOR_RUN_RECORDS_ID_CACHE_STORE = "inspector_run_records_id_cache_store",
        INSPECTOR_RUN_RECORDS_ID_WIDGET = "inspector_run_records_id_widget",
        INSPECTOR_RUN_RECORD_ARM = "inspector_run_record_arm",
        INSPECTOR_AGENT_TRACE_LLM_TRACE = "inspector_agent_trace_llm_trace",
        INSPECTOR_PATCH_GENERATION = "inspector_patch_generation",
        INSPECTOR_PATCH_GENERATION_RECORD = "inspector_patch_generation_record",
        INSPECTOR_RUN_RECORD_TOOL_STEPS = "inspector_run_record_tool_steps",
        INSPECTOR_RUN_RECORD_TOOL_STEP = "inspector_run_record_tool_step",
        INSPECTOR_TOOL_ARGUMENTS = "inspector_tool_arguments",
        INSPECTOR_TOOL_RAW_ARGUMENTS = "inspector_tool_raw_arguments",
        INSPECTOR_TOOL_RESULT = "inspector_tool_result",
        INSPECTOR_TOOL_RAW_RESULT = "inspector_tool_raw_result",
        INSPECTOR_TOOL_UI_PAYLOAD = "inspector_tool_ui_payload",
        INSPECTOR_TOOL_UI_DETAILS = "inspector_tool_ui_details",
        INSPECTOR_TOOL_ERROR = "inspector_tool_error",
        INSPECTOR_TOOL_ARGUMENT_EDIT = "inspector_tool_argument_edit",
        INSPECTOR_TOOL_ARGUMENT_PATCH = "inspector_tool_argument_patch",
        INSPECTOR_TOOL_RESULT_LIST_DIR_DETAILS = "inspector_tool_result_list_dir_details",
        INSPECTOR_TOOL_RESULT_LIST_DIR_ENTRY = "inspector_tool_result_list_dir_entry",
        INSPECTOR_CONTEXT = "inspector_context",
        INSPECTOR_AGENT_TURN = "inspector_agent_turn",
        INSPECTOR_GRAPH_EDGES = "inspector_graph_edges",
        INSPECTOR_ARTIFACT_EDGES = "inspector_artifact_edges",
        INSPECTOR_PATCH_DEBUG = "inspector_patch_debug",
        INSPECTOR_PATCH_DEBUG_PATCH = "inspector_patch_debug_patch",
        INSPECTOR_PATCH_DEBUG_HEADER = "inspector_patch_debug_header",
        INSPECTOR_PATCH_DEBUG_DIFF_CACHE = "inspector_patch_debug_diff_cache",
        INSPECTOR_PATCH_DEBUG_DIFF_CACHE_HIT = "inspector_patch_debug_diff_cache_hit",
        INSPECTOR_PATCH_DEBUG_DIFF_BUILD_TEXT = "inspector_patch_debug_diff_build_text",
        INSPECTOR_PATCH_DEBUG_DIFF_HIGHLIGHT = "inspector_patch_debug_diff_highlight",
        INSPECTOR_PATCH_DEBUG_DIFF_LAYOUT = "inspector_patch_debug_diff_layout",
        INSPECTOR_PATCH_DEBUG_DIFF_STORE = "inspector_patch_debug_diff_store",
        INSPECTOR_PATCH_DEBUG_DIFF_WIDGET = "inspector_patch_debug_diff_widget",
        INSPECTOR_PATCH_DEBUG_DETAILS_HEADER = "inspector_patch_debug_details_header",
        INSPECTOR_PATCH_DEBUG_TOUCH = "inspector_patch_debug_touch",
        INSPECTOR_PATCH_DETAILS = "inspector_patch_details",
        INSPECTOR_SOURCE_REFS = "inspector_source_refs",
        INSPECTOR_SOURCE_REFS_ITER = "inspector_source_refs_iter",
        INSPECTOR_SOURCE_REFS_ROW = "inspector_source_refs_row",
        INSPECTOR_ARTIFACT_IDS = "inspector_artifact_ids",
        TRAJECTORY_ROWS_BUILD = "trajectory_rows_build",
        TRAJECTORY_PANE = "trajectory_pane",
        TRAJECTORY_SELECTION_DRILLDOWN = "trajectory_selection_drilldown",
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct AllocationSnapshot {
    pub enabled: bool,
    pub allocation_count: u64,
    pub deallocation_count: u64,
    pub allocated_bytes: u64,
    pub deallocated_bytes: u64,
}

impl AllocationSnapshot {
    pub fn delta_since(self, start: Self) -> AllocationDelta {
        AllocationDelta {
            enabled: self.enabled && start.enabled,
            allocation_count_delta: self.allocation_count.saturating_sub(start.allocation_count),
            deallocation_count_delta: self
                .deallocation_count
                .saturating_sub(start.deallocation_count),
            allocated_bytes_delta: self.allocated_bytes.saturating_sub(start.allocated_bytes),
            deallocated_bytes_delta: self
                .deallocated_bytes
                .saturating_sub(start.deallocated_bytes),
            scope:
                "tracking-allocator heap window; object bytes only, excludes GPU and driver memory"
                    .to_owned(),
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct AllocationDelta {
    pub enabled: bool,
    pub allocation_count_delta: u64,
    pub deallocation_count_delta: u64,
    pub allocated_bytes_delta: u64,
    pub deallocated_bytes_delta: u64,
    pub scope: String,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct HeapProfileTotals {
    pub allocation_count: u64,
    pub deallocation_count: u64,
    pub allocated_object_bytes: u64,
    pub deallocated_object_bytes: u64,
    pub allocated_wrapped_bytes: u64,
    pub deallocated_wrapped_bytes: u64,
    pub live_object_bytes: u64,
    pub live_wrapped_bytes: u64,
}

impl HeapProfileTotals {
    pub fn allocated(object_size: usize, wrapped_size: usize) -> Self {
        Self {
            allocation_count: 1,
            allocated_object_bytes: object_size as u64,
            allocated_wrapped_bytes: wrapped_size as u64,
            live_object_bytes: object_size as u64,
            live_wrapped_bytes: wrapped_size as u64,
            ..Self::default()
        }
    }

    pub fn add_allocated(&mut self, object_size: usize, wrapped_size: usize) {
        self.allocation_count = self.allocation_count.saturating_add(1);
        self.allocated_object_bytes = self
            .allocated_object_bytes
            .saturating_add(object_size as u64);
        self.allocated_wrapped_bytes = self
            .allocated_wrapped_bytes
            .saturating_add(wrapped_size as u64);
        self.live_object_bytes = self.live_object_bytes.saturating_add(object_size as u64);
        self.live_wrapped_bytes = self.live_wrapped_bytes.saturating_add(wrapped_size as u64);
    }

    pub fn add_allocated_scaled(&mut self, object_size: usize, wrapped_size: usize, scale: u64) {
        let scale = scale.max(1);
        self.allocation_count = self.allocation_count.saturating_add(scale);
        self.allocated_object_bytes = self
            .allocated_object_bytes
            .saturating_add((object_size as u64).saturating_mul(scale));
        self.allocated_wrapped_bytes = self
            .allocated_wrapped_bytes
            .saturating_add((wrapped_size as u64).saturating_mul(scale));
        self.live_object_bytes = self
            .live_object_bytes
            .saturating_add((object_size as u64).saturating_mul(scale));
        self.live_wrapped_bytes = self
            .live_wrapped_bytes
            .saturating_add((wrapped_size as u64).saturating_mul(scale));
    }

    pub fn add_deallocated(&mut self, object_size: usize, wrapped_size: usize) {
        self.deallocation_count = self.deallocation_count.saturating_add(1);
        self.deallocated_object_bytes = self
            .deallocated_object_bytes
            .saturating_add(object_size as u64);
        self.deallocated_wrapped_bytes = self
            .deallocated_wrapped_bytes
            .saturating_add(wrapped_size as u64);
        self.live_object_bytes = self.live_object_bytes.saturating_sub(object_size as u64);
        self.live_wrapped_bytes = self.live_wrapped_bytes.saturating_sub(wrapped_size as u64);
    }

    pub fn add_deallocated_scaled(&mut self, object_size: usize, wrapped_size: usize, scale: u64) {
        let scale = scale.max(1);
        self.deallocation_count = self.deallocation_count.saturating_add(scale);
        self.deallocated_object_bytes = self
            .deallocated_object_bytes
            .saturating_add((object_size as u64).saturating_mul(scale));
        self.deallocated_wrapped_bytes = self
            .deallocated_wrapped_bytes
            .saturating_add((wrapped_size as u64).saturating_mul(scale));
        self.live_object_bytes = self
            .live_object_bytes
            .saturating_sub((object_size as u64).saturating_mul(scale));
        self.live_wrapped_bytes = self
            .live_wrapped_bytes
            .saturating_sub((wrapped_size as u64).saturating_mul(scale));
    }

    pub fn has_activity(self) -> bool {
        self.allocation_count > 0
            || self.deallocation_count > 0
            || self.allocated_object_bytes > 0
            || self.deallocated_object_bytes > 0
            || self.allocated_wrapped_bytes > 0
            || self.deallocated_wrapped_bytes > 0
            || self.live_object_bytes > 0
            || self.live_wrapped_bytes > 0
    }

    pub fn delta_since(self, start: Self) -> Self {
        Self {
            allocation_count: self.allocation_count.saturating_sub(start.allocation_count),
            deallocation_count: self
                .deallocation_count
                .saturating_sub(start.deallocation_count),
            allocated_object_bytes: self
                .allocated_object_bytes
                .saturating_sub(start.allocated_object_bytes),
            deallocated_object_bytes: self
                .deallocated_object_bytes
                .saturating_sub(start.deallocated_object_bytes),
            allocated_wrapped_bytes: self
                .allocated_wrapped_bytes
                .saturating_sub(start.allocated_wrapped_bytes),
            deallocated_wrapped_bytes: self
                .deallocated_wrapped_bytes
                .saturating_sub(start.deallocated_wrapped_bytes),
            live_object_bytes: self
                .live_object_bytes
                .saturating_sub(start.live_object_bytes),
            live_wrapped_bytes: self
                .live_wrapped_bytes
                .saturating_sub(start.live_wrapped_bytes),
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct HeapGroupProfile {
    pub group_id: usize,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    pub totals: HeapProfileTotals,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct HeapCallsiteKey {
    pub symbol: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub file: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub line: Option<u32>,
}

impl HeapCallsiteKey {
    pub fn synthetic(symbol: impl Into<String>) -> Self {
        Self {
            symbol: symbol.into(),
            file: None,
            line: None,
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct HeapCallsiteProfile {
    pub callsite: HeapCallsiteKey,
    pub totals: HeapProfileTotals,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct HeapProfileSnapshot {
    pub enabled: bool,
    pub totals: HeapProfileTotals,
    pub groups: Vec<HeapGroupProfile>,
    pub callsites: Vec<HeapCallsiteProfile>,
    pub unmatched_deallocations: u64,
}

impl HeapProfileSnapshot {
    pub fn allocation_snapshot(&self) -> AllocationSnapshot {
        AllocationSnapshot {
            enabled: self.enabled,
            allocation_count: self.totals.allocation_count,
            deallocation_count: self.totals.deallocation_count,
            allocated_bytes: self.totals.allocated_object_bytes,
            deallocated_bytes: self.totals.deallocated_object_bytes,
        }
    }

    pub fn delta_since(&self, start: &Self) -> Self {
        let start_groups = start
            .groups
            .iter()
            .map(|group| (group.group_id, group.totals))
            .collect::<BTreeMap<_, _>>();
        let groups = self
            .groups
            .iter()
            .filter_map(|group| {
                let start_totals = start_groups
                    .get(&group.group_id)
                    .copied()
                    .unwrap_or_default();
                let totals = group.totals.delta_since(start_totals);
                totals.has_activity().then(|| HeapGroupProfile {
                    group_id: group.group_id,
                    name: group.name.clone(),
                    totals,
                })
            })
            .collect::<Vec<_>>();

        let start_callsites = start
            .callsites
            .iter()
            .map(|callsite| (callsite.callsite.clone(), callsite.totals))
            .collect::<BTreeMap<_, _>>();
        let callsites = self
            .callsites
            .iter()
            .filter_map(|callsite| {
                let start_totals = start_callsites
                    .get(&callsite.callsite)
                    .copied()
                    .unwrap_or_default();
                let totals = callsite.totals.delta_since(start_totals);
                totals.has_activity().then(|| HeapCallsiteProfile {
                    callsite: callsite.callsite.clone(),
                    totals,
                })
            })
            .collect::<Vec<_>>();

        Self {
            enabled: self.enabled && start.enabled,
            totals: self.totals.delta_since(start.totals),
            groups,
            callsites,
            unmatched_deallocations: self
                .unmatched_deallocations
                .saturating_sub(start.unmatched_deallocations),
        }
    }

    pub fn top_groups_by_allocated_bytes(&self, limit: usize) -> Vec<HeapGroupProfile> {
        top_by(self.groups.clone(), limit, |group| {
            group.totals.allocated_wrapped_bytes
        })
    }

    pub fn top_groups_by_allocation_count(&self, limit: usize) -> Vec<HeapGroupProfile> {
        top_by(self.groups.clone(), limit, |group| {
            group.totals.allocation_count
        })
    }

    pub fn top_groups_by_retained_bytes(&self, limit: usize) -> Vec<HeapGroupProfile> {
        top_by(self.groups.clone(), limit, |group| {
            group.totals.live_wrapped_bytes
        })
    }

    pub fn top_callsites_by_allocated_bytes(&self, limit: usize) -> Vec<HeapCallsiteProfile> {
        top_by(self.callsites.clone(), limit, |callsite| {
            callsite.totals.allocated_wrapped_bytes
        })
    }

    pub fn top_callsites_by_allocation_count(&self, limit: usize) -> Vec<HeapCallsiteProfile> {
        top_by(self.callsites.clone(), limit, |callsite| {
            callsite.totals.allocation_count
        })
    }

    pub fn top_callsites_by_retained_bytes(&self, limit: usize) -> Vec<HeapCallsiteProfile> {
        top_by(self.callsites.clone(), limit, |callsite| {
            callsite.totals.live_wrapped_bytes
        })
    }
}

fn top_by<T>(mut values: Vec<T>, limit: usize, value: impl Fn(&T) -> u64) -> Vec<T> {
    values.sort_by(|left, right| value(right).cmp(&value(left)));
    values.truncate(limit);
    values
}

#[cfg(all(not(target_arch = "wasm32"), feature = "native-benchmark"))]
mod tracking {
    use std::cell::RefCell;
    use std::collections::BTreeMap;
    use std::error::Error;
    use std::sync::OnceLock;
    use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
    use std::sync::{Arc, Mutex};
    use std::thread;

    use tracing::{Id, Subscriber};
    use tracing_subscriber::Layer;
    use tracing_subscriber::layer::{Context, SubscriberExt};
    use tracing_subscriber::registry::LookupSpan;
    use tracking_allocator::{
        AllocationGroupId, AllocationGroupToken, AllocationGuard, AllocationRegistry,
        AllocationTracker,
    };

    use super::{
        AllocationSnapshot, HeapCallsiteKey, HeapCallsiteProfile, HeapGroupProfile,
        HeapProfileSnapshot, HeapProfileTotals,
    };

    static GLOBAL_TRACKER: OnceLock<HeapProfileTracker> = OnceLock::new();
    static GROUP_SCOPE_MAP: OnceLock<GroupScopeMap> = OnceLock::new();
    static UI_THREAD_ID: OnceLock<thread::ThreadId> = OnceLock::new();
    static CURRENT_GENERATION: AtomicU64 = AtomicU64::new(0);
    const GROUP_SCOPE_MAP_CAPACITY: usize = 1 << 20;

    #[derive(Debug, Clone, Default)]
    pub struct HeapProfileTracker {
        inner: Arc<HeapProfileState>,
    }

    #[derive(Debug)]
    struct HeapProfileState {
        enabled: AtomicBool,
        totals: AtomicHeapTotals,
        scopes: Box<[ScopeCounter]>,
        callsite_sampling: CallsiteSamplingState,
        unmatched_deallocations: AtomicU64,
    }

    impl Default for HeapProfileState {
        fn default() -> Self {
            Self {
                enabled: AtomicBool::new(false),
                totals: AtomicHeapTotals::default(),
                scopes: SCOPE_NAMES
                    .iter()
                    .map(|name| ScopeCounter {
                        name,
                        totals: AtomicHeapTotals::default(),
                    })
                    .collect::<Vec<_>>()
                    .into_boxed_slice(),
                callsite_sampling: CallsiteSamplingState::new(SCOPE_NAMES.len()),
                unmatched_deallocations: AtomicU64::new(0),
            }
        }
    }

    #[derive(Debug)]
    struct CallsiteSamplingState {
        enabled: AtomicBool,
        sample_every: AtomicU64,
        sequence: AtomicU64,
        scopes: Box<[AtomicBool]>,
        callsites: Mutex<BTreeMap<HeapCallsiteKey, HeapProfileTotals>>,
        sampled_allocations: Mutex<BTreeMap<usize, SampledAllocation>>,
    }

    impl CallsiteSamplingState {
        fn new(scope_count: usize) -> Self {
            Self {
                enabled: AtomicBool::new(false),
                sample_every: AtomicU64::new(1),
                sequence: AtomicU64::new(0),
                scopes: (0..scope_count)
                    .map(|_| AtomicBool::new(false))
                    .collect::<Vec<_>>()
                    .into_boxed_slice(),
                callsites: Mutex::new(BTreeMap::new()),
                sampled_allocations: Mutex::new(BTreeMap::new()),
            }
        }

        fn configure(&self, sample_every: Option<u64>, scope_indices: &[usize]) {
            self.enabled.store(false, Ordering::Relaxed);
            for scope in self.scopes.iter() {
                scope.store(false, Ordering::Relaxed);
            }
            self.reset_window();
            let Some(sample_every) = sample_every else {
                return;
            };
            self.sample_every
                .store(sample_every.max(1), Ordering::Relaxed);
            for index in scope_indices {
                if let Some(scope) = self.scopes.get(*index) {
                    scope.store(true, Ordering::Relaxed);
                }
            }
            self.enabled.store(true, Ordering::Relaxed);
        }

        fn reset_window(&self) {
            self.sequence.store(0, Ordering::Relaxed);
            if let Ok(mut callsites) = self.callsites.lock() {
                callsites.clear();
            }
            if let Ok(mut sampled_allocations) = self.sampled_allocations.lock() {
                sampled_allocations.clear();
            }
        }

        fn sample_scale(&self, scope_index: usize) -> Option<u64> {
            if !self.enabled.load(Ordering::Relaxed) {
                return None;
            }
            if !self
                .scopes
                .get(scope_index)
                .is_some_and(|scope| scope.load(Ordering::Relaxed))
            {
                return None;
            }
            let sample_every = self.sample_every.load(Ordering::Relaxed).max(1);
            let sequence = self.sequence.fetch_add(1, Ordering::Relaxed);
            (sequence % sample_every == 0).then_some(sample_every)
        }

        fn record_allocated(
            &self,
            addr: usize,
            object_size: usize,
            wrapped_size: usize,
            scale: u64,
            callsite: HeapCallsiteKey,
        ) {
            if let Ok(mut callsites) = self.callsites.lock() {
                callsites
                    .entry(callsite.clone())
                    .or_default()
                    .add_allocated_scaled(object_size, wrapped_size, scale);
            }
            if let Ok(mut sampled_allocations) = self.sampled_allocations.lock() {
                sampled_allocations.insert(addr, SampledAllocation { callsite, scale });
            }
        }

        fn record_deallocated(&self, addr: usize, object_size: usize, wrapped_size: usize) {
            let sampled = self
                .sampled_allocations
                .lock()
                .ok()
                .and_then(|mut sampled_allocations| sampled_allocations.remove(&addr));
            let Some(sampled) = sampled else {
                return;
            };
            if let Ok(mut callsites) = self.callsites.lock() {
                if let Some(totals) = callsites.get_mut(&sampled.callsite) {
                    totals.add_deallocated_scaled(object_size, wrapped_size, sampled.scale);
                }
            }
        }

        fn snapshot(&self) -> Vec<HeapCallsiteProfile> {
            self.callsites
                .lock()
                .map(|callsites| {
                    callsites
                        .iter()
                        .map(|(callsite, totals)| HeapCallsiteProfile {
                            callsite: callsite.clone(),
                            totals: *totals,
                        })
                        .collect()
                })
                .unwrap_or_default()
        }
    }

    #[derive(Debug, Clone)]
    struct SampledAllocation {
        callsite: HeapCallsiteKey,
        scale: u64,
    }

    #[derive(Debug)]
    struct ScopeCounter {
        name: &'static str,
        totals: AtomicHeapTotals,
    }

    #[derive(Debug, Default)]
    struct AtomicHeapTotals {
        allocation_count: AtomicU64,
        deallocation_count: AtomicU64,
        allocated_object_bytes: AtomicU64,
        deallocated_object_bytes: AtomicU64,
        allocated_wrapped_bytes: AtomicU64,
        deallocated_wrapped_bytes: AtomicU64,
        live_object_bytes: AtomicU64,
        live_wrapped_bytes: AtomicU64,
    }

    impl AtomicHeapTotals {
        fn reset(&self) {
            self.allocation_count.store(0, Ordering::Relaxed);
            self.deallocation_count.store(0, Ordering::Relaxed);
            self.allocated_object_bytes.store(0, Ordering::Relaxed);
            self.deallocated_object_bytes.store(0, Ordering::Relaxed);
            self.allocated_wrapped_bytes.store(0, Ordering::Relaxed);
            self.deallocated_wrapped_bytes.store(0, Ordering::Relaxed);
            self.live_object_bytes.store(0, Ordering::Relaxed);
            self.live_wrapped_bytes.store(0, Ordering::Relaxed);
        }

        fn add_allocated(&self, object_size: usize, wrapped_size: usize) {
            self.allocation_count.fetch_add(1, Ordering::Relaxed);
            self.allocated_object_bytes
                .fetch_add(object_size as u64, Ordering::Relaxed);
            self.allocated_wrapped_bytes
                .fetch_add(wrapped_size as u64, Ordering::Relaxed);
            self.live_object_bytes
                .fetch_add(object_size as u64, Ordering::Relaxed);
            self.live_wrapped_bytes
                .fetch_add(wrapped_size as u64, Ordering::Relaxed);
        }

        fn add_deallocated(&self, object_size: usize, wrapped_size: usize, subtract_live: bool) {
            self.deallocation_count.fetch_add(1, Ordering::Relaxed);
            self.deallocated_object_bytes
                .fetch_add(object_size as u64, Ordering::Relaxed);
            self.deallocated_wrapped_bytes
                .fetch_add(wrapped_size as u64, Ordering::Relaxed);
            if subtract_live {
                saturating_sub(&self.live_object_bytes, object_size as u64);
                saturating_sub(&self.live_wrapped_bytes, wrapped_size as u64);
            }
        }

        fn snapshot(&self) -> HeapProfileTotals {
            HeapProfileTotals {
                allocation_count: self.allocation_count.load(Ordering::Relaxed),
                deallocation_count: self.deallocation_count.load(Ordering::Relaxed),
                allocated_object_bytes: self.allocated_object_bytes.load(Ordering::Relaxed),
                deallocated_object_bytes: self.deallocated_object_bytes.load(Ordering::Relaxed),
                allocated_wrapped_bytes: self.allocated_wrapped_bytes.load(Ordering::Relaxed),
                deallocated_wrapped_bytes: self.deallocated_wrapped_bytes.load(Ordering::Relaxed),
                live_object_bytes: self.live_object_bytes.load(Ordering::Relaxed),
                live_wrapped_bytes: self.live_wrapped_bytes.load(Ordering::Relaxed),
            }
        }
    }

    fn saturating_sub(value: &AtomicU64, decrement: u64) {
        let _ = value.fetch_update(Ordering::Relaxed, Ordering::Relaxed, |current| {
            Some(current.saturating_sub(decrement))
        });
    }

    impl HeapProfileTracker {
        pub fn reset(&self) {
            AllocationRegistry::untracked(|| {
                self.inner.totals.reset();
                for scope in self.inner.scopes.iter() {
                    scope.totals.reset();
                }
                self.inner.callsite_sampling.reset_window();
                self.inner
                    .unmatched_deallocations
                    .store(0, Ordering::Relaxed);
            });
        }

        pub fn set_enabled(&self, enabled: bool) {
            self.inner.enabled.store(enabled, Ordering::Relaxed);
        }

        pub fn snapshot(&self) -> HeapProfileSnapshot {
            AllocationRegistry::untracked(|| {
                let groups = self
                    .inner
                    .scopes
                    .iter()
                    .enumerate()
                    .filter_map(|(index, scope)| {
                        let totals = scope.totals.snapshot();
                        totals.has_activity().then(|| HeapGroupProfile {
                            group_id: index + 1,
                            name: Some(scope.name.to_owned()),
                            totals,
                        })
                    })
                    .collect::<Vec<_>>();
                let callsites = self.inner.callsite_sampling.snapshot();

                HeapProfileSnapshot {
                    enabled: self.inner.enabled.load(Ordering::Relaxed),
                    totals: self.inner.totals.snapshot(),
                    groups,
                    callsites,
                    unmatched_deallocations: self
                        .inner
                        .unmatched_deallocations
                        .load(Ordering::Relaxed),
                }
            })
        }

        pub fn totals_snapshot(&self) -> HeapProfileSnapshot {
            AllocationRegistry::untracked(|| HeapProfileSnapshot {
                enabled: self.inner.enabled.load(Ordering::Relaxed),
                totals: self.inner.totals.snapshot(),
                groups: Vec::new(),
                callsites: Vec::new(),
                unmatched_deallocations: self.inner.unmatched_deallocations.load(Ordering::Relaxed),
            })
        }

        pub fn configure_callsite_sampling(
            &self,
            sample_every: Option<u64>,
            scope_names: &[&str],
        ) -> Result<(), String> {
            let mut scope_indices = Vec::with_capacity(scope_names.len());
            for name in scope_names {
                let Some(index) = scope_index(name) else {
                    return Err(format!("unknown allocation sampling scope '{name}'"));
                };
                scope_indices.push(index);
            }
            self.inner
                .callsite_sampling
                .configure(sample_every, &scope_indices);
            Ok(())
        }

        fn record_allocated(
            &self,
            addr: usize,
            object_size: usize,
            wrapped_size: usize,
            group_id: usize,
        ) {
            self.inner.totals.add_allocated(object_size, wrapped_size);
            if let Some(scope_index) = current_window_scope_index(group_id) {
                self.inner.scopes[scope_index]
                    .totals
                    .add_allocated(object_size, wrapped_size);
                self.record_callsite_allocated(addr, object_size, wrapped_size, scope_index);
            }
        }

        fn record_deallocated(
            &self,
            addr: usize,
            object_size: usize,
            wrapped_size: usize,
            source_group_id: usize,
        ) {
            let source_scope = current_window_scope_index(source_group_id);
            self.inner
                .totals
                .add_deallocated(object_size, wrapped_size, source_scope.is_some());
            if let Some(scope_index) = source_scope {
                self.inner.scopes[scope_index].totals.add_deallocated(
                    object_size,
                    wrapped_size,
                    true,
                );
            } else {
                self.inner
                    .unmatched_deallocations
                    .fetch_add(1, Ordering::Relaxed);
            }
            self.inner
                .callsite_sampling
                .record_deallocated(addr, object_size, wrapped_size);
        }

        fn record_callsite_allocated(
            &self,
            addr: usize,
            object_size: usize,
            wrapped_size: usize,
            scope_index: usize,
        ) {
            let Some(scale) = self.inner.callsite_sampling.sample_scale(scope_index) else {
                return;
            };
            let callsite = capture_heap_callsite(SCOPE_NAMES[scope_index]);
            self.inner.callsite_sampling.record_allocated(
                addr,
                object_size,
                wrapped_size,
                scale,
                callsite,
            );
        }

        #[cfg(test)]
        pub fn record_synthetic_allocated(
            &self,
            object_size: usize,
            wrapped_size: usize,
            scope_name: &'static str,
        ) {
            let scope_index = scope_index(scope_name).expect("known synthetic scope");
            self.inner.totals.add_allocated(object_size, wrapped_size);
            self.inner.scopes[scope_index]
                .totals
                .add_allocated(object_size, wrapped_size);
        }

        #[cfg(test)]
        pub fn record_synthetic_deallocated(
            &self,
            object_size: usize,
            wrapped_size: usize,
            scope_name: &'static str,
        ) {
            let scope_index = scope_index(scope_name).expect("known synthetic scope");
            self.inner
                .totals
                .add_deallocated(object_size, wrapped_size, true);
            self.inner.scopes[scope_index]
                .totals
                .add_deallocated(object_size, wrapped_size, true);
        }
    }

    impl AllocationTracker for HeapProfileTracker {
        fn allocated(
            &self,
            addr: usize,
            object_size: usize,
            wrapped_size: usize,
            group_id: AllocationGroupId,
        ) {
            self.record_allocated(addr, object_size, wrapped_size, group_id.as_usize().get());
        }

        fn deallocated(
            &self,
            addr: usize,
            object_size: usize,
            wrapped_size: usize,
            source_group_id: AllocationGroupId,
            _current_group_id: AllocationGroupId,
        ) {
            self.record_deallocated(
                addr,
                object_size,
                wrapped_size,
                source_group_id.as_usize().get(),
            );
        }
    }

    pub fn install_global_tracker() -> Result<HeapProfileTracker, Box<dyn Error>> {
        if let Some(tracker) = GLOBAL_TRACKER.get() {
            return Ok(tracker.clone());
        }

        let tracker = HeapProfileTracker::default();
        match AllocationRegistry::set_global_tracker(tracker.clone()) {
            Ok(()) => {
                let _ = GLOBAL_TRACKER.set(tracker);
                Ok(GLOBAL_TRACKER
                    .get()
                    .expect("tracker was just initialized")
                    .clone())
            }
            Err(error) => GLOBAL_TRACKER
                .get()
                .cloned()
                .ok_or_else(|| Box::new(error) as Box<dyn Error>),
        }
    }

    pub fn configure_callsite_sampling(
        tracker: &HeapProfileTracker,
        sample_every: Option<u64>,
        scope_names: &[&str],
    ) -> Result<(), String> {
        tracker.configure_callsite_sampling(sample_every, scope_names)
    }

    pub fn with_tracing_subscriber<R>(f: impl FnOnce() -> R) -> R {
        let subscriber = tracing_subscriber::Registry::default().with(AllocationScopeLayer);
        tracing::subscriber::with_default(subscriber, f)
    }

    pub fn begin_tracking_window(tracker: &HeapProfileTracker) {
        AllocationRegistry::disable_tracking();
        tracker.reset();
        let _ = UI_THREAD_ID.get_or_init(|| thread::current().id());
        let generation = CURRENT_GENERATION
            .fetch_add(1, Ordering::Relaxed)
            .saturating_add(1);
        refresh_active_scope_generations(generation);
        tracker.set_enabled(true);
        AllocationRegistry::enable_tracking();
    }

    pub fn finish_tracking_window(tracker: &HeapProfileTracker) -> HeapProfileSnapshot {
        AllocationRegistry::disable_tracking();
        let snapshot = tracker.snapshot();
        tracker.set_enabled(false);
        snapshot
    }

    pub fn profile_snapshot(tracker: &HeapProfileTracker) -> HeapProfileSnapshot {
        tracker.snapshot()
    }

    struct AllocationScopeLayer;

    impl<S> Layer<S> for AllocationScopeLayer
    where
        S: Subscriber + for<'span> LookupSpan<'span>,
    {
        fn on_enter(&self, id: &Id, ctx: Context<'_, S>) {
            let Some(span) = ctx.span(id) else {
                return;
            };
            if let Some(scope_index) = scope_index(span.metadata().name()) {
                enter_scope(scope_index);
            }
        }

        fn on_exit(&self, id: &Id, ctx: Context<'_, S>) {
            let Some(span) = ctx.span(id) else {
                return;
            };
            if let Some(scope_index) = scope_index(span.metadata().name()) {
                exit_scope(scope_index);
            }
        }
    }

    struct ActiveScopeGuard {
        scope_index: usize,
        group_id: usize,
        _guard: AllocationGuard<'static>,
    }

    thread_local! {
        static ACTIVE_SCOPE_GUARDS: RefCell<Vec<ActiveScopeGuard>> = const { RefCell::new(Vec::new()) };
    }

    fn enter_scope(scope_index: usize) {
        let Some(token) = AllocationRegistry::untracked(|| {
            AllocationGroupToken::register().map(|token| Box::leak(Box::new(token)))
        }) else {
            return;
        };
        let group_id = token.id().as_usize().get();
        register_group_scope(
            group_id,
            scope_index,
            CURRENT_GENERATION.load(Ordering::Relaxed),
        );
        let guard = token.enter();
        AllocationRegistry::untracked(|| {
            ACTIVE_SCOPE_GUARDS.with(|guards| {
                guards.borrow_mut().push(ActiveScopeGuard {
                    scope_index,
                    group_id,
                    _guard: guard,
                });
            });
        });
    }

    fn exit_scope(scope_index: usize) {
        let guard = AllocationRegistry::untracked(|| {
            ACTIVE_SCOPE_GUARDS.with(|guards| {
                let mut guards = guards.borrow_mut();
                match guards.pop() {
                    Some(guard) if guard.scope_index == scope_index => Some(guard),
                    Some(guard) => {
                        guards.push(guard);
                        None
                    }
                    None => None,
                }
            })
        });
        drop(guard);
    }

    fn refresh_active_scope_generations(generation: u64) {
        AllocationRegistry::untracked(|| {
            ACTIVE_SCOPE_GUARDS.with(|guards| {
                for guard in guards.borrow().iter() {
                    register_group_scope(guard.group_id, guard.scope_index, generation);
                }
            });
        });
    }

    struct GroupScopeMap {
        scopes: Box<[AtomicUsize]>,
        generations: Box<[AtomicU64]>,
        overflowed_registrations: AtomicU64,
    }

    impl GroupScopeMap {
        fn new() -> Self {
            Self {
                scopes: (0..GROUP_SCOPE_MAP_CAPACITY)
                    .map(|_| AtomicUsize::new(0))
                    .collect::<Vec<_>>()
                    .into_boxed_slice(),
                generations: (0..GROUP_SCOPE_MAP_CAPACITY)
                    .map(|_| AtomicU64::new(0))
                    .collect::<Vec<_>>()
                    .into_boxed_slice(),
                overflowed_registrations: AtomicU64::new(0),
            }
        }

        fn register(&self, group_id: usize, scope_index: usize, generation: u64) {
            if group_id >= self.scopes.len() {
                self.overflowed_registrations
                    .fetch_add(1, Ordering::Relaxed);
                return;
            }
            self.scopes[group_id].store(scope_index + 1, Ordering::Release);
            self.generations[group_id].store(generation, Ordering::Release);
        }

        fn current_scope_index(&self, group_id: usize, generation: u64) -> Option<usize> {
            if group_id == AllocationGroupId::ROOT.as_usize().get() {
                return Some(root_scope_index());
            }
            if group_id >= self.scopes.len() {
                return None;
            }
            if self.generations[group_id].load(Ordering::Acquire) != generation {
                return None;
            }
            self.scopes[group_id].load(Ordering::Acquire).checked_sub(1)
        }
    }

    fn register_group_scope(group_id: usize, scope_index: usize, generation: u64) {
        group_scope_map().register(group_id, scope_index, generation);
    }

    fn current_window_scope_index(group_id: usize) -> Option<usize> {
        group_scope_map().current_scope_index(group_id, CURRENT_GENERATION.load(Ordering::Relaxed))
    }

    fn group_scope_map() -> &'static GroupScopeMap {
        GROUP_SCOPE_MAP.get_or_init(GroupScopeMap::new)
    }

    const ROOT_SCOPE_INDEX: usize = super::scope::ROOT_INDEX;
    const ROOT_UI_THREAD_SCOPE_INDEX: usize = super::scope::ROOT_UI_THREAD_INDEX;
    const ROOT_OTHER_THREAD_SCOPE_INDEX: usize = super::scope::ROOT_OTHER_THREAD_INDEX;
    const SCOPE_NAMES: &[&str] = super::scope::ALL;

    fn scope_index(name: &str) -> Option<usize> {
        super::scope::index(name)
    }

    fn root_scope_index() -> usize {
        let current = thread::current().id();
        match UI_THREAD_ID.get() {
            Some(ui_thread) if ui_thread == &current => ROOT_UI_THREAD_SCOPE_INDEX,
            Some(_) => ROOT_OTHER_THREAD_SCOPE_INDEX,
            None => ROOT_SCOPE_INDEX,
        }
    }

    fn capture_heap_callsite(scope_name: &'static str) -> HeapCallsiteKey {
        let backtrace = backtrace::Backtrace::new();
        for frame in backtrace.frames() {
            for symbol in frame.symbols() {
                let symbol_name = symbol
                    .name()
                    .map(|name| name.to_string())
                    .unwrap_or_else(|| "<unknown>".to_owned());
                let file = symbol.filename().map(|path| path.display().to_string());
                if is_internal_allocation_frame(&symbol_name, file.as_deref()) {
                    continue;
                }
                return HeapCallsiteKey {
                    symbol: format!("{scope_name} -> {symbol_name}"),
                    file,
                    line: symbol.lineno(),
                };
            }
        }
        HeapCallsiteKey::synthetic(format!("{scope_name} -> <unknown>"))
    }

    fn is_internal_allocation_frame(symbol: &str, file: Option<&str>) -> bool {
        let file = file.unwrap_or_default();
        file.contains("crates/ploke-egui/src/allocation.rs")
            || file.contains("tracking-allocator")
            || file.contains("/backtrace-")
            || symbol.contains("ploke_egui::allocation")
            || symbol.contains("tracking_allocator")
            || symbol.contains("backtrace::")
            || symbol.contains("__rust_alloc")
            || symbol.contains("__rust_realloc")
            || symbol.contains("__rust_dealloc")
            || symbol.contains("__rdl_alloc")
            || symbol.contains("__rdl_realloc")
            || symbol.contains("__rdl_dealloc")
            || symbol.contains("std::alloc")
            || symbol == "alloc"
            || file.ends_with("/rust/library/alloc/src/alloc.rs")
            || symbol.contains("alloc::alloc")
            || symbol.contains("alloc::raw_vec")
            || symbol.contains("alloc::boxed")
            || symbol.contains("alloc::sync")
            || symbol.contains("alloc::vec")
            || symbol.contains("alloc::string")
    }

    pub fn process_snapshot() -> AllocationSnapshot {
        GLOBAL_TRACKER
            .get()
            .map(|tracker| tracker.totals_snapshot().allocation_snapshot())
            .unwrap_or_default()
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn heap_profile_tracker_aggregates_group_liveness() {
            let tracker = HeapProfileTracker::default();
            tracker.record_synthetic_allocated(64, 96, "selection_inspector");
            tracker.record_synthetic_allocated(32, 64, "selection_inspector");
            tracker.record_synthetic_deallocated(64, 96, "selection_inspector");

            let snapshot = tracker.snapshot();
            assert_eq!(snapshot.totals.allocation_count, 2);
            assert_eq!(snapshot.totals.deallocation_count, 1);
            assert_eq!(snapshot.totals.allocated_wrapped_bytes, 160);
            assert_eq!(snapshot.totals.live_wrapped_bytes, 64);
            let group = snapshot
                .groups
                .iter()
                .find(|group| group.name.as_deref() == Some("selection_inspector"))
                .expect("selection inspector group");
            assert_eq!(group.totals.live_wrapped_bytes, 64);
        }

        #[test]
        fn standard_heap_tracker_omits_callsites() {
            let tracker = HeapProfileTracker::default();
            tracker.record_synthetic_allocated(11, 32, "top_strip");

            let snapshot = tracker.snapshot();
            assert!(snapshot.callsites.is_empty());
        }

        #[test]
        fn heap_profile_snapshot_delta_preserves_group_attribution() {
            let tracker = HeapProfileTracker::default();
            tracker.record_synthetic_allocated(64, 96, "selection_inspector");
            let start = tracker.snapshot();

            tracker.record_synthetic_allocated(11, 32, "top_strip");
            tracker.record_synthetic_allocated(19, 48, "selection_inspector");
            let end = tracker.snapshot();

            let delta = end.delta_since(&start);
            assert_eq!(delta.totals.allocation_count, 2);
            assert_eq!(delta.totals.allocated_object_bytes, 30);
            assert_eq!(delta.totals.live_wrapped_bytes, 80);

            let top_strip = delta
                .groups
                .iter()
                .find(|group| group.name.as_deref() == Some("top_strip"))
                .expect("top strip delta group");
            assert_eq!(top_strip.totals.allocation_count, 1);
            assert_eq!(top_strip.totals.allocated_wrapped_bytes, 32);

            let selection = delta
                .groups
                .iter()
                .find(|group| group.name.as_deref() == Some("selection_inspector"))
                .expect("selection inspector delta group");
            assert_eq!(selection.totals.allocation_count, 1);
            assert_eq!(selection.totals.allocated_object_bytes, 19);
        }

        #[test]
        fn run_record_measurement_scopes_are_registered() {
            let tracker = HeapProfileTracker::default();
            for scope in [
                "inspector_run_records_resolve_slot",
                "inspector_run_records_row",
                "inspector_run_records_widget_row",
                "inspector_run_records_text_galley",
                "inspector_run_records_text_cache_lookup",
                "inspector_run_records_text_cache_hit",
                "inspector_run_records_text_layout_owned_string",
                "inspector_run_records_text_egui_layout",
                "inspector_run_records_text_cache_store",
                "inspector_run_records_label_widget",
                "inspector_run_records_id_galley",
                "inspector_run_records_id_cache_lookup",
                "inspector_run_records_id_cache_hit",
                "inspector_run_records_id_label_prep",
                "inspector_run_records_id_egui_layout",
                "inspector_run_records_id_cache_store",
                "inspector_run_records_id_widget",
            ] {
                tracker.record_synthetic_allocated(1, 1, scope);
            }

            let snapshot = tracker.snapshot();
            for scope in [
                "inspector_run_records_resolve_slot",
                "inspector_run_records_row",
                "inspector_run_records_widget_row",
                "inspector_run_records_text_galley",
                "inspector_run_records_text_cache_lookup",
                "inspector_run_records_text_cache_hit",
                "inspector_run_records_text_layout_owned_string",
                "inspector_run_records_text_egui_layout",
                "inspector_run_records_text_cache_store",
                "inspector_run_records_label_widget",
                "inspector_run_records_id_galley",
                "inspector_run_records_id_cache_lookup",
                "inspector_run_records_id_cache_hit",
                "inspector_run_records_id_label_prep",
                "inspector_run_records_id_egui_layout",
                "inspector_run_records_id_cache_store",
                "inspector_run_records_id_widget",
            ] {
                assert!(
                    snapshot
                        .groups
                        .iter()
                        .any(|group| group.name.as_deref() == Some(scope)),
                    "{scope} should be tracked"
                );
            }
        }

        #[test]
        fn root_investigation_scopes_are_registered() {
            let tracker = HeapProfileTracker::default();
            for scope in [
                "root",
                "root_ui_thread",
                "root_other_thread",
                "eframe_run_native",
                "egui_text_font_layout",
                "central_graph_widget_add",
            ] {
                tracker.record_synthetic_allocated(1, 1, scope);
            }

            let snapshot = tracker.snapshot();
            for scope in [
                "root",
                "root_ui_thread",
                "root_other_thread",
                "eframe_run_native",
                "egui_text_font_layout",
                "central_graph_widget_add",
            ] {
                assert!(
                    snapshot
                        .groups
                        .iter()
                        .any(|group| group.name.as_deref() == Some(scope)),
                    "{scope} should be tracked"
                );
            }
        }

        #[test]
        fn layout_investigation_scopes_are_registered() {
            let tracker = HeapProfileTracker::default();
            for scope in [
                "benchmark_frame_begin",
                "benchmark_apply_action",
                "frame_prepare_selection",
                "emit_diagnostics",
                "puffin_capture",
                "benchmark_frame_end",
                "benchmark_finish_frame",
                "benchmark_end_frame",
                "egui_panel_top_strip_layout",
                "egui_panel_run_navigation_layout",
                "egui_panel_selection_inspector_layout",
                "egui_panel_timeline_layout",
                "egui_panel_central_layout",
                "inspector_scroll_area_layout",
                "inspector_collapsing_header_layout",
                "central_graph_layout_state_restore",
                "central_graph_navigation_prepare",
                "central_graph_widget_build",
                "central_graph_diagnostics_update",
                "egui_graphs_node_update",
                "egui_graphs_node_shape_layout",
                "egui_graphs_edge_update",
                "egui_graphs_edge_shape_layout",
                "egui_graphs_edge_curve_layout",
                "egui_graphs_edge_label_layout",
            ] {
                tracker.record_synthetic_allocated(1, 1, scope);
            }

            let snapshot = tracker.snapshot();
            for scope in [
                "benchmark_frame_begin",
                "benchmark_apply_action",
                "frame_prepare_selection",
                "emit_diagnostics",
                "puffin_capture",
                "benchmark_frame_end",
                "benchmark_finish_frame",
                "benchmark_end_frame",
                "egui_panel_top_strip_layout",
                "egui_panel_run_navigation_layout",
                "egui_panel_selection_inspector_layout",
                "egui_panel_timeline_layout",
                "egui_panel_central_layout",
                "inspector_scroll_area_layout",
                "inspector_collapsing_header_layout",
                "central_graph_layout_state_restore",
                "central_graph_navigation_prepare",
                "central_graph_widget_build",
                "central_graph_diagnostics_update",
                "egui_graphs_node_update",
                "egui_graphs_node_shape_layout",
                "egui_graphs_edge_update",
                "egui_graphs_edge_shape_layout",
                "egui_graphs_edge_curve_layout",
                "egui_graphs_edge_label_layout",
            ] {
                assert!(
                    snapshot
                        .groups
                        .iter()
                        .any(|group| group.name.as_deref() == Some(scope)),
                    "{scope} should be tracked"
                );
            }
        }
    }
}

#[cfg(all(not(target_arch = "wasm32"), feature = "native-benchmark"))]
pub use tracking::{
    HeapProfileTracker, begin_tracking_window, configure_callsite_sampling, finish_tracking_window,
    install_global_tracker, profile_snapshot as heap_profile_snapshot, with_tracing_subscriber,
};

#[cfg(any(target_arch = "wasm32", not(feature = "native-benchmark")))]
#[derive(Debug, Clone, Default)]
pub struct HeapProfileTracker;

#[cfg(any(target_arch = "wasm32", not(feature = "native-benchmark")))]
pub fn install_global_tracker() -> Result<HeapProfileTracker, Box<dyn std::error::Error>> {
    Ok(HeapProfileTracker)
}

#[cfg(any(target_arch = "wasm32", not(feature = "native-benchmark")))]
pub fn begin_tracking_window(_tracker: &HeapProfileTracker) {}

#[cfg(any(target_arch = "wasm32", not(feature = "native-benchmark")))]
pub fn configure_callsite_sampling(
    _tracker: &HeapProfileTracker,
    _sample_every: Option<u64>,
    _scope_names: &[&str],
) -> Result<(), String> {
    Ok(())
}

#[cfg(any(target_arch = "wasm32", not(feature = "native-benchmark")))]
pub fn finish_tracking_window(_tracker: &HeapProfileTracker) -> HeapProfileSnapshot {
    HeapProfileSnapshot::default()
}

#[cfg(any(target_arch = "wasm32", not(feature = "native-benchmark")))]
pub fn heap_profile_snapshot(_tracker: &HeapProfileTracker) -> HeapProfileSnapshot {
    HeapProfileSnapshot::default()
}

#[cfg(all(not(target_arch = "wasm32"), feature = "native-benchmark"))]
pub fn untracked<R>(f: impl FnOnce() -> R) -> R {
    tracking_allocator::AllocationRegistry::untracked(f)
}

#[cfg(any(target_arch = "wasm32", not(feature = "native-benchmark")))]
pub fn untracked<R>(f: impl FnOnce() -> R) -> R {
    f()
}

#[cfg(all(not(target_arch = "wasm32"), feature = "native-benchmark"))]
pub fn snapshot() -> AllocationSnapshot {
    tracking::process_snapshot()
}

#[cfg(any(target_arch = "wasm32", not(feature = "native-benchmark")))]
pub fn snapshot() -> AllocationSnapshot {
    AllocationSnapshot::default()
}
