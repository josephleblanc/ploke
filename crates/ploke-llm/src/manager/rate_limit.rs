//! Per-router request rate limiter for outbound LLM API calls.
//!
//! # Why this exists
//!
//! Vertex AI (the route Ploke uses for live Gemini runs) imposes a
//! **Dynamic Shared Quota (DSQ)** on Gemini 2.5+ / 3.x models. The
//! cap is per-region, dynamic, and not directly increaseable via the
//! Cloud Console; it grows only with rolling 30-day paid spend. When
//! the cap is hit, the API returns `HTTP 429` with body
//! `{"error":{"status":"RESOURCE_EXHAUSTED", ...}}`.
//!
//! A live sustained-RPM probe against
//! `cs-poc-gtxw7jmtfuwfsiauziui9yx / us-central1` on 2026-06-11
//! measured:
//!
//! - `gemini-2.5-flash` clean sustained: **~360 RPM** (5-6 RPS, 0
//!   failures on a fresh 60s window).
//! - `gemini-2.5-pro`  clean sustained: **~180 RPM** (3 RPS, 0
//!   failures on a fresh 60s window).
//! - Combined warm: ~500-540 RPM (shared regional DSQ pool).
//!
//! Burst ceiling (500 concurrent curl): 376/500 succeed, 124/500 hit
//! 429. Short-window headroom exists but DSQ reverts.
//!
//! 1.5-flash / 1.5-pro / 2.0-flash / 3.x preview are all
//! `404 NOT_FOUND` on this project — retired or not enabled — so
//! "downgrade to an older model" is not an actionable lever. The
//! only frontier-class models reachable on this project are
//! 2.5-flash and 2.5-pro.
//!
//! Without a client-side cap, parallel `chat_step` calls in
//! `prototype1-state`'s parent/child fan-out routinely drive RPS
//! past the DSQ wall, returning 429 mid-experiment. The 429s
//! swallow real model errors (they get logged as
//! `RESOURCE_EXHAUSTED` and look identical to live-API failures)
//! and have been a serious time-waster in past sessions.
//!
//! # What this does
//!
//! On every call to [`acquire()`], this module:
//!
//! 1. Reads the configured RPM (process-global `OnceLock<u32>`,
//!    initialised from env on first use).
//! 2. Computes the per-call minimum interval
//!    `interval = 60_000ms / rpm`.
//! 3. Acquires a `tokio::sync::Semaphore` permit (capacity = 1)
//!    and waits until `now - last_acquire >= interval`. The
//!    returned [`Permit`] releases on drop.
//!
//! RPM=0 disables pacing (permits still bounded by semaphore
//! capacity).
//!
//! # How to configure
//!
//! Set `PLOKE_GOOGLE_CHAT_RPM` before the binary starts. Read once
//! per process on first use; mutating at runtime has no effect.
//!
//! | Env var | Default (RPM) | Notes |
//! |---|---|---|
//! | `PLOKE_GOOGLE_CHAT_RPM` | `300` | Sized for gemini-2.5-flash @ ~360 RPM DSQ floor with ~20% safety margin. |
//! | `PLOKE_OPENROUTER_CHAT_RPM` | `0` (unlimited) | OpenRouter has its own per-account limits, not DSQ. |
//!
//! To raise the cap, file a **Quota Increase Request (QIR)** for
//! the project in the Cloud Console (IAM & Admin → Quotas, filter on
//! the Vertex AI metric
//! `aiplatform.googleapis.com/generate_content_requests_per_minute_per_base_model`).
//! QIRs are typically approved within 2-5 business days for a
//! Standard PayGo account.
//!
//! # References
//!
//! Citations for the rate-limit mechanics and the 429 behaviour we
//! work around:
//!
//! - Dynamic Shared Quota on Vertex AI (canonical doc):
//!   <https://cloud.google.com/vertex-ai/generative-ai/docs/resources/dynamic-shared-quota>
//!   > "With DSQ, there are no predefined quota limits on your
//!   > usage."
//! - Vertex AI quotas and system limits:
//!   <https://cloud.google.com/vertex-ai/generative-ai/docs/quotas>
//!   > "Gemini throughput: Depends on model and consumption option."
//!   > (Vertex does NOT publish per-model RPM defaults.)
//! - Purchase Provisioned Throughput (alternative to relying on DSQ):
//!   <https://cloud.google.com/vertex-ai/generative-ai/docs/purchase-provisioned-throughput>
//! - Calculate Provisioned Throughput requirements (GSU math):
//!   <https://docs.cloud.google.com/vertex-ai/generative-ai/docs/provisioned-throughput/measure-provisioned-throughput>
//! - Gemini API rate limits and Tier 1/2/3 table (parallel route on
//!   `ai.google.dev`; tier system differs from Vertex DSQ):
//!   <https://ai.google.dev/gemini-api/docs/rate-limits>
//! - Free trial credits do NOT unlock tier upgrades:
//!   <https://discuss.ai.google.dev/t/free-trial-credits-not-applied-to-gemini-api-charges-requesting-billing-adjustment-refund/143336>
//! - Tier-2-stuck-on-Tier-1 example (paid spend required, not credits):
//!   <https://discuss.ai.google.dev/t/tier-2-upgrade-not-working-despite-meeting-all-requirements-im-still-on-tier-1/146946>
//! - Local live-probe notes for `cs-poc-gtxw7jmtfuwfsiauziui9yx`:
//!   `/home/team_ploke_dev/work/notes/ploke-vm/rate-limits-2026-06-11.md`
//! - Ploke-loop 429 blocker doc (the original symptom in Ploke's
//!   own docs): `docs/active/bugs/2026-06-09-prototype1-broad-child-google-429-zero-admission.md`
//!   on `feature/ploke-loop` (commit `1d691469`).

