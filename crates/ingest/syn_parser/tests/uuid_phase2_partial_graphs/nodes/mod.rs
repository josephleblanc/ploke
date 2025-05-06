// Declare node-specific test modules
mod const_static;
#[cfg(not(feature = "type_bearing_ids"))]
mod enums;
#[cfg(not(feature = "type_bearing_ids"))]
mod functions;
#[cfg(not(feature = "type_bearing_ids"))]
mod impls;
#[cfg(not(feature = "type_bearing_ids"))]
mod imports;
#[cfg(not(feature = "type_bearing_ids"))]
mod macros;
#[cfg(not(feature = "type_bearing_ids"))]
mod modules;
#[cfg(not(feature = "type_bearing_ids"))]
mod structs;
#[cfg(not(feature = "type_bearing_ids"))]
mod traits;
#[cfg(not(feature = "type_bearing_ids"))]
mod type_alias;
#[cfg(not(feature = "type_bearing_ids"))]
mod unions;
// Add other node types here later:
//   const_alias
