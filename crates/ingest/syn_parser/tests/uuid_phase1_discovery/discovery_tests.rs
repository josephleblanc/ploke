#![cfg(feature = "uuid_ids")]

use crate::common::FixtureError; // Use common error type if applicable
use ploke_common::workspace_root;
use std::{
    collections::HashMap,
    fs::{self, File},
    io::Write,
    path::PathBuf,
};
use syn_parser::discovery::{
    derive_crate_namespace, run_discovery_phase, CrateContext, DiscoveryError, DiscoveryOutput,
    PROJECT_NAMESPACE_UUID,
};
use tempfile::tempdir;
use uuid::Uuid;

// --- Unit Tests ---

#[test]
fn test_derive_crate_namespace_consistency() {
    let name = "my-crate";
    let version = "1.2.3";
    let uuid1 = derive_crate_namespace(name, version);
    let uuid2 = derive_crate_namespace(name, version);
    assert_eq!(uuid1, uuid2, "UUID should be consistent for same input");
}

#[test]
fn test_derive_crate_namespace_uniqueness() {
    let uuid1 = derive_crate_namespace("crate-a", "1.0.0");
    let uuid2 = derive_crate_namespace("crate-b", "1.0.0");
    let uuid3 = derive_crate_namespace("crate-a", "1.0.1");
    assert_ne!(uuid1, uuid2, "Different names should yield different UUIDs");
    assert_ne!(
        uuid1, uuid3,
        "Different versions should yield different UUIDs"
    );
}

#[test]
fn test_run_discovery_phase_valid_crate() -> Result<(), Box<dyn std::error::Error>> {
    let temp_dir = tempdir()?;
    let crate_root = temp_dir.path().join("test_crate");
    let src_dir = crate_root.join("src");
    fs::create_dir_all(&src_dir)?;

    // Create Cargo.toml
    let cargo_content = r#"
[package]
name = "test_crate"
version = "0.1.0"
edition = "2021"
"#;
    fs::write(crate_root.join("Cargo.toml"), cargo_content)?;

    // Create source files
    fs::write(src_dir.join("lib.rs"), "fn main() {}")?;
    fs::write(src_dir.join("module.rs"), "pub struct Test;")?;
    fs::write(src_dir.join("other.txt"), "ignore me")?; // Non-rs file

    let project_root = temp_dir.path().to_path_buf(); // Dummy project root
    let target_crates = vec![crate_root.clone()];

    let result = run_discovery_phase(&project_root, &target_crates);

    assert!(result.is_ok(), "Discovery should succeed for valid crate");
    let output = result.unwrap();

    assert_eq!(output.crate_contexts.len(), 1);
    let context = output.crate_contexts.get("test_crate").unwrap();

    assert_eq!(context.name, "test_crate");
    assert_eq!(context.version, "0.1.0");
    assert_eq!(context.root_path, crate_root);
    assert_eq!(
        context.namespace,
        derive_crate_namespace("test_crate", "0.1.0")
    );

    assert_eq!(context.files.len(), 2);
    assert!(context.files.contains(&src_dir.join("lib.rs")));
    assert!(context.files.contains(&src_dir.join("module.rs")));
    assert!(!context.files.contains(&src_dir.join("other.txt"))); // Ensure non-rs is excluded

    // TODO: Add check for initial_module_map once scan_for_mods is integrated

    Ok(())
}

#[test]
fn test_run_discovery_phase_missing_cargo_toml() -> Result<(), Box<dyn std::error::Error>> {
    let temp_dir = tempdir()?;
    let crate_root = temp_dir.path().join("test_crate");
    fs::create_dir_all(&crate_root)?; // No Cargo.toml

    let project_root = temp_dir.path().to_path_buf();
    let target_crates = vec![crate_root.clone()];

    let result = run_discovery_phase(&project_root, &target_crates);

    assert!(
        result.is_err(),
        "Discovery should fail if Cargo.toml is missing"
    );
    let errors = result.unwrap_err();
    assert_eq!(errors.len(), 1);
    assert!(matches!(errors[0], DiscoveryError::Io { .. })); // Expecting IO error reading Cargo.toml

    Ok(())
}

