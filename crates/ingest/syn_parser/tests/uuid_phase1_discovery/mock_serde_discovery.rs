//! Discovery tests for the mock_serde workspace fixture.
//!
//! This test file verifies that `run_discovery_phase` correctly discovers the
//! mock_serde workspace structure, which mimics the real serde workspace with
//! its non-standard crate layouts.
//!
//! ## Fixture Structure (Expected)
//!
//! ```text
//! tests/fixture_workspace/fixture_mock_serde/
//! ├── Cargo.toml                    # Workspace manifest
//! ├── mock_serde/
//! │   ├── Cargo.toml
//! │   └── src/
//! │       ├── lib.rs
//! │       └── core/
//! │           ├── crate_root.rs     # #[path = "..."] module
//! │           └── macros.rs         # #[path = "..."] module
//! ├── mock_serde_core/
//! │   ├── Cargo.toml
//! │   └── src/
//! │       └── lib.rs
//! ├── mock_serde_derive/
//! │   ├── Cargo.toml
//! │   └── src/
//! │       └── lib.rs
//! └── mock_serde_derive_internals/  # NON-STANDARD LAYOUT
//!     ├── Cargo.toml                # Has [lib] path = "lib.rs"
//!     ├── lib.rs                    # Library root at crate root (not in src/)
//!     └── src/                      # Additional source directory
//!         └── mod.rs                # Symlinked or additional module files
//! ```
//!
//! ## Key Test Scenarios
//!
//! 1. **Basic Discovery**: All 4 crate contexts are found
//! 2. **File Discovery**: Standard src/ layout files discovered
//! 3. **Path Attribute Discovery**: #[path] module files discovered
//! 4. **Non-standard Layout**: lib.rs at crate root (not in src/)
//! 5. **Workspace Metadata**: Correct workspace members and paths

use ploke_common::workspace_root;
use syn_parser::discovery::{run_discovery_phase, DiscoveryError, DiscoveryOutput};
use std::path::PathBuf;

/// Returns the absolute path to the fixture_mock_serde workspace root.
fn fixture_mock_serde_path() -> PathBuf {
    workspace_root().join("tests/fixture_workspace/fixture_mock_serde")
}

/// Returns the paths to all 4 crate roots in the mock_serde workspace.
fn mock_serde_crate_paths() -> Vec<PathBuf> {
    let workspace_root = fixture_mock_serde_path();
    vec![
        workspace_root.join("mock_serde"),
        workspace_root.join("mock_serde_core"),
        workspace_root.join("mock_serde_derive"),
        workspace_root.join("mock_serde_derive_internals"),
    ]
}

/// Helper to run discovery on the mock_serde workspace.
fn run_mock_serde_discovery() -> Result<DiscoveryOutput, DiscoveryError> {
    let workspace_path = fixture_mock_serde_path();
    let target_crates = mock_serde_crate_paths();
    run_discovery_phase(Some(&workspace_path), &target_crates)
}

/// Test that discovery finds all 4 crates in the mock_serde workspace.
#[test]
fn test_mock_serde_discovery_finds_all_crates() {
    // Skip test if fixture doesn't exist (not yet created)
    let fixture_path = fixture_mock_serde_path();
    if !fixture_path.exists() {
        eprintln!(
            "Skipping test: fixture_mock_serde not found at {}",
            fixture_path.display()
        );
        return;
    }

    let output = run_mock_serde_discovery().expect("Discovery should succeed for mock_serde");

    // Assert all 4 crate contexts are found
    assert_eq!(
        output.crate_contexts.len(),
        4,
        "Expected exactly 4 crate contexts (mock_serde, mock_serde_core, mock_serde_derive, mock_serde_derive_internals)"
    );

    // Verify each expected crate is present
    let expected_crate_names = vec![
        "mock_serde",
        "mock_serde_core",
        "mock_serde_derive",
        "mock_serde_derive_internals",
    ];

    for crate_path in mock_serde_crate_paths() {
        let context = output
            .crate_contexts
            .get(&crate_path)
            .expect(&format!("Context for crate at {:?} should exist", crate_path));

        let expected_name = crate_path
            .file_name()
            .expect("Crate path should have a file name")
            .to_str()
            .expect("Crate name should be valid UTF-8");

        assert_eq!(
            context.name, expected_name,
            "Crate name should match directory name"
        );
        assert_eq!(
            context.root_path, crate_path,
            "Root path should match the crate path"
        );
        assert!(
            !context.files.is_empty(),
            "Each crate should have at least one discovered file"
        );
    }

    // Verify the specific crate names match expectations
    let actual_names: Vec<String> = output
        .crate_contexts
        .values()
        .map(|ctx| ctx.name.clone())
        .collect();

    for expected in &expected_crate_names {
        assert!(
            actual_names.contains(&expected.to_string()),
            "Expected crate '{}' not found in discovered crates: {:?}",
            expected,
            actual_names
        );
    }
}

