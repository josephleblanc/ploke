#![cfg(test)]
use itertools::Itertools;
use ploke_core::{ItemKind, NodeId, TypeId, TypeKind};
use std::fs::File;
use std::io::{Read, Seek};
use std::path::{self, Path};
use syn_parser::error::SynParserError; // Import directly from ploke_core
use syn_parser::parser::graph::{CodeGraph, GraphAccess}; // Added GraphNode
use syn_parser::parser::types::{GenericParamKind, GenericParamNode, TypeNode}; // Remove TypeKind from here
use syn_parser::parser::visitor::calculate_cfg_hash_bytes;
use syn_parser::parser::{nodes::*, ExtractSpan, ParsedCodeGraph};
use syn_parser::utils::logging::LOG_TEST_ID_REGEN;
use syn_parser::utils::LogStyle; // Added LogStyle imports
use syn_parser::TestIds;
use thiserror::Error; // Ensure thiserror is imported

pub mod debug_printers;
pub mod macro_rule_tests;
pub mod paranoid;
pub mod parsed_fixtures;
pub use parsed_fixtures::{
    PARSED_FIXTURE_CRATE_DIR_DETECTION, PARSED_FIXTURE_CRATE_NODES, PARSED_FIXTURE_CRATE_TYPES,
};
pub mod resolution; // Add resolution module // Add new module for macros

pub fn run_phases_and_collect(fixture_name: &str) -> Vec<ParsedCodeGraph> {
    let crate_path = fixtures_crates_dir().join(fixture_name);
    let project_root = workspace_root(); // Use workspace root for context
    let discovery_output = run_discovery_phase(&project_root, &[crate_path.clone()])
        .unwrap_or_else(|e| panic!("Phase 1 Discovery failed for {}: {:?}", fixture_name, e));

    let results_with_errors: Vec<Result<ParsedCodeGraph, SynParserError>> =
        analyze_files_parallel(&discovery_output, 0); // num_workers ignored by rayon bridge

    // Collect successful results, panicking if any file failed to parse in Phase 2
    results_with_errors
        .into_iter()
        .map(|res| {
            res.unwrap_or_else(|e| {
                panic!(
                    "Phase 2 parsing failed for a file in fixture {}: {:?}",
                    fixture_name, e
                )
            })
        })
        .collect()
}

pub fn try_run_phases_and_collect(
    fixture_name: &str,
) -> Result<Vec<ParsedCodeGraph>, ploke_error::Error> {
    let crate_path = fixtures_crates_dir().join(fixture_name);
    let project_root = workspace_root(); // Use workspace root for context
    let discovery_output = run_discovery_phase(&project_root, &[crate_path.clone()])?;

    let results_with_errors: Vec<Result<ParsedCodeGraph, SynParserError>> =
        analyze_files_parallel(&discovery_output, 0); // num_workers ignored by rayon bridge

    let root_graph = results_with_errors
        .iter()
        .filter_map(|pr| pr.as_ref().ok())
        .find(|pr| pr.crate_context.is_some());

    // Collect successful results, panicking if any file failed to parse in Phase 2
    let mut results = Vec::new();
    for result in results_with_errors {
        eprintln!("result is ok? | {}", result.is_ok());
        results.push(result?);
    }
    Ok(results)
}

#[derive(Debug, Clone)]
/// Args for the paranoid helper test functions.
/// Includes all information required to regenerate the NodeId of the target node.
pub struct ParanoidArgs<'a> {
    // parsed_graphs: &'a [ParsedCodeGraph], // Operate on the collection - Passed separately now
    /// The name of the test fixture directory (e.g., "fixture_nodes").
    /// Used to construct the absolute path to the fixture crate root.
    pub fixture: &'a str,
    /// The path to the specific source file within the fixture, relative to the fixture root
    /// (e.g., "src/const_static.rs"). Used to find the correct `ParsedCodeGraph` and
    /// as input for `NodeId::generate_synthetic`.
    pub relative_file_path: &'a str,
    /// The expected fully-qualified module path of the *parent* module containing the target item
    /// (e.g., `["crate", "my_module"]`). Used to find the parent `ModuleNodeId` for ID generation.
    pub expected_path: &'a [&'a str],
    /// The identifier (name) of the target item (e.g., "MY_CONST").
    /// Used as input for `NodeId::generate_synthetic`.
    pub ident: &'a str,
    /// The expected `ItemKind` of the target item (e.g., `ItemKind::Const`).
    /// Used to select the correct `PrimaryNodeId` type and for ID generation.
    pub item_kind: ItemKind,
    /// An optional slice of cfg strings expected to be active for the item
    /// (e.g., `Some(&["target_os = \"linux\""])`). Used to calculate the cfg hash
    /// for ID generation. `None` or `Some(&[])` indicates no cfgs.
    pub expected_cfg: Option<&'a [&'a str]>,
}

