//! **Macro Visibility**:
//!    - Tests for `#[macro_export]` macros
//!    TODO: Add more test documentation and edge cases
#![cfg(feature = "visibility_resolution")]

use crate::common::{
    find_function_by_name, find_struct_by_name, get_visibility_info, parse_fixture,
};
use syn_parser::{
    parser::nodes::{NodeId, TypeDefNode, VisibilityResult},
    CodeGraph,
};
