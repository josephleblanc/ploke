// **Module System Edge Cases**:
//    - Nested module visibility (`mod outer { pub mod inner {} }`)
//    - More complex module hierarchies than tested in visibility files
//!    TODO: Add more test documentation and edge cases
#![cfg(feature = "visibility_resolution")]

use crate::common::{
    find_function_by_name, find_struct_by_name, get_visibility_info, parse_fixture,
};
use syn_parser::{
    parser::nodes::{NodeId, TypeDefNode, VisibilityResult},
    CodeGraph,
};
