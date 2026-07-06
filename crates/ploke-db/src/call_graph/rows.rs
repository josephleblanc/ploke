use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::{
    CallNodeKind, CallReceiver, CallRelationKind, CallResolutionKind, CallSiteKind, CallStatusKind,
    CallTargetKind,
};

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct CallSiteRow {
    pub id: Uuid,
    pub owner_id: Uuid,
    pub kind: CallSiteKind,
    pub span: (u32, u32),
    pub cfgs: Vec<String>,
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

/// Resolved direct call edge whose caller and callee live in different modules.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ModuleBoundaryEdge {
    pub edge: CallPathEdge,
    pub caller: CallNodeInfo,
    pub callee: CallNodeInfo,
    pub site: CallSiteRow,
}

/// Stable source metadata for a node that participates in call-graph queries.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct CallNodeInfo {
    pub id: Uuid,
    pub kind: CallNodeKind,
    pub name: String,
    pub visibility: String,
    pub is_public: bool,
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
