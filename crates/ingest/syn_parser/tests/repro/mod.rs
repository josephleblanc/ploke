use std::{path::Path, process::Command};

// repros that were identified, and the underlying issue addressed. 
// When these tests pass, it indicates the parser is successfully handling the target as intended.
mod success;
// repros that still lead to fail cases.
// Should be reviewed, and the underlying issue addressed.
// Once the underlying issue is addressed, should move into `success` module.
mod fail;

/// Validate that the target fixture exists and compiles.
/// We are only planning to handle correct, valid rust, and this test helper validates that the
/// target fixture is well-formed.
fn validate_fixture(member_root: &Path) {
    assert!(
        member_root.is_dir(),
        "fixture crate must exist: {}",
        member_root.display()
    );

    let output = Command::new("cargo")
        .arg("check")
        .current_dir(member_root)
        .output()
        .expect("run cargo check for committed fixture");

    assert!(
        output.status.success(),
        "fixture must compile successfully.\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}
