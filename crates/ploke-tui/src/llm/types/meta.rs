use std::time::Duration;

use crate::llm::response::{FinishReason, TokenUsage};

use super::*;

/// Metadata about LLM execution
#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct LLMMetadata {
    /// Actual model used (may differ from request)
    pub(crate) model: String,

    /// Token usage metrics
    pub(crate) usage: TokenUsage,

    /// Generation completion reason
    pub(crate) finish_reason: FinishReason,

    /// Time spent in LLM processing
    pub(crate) processing_time: Duration,

    /// Cost calculation in USD
    pub(crate) cost: f64,

    /// Performance metrics
    pub(crate) performance: PerformanceMetrics,
}

/// Performance metrics
#[derive(Debug, Clone, Serialize, Deserialize, Copy)]
pub(crate) struct PerformanceMetrics {
    pub(crate) tokens_per_second: f32,
    pub(crate) time_to_first_token: Duration,
    pub(crate) queue_time: Duration,
}

