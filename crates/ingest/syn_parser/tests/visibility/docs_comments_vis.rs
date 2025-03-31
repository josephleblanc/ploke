#![cfg(feature = "visibility_resolution")]
//! Documentation Visibility:
//!     - Tests that check doc visibility (`/// [Visibility]` in docs)
//!     - Not a focus of the visibility test files
//!    TODO: Add more test documentation

use crate::common::{
    find_function_by_name, find_struct_by_name, get_visibility_info, parse_fixture,
};
use syn_parser::{
    parser::nodes::{NodeId, TypeDefNode, VisibilityResult},
    CodeGraph,
};
