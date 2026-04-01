use std::path::PathBuf;
use std::process::Command;

use ploke_common::workspace_root;
use syn_parser::try_run_phases_and_resolve;

use crate::repro::validate_fixture;

fn fixture_workspace_root() -> PathBuf {
    workspace_root().join("tests/fixture_workspace/ws_fixture_02_assoc_local_enum_ids")
}

// TEST_NOTE:2026-03-29
//
// Provenance:
// - Corpus run: `run-1774765997311`
// - Target repo: `GraphiteEditor/Graphite`
// - Target crate: `vector-types`
// - Saved failing member: `vector-types`
// - Saved hotspot file: `src/subpath/core.rs`
//
// The original corpus failure was a resolve-stage panic:
// `Expected unique relations, found invalid duplicate with error: Duplicate node
// found for ID AnyNodeId::Const(...) when only one was expected.`
//
// The saved resolve artifact showed two `const HANDLE_OFFSET_FACTOR` items in the
// same `impl`: one inside a closure body (`new_rounded_rectangle`) and one in a
// sibling method (`new_ellipse`), both listed twice under the module with the same
// synthetic `Const` ID. This differs from `member_const_repro` (duplicate `const`
// names in separate `match` arms).
//
// This fixture keeps the example valid Rust and small enough to verify:
// - the crate compiles under `cargo check`
// - `syn_parser` can distinguish closure-local `const` items from sibling
//   method-local `const` items in the same impl
#[test]
fn repro_duplicate_closure_local_consts_resolves_successfully() {
    let member_root = fixture_workspace_root().join("member_closure_const_repro");

    validate_fixture(&member_root);

    try_run_phases_and_resolve(&member_root).expect(
        "parser should distinguish closure-local const items from sibling method-local const items",
    );
}

// TEST_NOTE:2026-03-29
//
// Provenance:
// - Corpus run: `run-1774765997311`
// - Target repo: `astral-sh/ruff`
// - Target crate: `ruff_formatter`
// - Saved failing member: `ruff_formatter`
// - Saved hotspot file: `src/line_width.rs`
//
// The original corpus failure was a resolve-stage panic:
// `Expected unique relations, found invalid duplicate with error: Duplicate node
// found for ID AnyNodeId::Const(...) when only one was expected.`
//
// The minimized shape is one inherent `impl` with one method containing an
// `if/else`, where both branches define the same local `const HARD_LINE_BREAK`.
// This fixture keeps that shape valid Rust and small enough to verify:
// - the crate compiles under `cargo check`
// - `syn_parser` skips executable-body local items instead of panicking on duplicate
//   local const IDs
#[test]
fn repro_duplicate_if_branch_hard_line_break_consts_resolves_without_executable_local_item_nodes() {
    let member_root =
        fixture_workspace_root().join("member_if_branch_hard_line_break_consts_repro");

    validate_fixture(&member_root);

    try_run_phases_and_resolve(&member_root)
        .expect("parser should skip executable-body local const items instead of panicking");
}

// TEST_NOTE:2026-03-29
//
// Provenance:
// - Corpus run: `run-1774765997311`
// - Target repo: `RustPython/RustPython`
// - Target crate: `stdlib`
// - Saved failing member: `stdlib`
// - Saved hotspot file: `src/resource.rs`
//
// The original corpus failure was a resolve-stage panic:
// `Expected unique relations, found invalid duplicate with error: Duplicate node
// found for ID AnyNodeId::Const(...) when only one was expected.`
//
// The persisted source minimized down to one inherent `impl` method with a plain
// `if/else` and the same local `const Y` in both branches. This fixture keeps that
// shape valid Rust and small enough to verify:
// - the crate compiles under `cargo check`
// - `syn_parser` skips executable-body local items instead of panicking on duplicate
//   local const IDs
#[test]
fn repro_duplicate_if_branch_local_consts_resolves_without_executable_local_item_nodes() {
    let member_root = fixture_workspace_root().join("member_if_branch_local_consts_repro");

    validate_fixture(&member_root);

    try_run_phases_and_resolve(&member_root)
        .expect("parser should skip executable-body local const items instead of panicking");
}

// TEST_NOTE:2026-03-29
//
// Provenance:
// - Corpus run: `run-1774765997311`
// - Target repo: `actix/actix-web`
// - Target crate: `awc`
// - Saved failing member: `awc`
// - Saved hotspot file: `src/client/connector.rs`
//
// The original corpus failure was a resolve-stage panic:
// `Expected unique relations, found invalid duplicate with error: Duplicate node
// found for ID AnyNodeId::Const(...) when only one was expected.`
//
// The persisted source minimized down to a `match` with multiple TLS branches
// that each define the same local `const H2`, plus a nested `impl` item in each
// branch. This fixture keeps that shape valid Rust and small enough to verify:
// - the crate compiles under `cargo check`
// - `syn_parser` skips executable-body local items instead of panicking on duplicate
//   local const IDs
#[test]
fn repro_duplicate_tls_branch_h2_consts_resolves_without_executable_local_item_nodes() {
    let member_root = fixture_workspace_root().join("member_tls_branch_h2_consts_repro");

    validate_fixture(&member_root);

    try_run_phases_and_resolve(&member_root)
        .expect("parser should skip executable-body local const items instead of panicking");
}

