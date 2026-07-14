use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::{
    CallNodeKind, CallReceiver, CallRelationKind, CallResolutionKind, CallSiteKind, CallStatusKind,
    CallTargetKind, LocalBindingRelationKind,
};

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct CallSiteRow {
    pub id: Uuid,
    pub owner_id: Uuid,
    pub kind: CallSiteKind,
    pub span: (u32, u32),
    pub cfgs: Vec<String>,
    pub unsafe_block: bool,
    pub path: Option<Vec<String>>,
    pub method: Option<String>,
    pub macro_name: Option<String>,
    pub receiver: Option<CallReceiver>,
    pub arg_count: Option<u32>,
    pub generic_arg_count: Option<u32>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct CallTargetRow {
    pub site_id: Uuid,
    pub target_id: Uuid,
    pub relation: CallRelationKind,
    pub source_kind: CallSiteKind,
    pub target_kind: CallTargetKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct CallResolutionRow {
    pub site_id: Uuid,
    pub site_kind: CallSiteKind,
    pub status: CallStatusKind,
    pub resolution: Option<CallResolutionKind>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct CallContextRow {
    pub site: CallSiteRow,
    pub status: CallResolutionRow,
    pub targets: Vec<CallTargetRow>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct LocalBindingRow {
    pub id: Uuid,
    pub owner_id: Uuid,
    pub owner_kind: String,
    pub kind: String,
    pub name: String,
    pub span: (u32, u32),
    pub cfgs: Vec<String>,
    pub source_kind: String,
    pub source_id: Option<Uuid>,
    pub source_call_kind: Option<String>,
    pub source_path: Option<Vec<String>>,
    pub callee_kind: Option<String>,
    pub callee_path: Option<Vec<String>>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct LocalBindingEdgeRow {
    pub source_id: Uuid,
    pub target_id: Uuid,
    pub relation: LocalBindingRelationKind,
    pub source_kind: String,
    pub target_kind: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ReturnedCallSite {
    pub id: Uuid,
    pub span: (u32, u32),
    pub path: Vec<String>,
    pub target_id: Uuid,
    pub relation: CallRelationKind,
    pub target_kind: CallTargetKind,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ReturnedCallProducer {
    pub id: Uuid,
    pub site_id: Uuid,
    pub span: (u32, u32),
    pub path: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ReturnedCallSource {
    pub id: Uuid,
    pub relation: LocalBindingRelationKind,
    pub kind: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ReturnedCallBinding {
    pub id: Uuid,
    pub source: ReturnedCallSource,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ReturnedCallBindingFlow {
    pub caller_id: Uuid,
    pub dynamic: ReturnedCallSite,
    pub producer: ReturnedCallProducer,
    pub binding: ReturnedCallBinding,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct CallSiteBucket {
    pub kind: CallSiteKind,
    pub relation: CallRelationKind,
    pub count: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct CallNodeContext {
    pub node_id: Uuid,
    pub outgoing: Vec<CallContextRow>,
    pub incoming: Vec<CallContextRow>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct CallCallerRow {
    pub site: CallSiteRow,
    pub status: CallResolutionRow,
    pub target: CallTargetRow,
}

/// Starting point for call-context expansion.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum CallContextSeed {
    Owner(Uuid),
    Target(Uuid),
}

/// Why a candidate was returned by [`crate::Database::expand_call_context`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum CallContextRelation {
    OutgoingTarget,
    IncomingCaller,
}

/// Code graph node reachable from a call-context seed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct CallContextCandidate {
    pub node_id: Uuid,
    pub relation: CallContextRelation,
    pub call_site_id: Uuid,
    pub target_id: Uuid,
    pub distance: u32,
}

/// Controls for call-context expansion.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct CallContextOptions {
    pub include_outgoing_targets: bool,
    pub include_incoming_callers: bool,
    pub max_candidates: usize,
}

impl Default for CallContextOptions {
    fn default() -> Self {
        Self {
            include_outgoing_targets: true,
            include_incoming_callers: true,
            max_candidates: 64,
        }
    }
}

/// Controls for bounded call-path traversal.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct CallPathOptions {
    pub max_depth: u32,
    pub max_paths: usize,
}

impl Default for CallPathOptions {
    fn default() -> Self {
        Self {
            max_depth: 3,
            max_paths: 64,
        }
    }
}

/// One resolved call edge inside a bounded call path.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct CallPathEdge {
    pub caller_id: Uuid,
    pub callee_id: Uuid,
    pub call_site_id: Uuid,
    pub span: (u32, u32),
    pub relation: CallRelationKind,
    pub source_kind: CallSiteKind,
    pub target_kind: CallTargetKind,
}

/// Ordered resolved call chain.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct CallPath {
    pub start_id: Uuid,
    pub end_id: Uuid,
    pub depth: u32,
    pub edges: Vec<CallPathEdge>,
}

/// Resolved source-to-target paths classified by a required guard node.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct CallGuardReport {
    pub source: CallNodeInfo,
    pub target: CallNodeInfo,
    pub guard: CallNodeInfo,
    pub guarded: bool,
    pub paths: Vec<CallPath>,
    pub violations: Vec<CallPath>,
}

/// Resolved direct call edge whose caller and callee live in different modules.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ModuleBoundaryEdge {
    pub edge: CallPathEdge,
    pub caller: CallNodeInfo,
    pub callee: CallNodeInfo,
    pub site: CallSiteRow,
}

/// Resolved direct call edge whose caller and callee live in different crates.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct CrateBoundaryEdge {
    pub edge: CallPathEdge,
    pub caller: CallNodeInfo,
    pub caller_crate: String,
    pub callee: CallNodeInfo,
    pub callee_crate: String,
    pub site: CallSiteRow,
}

/// Caller-supplied dependency rule that marks a crate boundary as forbidden.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct CrateBoundaryPolicyRule {
    pub rule_id: String,
    pub caller_crate: String,
    pub callee_crate: String,
}

/// Resolved crate-boundary edge that matched a forbidden dependency rule.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct CrateBoundaryPolicyViolation {
    pub rule_id: String,
    pub edge: CrateBoundaryEdge,
}

/// Caller-supplied architecture rule that marks a module boundary as forbidden.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ModuleBoundaryPolicyRule {
    pub rule_id: String,
    pub caller_module_prefix: Vec<String>,
    pub callee_module_prefix: Vec<String>,
}

