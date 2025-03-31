//! **Type Alias Visibility**:
//!    - Tests for `pub type StringVec` and other type aliases
//!    - Visibility tests focus on structs/enums but don't explicitly test type alias visibility
//!    TODO: Add more test documentation and edge cases
#![cfg(feature = "visibility_resolution")]

use crate::common::{
    find_function_by_name, find_struct_by_name, get_visibility_info, parse_fixture,
};
use syn_parser::{
    parser::nodes::{NodeId, TypeDefNode, VisibilityResult},
    CodeGraph,
};