#[test]
fn test_run_discovery_phase_invalid_cargo_toml() -> Result<(), Box<dyn std::error::Error>> {
    let temp_dir = tempdir()?;
    let crate_root = temp_dir.path().join("test_crate");
    fs::create_dir_all(&crate_root)?;

    // Create invalid Cargo.toml
    fs::write(crate_root.join("Cargo.toml"), "this is not valid toml")?;

    let project_root = temp_dir.path().to_path_buf();
    let target_crates = vec![crate_root.clone()];

    let result = run_discovery_phase(&project_root, &target_crates);

    assert!(
        result.is_err(),
        "Discovery should fail for invalid Cargo.toml"
    );
    let errors = result.unwrap_err();
    assert_eq!(errors.len(), 1);
    assert!(matches!(errors[0], DiscoveryError::TomlParse { .. }));

    Ok(())
}

#[test]
fn test_run_discovery_phase_missing_src() -> Result<(), Box<dyn std::error::Error>> {
    let temp_dir = tempdir()?;
    let crate_root = temp_dir.path().join("test_crate");
    fs::create_dir_all(&crate_root)?;

    // Create Cargo.toml but no src directory
    let cargo_content = r#"
[package]
name = "test_crate_no_src"
version = "0.1.0"
"#;
    fs::write(crate_root.join("Cargo.toml"), cargo_content)?;

    let project_root = temp_dir.path().to_path_buf();
    let target_crates = vec![crate_root.clone()];

    let result = run_discovery_phase(&project_root, &target_crates);

    assert!(result.is_err(), "Discovery should fail if src is missing");
    let errors = result.unwrap_err();
    assert_eq!(errors.len(), 1);
    assert!(matches!(errors[0], DiscoveryError::SrcNotFound { .. }));

    Ok(())
}

#[test]
fn test_run_discovery_phase_crate_path_not_found() -> Result<(), Box<dyn std::error::Error>> {
    let temp_dir = tempdir()?;
    let crate_root = temp_dir.path().join("non_existent_crate"); // Path does not exist

    let project_root = temp_dir.path().to_path_buf();
    let target_crates = vec![crate_root.clone()];

    let result = run_discovery_phase(&project_root, &target_crates);

    assert!(
        result.is_err(),
        "Discovery should fail if crate path doesn't exist"
    );
    let errors = result.unwrap_err();
    assert_eq!(errors.len(), 1);
    assert!(matches!(
        errors[0],
        DiscoveryError::CratePathNotFound { .. }
    ));

    Ok(())
}

#[test]
fn test_run_discovery_phase_multiple_crates() -> Result<(), Box<dyn std::error::Error>> {
    let temp_dir = tempdir()?;
    let project_root = temp_dir.path().to_path_buf();

    // Crate 1 (Valid)
    let crate1_root = temp_dir.path().join("crate1");
    let crate1_src = crate1_root.join("src");
    fs::create_dir_all(&crate1_src)?;
    fs::write(
        crate1_root.join("Cargo.toml"),
        "[package]\nname = \"crate1\"\nversion = \"1.0\"",
    )?;
    fs::write(crate1_src.join("lib.rs"), "")?;

    // Crate 2 (Valid)
    let crate2_root = temp_dir.path().join("crate2");
    let crate2_src = crate2_root.join("src");
    fs::create_dir_all(&crate2_src)?;
    fs::write(
        crate2_root.join("Cargo.toml"),
        "[package]\nname = \"crate2\"\nversion = \"2.0\"",
    )?;
    fs::write(crate2_src.join("main.rs"), "")?;

    // Crate 3 (Missing src)
    let crate3_root = temp_dir.path().join("crate3");
    fs::create_dir_all(&crate3_root)?;
    fs::write(
        crate3_root.join("Cargo.toml"),
        "[package]\nname = \"crate3\"\nversion = \"3.0\"",
    )?;

    let target_crates = vec![
        crate1_root.clone(),
        crate2_root.clone(),
        crate3_root.clone(),
    ];
    let result = run_discovery_phase(&project_root, &target_crates);

    assert!(result.is_err(), "Should return error due to crate3");
    let errors = result.unwrap_err();
    assert_eq!(errors.len(), 1, "Only crate3 should produce an error");
    assert!(matches!(errors[0], DiscoveryError::SrcNotFound { .. }));

    // We could potentially check that the *output* contains crate1 and crate2
    // if the function were modified to return partial results on error,
    // but current design returns Err if *any* crate fails critically.

    Ok(())
}

