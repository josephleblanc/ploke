use std::path::PathBuf;

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
// - Target repo: `FuelLabs/sway`
// - Target crate: `sway-core`
// - Saved failing member: `sway-core`
// - Saved hotspot file: `src/ir_generation/function.rs`
//
// The original corpus failure was a resolve-stage panic:
// `Expected unique relations, found invalid duplicate with error: Node with ID
// AnyNodeId::Field(...) not found in the graph.`
//
// The saved diagnostics pointed at repeated local enums named `InitializationKind`
// with overlapping variant names and unnamed tuple fields. This committed fixture
// reduces that shape into valid Rust so we can separately verify:
// - the example compiles under `cargo check`
// - `syn_parser` can resolve repeated method-local enums in sibling methods
//   without colliding secondary IDs
#[test]
fn repro_assoc_local_enum_ids_resolves_successfully() {
    let member_root = fixture_workspace_root().join("member_repro");

    validate_fixture(&member_root);

    try_run_phases_and_resolve(&member_root).expect(
        "parser should resolve repeated method-local enums without colliding field or variant IDs",
    );
}
