//! Consolidated timeout knobs for headless edit harness attempts.

use std::time::{Duration, Instant};

use super::super::Budget;

/// Headless broad-harness slots run request-declared cargo validation against a
/// cold per-slot `target/` dir. The default 60s `cargo check` budget is too
/// small for a cold compile of `ploke-eval`, so harness validation was being
/// killed before it could pass.
pub const HEADLESS_VALIDATION_CARGO_CHECK_TIMEOUT_SECS: u64 = 300;
pub const HEADLESS_VALIDATION_CARGO_TEST_TIMEOUT_SECS: u64 = 600;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Timeouts {
    /// Whole attempt wall clock (from published contract or CLI override).
    pub attempt_secs: u64,
    /// Poll for proposal apply status after approval.
    pub post_apply_status_secs: u64,
    /// Wait for indexing output after apply.
    pub post_apply_index_secs: u64,
    /// Grace before treating missing index output as acceptable.
    pub post_apply_index_start_grace_ms: u64,
    pub validation_cargo_check_secs: u64,
    pub validation_cargo_test_secs: u64,
}

impl Timeouts {
    pub(crate) fn from_budget(budget: Budget) -> Self {
        Self {
            attempt_secs: budget.timeout_secs(),
            ..Self::default()
        }
    }

    pub(crate) fn attempt_deadline(&self, started: Instant) -> Instant {
        started + Duration::from_secs(self.attempt_secs)
    }

    /// Compose an outer attempt deadline with an inner phase budget.
    pub(crate) fn phase_deadline(&self, outer: Instant, inner: Duration) -> Instant {
        let inner = Instant::now() + inner;
        outer.min(inner)
    }

    pub(crate) fn post_apply_status_duration(&self) -> Duration {
        Duration::from_secs(self.post_apply_status_secs)
    }

    pub(crate) fn post_apply_index_duration(&self) -> Duration {
        Duration::from_secs(self.post_apply_index_secs)
    }

    pub(crate) fn post_apply_index_start_grace(&self) -> Duration {
        Duration::from_millis(self.post_apply_index_start_grace_ms)
    }
}

impl Default for Timeouts {
    fn default() -> Self {
        Self {
            attempt_secs: 900,
            post_apply_status_secs: super::super::POST_APPLY_STATUS_TIMEOUT_SECS,
            post_apply_index_secs: super::super::POST_APPLY_INDEX_TIMEOUT_SECS,
            post_apply_index_start_grace_ms: super::super::POST_APPLY_INDEX_START_GRACE_MS,
            validation_cargo_check_secs: HEADLESS_VALIDATION_CARGO_CHECK_TIMEOUT_SECS,
            validation_cargo_test_secs: HEADLESS_VALIDATION_CARGO_TEST_TIMEOUT_SECS,
        }
    }
}
