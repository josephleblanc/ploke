mod cfg_gates_quantized_metal;
mod edition_2015_async_keyword_fallback;
mod edition_2015_bare_trait_object_fallback;
mod file_links_staged_merge;
mod kl005_manifest_cargo_alignment;
mod path_attr_stem_collision;
mod raw_async_identifier;
// tests on items within an executable scope, like a function body or closure.
mod executable_scope;
// tests on items within a secondary scope, such as within an enum definition
mod secondary_scope;
// tests on consts that are siblings in primary scope, such as consts defined with a direct module
// parent
mod const_underscore;
