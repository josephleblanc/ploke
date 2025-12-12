pub mod mod_tree_construction;

pub mod canon_resolver;

pub mod path_attribute;

#[cfg(not(feature = "type_bearing_ids"))]
pub mod shortest_path;

#[cfg(not(feature = "type_bearing_ids"))]
pub mod edge_cases;
#[cfg(not(feature = "type_bearing_ids"))]
pub mod exports; // Add the new module for export tests
#[cfg(not(feature = "type_bearing_ids"))]
pub mod mod_tree;
