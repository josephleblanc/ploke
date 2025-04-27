use std::{alloc::Global, collections::HashMap};

use ploke_core::{CanonId, IdInfo, NodeId, PubPathId, ResolvedIds, TrackingHash, TypeId};
use uuid::Uuid;

use crate::parser::nodes::NodePath;

// AI: I agree with your assessment that we didn't need the other fields, and can even remove
// `pending_node_info`. However, I would like to make it an iterator or similar structure that can
// operate by accepting a reference to the `ModuleTree` and `ParsedCodeGraph` as you described. AI!
pub struct CanonIdResolver {
    /// Project namespace of the defining crate, used as namespace for Id generation using v5 hash.
    namespace: Uuid,
    /// Stores the node info required for processing nodes.
    /// Should be initialized with `Some`
    /// Invariant: Is always `Some` until Id processing begins. Is only `None` during error state,
    /// since this struct must be consumed after resolution.
    pending_node_info: Option<Vec<IdInfo>>,
    // AI: I removed the resoved_definition_ids
}

impl CanonIdResolver {
    pub fn new(namespace: Uuid, pending_node_info) -> Self {
        Self { 
            namespace, 
            pending_node_info: Some(Vec::new()), 
        }
    }

    pub fn namespace(&self) -> Uuid {
        self.namespace
    }

    pub fn pending_node_info(&self) -> Option<&Vec<IdInfo<'_>, Global>> {
        self.pending_node_info.as_ref()
    }
}