impl<'a> ParanoidArgs<'a> {
    /// Regenerates the exact uuid::Uuid using the v5 hashing method to check that the node id
    /// correctly matches when using the expected inputs for the typed id node generation.
    /// - Returns a result with the typed PrimaryNodeId matching the input type of `item_kind` provided
    ///   in the `ParanoidArgs`.
    pub fn generate_pid(
        &'a self,
        parsed_graphs: &'a [ParsedCodeGraph],
    ) -> Result<TestInfo<'a>, SynParserError> {
        // 1. Construct the absolute expected file path
        let fixture_root = fixtures_crates_dir().join(self.fixture);
        let target_file_path = fixture_root.join(self.relative_file_path);

        // 2. Find the specific ParsedCodeGraph for the target file
        let target_data = parsed_graphs
            .iter()
            .find(|data| data.file_path == target_file_path)
            .unwrap_or_else(|| {
                panic!(
                    "ParsedCodeGraph for '{}' not found in results",
                    target_file_path.display()
                )
            });
        let graph = &target_data.graph;
        let exp_path_string = self
            .expected_path
            .iter()
            .copied()
            .map(|s| s.to_string())
            .collect_vec();

        let cfgs_bytes_option: Option<Vec<u8>> = self
            .expected_cfg
            .filter(|cfgs_slice| !cfgs_slice.is_empty()) // Only proceed if there are actual CFG strings
            .and_then(|cfgs_slice| calculate_cfg_hash_bytes(&strs_to_strings(cfgs_slice))); // Results in None if expected_cfg is None, or if cfgs_slice is empty, or if calculate_cfg_hash_bytes returns None.

        let actual_parent_scope_id_for_id_gen = match graph
            .find_module_by_path_checked(&exp_path_string)
        {
            Ok(parent_module) => Some(parent_module.id.base_tid()),
            Err(_) => {
                graph.find_module_by_file_path_checked(path::Path::new(self.relative_file_path))?;
                None
            }
        };

        // let actual_parent_scope_id_for_id_gen = Some(parent_module.id.base_tid());
        let actual_cfg_bytes_for_id_gen = cfgs_bytes_option.as_deref();

        // New structured logging:
        if log::log_enabled!(target: LOG_TEST_ID_REGEN, log::Level::Debug) {
            // Check if specific log is enabled
            log::debug!(target: LOG_TEST_ID_REGEN, "ParanoidArgs::generate_pid");
            log::debug!(target: LOG_TEST_ID_REGEN,
                "  Inputs for {} ({:?}):\n    crate_namespace: {}\n    file_path: {:?}\n    relative_path: {:?}\n    item_name: {}\n    item_kind: {:?}\n    parent_scope_id: {:?}\n    cfg_bytes: {:?}",
                self.ident,
                self.item_kind, // Use self.item_kind from ParanoidArgs
                target_data.crate_namespace,
                &target_file_path,
                &exp_path_string, // This is the 'relative_path' for the item's ID context
                self.ident,
                self.item_kind, // Use self.item_kind from ParanoidArgs
                actual_parent_scope_id_for_id_gen,
                actual_cfg_bytes_for_id_gen
            );
        }

        let generated_id = NodeId::generate_synthetic(
            target_data.crate_namespace,
            &target_file_path,
            &exp_path_string,
            self.ident,
            self.item_kind,                    // Use self.item_kind from ParanoidArgs
            actual_parent_scope_id_for_id_gen, // Use the determined parent scope ID
            actual_cfg_bytes_for_id_gen,       // Use the determined CFG bytes
        );

