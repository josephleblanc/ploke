use std::{collections::BTreeSet, time::Duration};

use serde::{Deserialize, Serialize};

use crate::{
    ModelKey, ProviderKey, Router,
    router_only::{
        ChatCompRequest,
        google::Google,
        openrouter::{OpenRouter, ProviderPreferences},
    },
};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum AttemptTimeout {
    Fixed(Duration),
    Backoff {
        initial: Duration,
        multiplier_percent: u16,
        max: Duration,
    },
}

impl AttemptTimeout {
    pub fn fixed(timeout: Duration) -> Self {
        Self::Fixed(timeout)
    }

    pub fn backoff(initial: Duration, multiplier_percent: u16, max: Duration) -> Self {
        Self::Backoff {
            initial,
            multiplier_percent: multiplier_percent.max(100),
            max: max.max(initial),
        }
    }

    pub fn for_attempt(&self, attempt: u32) -> Duration {
        match self {
            Self::Fixed(timeout) => *timeout,
            Self::Backoff {
                initial,
                multiplier_percent,
                max,
            } => {
                let mut timeout = *initial;
                for _ in 1..attempt.max(1) {
                    timeout = duration_mul_percent(timeout, *multiplier_percent).min(*max);
                }
                timeout
            }
        }
    }

    pub fn first_attempt(&self) -> Duration {
        self.for_attempt(1)
    }
}

