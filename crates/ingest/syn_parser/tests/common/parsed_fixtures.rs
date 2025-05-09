//! Caches the results of `run_phases_and_collect` for commonly used test fixtures.
//! This avoids re-parsing the same fixtures repeatedly across multiple tests,
//! significantly speeding up test execution.

use crate::common::run_phases_and_collect;
use lazy_static::lazy_static;
use std::vec::Vec;
use syn_parser::parser::ParsedCodeGraph; // Assuming run_phases_and_collect is in common::uuid_ids_utils or similar

// --- Parsed Fixture Data ---

lazy_static! {
    /// Parsed data for the "fixture_nodes" crate.
    /// Contains various node types for individual node parsing tests.
    pub static ref PARSED_FIXTURE_CRATE_NODES: Vec<ParsedCodeGraph> =
        run_phases_and_collect("fixture_nodes");
}

lazy_static! {
    /// Parsed data for the "fixture_crate_dir_detection" crate.
    /// Used for testing crate discovery and basic module structure.
    pub static ref PARSED_FIXTURE_CRATE_DIR_DETECTION: Vec<ParsedCodeGraph> =
        run_phases_and_collect("file_dir_detection");
}

// Add other fixtures here as needed, for example:
lazy_static! {
    /// Parsed data for the "fixture_types" crate.
    pub static ref PARSED_FIXTURE_CRATE_TYPES: Vec<ParsedCodeGraph> =
        run_phases_and_collect("fixture_types");
}