use std::sync::{Arc, OnceLock};
use std::time::{Duration, Instant};

use tokio::sync::{Mutex, OwnedSemaphorePermit, Semaphore};

/// Per-call minimum interval for a given RPM.
/// Returns `None` if pacing is disabled (rpm == 0).
fn interval_for_rpm(rpm: u32) -> Option<Duration> {
    if rpm == 0 {
        None
    } else {
        // 60_000 ms / rpm = per-call gap. u128 avoids overflow at
        // extreme rpm (u32::MAX = 14 µs, fits).
        let ms = 60_000u128 / u128::from(rpm);
        Some(Duration::from_millis(ms as u64))
    }
}

/// Per-router limiter state.
struct RouterLimiter {
    rpm: u32,
    permits: Arc<Semaphore>,
    /// Timestamp of the last completed acquire. Guarded by a Mutex
    /// so concurrent acquire calls serialise on this field.
    last_acquire: Mutex<Instant>,
}

impl RouterLimiter {
    fn new(rpm: u32) -> Self {
        Self {
            rpm,
            // Capacity 1: only one in-flight request at a time.
            // Strict concurrency; RPM pacing is the primary
            // backpressure, and serial in-flight makes 429 bursts
            // structurally impossible below the cap.
            permits: Arc::new(Semaphore::new(1)),
            // Initialise to "long ago" so the first call does
            // not wait.
            last_acquire: Mutex::new(Instant::now() - Duration::from_secs(3600)),
        }
    }

    /// Block until it is safe to issue another request, then
    /// return a guard that releases the permit on drop.
    async fn acquire(&self) -> Permit {
        let permit = Arc::clone(&self.permits)
            .acquire_owned()
            .await
            .expect("Semaphore is never closed");
        if let Some(interval) = interval_for_rpm(self.rpm) {
            let mut last = self.last_acquire.lock().await;
            let elapsed = last.elapsed();
            if elapsed < interval {
                let wait = interval - elapsed;
                tokio::time::sleep(wait).await;
            }
            *last = Instant::now();
        }
        Permit { _permit: permit }
    }
}

