use std::{
    collections::{BTreeMap, BTreeSet},
    fs, io,
    path::Path,
    time::Duration,
};

use serde::{Deserialize, Serialize};

use crate::{
    HttpBodyFailure, ModelKey, ProviderAttempt, ProviderKey, ProviderRetryDecision, Router,
    router_only::{
        ChatCompRequest,
        openrouter::{OpenRouter, ProviderPreferences},
    },
};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProviderTiming {
    pub timeout: Duration,
    pub max_attempts: u32,
    pub initial_backoff: Duration,
    pub max_backoff: Duration,
    pub retry: RetryTuning,
}

impl ProviderTiming {
    pub fn with_timeout_ceiling(mut self, ceiling: Duration) -> Self {
        self.timeout = self.timeout.min(ceiling);
        self
    }
}

impl Default for ProviderTiming {
    fn default() -> Self {
        Self {
            timeout: Duration::from_secs(crate::LLM_TIMEOUT_SECS),
            max_attempts: 3,
            initial_backoff: Duration::from_millis(250),
            max_backoff: Duration::from_secs(2),
            retry: RetryTuning::default(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RetryTuning {
    pub retry_send_timeout: bool,
    pub retry_send_failure: bool,
    pub retry_body_timeout: bool,
    pub retry_body_read_failed: bool,
    pub body_timeout_retry_limit: Option<u32>,
    pub retry_statuses: BTreeSet<u16>,
}

impl Default for RetryTuning {
    fn default() -> Self {
        Self {
            retry_send_timeout: true,
            retry_send_failure: true,
            retry_body_timeout: true,
            retry_body_read_failed: true,
            body_timeout_retry_limit: None,
            retry_statuses: [408, 409, 425, 429, 500, 502, 503, 504]
                .into_iter()
                .collect(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct CalibrationInput<R: RouterCalibration> {
    pub model: Option<R::Model>,
    pub provider: Option<R::Provider>,
    pub provider_preferences: Option<R::Preferences>,
    pub key: Option<R::Key>,
}

impl<R: RouterCalibration> Default for CalibrationInput<R> {
    fn default() -> Self {
        Self {
            model: None,
            provider: None,
            provider_preferences: None,
            key: None,
        }
    }
}

pub trait RouterCalibration: Router {
    type Model: Clone + std::fmt::Debug + PartialEq + Eq;
    type Provider: Clone + std::fmt::Debug + PartialEq + Eq;
    type Preferences: Clone + std::fmt::Debug;
    type Key: Clone + std::fmt::Debug + PartialEq + Eq;

    fn calibration_input(req: &ChatCompRequest<Self>) -> CalibrationInput<Self> {
        let _ = req;
        CalibrationInput::default()
    }

    fn calibration_key(_input: &CalibrationInput<Self>) -> Option<String> {
        None
    }

    fn default_provider_timing() -> ProviderTiming {
        ProviderTiming::default()
    }

    fn resolve_provider_timing(_input: CalibrationInput<Self>) -> ProviderTiming {
        Self::default_provider_timing()
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct CalibrationStore {
    pub version: u32,
    pub entries: BTreeMap<String, CalibrationEntry>,
}

impl CalibrationStore {
    pub const VERSION: u32 = 1;

    pub fn load_from_path(path: &Path) -> io::Result<Self> {
        match fs::read_to_string(path) {
            Ok(raw) => serde_json::from_str(&raw).map_err(io::Error::other),
            Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(Self::default()),
            Err(error) => Err(error),
        }
    }

    pub fn save_to_path(&self, path: &Path) -> io::Result<()> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        let tmp = path.with_extension("json.tmp");
        let payload = serde_json::to_vec_pretty(self).map_err(io::Error::other)?;
        fs::write(&tmp, payload)?;
        fs::rename(tmp, path)
    }

    pub fn resolve<R: RouterCalibration>(&self, input: &CalibrationInput<R>) -> ProviderTiming {
        self.resolve_with_tuning(input, &CalibrationTuning::default())
    }

    pub fn resolve_with_tuning<R: RouterCalibration>(
        &self,
        input: &CalibrationInput<R>,
        tuning: &CalibrationTuning,
    ) -> ProviderTiming {
        let default = R::resolve_provider_timing(input.clone());
        self.resolve_with_default(input, default, tuning)
    }

    pub fn resolve_with_default<R: RouterCalibration>(
        &self,
        input: &CalibrationInput<R>,
        default: ProviderTiming,
        tuning: &CalibrationTuning,
    ) -> ProviderTiming {
        let Some(key) = R::calibration_key(input) else {
            return default;
        };
        self.entries
            .get(&key)
            .map(|entry| entry.provider_timing(default.clone(), tuning))
            .unwrap_or(default)
    }

    pub fn record_attempts(
        &mut self,
        key: impl Into<String>,
        provider_timing: &ProviderTiming,
        attempts: &[ProviderAttempt],
    ) {
        self.record_attempts_with_tuning(
            key,
            provider_timing,
            attempts,
            &CalibrationTuning::default(),
        );
    }

    pub fn record_attempts_with_tuning(
        &mut self,
        key: impl Into<String>,
        provider_timing: &ProviderTiming,
        attempts: &[ProviderAttempt],
        tuning: &CalibrationTuning,
    ) {
        if attempts.is_empty() {
            return;
        }
        if self.version == 0 {
            self.version = Self::VERSION;
        }
        let entry = self.entries.entry(key.into()).or_default();
        entry.record(
            provider_timing,
            attempts,
            tuning.clone().normalized().sample_limit,
        );
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CalibrationTuning {
    pub lower_quantile_percent: u8,
    pub upper_quantile_percent: u8,
    pub sample_limit: usize,
    pub min_success_samples: usize,
    pub min_timeout: Duration,
    pub max_timeout: Duration,
    pub min_margin: Duration,
    pub body_timeout_growth_percent: u16,
}

impl CalibrationTuning {
    pub fn new(lower_quantile_percent: u8, upper_quantile_percent: u8) -> Self {
        Self {
            lower_quantile_percent,
            upper_quantile_percent,
            ..Self::default()
        }
        .normalized()
    }

    pub fn normalized(mut self) -> Self {
        self.lower_quantile_percent = self.lower_quantile_percent.min(100);
        self.upper_quantile_percent = self.upper_quantile_percent.min(100);
        if self.lower_quantile_percent > self.upper_quantile_percent {
            std::mem::swap(
                &mut self.lower_quantile_percent,
                &mut self.upper_quantile_percent,
            );
        }
        self.sample_limit = self.sample_limit.max(1);
        self.min_success_samples = self.min_success_samples.max(1);
        if self.min_timeout > self.max_timeout {
            std::mem::swap(&mut self.min_timeout, &mut self.max_timeout);
        }
        self.body_timeout_growth_percent = self.body_timeout_growth_percent.max(100);
        self
    }
}

impl Default for CalibrationTuning {
    fn default() -> Self {
        Self {
            lower_quantile_percent: 50,
            upper_quantile_percent: 80,
            sample_limit: 128,
            min_success_samples: 3,
            min_timeout: Duration::from_secs(20),
            max_timeout: Duration::from_secs(crate::LLM_TIMEOUT_SECS.saturating_mul(2)),
            min_margin: Duration::from_secs(10),
            body_timeout_growth_percent: 150,
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct CalibrationEntry {
    pub observations: u64,
    pub successes: u64,
    pub failures: u64,
    pub send_timeouts: u64,
    pub body_timeouts: u64,
    pub retry_scheduled: u64,
    pub retry_exhausted: u64,
    #[serde(default)]
    pub recent_success_ms: Vec<u64>,
    #[serde(default)]
    pub recent_body_timeout_ms: Vec<u64>,
    #[serde(default)]
    pub recent_send_failure_ms: Vec<u64>,
    pub success_ewma_ms: Option<u64>,
    pub success_max_ms: Option<u64>,
    pub last_timing: Option<ProviderTiming>,
}

impl CalibrationEntry {
    fn record(
        &mut self,
        provider_timing: &ProviderTiming,
        attempts: &[ProviderAttempt],
        sample_limit: usize,
    ) {
        self.observations = self.observations.saturating_add(1);
        self.last_timing = Some(provider_timing.clone());

        let mut saw_success = false;
        for attempt in attempts {
            if matches!(attempt.retry_decision, ProviderRetryDecision::Scheduled) {
                self.retry_scheduled = self.retry_scheduled.saturating_add(1);
            }
            if matches!(attempt.retry_decision, ProviderRetryDecision::Exhausted) {
                self.retry_exhausted = self.retry_exhausted.saturating_add(1);
            }
            if attempt.body_failure == Some(HttpBodyFailure::Timeout) {
                self.body_timeouts = self.body_timeouts.saturating_add(1);
                if let Some(elapsed) = attempt.failed {
                    push_sample(
                        &mut self.recent_body_timeout_ms,
                        duration_ms(elapsed),
                        sample_limit,
                    );
                }
            }
            if attempt.failed.is_some()
                && attempt.status.is_none()
                && attempt.body_failure.is_none()
                && attempt.output_completed.is_none()
            {
                self.send_timeouts = self.send_timeouts.saturating_add(1);
                if let Some(elapsed) = attempt.failed {
                    push_sample(
                        &mut self.recent_send_failure_ms,
                        duration_ms(elapsed),
                        sample_limit,
                    );
                }
            }
            if attempt
                .status
                .is_some_and(|status| (200..300).contains(&status))
                && attempt.failed.is_none()
            {
                saw_success = true;
                if let Some(elapsed) = attempt.output_completed {
                    let elapsed_ms = duration_ms(elapsed);
                    push_sample(&mut self.recent_success_ms, elapsed_ms, sample_limit);
                    self.success_max_ms = Some(
                        self.success_max_ms
                            .map_or(elapsed_ms, |current| current.max(elapsed_ms)),
                    );
                    self.success_ewma_ms = Some(match self.success_ewma_ms {
                        Some(current) => ((current.saturating_mul(3)) + elapsed_ms) / 4,
                        None => elapsed_ms,
                    });
                }
            }
        }

        if saw_success {
            self.successes = self.successes.saturating_add(1);
        } else {
            self.failures = self.failures.saturating_add(1);
        }
    }

    fn provider_timing(
        &self,
        default: ProviderTiming,
        tuning: &CalibrationTuning,
    ) -> ProviderTiming {
        let tuning = tuning.clone().normalized();
        let mut timing = default;
        if self.recent_success_ms.len() >= tuning.min_success_samples {
            let lower = quantile_ms(&self.recent_success_ms, tuning.lower_quantile_percent);
            let upper = quantile_ms(&self.recent_success_ms, tuning.upper_quantile_percent);
            let spread = upper.saturating_sub(lower);
            let margin = Duration::from_millis(spread / 2).max(tuning.min_margin);
            let target = Duration::from_millis(upper).saturating_add(margin);
            timing.timeout = target.max(tuning.min_timeout).min(tuning.max_timeout);
        }
        if self.body_timeouts >= 2 && self.body_timeouts > self.successes {
            let base = self
                .last_timing
                .as_ref()
                .map(|timing| timing.timeout)
                .unwrap_or(timing.timeout);
            let grown = duration_mul_percent(base, tuning.body_timeout_growth_percent)
                .max(base.saturating_add(tuning.min_margin));
            timing.timeout = timing.timeout.max(grown).min(tuning.max_timeout);
            timing.max_attempts = timing.max_attempts.min(2);
            timing.retry.body_timeout_retry_limit = Some(1);
        }
        timing
    }
}

fn duration_ms(duration: Duration) -> u64 {
    duration.as_millis().min(u128::from(u64::MAX)) as u64
}

fn duration_mul_percent(duration: Duration, percent: u16) -> Duration {
    let millis = duration
        .as_millis()
        .saturating_mul(u128::from(percent))
        .saturating_div(100)
        .min(u128::from(u64::MAX)) as u64;
    Duration::from_millis(millis)
}

fn push_sample(samples: &mut Vec<u64>, sample: u64, sample_limit: usize) {
    samples.push(sample);
    let overflow = samples.len().saturating_sub(sample_limit.max(1));
    if overflow > 0 {
        samples.drain(0..overflow);
    }
}

fn quantile_ms(samples: &[u64], percent: u8) -> u64 {
    if samples.is_empty() {
        return 0;
    }
    let mut sorted = samples.to_vec();
    sorted.sort_unstable();
    let index = ((sorted.len() - 1) * usize::from(percent.min(100)) + 50) / 100;
    sorted[index]
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OpenRouterCalibrationKey {
    pub model: ModelKey,
    pub provider: Option<ProviderKey>,
}

impl RouterCalibration for OpenRouter {
    type Model = ModelKey;
    type Provider = ProviderKey;
    type Preferences = ProviderPreferences;
    type Key = OpenRouterCalibrationKey;

    fn calibration_input(req: &ChatCompRequest<Self>) -> CalibrationInput<Self> {
        let provider = req
            .router
            .provider
            .as_ref()
            .and_then(|prefs| {
                prefs
                    .order
                    .as_ref()
                    .and_then(|order| order.first())
                    .or_else(|| {
                        prefs.only.as_ref().and_then(|only| {
                            (only.len() == 1).then(|| only.iter().next()).flatten()
                        })
                    })
            })
            .cloned()
            .map(|slug| ProviderKey { slug });
        let key = req.model_key.clone().map(|model| OpenRouterCalibrationKey {
            model,
            provider: provider.clone(),
        });

        CalibrationInput {
            model: req.model_key.clone(),
            provider,
            provider_preferences: req.router.provider.clone(),
            key,
        }
    }

    fn calibration_key(input: &CalibrationInput<Self>) -> Option<String> {
        let key = input.key.as_ref()?;
        let provider = key
            .provider
            .as_ref()
            .map(|provider| provider.slug.as_str())
            .unwrap_or("any");
        Some(format!("openrouter:{}:provider:{provider}", key.model))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::manager::builders::attempt::ProviderAttemptOutcome;
    use crate::router_only::openrouter::ChatCompFields;

    #[test]
    fn default_provider_timing_matches_current_chat_timeout_defaults() {
        let timing = OpenRouter::default_provider_timing();

        assert_eq!(timing.timeout, Duration::from_secs(crate::LLM_TIMEOUT_SECS));
        assert_eq!(timing.max_attempts, 3);
        assert_eq!(timing.initial_backoff, Duration::from_millis(250));
        assert_eq!(timing.max_backoff, Duration::from_secs(2));
    }

    #[test]
    fn calibration_input_is_router_typed() {
        let model = ModelKey::try_from("moonshotai/kimi-k2").expect("model key");
        let provider = ProviderKey::new("chutes").expect("provider key");
        let input = CalibrationInput::<OpenRouter> {
            model: Some(model.clone()),
            provider: Some(provider.clone()),
            provider_preferences: Some(
                ProviderPreferences::default().with_only([provider.slug.clone()]),
            ),
            key: Some(OpenRouterCalibrationKey {
                model,
                provider: Some(provider),
            }),
        };

        let timing = OpenRouter::resolve_provider_timing(input);

        assert_eq!(timing, OpenRouter::default_provider_timing());
    }

    #[test]
    fn openrouter_calibration_input_uses_single_allowed_provider() {
        let req = OpenRouter::default_chat_completion()
            .with_model_str("x-ai/grok-4-fast")
            .expect("model id")
            .with_router_bundle(
                ChatCompFields::default().with_provider(
                    ProviderPreferences::default()
                        .with_only([ProviderKey::new("xai").expect("provider key").slug]),
                ),
            );

        let input = OpenRouter::calibration_input(&req);

        assert_eq!(
            OpenRouter::calibration_key(&input).as_deref(),
            Some("openrouter:x-ai/grok-4-fast:provider:xai")
        );
    }

    #[test]
    fn calibration_store_persists_observed_timing() {
        let key = "openrouter:test/model:provider:any";
        let attempt = ProviderAttempt {
            request_id: 1,
            attempt: 1,
            max_attempts: 3,
            started_at: Duration::ZERO,
            request_sent: Some(Duration::from_millis(1)),
            headers_received: Some(Duration::from_millis(100)),
            output_started: Some(Duration::from_millis(100)),
            output_progress: None,
            output_completed: Some(Duration::from_secs(12)),
            failed: None,
            status: Some(200),
            response_bytes: Some(128),
            outcome: ProviderAttemptOutcome::Completed,
            failure_phase: None,
            body_failure: None,
            retry_decision: ProviderRetryDecision::None,
            backoff: None,
        };
        let mut store = CalibrationStore::default();

        store.record_attempts(key, &ProviderTiming::default(), &[attempt]);

        let path = std::env::temp_dir().join(format!(
            "ploke-provider-calibration-test-{}.json",
            uuid::Uuid::new_v4()
        ));
        store.save_to_path(&path).expect("save calibration");
        let loaded = CalibrationStore::load_from_path(&path).expect("load calibration");
        let _ = std::fs::remove_file(path);

        let entry = loaded.entries.get(key).expect("stored entry");
        assert_eq!(entry.observations, 1);
        assert_eq!(entry.successes, 1);
        assert_eq!(entry.recent_success_ms, vec![12_000]);
        assert_eq!(entry.success_ewma_ms, Some(12_000));
    }

    #[test]
    fn provider_timing_uses_configurable_success_quantiles() {
        let mut entry = CalibrationEntry::default();
        for seconds in [10, 20, 30, 40, 50] {
            entry.record(
                &ProviderTiming::default(),
                &[successful_attempt(Duration::from_secs(seconds))],
                128,
            );
        }
        let tuning = CalibrationTuning {
            lower_quantile_percent: 20,
            upper_quantile_percent: 80,
            min_success_samples: 1,
            min_timeout: Duration::ZERO,
            max_timeout: Duration::from_secs(120),
            min_margin: Duration::ZERO,
            ..CalibrationTuning::default()
        };

        let timing = entry.provider_timing(ProviderTiming::default(), &tuning);

        assert_eq!(timing.timeout, Duration::from_secs(50));
    }

    fn successful_attempt(output_completed: Duration) -> ProviderAttempt {
        ProviderAttempt {
            request_id: 1,
            attempt: 1,
            max_attempts: 3,
            started_at: Duration::ZERO,
            request_sent: Some(Duration::from_millis(1)),
            headers_received: Some(Duration::from_millis(100)),
            output_started: Some(Duration::from_millis(100)),
            output_progress: None,
            output_completed: Some(output_completed),
            failed: None,
            status: Some(200),
            response_bytes: Some(128),
            outcome: ProviderAttemptOutcome::Completed,
            failure_phase: None,
            body_failure: None,
            retry_decision: ProviderRetryDecision::None,
            backoff: None,
        }
    }

    #[test]
    fn repeated_body_timeouts_cap_body_timeout_retries() {
        let timeout_attempt = ProviderAttempt {
            request_id: 1,
            attempt: 1,
            max_attempts: 3,
            started_at: Duration::ZERO,
            request_sent: Some(Duration::from_millis(1)),
            headers_received: Some(Duration::from_millis(100)),
            output_started: None,
            output_progress: None,
            output_completed: None,
            failed: Some(Duration::from_secs(120)),
            status: Some(200),
            response_bytes: None,
            outcome: ProviderAttemptOutcome::Failed,
            failure_phase: Some(crate::ProviderFailurePhase::Body),
            body_failure: Some(HttpBodyFailure::Timeout),
            retry_decision: ProviderRetryDecision::Exhausted,
            backoff: None,
        };
        let mut store = CalibrationStore::default();

        store.record_attempts(
            "openrouter:test/model:provider:any",
            &ProviderTiming::default(),
            &[timeout_attempt.clone()],
        );
        store.record_attempts(
            "openrouter:test/model:provider:any",
            &ProviderTiming::default(),
            &[timeout_attempt],
        );

        let timing = store.entries["openrouter:test/model:provider:any"]
            .provider_timing(ProviderTiming::default(), &CalibrationTuning::default());

        assert_eq!(timing.timeout, Duration::from_secs(180));
        assert_eq!(timing.max_attempts, 2);
        assert_eq!(timing.retry.body_timeout_retry_limit, Some(1));
    }
}
