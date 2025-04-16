// Entire file gated by not(feature_a)
#![cfg(not(feature = "feature_a"))]
#![allow(dead_code)]

// Items here should have distinct IDs from identically named items
// in cfg_file_a.rs due to the file path difference.

// 43a. Test NodeId disambiguation via file path
pub struct FileGatedStruct {
    // 44a. Test FieldNode.type_id disambiguation via file path (different type)
    pub field: String,
}

// 45a. Test NodeId disambiguation via file path
pub fn file_gated_func() -> String {
    "not_a".to_string()
}

// 46a. Test NodeId disambiguation via file path (generic)
pub fn file_gated_generic<T>(input: T) -> T {
    input
}
