use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::{
    CallReceiver, CallRelationKind, CallResolutionKind, CallSiteKind, CallStatusKind,
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
