//! TEST_NOTE:2026-03-29
//!
//! Provenance:
//! - Corpus run: `run-1774765997311`
//! - Target repo: `FuelLabs/sway`
//! - Target crate: `sway-core`
//! - Saved failing member: `sway-core`
//! - Saved hotspot file: `src/ir_generation/function.rs`
//!
//! The original corpus failure was a resolve-stage panic:
//! `Expected unique relations, found invalid duplicate with error: Node with ID
//! AnyNodeId::Field(...) not found in the graph.`
//!
//! The saved diagnostics pointed at repeated local enums named `InitializationKind`
//! with overlapping variant names and unnamed tuple fields. This committed fixture
//! reduces that shape into valid Rust so we can separately verify:
//! - the example compiles under `cargo check`
//! - `syn_parser` still trips the duplicate-relation guard on it
//!
//! If this test starts passing, we should reassess whether the remaining corpus
//! failure depends on additional details from the original `sway-core` source.

use std::path::PathBuf;
use std::process::Command;

use ploke_common::workspace_root;
use syn_parser::try_run_phases_and_resolve;

fn fixture_workspace_root() -> PathBuf {
    workspace_root().join("tests/fixture_workspace/ws_fixture_02_assoc_local_enum_ids")
}

fn fixture_member_root() -> PathBuf {
    fixture_workspace_root().join("member_repro")
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
fn fixture_assoc_local_enum_ids_is_valid_rust() {
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
fn repro_assoc_local_enum_ids_duplicate_relation_panic() {
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
        panic_msg.contains("Node with ID AnyNodeId::Field("),
        "panic should mention the missing field node lookup, got: {panic_msg}"
    );
}
