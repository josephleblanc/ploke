use syn::{
    spanned::Spanned, ImplItemFn, ItemEnum, ItemFn, ItemImpl, ItemStruct, ItemTrait, ItemType,
    ItemUse, TraitItemFn, UseName, UseRename, UseTree,
};

// Note: From the proc_macro2 crate documentation on Span::byte_range()
//When executing in a procedural macro context, the returned range is only accurate if compiled
//with a nightly toolchain. The stable toolchain does not have this information available. When
//executing outside of a procedural macro, such as main.rs or build.rs, the byte range is always
//accurate regardless of toolchain.
/// Helper trait to extract the byte span from a syn item node in the AST. The returned
/// (usize, usize) is the start/end byte position of the item definition in the target source code.
pub trait ExtractSpan
where
    Self: Spanned,
{
    fn extract_span_bytes(&self) -> (usize, usize) {
        let byte_range = self.span().byte_range();
        (byte_range.start, byte_range.end)
    }
}
impl ExtractSpan for ItemStruct {}
impl ExtractSpan for ItemFn {}
impl ExtractSpan for ItemEnum {}
impl ExtractSpan for ItemImpl {}
impl ExtractSpan for ImplItemFn {}
impl ExtractSpan for ItemTrait {}
impl ExtractSpan for TraitItemFn {}
impl ExtractSpan for ItemType {}
impl ExtractSpan for ItemUse {}
impl ExtractSpan for UseTree {}
impl ExtractSpan for UseName {}
impl ExtractSpan for UseRename {}
// impl ExtractSpan for ItemMod {}

// --- Helper potentially in syn_parser ---
// It's often useful to have a helper to get the string representation.
// Place this where appropriate (e.g., type_processing.rs or utils.rs).
use quote::ToTokens; // Needs quote dependency in syn_parser

/// Converts a syn::Type into a consistent string representation.
pub(crate) fn type_to_string(ty: &syn::Type) -> String {
    ty.to_token_stream().to_string()
}

// --- Example Usage in syn_parser ---
/*
// Inside get_or_create_type or similar logic:

let type_str = type_to_string(ty); // Use the helper

// Check cache... if not found:
let new_type_id = TypeId::generate_synthetic(
    state.crate_namespace,      // Namespace of the crate being parsed
    &state.current_file_path,   // File where 'ty' was encountered
    &type_str,                  // The string representation of 'ty'
);

// Store new_type_id in cache (mapping type_str -> new_type_id)
// Create TypeNode using new_type_id
// Return new_type_id
*/

// This implementation uses the string representation derived from `to_token_stream()` as the core
// input representing the type, combined with the necessary file and crate context, to generate a
// deterministic `TypeId::Synthetic`.
