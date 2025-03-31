//! **Trait Implementation Visibility**:
//!    - Visibility of default trait implementations
//!    TODO: Add more test documentation and edge cases
#![cfg(feature = "visibility_resolution")]

use crate::common::{
    find_function_by_name, find_struct_by_name, get_visibility_info, parse_fixture,
};
use syn_parser::{
    parser::nodes::{NodeId, TypeDefNode, VisibilityResult},
    CodeGraph,
};