// --- Integration Test ---

#[test]
fn test_discovery_on_fixture_crate() -> Result<(), Box<dyn std::error::Error>> {
    // Get the absolute path to the fixture_test_crate within the workspace
    let fixture_crate_root = workspace_root().join("fixture_test_crate");
    let project_root = workspace_root(); // Use workspace root as project root

    assert!(
        fixture_crate_root.exists(),
        "Fixture test crate not found at {}",
        fixture_crate_root.display()
    );
    assert!(
        fixture_crate_root.join("Cargo.toml").exists(),
        "Fixture test crate Cargo.toml not found"
    );
    assert!(
        fixture_crate_root.join("src").exists(),
        "Fixture test crate src directory not found"
    );

    let target_crates = vec![fixture_crate_root.clone()];
    let result = run_discovery_phase(&project_root, &target_crates);

    if let Err(ref errors) = result {
        eprintln!("Discovery failed with errors: {:?}", errors);
    }
    assert!(
        result.is_ok(),
        "Discovery should succeed for fixture_test_crate"
    );
    let output = result.unwrap();

    assert_eq!(output.crate_contexts.len(), 1);
    let context = output
        .crate_contexts
        .get("fixture_test_crate")
        .expect("Context for fixture_test_crate not found");

    assert_eq!(context.name, "fixture_test_crate");
    assert_eq!(context.version, "0.1.0"); // Assuming this version in fixture Cargo.toml
    assert_eq!(context.root_path, fixture_crate_root);

    // Check namespace derivation
    let expected_namespace = derive_crate_namespace("fixture_test_crate", "0.1.0");
    assert_eq!(context.namespace, expected_namespace);

    // Check file discovery (add more specific files as needed)
    let src_path = fixture_crate_root.join("src");
    let expected_files = vec![
        src_path.join("main.rs"),
        src_path.join("second_sibling.rs"),
        src_path.join("sibling_of_main.rs"),
        // Add other expected .rs files from the fixture crate here
    ];
    assert_eq!(
        context.files.len(),
        expected_files.len(),
        "Incorrect number of .rs files found"
    );
    for expected_file in &expected_files {
        assert!(
            context.files.contains(expected_file),
            "Expected file not found: {}",
            expected_file.display()
        );
    }
    let unexpected_files = context
        .files
        .iter()
        .filter(|cf| !expected_files.contains(cf))
        .collect::<Vec<_>>();
    assert!(
        !unexpected_files.is_empty(),
        "Unexpected file found: {:#?}",
        unexpected_files
    );

    // TODO: Check initial_module_map once scan_for_mods is integrated
    // Example assertion (adjust based on actual scan_for_mods logic):
    // let second_sibling_path = src_path.join("second_sibling.rs");
    // assert!(output.initial_module_map.contains_key(&second_sibling_path));
    // assert_eq!(output.initial_module_map[&second_sibling_path], vec!["crate", "second_sibling"]);

    Ok(())
}

// TODO: Add tests for scan_for_mods once it's integrated into run_discovery_phase
// #[test]
// fn test_scan_for_mods_basic() -> Result<(), Box<dyn std::error::Error>> { ... }
