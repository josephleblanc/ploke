pub mod builder;
pub mod semantic;

// Re-exporting with same module path for consumers, to keep changes in feature flag isolated from
// the changes.
pub mod callbacks {
    #[cfg(not(feature = "multi_embedding_db"))]
    pub use super::callbacks_single::*;
    #[cfg(feature = "multi_embedding_db")]
    pub use super::callbacks_multi::*;

}
mod callbacks_single;
mod callbacks_multi;

pub use builder::FieldValue;
pub use builder::QueryBuilder;