/// RAII permit guard. Releases the rate-limit slot on drop.
pub struct Permit {
    _permit: OwnedSemaphorePermit,
}

/// Acquire the rate-limit slot. Awaitable. The returned [`Permit`]
/// releases the slot when dropped; bind it to the request scope.
///
/// Reads the configured RPM from env on first use; subsequent
/// env-var changes have no effect.
pub async fn acquire() -> Permit {
    let limiter = global_limiter();
    limiter.acquire().await
}

/// Process-global limiter singleton. Per-router keying is reserved
/// for future work; for now a single limiter governs all routes
/// (the default RPM is sized for the worst case — Gemini).
fn global_limiter() -> &'static RouterLimiter {
    static GLOBAL: OnceLock<RouterLimiter> = OnceLock::new();
    GLOBAL.get_or_init(|| RouterLimiter::new(load_default_rpm()))
}

/// Load the default RPM from env. Order of preference:
/// 1. `PLOKE_GOOGLE_CHAT_RPM`
/// 2. `PLOKE_OPENROUTER_CHAT_RPM`
/// 3. Hard default 300 (sized for gemini-2.5-flash @ ~360 RPM
///    DSQ floor with ~20% safety margin).
fn load_default_rpm() -> u32 {
    for var in ["PLOKE_GOOGLE_CHAT_RPM", "PLOKE_OPENROUTER_CHAT_RPM"] {
        if let Ok(raw) = std::env::var(var) {
            match raw.trim().parse::<u32>() {
                Ok(v) => return v,
                Err(_) => eprintln!(
                    "ploke-llm rate_limit: {var}={raw:?} is not a u32; trying next source"
                ),
            }
        }
    }
    300
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn interval_for_rpm_zero_disables() {
        assert_eq!(interval_for_rpm(0), None);
    }

    #[test]
    fn interval_for_rpm_one_is_one_minute() {
        assert_eq!(interval_for_rpm(1), Some(Duration::from_millis(60_000)));
    }

    #[test]
    fn interval_for_rpm_300_is_200ms() {
        assert_eq!(interval_for_rpm(300), Some(Duration::from_millis(200)));
    }

    #[test]
    fn interval_for_rpm_360_truncates_to_166ms() {
        // 60_000 / 360 = 166.66 → 166 ms (truncated).
        assert_eq!(interval_for_rpm(360), Some(Duration::from_millis(166)));
    }

    #[test]
    fn interval_for_rpm_max_does_not_overflow() {
        let d = interval_for_rpm(u32::MAX).expect("u32::MAX is non-zero");
        assert!(d.as_millis() < 1_000);
    }

    #[tokio::test(flavor = "current_thread")]
    async fn acquire_blocks_for_min_interval() {
        // 60_000 RPM = 1ms per call. Two back-to-back acquires
        // should take at least 1ms in real wall time. This is a
        // loose test — wall-time assertions are inherently flaky
        // — but it catches a regression where pacing is
        // accidentally disabled.
        let lim = RouterLimiter::new(60_000);
        let t0 = Instant::now();
        let g1 = lim.acquire().await;
        drop(g1);
        let _g2 = lim.acquire().await;
        let elapsed = t0.elapsed();
        assert!(
            elapsed >= Duration::from_millis(1),
            "expected ≥1ms, got {elapsed:?}"
        );
    }

    #[tokio::test(flavor = "current_thread")]
    async fn acquire_does_not_pace_when_rpm_zero() {
        // RPM=0 disables pacing. Two back-to-back acquires with
        // a dropped permit between them should return promptly
        // (no 60_000/rpm wait).
        let lim = RouterLimiter::new(0);
        let t0 = Instant::now();
        let g1 = lim.acquire().await;
        drop(g1);
        let _g2 = lim.acquire().await;
        let paced = t0.elapsed();
        assert!(
            paced < Duration::from_millis(50),
            "RPM=0 should not pace; took {paced:?}"
        );
    }
}
