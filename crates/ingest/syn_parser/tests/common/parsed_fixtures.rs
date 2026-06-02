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

lazy_static! {
    /// Parsed data for the "fixture_path_resolution" crate.
    /// Used for path-resolution and relation-heavy phase 3 tests.
    pub static ref PARSED_FIXTURE_CRATE_PATH_RESOLUTION: Vec<ParsedCodeGraph> =
        run_phases_and_collect("fixture_path_resolution");
}

lazy_static! {
    /// Parsed data for the "fixture_spp_edge_cases_no_cfg" crate.
    /// Used for shortest-path and re-export edge cases without cfg duplication pressure.
    pub static ref PARSED_FIXTURE_CRATE_SPP_EDGE_CASES_NO_CFG: Vec<ParsedCodeGraph> =
        run_phases_and_collect("fixture_spp_edge_cases_no_cfg");
}

lazy_static! {
    /// Parsed data for the "fixture_spp_edge_cases" crate.
    /// Used for the cfg-heavy shortest-path and re-export canary tests.
    pub static ref PARSED_FIXTURE_CRATE_SPP_EDGE_CASES: Vec<ParsedCodeGraph> =
        run_phases_and_collect("fixture_spp_edge_cases");
}
