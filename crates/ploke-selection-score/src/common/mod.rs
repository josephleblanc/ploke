//! Shared numeric helpers used by paper mechanisms and Ploke selectors.

pub mod error;
pub mod numeric;
pub mod probability;
pub mod ranking;
pub mod rates;
pub mod selection;
pub mod vectors;

pub use error::ScoreError;
pub use probability::normalize_weights;
pub use ranking::rank_quality;
