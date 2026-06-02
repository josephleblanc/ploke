`cargo test benchmark filter` and a focused dev-gated serialization test were run; `native interactive window` was not run.

## Change Summary

- Added a dev-only Inspector button that prints the selected central graph item and its graph-resolved `SelectionInspectorSnapshot` as pretty JSON to stdout.
- Added `SelectedGraphItemSnapshot` as a diagnostic serialization wrapper around existing selected graph payload and inspector projection.
- Added a focused serialization test for the selected graph item output shape.

## Verification

- `cargo test -p ploke-egui --features dev selected_graph_item_snapshot_serializes_reference_and_inspector -- 2>&1 | tail -n 80`
- `cargo test -p ploke-egui benchmark 2>&1 | tail -n 120`

## Baseline

No native benchmark baseline comparison was valid for this change. The change adds an on-demand dev diagnostic print path and does not run during normal frames unless the button is clicked.

## Measurements

Allocation and native frame measurements: not measured; not requested.

## Residual Risk

The print path was not validated in a native interactive window. The focused test validates serialization shape, and the dev-feature test compile covers the UI button code path.
