
use uuid::Uuid;

#[derive(Clone, Debug)]
pub enum Event {
    Request {
        parent_id: Uuid,
        prompt: String,
        parameters: LLMParameters,
    },
    Response {
        parent_id: Uuid,
        content: String,
        metadata: LLMMetadata,
    },
}

// crates/ploke-tui/src/events/llm.rs
use std::time::Duration;
use serde::{Serialize, Deserialize};

/// Parameters for controlling LLM generation behavior
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LLMParameters {
    /// LLM model identifier (e.g., "gpt-4-turbo", "claude-3-opus")
    pub model: String,
    
    /// Sampling temperature (0.0 = deterministic, 1.0 = creative)
    #[serde(default = "default_temperature")]
    pub temperature: f32,
    
    /// Top-p nucleus sampling threshold (0.0-1.0)
    #[serde(default = "default_top_p")]
    pub top_p: f32,
    
    /// Maximum tokens to generate (None = model maximum)
    pub max_tokens: Option<u32>,
    
    /// Presence penalty (-2.0 to 2.0)
    #[serde(default)]
    pub presence_penalty: f32,
    
    /// Frequency penalty (-2.0 to 2.0)
    #[serde(default)]
    pub frequency_penalty: f32,
    
    /// Stop sequences to halt generation
    #[serde(default)]
    pub stop_sequences: Vec<String>,
    
    /// Enable parallel function calling
    #[serde(default = "default_true")]
    pub parallel_tool_calls: bool,
    
    /// JSON mode enforcement
    #[serde(default)]
    pub response_format: ResponseFormat,
    
    /// Safety/system controls
    #[serde(default)]
    pub safety_settings: SafetySettings,
}

/// Metadata about LLM execution
#[derive(Debug, Clone, Serialize)]
pub struct LLMMetadata {
    /// Actual model used (may differ from request)
    pub model: String,
    
    /// Token usage metrics
    pub usage: TokenUsage,
    
    /// Generation completion reason
    pub finish_reason: FinishReason,
    
    /// Time spent in LLM processing
    pub processing_time: Duration,
    
    /// Content safety scores
    #[serde(skip_serializing_if = "Option::is_none")]
    pub safety_scores: Option<SafetyScores>,
    
    /// Cost calculation in USD
    pub cost: f64,
    
    /// Performance metrics
    pub performance: PerformanceMetrics,
}

// --- Supporting Types ---

/// Response format specification
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub enum ResponseFormat {
    #[default]
    Text,
    JsonObject,
}

/// Safety control settings
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SafetySettings {
    pub hate: u8,           // 0-8 (0=disable, 4=moderate, 7=strict)
    pub harassment: u8,
    pub self_harm: u8,
    #[serde(rename = "sexual")]
    pub sexual_content: u8,
}

/// Token usage statistics
#[derive(Debug, Clone, Serialize)]
pub struct TokenUsage {
    pub prompt_tokens: u32,
    pub completion_tokens: u32,
    pub total_tokens: u32,
}

/// Generation completion reasons
#[derive(Debug, Clone, Serialize)]
pub enum FinishReason {
    Stop,           // Natural stop sequence
    Length,         // Max tokens reached
    ContentFilter,  // Blocked by safety system
    ToolCalls,      // Stopped for tool execution
    Timeout,        // Processing time exceeded
    Error(String),  // Error description
}

/// Content safety assessment
#[derive(Debug, Clone, Serialize)]
pub struct SafetyScores {
    pub hate: f32,            // 0-1 probability score
    pub harassment: f32,
    pub self_harm: f32,
    pub sexual_content: f32,
}

/// Performance metrics
#[derive(Debug, Clone, Serialize)]
pub struct PerformanceMetrics {
    pub tokens_per_second: f32,
    pub time_to_first_token: Duration,
    pub queue_time: Duration,
}

// --- Default Implementations ---
impl Default for LLMParameters {
    fn default() -> Self {
        Self {
            model: "gpt-4-turbo".to_string(),
            temperature: default_temperature(),
            top_p: default_top_p(),
            max_tokens: None,
            presence_penalty: 0.0,
            frequency_penalty: 0.0,
            stop_sequences: vec!["\n".to_string()],
            parallel_tool_calls: true,
            response_format: Default::default(),
            safety_settings: Default::default(),
        }
    }
}

fn default_temperature() -> f32 { 0.7 }
fn default_top_p() -> f32 { 0.9 }
fn default_true() -> bool { true }