        let pid = match self.item_kind {
            ItemKind::Function => FunctionNodeId::new_test(generated_id).into(),
            ItemKind::Struct => StructNodeId::new_test(generated_id).into(),
            ItemKind::Enum => EnumNodeId::new_test(generated_id).into(),
            ItemKind::Union => UnionNodeId::new_test(generated_id).into(),
            ItemKind::TypeAlias => TypeAliasNodeId::new_test(generated_id).into(),
            ItemKind::Trait => TraitNodeId::new_test(generated_id).into(),
            ItemKind::Impl => ImplNodeId::new_test(generated_id).into(),
            ItemKind::Module => ModuleNodeId::new_test(generated_id).into(),
            ItemKind::Const => ConstNodeId::new_test(generated_id).into(),
            ItemKind::Static => StaticNodeId::new_test(generated_id).into(),
            ItemKind::Macro => MacroNodeId::new_test(generated_id).into(),
            ItemKind::Import => ImportNodeId::new_test(generated_id).into(),
            // TODO: Decide what to do about handling ExternCrate. We kind of do want everything to
            // have a NodeId of some kind, and this will do for now, but we also want to
            // distinguish between an ExternCrate statement and something else... probably.
            ItemKind::ExternCrate => ImportNodeId::new_test(generated_id).into(),
            _ => {
                panic!("You can't use this test helper on Secondary/Assoc nodes, at least not yet.")
            }
        };

        let test_info = TestInfo {
            args: self,
            target_data,
            test_pid: pid,
        };
        Ok(test_info)
    }
}

pub const LOG_PARANOID_CHECK: &str = "log_paranoid_check";

impl ParanoidArgs<'_> {
    /// Logs the expected information stored in these ParanoidArgs.
    /// Returns a SynParserError if checks fail (e.g., no root module, duplicate root modules).
    pub fn check_graph(&self, parsed: &ParsedCodeGraph) -> Result<(), SynParserError> {
        // Changed return type
        let exp_fp = self.relative_file_path;

        log::debug!(target: LOG_PARANOID_CHECK,
            "{}\n{}: {}\n\t{}: {}\n\t{}: {}",
                "Checking Graph".log_header(),
                "Expected Relative File Path: ".log_step(),
                "Expected Relative File Path: ".log_step(),
                exp_fp.log_path(),
                parsed.file_path.to_str().unwrap().log_path(),
                "Match?".log_orange(),
                parsed.file_path.ends_with(exp_fp).to_string().log_orange(),
        );

        let mut file_modules = parsed.modules().iter().filter(|m| m.is_file_based());
        // Use ok_or to handle potential absence of a file-based root module gracefully
        let local_root_module = file_modules.next().ok_or_else(|| {
            SynParserError::InternalState(format!(
                "No file-based root module found in ParsedCodeGraph for file: {}",
                parsed.file_path.display()
            ))
        })?;
        log::debug!(target: LOG_PARANOID_CHECK,
            "\t{}: \n\t{} \n\t{}\n\t{}: {}",
                "Root Module Path of Graph".log_step(),
                self.expected_path.join("::").log_path(),
                local_root_module.path().join("::").log_path(),
                "Match?".log_orange(),
                self.expected_path == local_root_module.path(),
        );
        // Check for duplicate file-based modules and return an error instead of panicking
        if let Some(dup_local_file_mod) = file_modules.next() {
            Err(SynParserError::InternalState(format!(
                "Duplicate file-level module found in ParsedCodeGraph for file '{}': First: '{}' ({}), Second: '{}' ({})",
                parsed.file_path.display(),
                local_root_module.name,
                local_root_module.id,
                dup_local_file_mod.name,
                dup_local_file_mod.id
            )))
        } else {
            Ok(()) // Return Ok if no duplicates found
        }
    }
}

/// Helper test function.
/// Creates a new Attribute representing `#[path = "some_value"]`.
pub fn new_path_attribute(value: &str) -> Attribute {
    // Note: This does not exist in the `Attrbiute` definition in the actual crate because there
    // should not be a case in which we are creating an attribute, we only parse them.
    Attribute {
        name: "path".to_string(),
        args: Vec::new(),
        value: Some(value.to_string()),
    }
}

/// Helper to find a TypeNode by its ID. Panics if not found.
pub fn find_type_node(graph: &CodeGraph, type_id: TypeId) -> &TypeNode {
    graph
        .type_graph
        .iter()
        .find(|tn| tn.id == type_id)
        .unwrap_or_else(|| panic!("TypeNode not found for TypeId: {}", type_id))
}

#[derive(Debug, Clone)]
pub struct TestInfo<'a> {
    args: &'a ParanoidArgs<'a>,
    target_data: &'a ParsedCodeGraph,
    test_pid: PrimaryNodeId,
}

impl<'a> TestInfo<'a> {
    pub fn new(
        args: &'a ParanoidArgs,
        target_data: &'a ParsedCodeGraph,
        test_pid: PrimaryNodeId,
    ) -> Self {
        Self {
            args,
            target_data,
            test_pid,
        }
    }

