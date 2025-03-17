use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

// Globally unique identifier for nodes
pub type NodeId = u64;

// Atomic counter for generating unique node IDs
pub struct NodeIdGenerator {
    id: Arc<AtomicUsize>,
}

impl NodeIdGenerator {
    pub fn new() -> Self {
        NodeIdGenerator {
            id: Arc::new(AtomicUsize::new(0)),
        }
    }

    pub fn next_id(&self) -> NodeId {
        self.id.fetch_add(1, Ordering::SeqCst) as NodeId
    }
}

// Type identifier
pub type TypeId = u64;

// Atomic counter for generating unique type IDs
pub struct TypeIdGenerator {
    id: Arc<AtomicUsize>,
}

impl TypeIdGenerator {
    pub fn new() -> Self {
        TypeIdGenerator {
            id: Arc::new(AtomicUsize::new(0)),
        }
    }

    pub fn next_id(&self) -> TypeId {
        self.id.fetch_add(1, Ordering::SeqCst) as TypeId
    }
}

// Content hash (Blake3)
pub type ContentHash = [u8; 32];

// Type stamp (Content hash + timestamp)
pub struct TypeStamp {
    pub content_hash: ContentHash,
    pub modified_ns: u64, // Nanoseconds since epoch
}
