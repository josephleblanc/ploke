pub mod mod_tree_construction;

pub mod canon_resolver;

pub mod backlink_imports;
pub mod backlink_imports_spp;
pub mod backlink_imports_spp_cfg;

pub mod path_attribute;

#[cfg(feature = "typed_type_graph")]
pub mod call_sites;
pub mod prune_unlinked_imports;
#[cfg(feature = "typed_type_graph")]
pub mod type_relations_v2;
#[cfg(not(feature = "typed_type_graph"))]
pub mod type_use_resolution;

#[cfg(not(feature = "type_bearing_ids"))]
pub mod shortest_path;

#[cfg(not(feature = "type_bearing_ids"))]
pub mod edge_cases;
#[cfg(not(feature = "type_bearing_ids"))]
pub mod exports; // Add the new module for export tests
#[cfg(not(feature = "type_bearing_ids"))]
pub mod mod_tree;