    pub fn args(&self) -> &ParanoidArgs<'a> {
        self.args
    }

    pub fn target_data(&self) -> &ParsedCodeGraph {
        self.target_data
    }

    pub fn test_pid(&self) -> PrimaryNodeId {
        self.test_pid
    }
}

fn strs_to_strings(strs: &[&str]) -> Vec<String> {
    strs.iter().copied().map(String::from).collect()
}

use {
    std::path::PathBuf, syn_parser::discovery::run_discovery_phase,
    syn_parser::parser::analyze_files_parallel,
};

use ploke_common::{fixtures_crates_dir, workspace_root};
pub use resolution::build_tree_for_tests;

pub mod uuid_ids_utils;

pub fn run_phase1_phase2(fixture_name: &str) -> Vec<Result<ParsedCodeGraph, SynParserError>> {
    let crate_path = fixtures_crates_dir().join(fixture_name);
    let discovery_output = run_discovery_phase(&PathBuf::from("."), &[crate_path]) // Adjust project_root if needed
        .expect("Phase 1 Discovery failed");
    analyze_files_parallel(&discovery_output, 0) // num_workers often ignored by rayon bridge
}

#[test]
#[cfg(not(feature = "type_bearing_ids"))]
fn test_paths() {
    let fixture_path = fixtures_dir().join("my_fixture.rs");
    println!("Fixture path: {}", fixture_path.display());
}

pub fn print_typedef_names(code_graph: &CodeGraph) -> Vec<&str> {
    code_graph
        .defined_types
        .iter()
        .map(|t| match t {
            TypeDefNode::Struct(struct_node) => &struct_node.name,
            TypeDefNode::Enum(enum_node) => &enum_node.name,
            TypeDefNode::TypeAlias(type_alias_node) => &type_alias_node.name,
            TypeDefNode::Union(union_node) => union_node.name.as_str(),
        })
        .collect::<Vec<&str>>()
}

pub const FIXTURES_DIR: &str = "tests/fixtures";

#[derive(Error, Debug)]
pub enum TestError {
    #[error(transparent)]
    FixtureError(#[from] FixtureError),
    #[error(transparent)]
    SmokeTestError(#[from] SmokeTestError),
}

#[derive(Error, Debug)]
pub enum FixtureError {
    #[error("IO error reading fixture: {0}")]
    Io(#[from] std::io::Error),
    #[error("Parse error in fixture: {0}")]
    Parse(#[from] syn::Error),
    #[error("Fixture not found: {0}")]
    NotFound(String),
    #[error("Invalid UTF-8 in fixture: {0}")]
    Utf8(#[from] std::string::FromUtf8Error),
    #[error("Test assertion failed: {0}")]
    Assertion(String),
}

/// Errors specific to the setup or execution of test helper functions.
#[derive(Error, Debug, Clone, PartialEq)]
pub enum TestHelperError {
    #[error("Invalid argument provided to test helper: {0}")]
    InvalidArgument(String),

    #[error(
        "Could not find ParsedCodeGraph for fixture '{fixture_name}' file '{relative_file_path}'"
    )]
    FixtureGraphNotFound {
        fixture_name: String,
        relative_file_path: String,
    },

    #[error("Expected path was empty")]
    EmptyExpectedPath,

    #[error("Unsupported ItemKind for helper: {0:?}")]
    UnsupportedItemKind(ItemKind),
    // Add other helper-specific errors as needed
}

/// A small group of errors that are indicative of some basic properties of the nodes or the
/// fixture being broken. Should only be used rarely and with care.
#[derive(Error, Debug)]
pub enum SmokeTestError {
    /// A basic test for the name of a given node, where the name is the visible name. This is a
    /// smoke test and it should not be taken as a thought indication of uniqueness.
    #[error("Fixture not found: {0}")]
    NotFoundByName(String),
}

/// Helper to assert a condition with a descriptive error             
pub fn assert_fixture<T>(condition: bool, message: &str, ok_value: T) -> Result<T, FixtureError> {
    if condition {
        Ok(ok_value)
    } else {
        Err(FixtureError::Assertion(message.to_string()))
    }
}

/// Finds the NodeId of an ImportNode representing a re-export based on its visible name
/// within a specific module.
pub fn find_reexport_import_node_by_name(
    graph: &ParsedCodeGraph,
    module_path: &[String],
    visible_name: &str,
) -> Result<ImportNodeId, SynParserError> {
    // Find the module where the re-export is defined
    let module_node = graph.find_module_by_path_checked(module_path)?;

    // Search through all use statements in the graph
    graph
        .use_statements()
        .iter()
        .find(|imp| {
            // Check if the import has the correct visible name
            imp.visible_name == visible_name &&
            // Check if this import is contained within the target module
            graph.module_contains_node(module_node.id, imp.id.to_pid()) &&
            // Ensure it's actually a re-export (pub use, pub(crate) use, etc.)
            imp.is_any_reexport()
        })
        .map(|imp| imp.id) // Get the NodeId if found
        .ok_or_else(|| {
            SynParserError::ItemPathNotFound(module_path.to_vec()) // Placeholder error
        })
}

/// Find a struct by name in the code graph
pub fn find_struct_by_name<'a>(graph: &'a CodeGraph, name: &str) -> Option<&'a StructNode> {
    graph.defined_types.iter().find_map(|def| {
        if let TypeDefNode::Struct(s) = def {
            if s.name == name {
                return Some(s);
            }
        }
        None
    })
}

/// Find an enum by name in the code graph
pub fn find_enum_by_name<'a>(graph: &'a CodeGraph, name: &str) -> Option<&'a EnumNode> {
    graph.defined_types.iter().find_map(|def| {
        if let TypeDefNode::Enum(e) = def {
            if e.name == name {
                return Some(e);
            }
        }
        None
    })
}

