use syn::{
    spanned::Spanned, ImplItemFn, ItemEnum, ItemFn, ItemImpl, ItemStruct, ItemTrait, ItemType,
    TraitItemFn,
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
// impl ExtractSpan for ItemMod {}
