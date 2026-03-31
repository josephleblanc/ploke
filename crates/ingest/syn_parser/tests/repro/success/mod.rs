mod cfg_gates_quantized_metal;
mod kl005_manifest_cargo_alignment;
mod file_links_staged_merge;
mod path_attr_stem_collision;
// tests on items within an executable scope, like a function body or closure.
mod executable_scope;
// tests on items within a secondary scope, such as within an enum definition
mod secondary_scope;
// tests on consts that are siblings in primary scope, such as consts defined with a direct module
// parent
mod const_underscore;
