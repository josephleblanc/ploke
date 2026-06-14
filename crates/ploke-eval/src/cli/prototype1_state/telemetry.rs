//! Runtime-scoped telemetry projection for Prototype 1.
//!
//! This module does not grant authority. It projects already-admitted runtime
//! context into tracing spans so downstream events can be attributed without
//! duplicating identity into every telemetry record.

use tracing::Span;

// these were only being used in the `child` accessor below, but `child` was never actually being used
// use crate::intervention::Prototype1NodeRecord;
// use super::event::RuntimeId;

use super::{
    edit_surface::harness_request::PublishedBroadHarnessRequest, identity::ParentIdentity,
};

/// Trace context derived from an admitted Prototype 1 runtime path.
#[derive(Debug, Clone)]
pub(crate) struct RuntimeTelemetry {
    role: &'static str,
    runtime_phase: &'static str,
    campaign_id: String,
    node_id: String,
    branch_id: String,
    generation: u32,
    runtime_id: Option<String>,
}

impl RuntimeTelemetry {
    pub(crate) fn parent(identity: &ParentIdentity, runtime_phase: &'static str) -> Self {
        Self {
            role: "parent",
            runtime_phase,
            campaign_id: identity.campaign_id().to_string(),
            node_id: identity.node_id().to_string(),
            branch_id: identity.branch_id().to_string(),
            generation: identity.generation(),
            runtime_id: None,
        }
    }

    pub(crate) fn broad_harness_parent(
        request: &PublishedBroadHarnessRequest,
        runtime_phase: &'static str,
    ) -> Self {
        let campaign_id = request
            .request_path()
            .ancestors()
            .nth(4)
            .and_then(|path| path.file_name())
            .and_then(|name| name.to_str())
            .unwrap_or("unknown-campaign")
            .to_string();
        let node_id = request.request().parent_node_id.as_str().to_string();

        Self {
            role: "parent",
            runtime_phase,
            campaign_id,
            node_id: node_id.clone(),
            branch_id: node_id,
            generation: 0,
            runtime_id: Some(request.request_id().to_string()),
        }
    }

    // pub(crate) fn child(
    //     campaign_id: &str,
    //     node: &Prototype1NodeRecord,
    //     runtime_id: RuntimeId,
    //     runtime_phase: &'static str,
    // ) -> Self {
    //     Self {
    //         role: "child",
    //         runtime_phase,
    //         campaign_id: campaign_id.to_string(),
    //         node_id: node.node_id.clone(),
    //         branch_id: node.branch_id.clone(),
    //         generation: node.generation,
    //         runtime_id: Some(runtime_id.to_string()),
    //     }
    // }

    pub(crate) fn span(&self) -> Span {
        tracing::info_span!(
            target: ploke_core::EXECUTION_DEBUG_TARGET,
            "prototype1.runtime",
            prototype1 = true,
            role = self.role,
            phase = self.runtime_phase,
            campaign = %self.campaign_id,
            node_id = %self.node_id,
        )
    }

    pub(crate) fn install_for_chat_requests(&self) {
        ploke_tui::llm::set_prototype1_trace_context(ploke_tui::llm::Prototype1TraceContext {
            role: self.role.to_string(),
            runtime_phase: self.runtime_phase.to_string(),
            campaign_id: self.campaign_id.clone(),
            node_id: self.node_id.clone(),
            branch_id: self.branch_id.clone(),
            generation: self.generation,
            runtime_id: self.runtime_id.clone(),
        });
    }
}
