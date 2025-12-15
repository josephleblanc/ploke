pub mod builder;
pub mod semantic;

// Re-exporting with same module path for consumers, to keep changes in feature flag isolated from
// the changes.
pub mod callbacks {
    pub use super::callbacks_multi::*;
}
mod callbacks_multi;
mod callbacks_single;

pub use builder::FieldValue;
pub use builder::QueryBuilder;
