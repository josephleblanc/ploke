//! Runtime-scoped telemetry projection for Prototype 1.
//!
//! This module does not grant authority. It projects already-admitted runtime
//! context into tracing spans so downstream events can be attributed without
//! duplicating identity into every telemetry record.

use tracing::Span;

use crate::intervention::Prototype1NodeRecord;

use super::{event::RuntimeId, identity::ParentIdentity};

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
            campaign_id: identity.campaign_id.clone(),
            node_id: identity.node_id.clone(),
            branch_id: identity.branch_id.clone(),
            generation: identity.generation,
            runtime_id: None,
        }
    }

    pub(crate) fn child(
        campaign_id: &str,
        node: &Prototype1NodeRecord,
        runtime_id: RuntimeId,
        runtime_phase: &'static str,
    ) -> Self {
        Self {
            role: "child",
            runtime_phase,
            campaign_id: campaign_id.to_string(),
            node_id: node.node_id.clone(),
            branch_id: node.branch_id.clone(),
            generation: node.generation,
            runtime_id: Some(runtime_id.to_string()),
        }
    }

    pub(crate) fn span(&self) -> Span {
        tracing::info_span!(
            target: ploke_core::EXECUTION_DEBUG_TARGET,
            "prototype1.runtime",
            prototype1 = true,
            role = self.role,
            runtime_phase = self.runtime_phase,
            campaign_id = %self.campaign_id,
            node_id = %self.node_id,
            branch_id = %self.branch_id,
            generation = self.generation,
            runtime_id = self.runtime_id.as_deref().unwrap_or("")
        )
    }
}