/// Test that discovery finds expected files including #[path] modules.
#[test]
fn test_mock_serde_discovery_finds_expected_files() {
    // Skip test if fixture doesn't exist
    let fixture_path = fixture_mock_serde_path();
    if !fixture_path.exists() {
        eprintln!(
            "Skipping test: fixture_mock_serde not found at {}",
            fixture_path.display()
        );
        return;
    }

    let output = run_mock_serde_discovery().expect("Discovery should succeed");

    let mock_serde_root = fixture_mock_serde_path().join("mock_serde");
    let mock_serde_src = mock_serde_root.join("src");
    let mock_serde_core = mock_serde_src.join("core");

    // Get the mock_serde crate context
    let mock_serde_ctx = output
        .crate_contexts
        .get(&mock_serde_root)
        .expect("mock_serde crate context should exist");

    // Verify standard files are discovered
    assert!(
        mock_serde_ctx.files.contains(&mock_serde_src.join("lib.rs")),
        "mock_serde/src/lib.rs should be discovered"
    );

    // Verify #[path] module files are discovered
    // These would be referenced in lib.rs like: #[path = "core/crate_root.rs"] mod crate_root;
    let crate_root_path = mock_serde_core.join("crate_root.rs");
    let macros_path = mock_serde_core.join("macros.rs");

    // Note: These assertions assume the discovery mechanism scans for #[path] attributes
    // If the fixture uses #[path] attributes, these files should be discovered
    // even if they're not standard module files
    if crate_root_path.exists() {
        assert!(
            mock_serde_ctx.files.contains(&crate_root_path),
            "#[path] module core/crate_root.rs should be discovered"
        );
    }

    if macros_path.exists() {
        assert!(
            mock_serde_ctx.files.contains(&macros_path),
            "#[path] module core/macros.rs should be discovered"
        );
    }

    // Check mock_serde_derive_internals non-standard layout
    let derive_internals_root = fixture_mock_serde_path().join("mock_serde_derive_internals");
    let derive_internals_ctx = output
        .crate_contexts
        .get(&derive_internals_root)
        .expect("mock_serde_derive_internals crate context should exist");

    // Verify lib.rs at crate root is discovered (non-standard layout)
    let lib_at_root = derive_internals_root.join("lib.rs");
    if lib_at_root.exists() {
        assert!(
            derive_internals_ctx.files.contains(&lib_at_root),
            "lib.rs at crate root (non-standard) should be discovered"
        );
    }

    // Verify src/mod.rs is also discovered if it exists
    let src_mod = derive_internals_root.join("src/mod.rs");
    if src_mod.exists() {
        assert!(
            derive_internals_ctx.files.contains(&src_mod),
            "src/mod.rs should be discovered"
        );
    }
}

/// Test that workspace metadata is correctly parsed and populated.
#[test]
fn test_mock_serde_workspace_metadata() {
    // Skip test if fixture doesn't exist
    let fixture_path = fixture_mock_serde_path();
    if !fixture_path.exists() {
        eprintln!(
            "Skipping test: fixture_mock_serde not found at {}",
            fixture_path.display()
        );
        return;
    }

    let output = run_mock_serde_discovery().expect("Discovery should succeed");

    // Verify workspace metadata is present
    let workspace = output
        .workspace
        .as_ref()
        .expect("Workspace metadata should be present");

    let workspace_section = workspace
        .workspace
        .as_ref()
        .expect("Workspace section should be present");

    // Verify workspace path is correct
    assert_eq!(
        workspace_section.path, fixture_path,
        "Workspace path should point to fixture_mock_serde"
    );

    // Verify all 4 members are in workspace.members (as full paths)
    let expected_members: Vec<PathBuf> = vec![
        workspace_section.path.join("mock_serde"),
        workspace_section.path.join("mock_serde_core"),
        workspace_section.path.join("mock_serde_derive"),
        workspace_section.path.join("mock_serde_derive_internals"),
    ];

    for expected in &expected_members {
        assert!(
            workspace_section.members.contains(expected),
            "Workspace member '{}' should be present in {:?}",
            expected.display(),
            workspace_section.members
        );
    }

    assert_eq!(
        workspace_section.members.len(),
        4,
        "Workspace should have exactly 4 members"
    );
}

