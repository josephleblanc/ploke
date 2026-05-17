//! Process-wide allocation counters used by native benchmark reports.

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
            scope: "process-wide; excludes GPU and driver memory".to_owned(),
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

#[cfg(all(not(target_arch = "wasm32"), feature = "native-benchmark"))]
mod counting {
    use std::alloc::{GlobalAlloc, Layout, System};
    use std::sync::atomic::{AtomicU64, Ordering};

    use super::AllocationSnapshot;

    static ALLOCATION_COUNT: AtomicU64 = AtomicU64::new(0);
    static DEALLOCATION_COUNT: AtomicU64 = AtomicU64::new(0);
    static ALLOCATED_BYTES: AtomicU64 = AtomicU64::new(0);
    static DEALLOCATED_BYTES: AtomicU64 = AtomicU64::new(0);

    pub struct CountingAllocator;

    unsafe impl GlobalAlloc for CountingAllocator {
        unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
            let ptr = unsafe { System.alloc(layout) };
            if !ptr.is_null() {
                ALLOCATION_COUNT.fetch_add(1, Ordering::Relaxed);
                ALLOCATED_BYTES.fetch_add(layout.size() as u64, Ordering::Relaxed);
            }
            ptr
        }

        unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
            let ptr = unsafe { System.alloc_zeroed(layout) };
            if !ptr.is_null() {
                ALLOCATION_COUNT.fetch_add(1, Ordering::Relaxed);
                ALLOCATED_BYTES.fetch_add(layout.size() as u64, Ordering::Relaxed);
            }
            ptr
        }

        unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
            unsafe { System.dealloc(ptr, layout) };
            DEALLOCATION_COUNT.fetch_add(1, Ordering::Relaxed);
            DEALLOCATED_BYTES.fetch_add(layout.size() as u64, Ordering::Relaxed);
        }

        unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
            let new_ptr = unsafe { System.realloc(ptr, layout, new_size) };
            if !new_ptr.is_null() {
                DEALLOCATION_COUNT.fetch_add(1, Ordering::Relaxed);
                DEALLOCATED_BYTES.fetch_add(layout.size() as u64, Ordering::Relaxed);
                ALLOCATION_COUNT.fetch_add(1, Ordering::Relaxed);
                ALLOCATED_BYTES.fetch_add(new_size as u64, Ordering::Relaxed);
            }
            new_ptr
        }
    }

    pub fn snapshot() -> AllocationSnapshot {
        AllocationSnapshot {
            enabled: true,
            allocation_count: ALLOCATION_COUNT.load(Ordering::Relaxed),
            deallocation_count: DEALLOCATION_COUNT.load(Ordering::Relaxed),
            allocated_bytes: ALLOCATED_BYTES.load(Ordering::Relaxed),
            deallocated_bytes: DEALLOCATED_BYTES.load(Ordering::Relaxed),
        }
    }

    pub use CountingAllocator as GlobalCountingAllocator;
}

#[cfg(all(not(target_arch = "wasm32"), feature = "native-benchmark"))]
pub use counting::GlobalCountingAllocator;

#[cfg(all(not(target_arch = "wasm32"), feature = "native-benchmark"))]
pub fn snapshot() -> AllocationSnapshot {
    counting::snapshot()
}

#[cfg(any(target_arch = "wasm32", not(feature = "native-benchmark")))]
pub fn snapshot() -> AllocationSnapshot {
    AllocationSnapshot::default()
}