impl Default for AttemptTimeout {
    fn default() -> Self {
        Self::fixed(Duration::from_secs(crate::LLM_TIMEOUT_SECS))
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProviderTiming {
    pub attempt_timeout: AttemptTimeout,
    pub max_attempts: u32,
    pub initial_backoff: Duration,
    pub max_backoff: Duration,
    /// Optional total wall-clock budget for a single chat-step's HTTP retry
    /// sequence. When `Some`, the retry loop stops scheduling further retries
    /// once this much time has elapsed since the request began, and each retry
    /// attempt's own timeout is clamped to the remaining budget, so a turn cannot
    /// keep *re-issuing* requests indefinitely while riding out transient
    /// provider errors. `None` leaves the sequence bounded only by
    /// `max_attempts`/`attempt_timeout` (legacy behavior for routers that do not
    /// opt in).
    ///
    /// The first/initial attempt is intentionally not clamped to this budget (it
    /// keeps its full `attempt_timeout`), so a chat step can still overshoot the
    /// budget by up to one first-attempt `attempt_timeout`; retries cannot extend
    /// the sequence further. See `ChatHttpConfig::max_total_elapsed` in
    /// `manager::session` for the precise contract and worst case.
    #[serde(default)]
    pub max_total_elapsed: Option<Duration>,
    pub retry: RetryTuning,
}

impl ProviderTiming {
    pub fn first_timeout(&self) -> Duration {
        self.attempt_timeout.first_attempt()
    }
}

impl Default for ProviderTiming {
    fn default() -> Self {
        Self {
            attempt_timeout: AttemptTimeout::default(),
            max_attempts: 1,
            initial_backoff: Duration::from_millis(250),
            max_backoff: Duration::from_secs(2),
            max_total_elapsed: None,
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

fn duration_mul_percent(duration: Duration, percent: u16) -> Duration {
    let millis = duration
        .as_millis()
        .saturating_mul(u128::from(percent))
        .saturating_div(100)
        .min(u128::from(u64::MAX)) as u64;
    Duration::from_millis(millis)
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OpenRouterCalibrationKey {
    pub model: ModelKey,
    pub provider: Option<ProviderKey>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GoogleCalibrationKey {
    pub model: ModelKey,
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

/// Direct-Google (Vertex OpenAI-compat) retry budget.
///
/// Vertex Gemini text models run on Dynamic Shared Quota, where HTTP 429
/// `RESOURCE_EXHAUSTED` and 503 `UNAVAILABLE` are transient shared-capacity
/// contention rather than a hard per-key cap. Google's own guidance is to ride
/// these out with exponential backoff (honoring any `Retry-After`/RetryInfo
/// delay). The previous shared default (2 attempts, ~250ms fixed backoff) was
/// far too short to survive contention and aborted eval turns immediately.
///
/// These are intentionally Google-scoped via `default_provider_timing` so the
/// OpenRouter path keeps its existing behavior. The backoff schedule itself
/// (exponential + full jitter, capped per attempt by `max_backoff`) lives in
/// `manager::session`.
///
/// `max_total_elapsed` (60s) stops scheduling new retries once 60s has elapsed
/// and clamps each *retry* attempt's timeout to the remaining budget. Because
/// this budget (60s) is smaller than the per-attempt `attempt_timeout` (the
/// shared `LLM_TIMEOUT_SECS` default, 300s), the *first* attempt is left
/// unclamped so a normal single attempt is never truncated — it can legitimately
/// run past the 60s budget. Retries cannot extend the sequence further, so the
/// worst-case wall-clock is one first-attempt `attempt_timeout`.
const GOOGLE_MAX_ATTEMPTS: u32 = 6;
const GOOGLE_INITIAL_BACKOFF: Duration = Duration::from_millis(500);
const GOOGLE_MAX_BACKOFF: Duration = Duration::from_secs(8);
const GOOGLE_RETRY_TOTAL_BUDGET: Duration = Duration::from_secs(60);

impl RouterCalibration for Google {
    type Model = ModelKey;
    type Provider = ();
    type Preferences = ();
    type Key = GoogleCalibrationKey;

    fn default_provider_timing() -> ProviderTiming {
        ProviderTiming {
            max_attempts: GOOGLE_MAX_ATTEMPTS,
            initial_backoff: GOOGLE_INITIAL_BACKOFF,
            max_backoff: GOOGLE_MAX_BACKOFF,
            max_total_elapsed: Some(GOOGLE_RETRY_TOTAL_BUDGET),
            ..ProviderTiming::default()
        }
    }

    fn calibration_input(req: &ChatCompRequest<Self>) -> CalibrationInput<Self> {
        let key = req
            .model_key
            .clone()
            .map(|model| GoogleCalibrationKey { model });

        CalibrationInput {
            model: req.model_key.clone(),
            provider: None,
            provider_preferences: None,
            key,
        }
    }

    fn calibration_key(input: &CalibrationInput<Self>) -> Option<String> {
        let key = input.key.as_ref()?;
        Some(format!("google:{}", key.model))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::router_only::{Router as _, google::Google, openrouter::ChatCompFields};

    #[test]
    fn default_provider_timing_matches_current_chat_timeout_defaults() {
        let timing = OpenRouter::default_provider_timing();

        assert_eq!(
            timing.attempt_timeout.for_attempt(1),
            Duration::from_secs(crate::LLM_TIMEOUT_SECS)
        );
        assert_eq!(
            timing.attempt_timeout.for_attempt(2),
            Duration::from_secs(crate::LLM_TIMEOUT_SECS)
        );
        assert_eq!(timing.max_attempts, 1);
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
    fn google_provider_timing_uses_exponential_dsq_retry_budget() {
        let timing = Google::default_provider_timing();

        assert_eq!(timing.max_attempts, GOOGLE_MAX_ATTEMPTS);
        assert_eq!(timing.initial_backoff, Duration::from_millis(500));
        assert_eq!(timing.max_backoff, Duration::from_secs(8));
        assert_eq!(timing.max_total_elapsed, Some(Duration::from_secs(60)));
        // 429 RESOURCE_EXHAUSTED and 503 UNAVAILABLE remain retryable; fatal 4xx
        // are not added to the retryable set.
        assert!(timing.retry.retry_statuses.contains(&429));
        assert!(timing.retry.retry_statuses.contains(&503));
        assert!(!timing.retry.retry_statuses.contains(&400));
        assert!(!timing.retry.retry_statuses.contains(&404));
    }

    #[test]
    fn openrouter_provider_timing_keeps_unbudgeted_defaults() {
        // The Google-specific budget must not leak into the OpenRouter path.
        let timing = OpenRouter::default_provider_timing();
        assert_eq!(timing.max_attempts, 1);
        assert_eq!(timing.initial_backoff, Duration::from_millis(250));
        assert_eq!(timing.max_backoff, Duration::from_secs(2));
        assert_eq!(timing.max_total_elapsed, None);
    }

    #[test]
    fn google_calibration_input_uses_direct_model_key() {
        let req = Google::default_chat_completion()
            .with_model_str("google/gemini-2.5-flash")
            .expect("model id");

        let input = Google::calibration_input(&req);

        assert_eq!(
            input.model.as_ref().map(ToString::to_string).as_deref(),
            Some("google/gemini-2.5-flash")
        );
        assert!(input.provider.is_none());
        assert!(input.provider_preferences.is_none());
        assert_eq!(
            Google::calibration_key(&input).as_deref(),
            Some("google:google/gemini-2.5-flash")
        );
    }
}