/// Test specifically for non-standard crate layout.
/// 
/// This test verifies that the discovery mechanism correctly handles crates
/// where the library root is not in the standard src/lib.rs location.
/// This mimics the real serde_derive_internals crate structure.
#[test]
fn test_mock_serde_non_standard_layout() {
    // Skip test if fixture doesn't exist
    let fixture_path = fixture_mock_serde_path();
    if !fixture_path.exists() {
        eprintln!(
            "Skipping test: fixture_mock_serde not found at {}",
            fixture_path.display()
        );
        return;
    }

    let output = run_mock_serde_discovery().expect("Discovery should succeed");

    let derive_internals_root = fixture_mock_serde_path().join("mock_serde_derive_internals");
    let ctx = output
        .crate_contexts
        .get(&derive_internals_root)
        .expect("mock_serde_derive_internals context should exist");

    // The key assertion: lib.rs is at crate root, not in src/
    let lib_at_root = derive_internals_root.join("lib.rs");
    let lib_at_src = derive_internals_root.join("src/lib.rs");

    // Verify the crate was discovered (has files)
    assert!(!ctx.files.is_empty(), "Non-standard crate should have files");

    // Verify lib.rs at root is discovered (the non-standard aspect)
    if lib_at_root.exists() {
        assert!(
            ctx.files.contains(&lib_at_root),
            "Non-standard layout: lib.rs at crate root should be discovered"
        );
    }

    // The non-standard layout means src/lib.rs might NOT exist
    // If it doesn't exist, we shouldn't have it in files
    if !lib_at_src.exists() {
        assert!(
            !ctx.files.contains(&lib_at_src),
            "src/lib.rs should not be in files if it doesn't exist"
        );
    }

    // Verify namespace is still correctly derived
    assert_eq!(
        ctx.name, "mock_serde_derive_internals",
        "Crate name should be correct for non-standard layout"
    );
    assert!(
        !ctx.version.is_empty(),
        "Version should be populated for non-standard layout"
    );
}

/// Test that validates the expected crate versions.
#[test]
fn test_mock_serde_crate_versions() {
    // Skip test if fixture doesn't exist
    let fixture_path = fixture_mock_serde_path();
    if !fixture_path.exists() {
        eprintln!(
            "Skipping test: fixture_mock_serde not found at {}",
            fixture_path.display()
        );
        return;
    }

    let output = run_mock_serde_discovery().expect("Discovery should succeed");

    // Verify each crate has a valid version
    for (crate_path, context) in &output.crate_contexts {
        assert!(
            !context.version.is_empty(),
            "Crate at {:?} should have a non-empty version",
            crate_path
        );

        // Version should follow semver pattern (at least contain digits)
        assert!(
            context.version.chars().any(|c| c.is_ascii_digit()),
            "Crate at {:?} should have a version with digits, got: {}",
            crate_path,
            context.version
        );
    }
}

/// Test discovery with partial crate list (only some crates in workspace).
#[test]
fn test_mock_serde_partial_discovery() {
    // Skip test if fixture doesn't exist
    let fixture_path = fixture_mock_serde_path();
    if !fixture_path.exists() {
        eprintln!(
            "Skipping test: fixture_mock_serde not found at {}",
            fixture_path.display()
        );
        return;
    }

    let workspace_path = fixture_mock_serde_path();

    // Only discover mock_serde and mock_serde_core
    let partial_crates = vec![
        workspace_path.join("mock_serde"),
        workspace_path.join("mock_serde_core"),
    ];

    let output =
        run_discovery_phase(Some(&workspace_path), &partial_crates).expect("Discovery should succeed");

    // Should only have 2 crate contexts
    assert_eq!(
        output.crate_contexts.len(),
        2,
        "Partial discovery should find only 2 crates"
    );

    // Verify workspace metadata is still present
    assert!(
        output.workspace.is_some(),
        "Workspace metadata should still be present for partial discovery"
    );

    // Verify the specific crates
    assert!(
        output.crate_contexts.contains_key(&workspace_path.join("mock_serde")),
        "mock_serde should be present"
    );
    assert!(
        output.crate_contexts.contains_key(&workspace_path.join("mock_serde_core")),
        "mock_serde_core should be present"
    );
    assert!(
        !output.crate_contexts.contains_key(&workspace_path.join("mock_serde_derive")),
        "mock_serde_derive should NOT be present in partial discovery"
    );
}
