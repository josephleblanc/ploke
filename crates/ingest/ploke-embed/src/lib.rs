#![allow(unused_variables, unused_imports, dead_code)]
pub mod indexer;
// Removed embedding_service (replaced by concrete type in indexer)
pub mod events;
pub mod error;
pub mod providers;
pub mod local;
pub mod cancel_token;
pub mod config;
pub mod partial;
pub mod process_bm25;

#[cfg(test)]
mod tests {
    // Removed dummy add test
}
