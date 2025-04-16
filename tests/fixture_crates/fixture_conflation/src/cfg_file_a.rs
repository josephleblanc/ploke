// Entire file gated by feature_a
#![cfg(feature = "feature_a")]
#![allow(dead_code)]

// Items here should have distinct IDs from identically named items
// in cfg_file_not_a.rs due to the file path difference.

// 43. Test NodeId disambiguation via file path
pub struct FileGatedStruct {
    // 44. Test FieldNode.type_id disambiguation via file path
    pub field: i32,
}

// 45. Test NodeId disambiguation via file path
pub fn file_gated_func() -> i32 {
    42
}

// 46. Test NodeId disambiguation via file path (generic)
pub fn file_gated_generic<T>(input: T) -> T {
    input
}