/// Resolved module-boundary edge that matched a forbidden architecture rule.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ModuleBoundaryPolicyViolation {
    pub rule_id: String,
    pub edge: ModuleBoundaryEdge,
}

/// Stable source metadata for a node that participates in call-graph queries.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct CallNodeInfo {
    pub id: Uuid,
    pub kind: CallNodeKind,
    pub name: String,
    pub visibility: String,
    pub is_public: bool,
    pub is_unsafe: bool,
    pub is_async: bool,
    pub module_path: Vec<String>,
    pub file_path: String,
}

/// Target-centered summary for impact/navigation usage questions.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct CallImpactReport {
    pub target: CallNodeInfo,
    pub paths: Vec<CallPath>,
    pub callers: Vec<CallNodeInfo>,
    pub direct_callers: Vec<CallNodeInfo>,
    pub direct_call_sites: Vec<CallContextRow>,
    pub callsite_buckets: Vec<CallSiteBucket>,
    pub public_callers: Vec<CallNodeInfo>,
    pub test_callers: Vec<CallNodeInfo>,
    pub non_test_callers: Vec<CallNodeInfo>,
    pub source_files: Vec<String>,
    pub source_crates: Vec<String>,
    #[serde(default)]
    pub source_cfgs: Vec<String>,
    pub source_modules: Vec<Vec<String>>,
}

