#![cfg(feature = "uuid_ids")]

// Removed unused imports: FixtureError, HashMap, File, Write, PathBuf,
// CrateContext, DiscoveryOutput, PROJECT_NAMESPACE_UUID, Uuid
use ploke_common::workspace_root;
use std::fs;
use syn_parser::discovery::{
    derive_crate_namespace, run_discovery_phase, DiscoveryError,
};
use tempfile::tempdir;

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

    assert!(result.is_ok(), "Discovery should succeed for valid crate, got: {:?}", result.err());
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

    // Check initial_module_map (assuming lib.rs contains `mod module;`)
    // Create lib.rs with mod declaration BEFORE running discovery
    fs::write(src_dir.join("lib.rs"), "mod module;")?;
    let result = run_discovery_phase(&project_root, &target_crates); // Re-run after modifying lib.rs
    assert!(result.is_ok(), "Discovery should still succeed, got: {:?}", result.err());
    let output = result.unwrap();

    let module_file_path = src_dir.join("module.rs");
    assert!(output.initial_module_map.contains_key(&module_file_path), "Module map should contain module.rs");
    assert_eq!(output.initial_module_map[&module_file_path], vec!["crate".to_string(), "module".to_string()]);


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

    assert!(result.is_err(), "Discovery should fail if Cargo.toml is missing");
    let err = result.unwrap_err();
    assert!(matches!(err, DiscoveryError::Io { ref path, .. } if path.ends_with("Cargo.toml")));

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

    assert!(result.is_err(), "Discovery should fail for invalid Cargo.toml");
    let err = result.unwrap_err();
    assert!(matches!(err, DiscoveryError::TomlParse { .. }));

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
    let err = result.unwrap_err();
    assert!(matches!(err, DiscoveryError::SrcNotFound { .. }));

    Ok(())
}

#[test]
fn test_run_discovery_phase_crate_path_not_found() -> Result<(), Box<dyn std::error::Error>> {
    let temp_dir = tempdir()?;
    let crate_root = temp_dir.path().join("non_existent_crate"); // Path does not exist

    let project_root = temp_dir.path().to_path_buf();
    let target_crates = vec![crate_root.clone()];

    let result = run_discovery_phase(&project_root, &target_crates);

    assert!(result.is_err(), "Discovery should fail if crate path doesn't exist");
    let err = result.unwrap_err();
    assert!(matches!(err, DiscoveryError::CratePathNotFound { .. }));

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
        crate3_root.clone(), // Missing src
    ];
    let result = run_discovery_phase(&project_root, &target_crates);

    // Check for error (should fail because of crate3)
    assert!(result.is_err(), "Should fail due to crate3 missing src");
    let err = result.unwrap_err();
    assert!(matches!(err, DiscoveryError::SrcNotFound { ref path, .. } if path.ends_with("crate3/src")));

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

    assert!(result.is_ok(), "Discovery should succeed for fixture_test_crate, got: {:?}", result.err());
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
        .cloned() // Clone paths for assertion message
        .collect::<Vec<_>>();
    assert!(
        unexpected_files.is_empty(), // Assert that the list IS empty
        "Unexpected files found: {:#?}", // Corrected message
        unexpected_files
    );

    // Check initial_module_map based on fixture_test_crate/src/main.rs
    let second_sibling_path = src_path.join("second_sibling.rs");
    let sibling_of_main_path = src_path.join("sibling_of_main.rs");

    assert!(output.initial_module_map.contains_key(&second_sibling_path), "Module map missing second_sibling.rs");
    assert_eq!(output.initial_module_map[&second_sibling_path], vec!["crate".to_string(), "second_sibling".to_string()]);

    assert!(output.initial_module_map.contains_key(&sibling_of_main_path), "Module map missing sibling_of_main.rs");
     assert_eq!(output.initial_module_map[&sibling_of_main_path], vec!["crate".to_string(), "sibling_of_main".to_string()]);

    // Check that modules declared in other files (like second_sibling.rs) are NOT in the map yet
    // (as scan_for_mods only runs on lib.rs/main.rs in this phase)
    let intermediate_path = src_path.join("intermediate").join("mod.rs"); // Assuming this structure exists or similar
    assert!(!output.initial_module_map.contains_key(&intermediate_path), "Module map should not contain mods from non-entry points yet");

    Ok(())
}

// --- Tests for scan_for_mods ---
// Note: scan_for_mods is private, so we test it indirectly via run_discovery_phase
// or we could make it pub(crate) for testing if needed.
// The test `test_run_discovery_phase_valid_crate` and `test_discovery_on_fixture_crate`
// now cover the basic integration of scan_for_mods.

// Example of a dedicated test if scan_for_mods were made pub(crate)
/*
#[test]
fn test_scan_for_mods_basic() -> Result<(), Box<dyn std::error::Error>> {
    let temp_dir = tempdir()?;
    let src_dir = temp_dir.path();
    let file_to_scan = src_dir.join("lib.rs");
    let mod1_file = src_dir.join("mod1.rs");
    let mod2_dir = src_dir.join("mod2");
    let mod2_file = mod2_dir.join("mod.rs");
    fs::create_dir(&mod2_dir)?;

    fs::write(&file_to_scan, "mod mod1;\npub mod mod2;")?;
    fs::write(&mod1_file, "// mod1 content")?;
    fs::write(&mod2_file, "// mod2 content")?;

    let existing_files = vec![file_to_scan.clone(), mod1_file.clone(), mod2_file.clone()];

    // Assuming scan_for_mods is made pub(crate) or called via a helper
    // let mod_map = syn_parser::discovery::scan_for_mods(&file_to_scan, src_dir, &existing_files)?;

    // assert_eq!(mod_map.len(), 2);
    // assert!(mod_map.contains_key(&mod1_file));
    // assert_eq!(mod_map[&mod1_file], vec!["crate".to_string(), "mod1".to_string()]);
    // assert!(mod_map.contains_key(&mod2_file));
    // assert_eq!(mod_map[&mod2_file], vec!["crate".to_string(), "mod2".to_string()]);

    Ok(())
}
*/
