//! Eval-owned carrier boundary for a headless `ploke-tui` edit attempt.
//!
//! This module does not run chat sessions, route model calls, or duplicate the
//! `ploke-tui` app harness. It names the small amount of structure that
//! `ploke-eval` needs around the vanilla headless TUI path: an admitted request,
//! one or more attempts, observed event summaries, retry policy, and terminal
//! outcomes. `ploke-tui` remains the executor; `ploke-eval` owns the bounded
//! grant, surface check, and durable projection of what happened.

use std::{
    collections::{HashMap, VecDeque},
    fs,
    marker::PhantomData,
    path::{Path, PathBuf},
    sync::{Arc, Mutex, mpsc::Receiver},
    time::{Duration, Instant},
};

use ploke_llm::{
    ModelId, ProviderKey,
    manager::RecordedResponse,
    router_only::{RouterVariants, google::Google, openrouter::OpenRouter},
};
use ploke_records::{
    agent_turn::{
        AgentTurnArtifactRecord, MessageSnapshotRecord, ModelRouteRecord, ObservedTurnEventRecord,
        PatchArtifactRecord, ToolCompletedRecord, ToolFailedRecord, ToolRequestRecord,
        TurnFinishedRecord,
    },
    llm_response::RawFullResponseRecord,
};
use ploke_tui::app::commands::harness::TestAppAccessor;
use serde::{Deserialize, Serialize};
use thiserror::Error;
use tokio::sync::oneshot;
use uuid::Uuid;

use super::{
    ArtifactDelta,
    harness_request::{
        BroadEditPolicy, EvidenceRoot, EvidenceRootKind, EvidenceRootLocation, contract, request,
    },
    surface, tui,
};

#[cfg(test)]
use super::harness_request::{AttachedReport, EvidenceRole};

pub(super) const MAX_DEBUG_RELAY_EVENTS: usize = 128;
pub(super) const MAX_DEBUG_RELAY_EVENT_CHARS: usize = 2_000;
pub(super) const MAX_EVIDENCE_EVENT_CHARS: usize = 1_000;
pub(super) const MAX_PROMPT_MESSAGE_PREVIEWS: usize = 8;
pub(super) const MAX_PROMPT_MESSAGE_PREVIEW_CHARS: usize = 500;
pub(super) const MAX_RAG_PART_PREVIEWS: usize = 8;
pub(super) const LIVE_TRACE_ENV: &str = "PLOKE_EVAL_HEADLESS_TUI_LIVE";
pub(super) const POST_APPLY_STATUS_TIMEOUT_SECS: u64 = 120;
pub(super) const POST_APPLY_INDEX_TIMEOUT_SECS: u64 = 180;
pub(super) const POST_APPLY_INDEX_START_GRACE_MS: u64 = 2_000;

pub(crate) mod state {
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub(crate) enum Ready {}

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub(crate) enum Running {}

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub(crate) enum Done {}
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ModelSelection {
    model_id: ModelId,
    provider: Option<ProviderKey>,
    router: RouterVariants,
}

impl ModelSelection {
    pub(crate) fn openrouter(model_id: ModelId, provider: Option<ProviderKey>) -> Self {
        Self {
            model_id,
            provider,
            router: RouterVariants::OpenRouter(OpenRouter),
        }
    }

    pub(crate) fn direct_google(model_id: ModelId) -> Self {
        Self {
            model_id,
            provider: None,
            router: RouterVariants::Google(Google),
        }
    }

    pub(crate) fn model_id(&self) -> &ModelId {
        &self.model_id
    }

    pub(crate) fn provider(&self) -> Option<&ProviderKey> {
        self.provider.as_ref()
    }

    pub(crate) fn router(&self) -> RouterVariants {
        self.router
    }

    pub(crate) fn model_route_record(&self) -> ModelRouteRecord {
        match self.router {
            RouterVariants::Google(_) => ModelRouteRecord {
                route_source: "direct_google".to_string(),
                router: "google".to_string(),
                provider_slug: self
                    .provider
                    .as_ref()
                    .map(|provider| provider.slug.as_str().to_string()),
                endpoint_host: Some("aiplatform.googleapis.com".to_string()),
            },
            RouterVariants::OpenRouter(_) => ModelRouteRecord {
                route_source: "openrouter".to_string(),
                router: "openrouter".to_string(),
                provider_slug: self
                    .provider
                    .as_ref()
                    .map(|provider| provider.slug.as_str().to_string()),
                endpoint_host: Some("openrouter.ai".to_string()),
            },
            RouterVariants::Anthropic(_) => ModelRouteRecord {
                route_source: "anthropic".to_string(),
                router: "anthropic".to_string(),
                provider_slug: self
                    .provider
                    .as_ref()
                    .map(|provider| provider.slug.as_str().to_string()),
                endpoint_host: None,
            },
        }
    }
}

mod harness_io;
mod tui_bridge;

pub(crate) use harness_io::*;
pub(crate) use tui_bridge::{
    run_headless, run_headless_with_model, run_headless_with_model_capture_responses,
};

#[cfg(test)]
pub(in crate::cli::prototype1_state::edit_surface::tui_adapter) use tui_bridge::{
    AppliedItem, AttemptEnd, Candidate, LiveObserver, StagedItem, ToolBatch, attempt_prompt,
    classify_applied_terminal, classify_paths, command_display_matches, contract_cargo_args,
    drain_response_records, evidence_read_roots, next_event, policy_repair_prompt,
    provider_failure_from_message, provider_unavailable_reason, record_batch_terminal,
    record_post_approval_indeterminate, retry_feedback, run_attempt, select_disjoint,
    sparse_search_refresh_enabled, start_attempt_runtime, submit_prompt, terminal_ids,
    timeout_terminal_for_run, turn_aborted_after_apply_terminal, validation_command_display,
    wait_for_refresh,
};

#[cfg(test)]
pub(in crate::cli::prototype1_state::edit_surface::tui_adapter) use harness_io::display_cargo_command;

#[cfg(test)]
#[path = "tests.rs"]
mod tests;
