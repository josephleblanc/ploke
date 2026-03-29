//! TEST_NOTE:2026-03-29
//!
//! Provenance:
//! - Corpus run: `run-1774765997311`
//! - Target repo: `FuelLabs/sway`
//! - Target crate: `sway-error`
//! - Saved failing member: `sway-error`
//! - Saved hotspot file: `src/error.rs`
//!
//! The original corpus failure was a resolve-stage panic:
//! `Expected unique relations, found invalid duplicate with error: Duplicate node
//! found for ID AnyNodeId::Const(...) when only one was expected.`
//!
//! The saved resolve artifact showed two local `const NUM_OF_FIELDS_TO_DISPLAY`
//! items in the same source file producing the same synthetic `Const` ID. This
//! fixture keeps the example valid Rust and small enough to verify:
//! - the crate compiles under `cargo check`
//! - `syn_parser` still panics on duplicate local const IDs
//!
//! If this test starts passing, re-check whether the original corpus failure
//! depended on additional nesting or scope details from `sway-error/src/error.rs`.

use std::path::PathBuf;
use std::process::Command;

use ploke_common::workspace_root;
use syn_parser::try_run_phases_and_resolve;

fn fixture_workspace_root() -> PathBuf {
    workspace_root().join("tests/fixture_workspace/ws_fixture_02_assoc_local_enum_ids")
}

fn fixture_member_root() -> PathBuf {
    fixture_workspace_root().join("member_const_repro")
}

fn panic_payload_to_string(payload: &Box<dyn std::any::Any + Send>) -> String {
    if let Some(s) = payload.downcast_ref::<&str>() {
        s.to_string()
    } else if let Some(s) = payload.downcast_ref::<String>() {
        s.clone()
    } else {
        "<non-string panic payload>".to_string()
    }
}

#[test]
fn fixture_duplicate_local_consts_is_valid_rust() {
    let fixture_root = fixture_workspace_root();

    assert!(
        fixture_root.is_dir(),
        "fixture workspace must exist: {}",
        fixture_root.display()
    );

    let output = Command::new("cargo")
        .arg("check")
        .current_dir(&fixture_root)
        .output()
        .expect("run cargo check for committed fixture");

    assert!(
        output.status.success(),
        "fixture must compile successfully.\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn repro_duplicate_local_consts_duplicate_relation_panic() {
    let fixture_root = fixture_member_root();

    assert!(
        fixture_root.is_dir(),
        "fixture crate must exist: {}",
        fixture_root.display()
    );

    let result = std::panic::catch_unwind(|| {
        let _ = try_run_phases_and_resolve(&fixture_root);
    });

    let payload = result.expect_err(
        "parser unexpectedly succeeded on committed repro fixture; re-evaluate whether the fixture still captures the bug",
    );
    let panic_msg = panic_payload_to_string(&payload);

    assert!(
        panic_msg.contains("Expected unique relations"),
        "panic should preserve duplicate-relation context, got: {panic_msg}"
    );
    assert!(
        panic_msg.contains("Duplicate node found for ID AnyNodeId::Const("),
        "panic should mention the duplicate const node lookup, got: {panic_msg}"
    );
}