// TEST_NOTE:2026-03-29
//
// Provenance:
// - Corpus run: `run-1774765997311`
// - Target repo: `FuelLabs/sway`
// - Target crate: `sway-error`
// - Saved failing member: `sway-error`
// - Saved hotspot file: `src/error.rs`
//
// The original corpus failure was a resolve-stage panic:
// `Expected unique relations, found invalid duplicate with error: Duplicate node
// found for ID AnyNodeId::Const(...) when only one was expected.`
//
// The saved resolve artifact showed two local `const NUM_OF_FIELDS_TO_DISPLAY`
// items in the same source file producing the same synthetic `Const` ID. This
// fixture keeps the example valid Rust and small enough to verify:
// - the crate compiles under `cargo check`
// - `syn_parser` skips executable-body local items instead of panicking on duplicate
//   local const IDs
#[test]
fn repro_duplicate_local_consts_resolves_without_executable_local_item_nodes() {
    let member_root = fixture_workspace_root().join("member_const_repro");

    validate_fixture(&member_root);

    try_run_phases_and_resolve(&member_root)
        .expect("parser should skip executable-body local const items instead of panicking");
}

// TEST_NOTE:2026-03-29
//
// Provenance:
// - Corpus run: `run-1774765997311`
// - Target repo: `RustPython/RustPython`
// - Target crate: `vm`
// - Saved failing member: `vm`
// - Saved hotspot file: `src/lib.rs`
//
// The original corpus failure was a resolve-stage panic:
// `Expected unique relations, found invalid duplicate with error: Duplicate node
// found for ID AnyNodeId::Const(...) when only one was expected.`
//
// The minimized shape is one inherent `impl` with two methods, each declaring
// the same method-local `const COLLECTION_FLAGS`. This fixture keeps that shape
// valid Rust and small enough to verify:
// - the crate compiles under `cargo check`
// - `syn_parser` can resolve duplicate method-local `const` names in sibling
//   methods without colliding IDs
#[test]
fn repro_duplicate_method_collection_flags_resolves_successfully() {
    let member_root = fixture_workspace_root().join("member_method_collection_flags_repro");

    validate_fixture(&member_root);

    try_run_phases_and_resolve(&member_root)
        .expect("parser should resolve duplicate method-local const names under method scope");
}

// TEST_NOTE:2026-03-29
//
// Provenance:
// - Corpus run: `run-1774765997311`
// - Target repo: `HigherOrderCO/Bend`
// - Target crate: `bend`
// - Saved failing member: none
// - Saved hotspot file: none
//
// The original corpus failure was a resolve-stage panic:
// `Expected unique relations, found invalid duplicate with error: Duplicate node
// found for ID AnyNodeId::Function(...) when only one was expected.`
//
// The persisted triage and live replay pointed to one inherent `impl` method
// containing two sibling inner blocks, each defining a local `fn go_term`. This
// fixture keeps that shape valid Rust and small enough to verify:
// - the crate compiles under `cargo check`
// - `syn_parser` skips executable-body local items instead of panicking on duplicate
//   function IDs
#[test]
fn repro_duplicate_local_functions_resolves_without_executable_local_item_nodes() {
    let member_root = fixture_workspace_root().join("member_local_function_repro");

    validate_fixture(&member_root);

    try_run_phases_and_resolve(&member_root)
        .expect("parser should skip executable-body local function items instead of panicking");
}

// TEST_NOTE:2026-03-29
//
// Provenance:
// - Corpus run: `run-1774765997311`
// - Target repo: `GraphiteEditor/Graphite`
// - Target crate: `editor`
// - Saved failing member: `editor`
// - Saved hotspot file: `src/messages/portfolio/document/utility_types/document_metadata.rs`
//
// The original corpus failure was a resolve-stage panic:
// `Expected unique relations, found invalid duplicate with error: Duplicate node
// found for ID AnyNodeId::Static(...) when only one was expected.`
//
// The saved resolve artifact showed two local `static EMPTY` items in the same
// source file (two methods on the same `impl`) producing the same synthetic
// `Static` ID. This fixture keeps the example valid Rust and small enough to verify:
// - the crate compiles under `cargo check`
// - `syn_parser` can resolve duplicate method-local `static` names in sibling
//   methods without colliding IDs
#[test]
fn repro_duplicate_local_statics_resolves_successfully() {
    let member_root = fixture_workspace_root().join("member_static_repro");

    validate_fixture(&member_root);

    try_run_phases_and_resolve(&member_root)
        .expect("parser should resolve duplicate method-local static names under method scope");
}

// TEST_NOTE:2026-03-29
//
// Provenance:
// - Corpus run: `run-1774765997311`
// - Target repo: `RustPython/RustPython`
// - Target crate: `derive-impl`
// - Saved failing member: `derive-impl`
// - Saved hotspot file: `src/pystructseq.rs`
//
// The original corpus failure was a resolve-stage panic:
// `Expected unique relations, found invalid duplicate with error: Duplicate node
// found for ID AnyNodeId::Const(...) when only one was expected.`
//
// The saved source showed one inherent `impl` with three methods
// (`class_name`, `module`, `data_type`), each declaring the same local
// `const KEY`. This fixture keeps that shape valid Rust and small enough to
// verify:
// - the crate compiles under `cargo check`
// - `syn_parser` can resolve duplicate method-local `const` names in sibling
//   methods without colliding IDs
//
// If this test starts passing, re-check whether the original corpus failure
// depended on additional surrounding impl details from `pystructseq.rs`.
#[test]
fn repro_duplicate_method_local_consts_resolves_successfully() {
    let member_root = fixture_workspace_root().join("member_method_local_consts_repro");

    validate_fixture(&member_root);

    try_run_phases_and_resolve(&member_root)
        .expect("parser should resolve duplicate method-local const names under method scope");
}
