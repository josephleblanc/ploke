//! Heap attribution used by native benchmark reports.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

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
    use std::error::Error;
    use std::sync::Arc;
    use std::sync::OnceLock;
    use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
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
        AllocationSnapshot, HeapCallsiteProfile, HeapGroupProfile, HeapProfileSnapshot,
        HeapProfileTotals,
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
                unmatched_deallocations: AtomicU64::new(0),
            }
        }
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

                HeapProfileSnapshot {
                    enabled: self.inner.enabled.load(Ordering::Relaxed),
                    totals: self.inner.totals.snapshot(),
                    groups,
                    callsites: Vec::<HeapCallsiteProfile>::new(),
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

        fn record_allocated(&self, object_size: usize, wrapped_size: usize, group_id: usize) {
            self.inner.totals.add_allocated(object_size, wrapped_size);
            if let Some(scope_index) = current_window_scope_index(group_id) {
                self.inner.scopes[scope_index]
                    .totals
                    .add_allocated(object_size, wrapped_size);
            }
        }

        fn record_deallocated(
            &self,
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
            _addr: usize,
            object_size: usize,
            wrapped_size: usize,
            group_id: AllocationGroupId,
        ) {
            self.record_allocated(object_size, wrapped_size, group_id.as_usize().get());
        }

        fn deallocated(
            &self,
            _addr: usize,
            object_size: usize,
            wrapped_size: usize,
            source_group_id: AllocationGroupId,
            _current_group_id: AllocationGroupId,
        ) {
            self.record_deallocated(object_size, wrapped_size, source_group_id.as_usize().get());
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

    const ROOT_SCOPE_INDEX: usize = 0;
    const ROOT_UI_THREAD_SCOPE_INDEX: usize = 1;
    const ROOT_OTHER_THREAD_SCOPE_INDEX: usize = 2;
    const SCOPE_NAMES: &[&str] = &[
        "root",
        "root_ui_thread",
        "root_other_thread",
        "eframe_run_native",
        "frame_update",
        "benchmark_frame_begin",
        "benchmark_apply_action",
        "frame_prepare_selection",
        "emit_diagnostics",
        "puffin_capture",
        "benchmark_frame_end",
        "benchmark_finish_frame",
        "benchmark_end_frame",
        "top_strip",
        "run_navigation",
        "diagnostics",
        "selection_inspector",
        "central_graph",
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
        "central_graph_widget_add",
        "central_graph_diagnostics_update",
        "egui_graphs_node_update",
        "egui_graphs_node_shape_layout",
        "egui_graphs_edge_update",
        "egui_graphs_edge_shape_layout",
        "egui_graphs_edge_curve_layout",
        "egui_graphs_edge_label_layout",
        "timeline",
        "graph_projection_cache_refresh",
        "egui_text_font_layout",
        "inspector_parent_create_llm_calls",
        "inspector_parent_create_source_status",
        "inspector_run_records",
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
        "inspector_run_record_arm",
        "inspector_run_record_tool_step",
        "inspector_tool_arguments",
        "inspector_tool_raw_arguments",
        "inspector_tool_result",
        "inspector_tool_raw_result",
        "inspector_tool_ui_payload",
        "inspector_tool_ui_details",
        "inspector_tool_error",
        "inspector_tool_argument_edit",
        "inspector_tool_argument_patch",
        "inspector_tool_result_list_dir_details",
        "inspector_tool_result_list_dir_entry",
        "inspector_context",
        "inspector_agent_turn",
        "inspector_graph_edges",
        "inspector_artifact_edges",
        "inspector_patch_debug",
        "inspector_patch_debug_patch",
        "inspector_patch_debug_header",
        "inspector_patch_debug_diff_cache",
        "inspector_patch_debug_diff_cache_hit",
        "inspector_patch_debug_diff_build_text",
        "inspector_patch_debug_diff_highlight",
        "inspector_patch_debug_diff_layout",
        "inspector_patch_debug_diff_store",
        "inspector_patch_debug_diff_widget",
        "inspector_patch_debug_details_header",
        "inspector_patch_debug_touch",
        "inspector_patch_details",
        "inspector_source_refs",
        "inspector_source_refs_iter",
        "inspector_source_refs_row",
        "inspector_artifact_ids",
    ];

    fn scope_index(name: &str) -> Option<usize> {
        SCOPE_NAMES.iter().position(|scope| *scope == name)
    }

    fn root_scope_index() -> usize {
        let current = thread::current().id();
        match UI_THREAD_ID.get() {
            Some(ui_thread) if ui_thread == &current => ROOT_UI_THREAD_SCOPE_INDEX,
            Some(_) => ROOT_OTHER_THREAD_SCOPE_INDEX,
            None => ROOT_SCOPE_INDEX,
        }
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
    HeapProfileTracker, begin_tracking_window, finish_tracking_window, install_global_tracker,
    profile_snapshot as heap_profile_snapshot, with_tracing_subscriber,
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
