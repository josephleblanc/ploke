use serde::{Deserialize, Serialize};

/// Result of visibility resolution with detailed scoping information
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum VisibilityResult {
    /// Directly usable without imports
    Direct,
    /// Needs use statement with given path
    NeedsUse(Vec<String>),
    /// Not accessible with current scope
    OutOfScope {
        /// Why the item isn't accessible
        // reason: OutOfScopeReason,
        /// For pub(in path) cases, shows allowed scopes  
        allowed_scopes: Option<Vec<String>>,
    },
}
