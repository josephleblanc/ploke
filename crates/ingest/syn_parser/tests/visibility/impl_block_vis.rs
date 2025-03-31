#![cfg(feature = "visibility_resolution")]
//! **Impl Block Visibility**:
//!    - Tests visibility of methods within impl blocks
//!    TODO: Add more test documentation and edge cases

use crate::common::{
    find_function_by_name, find_struct_by_name, get_visibility_info, parse_fixture,
};
use syn_parser::{
    parser::nodes::{NodeId, TypeDefNode, VisibilityResult},
    CodeGraph,
};
