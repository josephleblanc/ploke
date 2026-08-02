use uuid::Uuid;

use crate::{Database, DbError};

use super::super::CallNodeContext;

impl Database {
    /// Returns both sides of the call graph for a code graph node.
    ///
    /// `outgoing` is the existing owner-centered context for calls contained by
    /// this node; `incoming` is the existing target-centered context for calls
    /// that resolve to this node.
    pub fn call_context_for_node(&self, node_id: Uuid) -> Result<CallNodeContext, DbError> {
        Ok(CallNodeContext {
            node_id,
            outgoing: self.call_context_for_owner(node_id)?,
            incoming: self.call_context_for_target(node_id)?,
        })
    }
}
