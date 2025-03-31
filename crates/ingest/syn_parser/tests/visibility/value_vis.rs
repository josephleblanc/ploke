//! **Value Visibility** (Constants/Statics):
//!    - Tests for `pub const MAX_ITEMS` and `pub static GLOBAL_COUNTER`
//!    - Visibility of constants/statics isn't explicitly tested elsewhere
//!    TODO: Add more test documentation and edge cases
#![cfg(feature = "visibility_resolution")]

use crate::common::{
    find_function_by_name, find_struct_by_name, get_visibility_info, parse_fixture,
};
use syn_parser::{
    parser::nodes::{NodeId, TypeDefNode, VisibilityResult},
    CodeGraph,
};
