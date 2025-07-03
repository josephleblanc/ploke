pub mod indexer;
// Removed embedding_service (replaced by concrete type in indexer)
pub mod events;
pub mod error;
pub mod providers;
pub mod local;

#[cfg(test)]
mod tests {
    // Removed dummy add test
}