/// Find a trait by name in the code graph
pub fn find_trait_by_name<'a>(graph: &'a CodeGraph, name: &str) -> Option<&'a TraitNode> {
    graph.traits.iter().find(|t| t.name == name)
}

/// Find a function by name in the code graph
pub fn find_function_by_name<'a>(graph: &'a CodeGraph, name: &str) -> Option<&'a FunctionNode> {
    if let Some(func) = graph.functions.iter().find(|f| f.name == name) {
        return Some(func);
    }

    if let Some(use_stmt) = graph
        .use_statements
        .iter()
        .find(|u| u.visible_name == name && !u.is_glob)
    {
        if let Some(original_name) = &use_stmt.original_name {
            // The last segment of the path should be the original name
            return graph.functions.iter().find(|f| &f.name == original_name);
        } else {
            // for non-renamed imports, use the last path segment
            let original_name = use_stmt.source_path.last()?;
            return graph.functions.iter().find(|f| &f.name == original_name);
        }
    }
    None
}

/// Reads bytes from a file at given positions
pub fn read_byte_range(path: &Path, start: usize, end: usize) -> String {
    let mut file = File::open(path).expect("Failed to open file");
    let mut buffer = vec![0; end - start];
    file.seek(std::io::SeekFrom::Start(start as u64))
        .expect("Failed to seek");
    file.read_exact(&mut buffer).expect("Failed to read bytes");
    String::from_utf8(buffer).expect("Invalid UTF-8 in span")
}

/// Verifies that a parsed item's span matches the expected text
pub fn verify_span(item: &impl ExtractSpan, path: &Path, expected: &str) {
    let (start, end) = item.extract_span_bytes();
    let actual = read_byte_range(path, start, end);

    assert_eq!(
        actual,
        expected,
        "\nSpan mismatch in {}:\nExpected:\n{}\nActual:\n{}\n",
        path.display(),
        expected,
        actual
    );
}

pub fn find_generic_param_by_name<'a>(
    params: &'a [GenericParamNode],
    name: &str,
) -> Option<&'a GenericParamNode> {
    params.iter().find(|param| match &param.kind {
        GenericParamKind::Lifetime {
            name: param_name, ..
        } => param_name == name,
        _ => false,
    })
}

/// Helper to create module path for tests
#[cfg(not(feature = "type_bearing_ids"))]
pub fn test_module_path(segments: &[&str]) -> Vec<String> {
    segments.iter().map(|s| s.to_string()).collect()
}

pub fn find_impl_for_type<'a>(graph: &'a CodeGraph, type_name: &str) -> Option<&'a ImplNode> {
    graph.impls.iter().find(|i| {
        if let Some(self_type) = graph
            .type_graph
            .iter()
            .find(|t| t.id == i.self_type && i.trait_type.is_none())
        {
            if let TypeKind::Named { path, .. } = &self_type.kind {
                return path.last().map(|s| s == type_name).unwrap_or(false);
            }
        }
        false
    })
}
