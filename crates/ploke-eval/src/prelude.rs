//! Convenience prelude for `ploke-eval`.
//!
//! Items re-exported here are used in a large majority of modules. Using
//! `use crate::prelude::*;` removes the boilerplate of importing these
//! pervasive types, traits, and helpers in every file.
//!
//! Keep this module small and stable: only include items that are truly
//! crate-wide. Domain-specific types should continue to be imported
//! explicitly from their owning modules.

pub use serde::{Deserialize, Serialize};
pub use std::fs;
pub use std::path::{Path, PathBuf};
pub use uuid::Uuid;

pub use crate::spec::PrepareError;
