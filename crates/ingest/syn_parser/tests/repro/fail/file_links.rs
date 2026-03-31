//! Merge-stage `DuplicatePath` repros outside
//! [ADR-025](../../../../../../docs/design/adrs/accepted/ADR-025-module-tree-staged-file-duplicate-definitions.md)
//! scope (e.g. lib + bin both contributing a `crate::cli` file-root collision, or `scheduler::queue`
//! collisions not resolved by file-vs-inline staging). Inline+file duplicates are under `repro::success`.
//!
//! Known limitation (L2 / KL-004):
//! [syn_parser_known_limitations.md](../../../../../../docs/design/syn_parser_known_limitations.md),
//! [KL-004-nested-main-rs-logical-path.md](../../../../../../docs/design/known_limitations/KL-004-nested-main-rs-logical-path.md).

use std::path::PathBuf;

use ploke_common::workspace_root;
use syn_parser::parse_workspace;

use crate::repro::validate_fixture;

fn fixture_workspace_root() -> PathBuf {
    workspace_root().join("tests/fixture_workspace/ws_fixture_02_assoc_local_enum_ids")
}

fn fixture_cli_workspace_root() -> PathBuf {
    workspace_root().join("tests/fixture_workspace/ws_fixture_03_cli_collision")
}

// TEST_NOTE:2026-03-30
//
// Provenance:
// - Corpus run: `run-1774867607815`
// - Target repo: `linera-io/linera-protocol`
// - Target crate: `linera-service`
// - Saved failing member: `linera-service`
// - Saved hotspot files:
//   - `src/lib.rs`
//   - `src/cli/main.rs`
//   - `src/cli/mod.rs`
//
// The original corpus failure was a merge-stage error:
// `Internal state error: Failed to build module tree: Feature not implemented:
// Duplicate definition path 'crate::cli' found in module tree.`
//
// This fixture keeps the source valid Rust: a library crate declares
// `pub mod cli;` and the package also exposes a binary target rooted at
// `src/cli/main.rs` with a sibling `src/cli/mod.rs`. That is enough to preserve
// the duplicate module-tree path while allowing `cargo check` to succeed.
#[test]
fn repro_duplicate_cli_binary_module_merge_error() {
    let fixture_root = fixture_cli_workspace_root();
    let member_root = fixture_root.join("member_cli_collision");

    validate_fixture(&member_root);

    let selected = [member_root.as_path()];
    let err = match parse_workspace(&fixture_root, Some(&selected)) {
        Ok(_) => panic!("workspace unexpectedly parsed successfully"),
        Err(err) => err,
    };
    let err_msg = err.to_string();

    assert!(
        err_msg.contains("Failed to build module tree"),
        "error should preserve module-tree context, got: {err_msg}"
    );
    assert!(
        err_msg.contains("Duplicate definition path 'crate::cli'"),
        "error should mention the duplicate module path, got: {err_msg}"
    );
}

// TEST_NOTE:2026-03-29
//
// Provenance:
// - Corpus run: `run-1774765997311`
// - Target repo: `ankitects/anki`
// - Target crate: `rslib`
// - Saved failing member: `rslib`
// - Saved hotspot file: `src/scheduler/queue/mod.rs`
//
// The original corpus failure was a merge-stage error:
// `Internal state error: Failed to build module tree: Feature not implemented:
// Duplicate definition path 'crate::scheduler::queue' found in module tree.`
//
// The minimized shape is a crate root with `mod scheduler;`, `scheduler/mod.rs`
// with `mod queue;`, and `scheduler/queue/mod.rs` with `mod main;`, plus an
// empty `scheduler/queue/main.rs`. That is small enough to verify while still
// reproducing the duplicate module-tree path.
#[test]
fn repro_duplicate_scheduler_queue_mod_merge_error() {
    let fixture_root = fixture_workspace_root();
    let member_root = fixture_workspace_root().join("member_scheduler_queue_repro");

    validate_fixture(&member_root);

    let selected = [member_root.as_path()];
    let err = match parse_workspace(&fixture_root, Some(&selected)) {
        Ok(_) => panic!("workspace unexpectedly parsed successfully"),
        Err(err) => err,
    };
    let err_msg = err.to_string();

    assert!(
        err_msg.contains("Failed to build module tree"),
        "error should preserve module-tree context, got: {err_msg}"
    );
    assert!(
        err_msg.contains("Duplicate definition path 'crate::scheduler::queue'"),
        "error should mention the duplicate module path, got: {err_msg}"
    );
}