/// Owner-centered summary for navigation/reachability usage questions.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct CallReachReport {
    pub owner: CallNodeInfo,
    pub paths: Vec<CallPath>,
    pub callees: Vec<CallNodeInfo>,
    pub direct_callees: Vec<CallNodeInfo>,
    pub direct_call_sites: Vec<CallContextRow>,
    pub boundary_call_sites: Vec<CallContextRow>,
    pub boundary_edges: Vec<CallPathEdge>,
    pub public_callees: Vec<CallNodeInfo>,
    pub frontier_calls: Vec<CallContextRow>,
    pub external_frontier_calls: Vec<CallContextRow>,
    pub unsupported_frontier_calls: Vec<CallContextRow>,
    #[serde(default)]
    pub unresolved_frontier_calls: Vec<CallContextRow>,
    #[serde(default)]
    pub ambiguous_frontier_calls: Vec<CallContextRow>,
    pub source_files: Vec<String>,
    pub source_crates: Vec<String>,
    #[serde(default)]
    pub source_cfgs: Vec<String>,
    pub source_modules: Vec<Vec<String>>,
}

/// Proof effect annotation attached to a callsite reachable from an owner.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct CallReachEffect {
    pub effect_seed_id: String,
    pub effect_class: String,
    pub confidence: Option<String>,
    pub blocker_if_unresolved: Option<bool>,
    #[serde(default)]
    pub paths_to_owner: Vec<CallPath>,
    pub call_site: CallContextRow,
    pub blocker_reasons: Vec<String>,
}

/// Reachable effect that is outside a caller-supplied policy allowlist.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct CallEffectPolicyViolation {
    pub allowed_effects: Vec<String>,
    pub effect: CallReachEffect,
}

/// Reachable proof effect annotations classified by a required guard node.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct CallEffectGuardReport {
    pub owner: CallNodeInfo,
    pub guard: CallNodeInfo,
    pub effect_class: String,
    pub guarded: bool,
    pub effects: Vec<CallReachEffect>,
    pub violations: Vec<CallReachEffect>,
}

/// Proof invariant finding scoped to callsites reachable from an owner.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct CallProofInvariantFinding {
    pub invariant: String,
    pub status: String,
    pub reason: String,
    #[serde(default)]
    pub call_site_id: Option<String>,
    #[serde(default)]
    pub call_site: Option<CallContextRow>,
}

/// Active external-summary blocker reachable from an owner.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ExternalSummaryNeed {
    #[serde(default)]
    pub paths_to_owner: Vec<CallPath>,
    pub call_site: CallContextRow,
    pub blocker_reasons: Vec<String>,
}

/// Active runtime-dispatch blocker attached to a callsite reachable from an owner.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct RuntimeDispatchNeed {
    #[serde(default)]
    pub paths_to_owner: Vec<CallPath>,
    pub call_site: CallContextRow,
    pub blocker_reasons: Vec<String>,
}

/// Build/test domain proof metadata linked to a call-graph node.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct CallBuildDomain {
    pub build_domain_id: String,
    pub target_kind: Option<String>,
    pub target_name: Option<String>,
    pub target_root: Option<String>,
    pub profile: Option<String>,
    pub rustc_version: Option<String>,
    pub proof_policy_version: Option<String>,
    pub active_cfg_hash: Option<String>,
    pub evidence_use: Option<String>,
    pub blocker_reasons: Vec<String>,
}

/// Generated or external test/build entrypoint proof metadata linked to a node.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct CallTestEntrypoint {
    pub entrypoint_summary_id: String,
    pub build_domain_id: Option<String>,
    pub definition_id: Option<String>,
    pub target_kind: Option<String>,
    pub target_name: Option<String>,
    pub target_root: Option<String>,
    pub summary_class: Option<String>,
    pub artifact_hash: Option<String>,
    pub summary_version: Option<String>,
    pub review_method: Option<String>,
    pub scope_of_validity: Option<String>,
    pub required_containment: Option<String>,
    pub invalidation_conditions: Option<String>,
    pub status: Option<String>,
    pub evidence_use: Option<String>,
    pub allowed_effects: Vec<String>,
    pub blocker_reasons: Vec<String>,
}

/// Conservative test-selection summary for a changed call-graph target.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct CallTestSelectionReport {
    pub target: CallNodeInfo,
    pub source_test_callers: Vec<CallNodeInfo>,
    pub source_test_paths: Vec<CallPath>,
    pub generated_entrypoints: Vec<CallTestEntrypoint>,
    pub build_domains: Vec<CallBuildDomain>,
}
