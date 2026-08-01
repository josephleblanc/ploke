//! Capabilities for non-authoritative projection reads.
//!
//! Projection files are operator views and compatibility mirrors. Active loop
//! execution must use History, channels, or artifact backends instead.

// ANCHOR: ploke_eval_operator_projection_read
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OperatorProjectionRead {
    _private: (),
}

impl OperatorProjectionRead {
    pub(crate) fn cli_operator() -> Self {
        Self { _private: () }
    }

    pub(crate) fn projection_module() -> Self {
        Self { _private: () }
    }
}
// ANCHOR_END: ploke_eval_operator_projection_read
