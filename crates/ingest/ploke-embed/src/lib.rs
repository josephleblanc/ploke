#![allow(unused_variables, unused_imports, dead_code)]
pub mod indexer;
// Removed embedding_service (replaced by concrete type in indexer)
pub mod cancel_token;
pub mod config;
pub mod error;
pub mod events;
pub mod local;
pub mod partial;
pub mod providers;
pub mod runtime;
pub mod utils;

#[cfg(test)]
mod tests {
    // Removed dummy add test
}
