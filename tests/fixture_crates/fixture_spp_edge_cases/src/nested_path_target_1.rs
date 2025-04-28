//! First level of nested #[path] target.

#[path = "nested_path_target_2.rs"]
pub mod nested_target_2; // This declaration points to the next file

pub fn item_in_nested_target_1() -> u8 {
    11
}
