pub mod indexer;
// Removed embedding_service (replaced by concrete type in indexer)
pub mod events;
pub mod error;
pub mod providers;
pub mod local;

// ploke-embed
//
// Now handles embedding generation via concrete processor type
// But still stubs real functionality until backend integrations are complete

#[cfg(test)]
mod tests {
    // Removed dummy add test
}
