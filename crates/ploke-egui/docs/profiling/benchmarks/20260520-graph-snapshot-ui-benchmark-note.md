focused egui renderer test plus cargo test benchmark filter; native interactive window not tested.

Change summary:
- Added native UI controls for loading and exporting `ploke_tree::GraphSnapshot` files from the run-navigation panel.
- Snapshot import keeps the loaded typed snapshot in app state so it can be exported again without reading the original run root.
- Loading a run root clears snapshot state; loading a snapshot clears run selection and rebuilds graph-facing caches.

Verification surfaces:
- `cargo check -p ploke-egui --features dev`
- `cargo test -p ploke-egui graph_snapshot_controls_load_and_export_snapshot --features dev 2>&1 | tail -n 120`
- `cargo test -p ploke-egui benchmark 2>&1 | tail -n 120`

Baseline comparison:
- No native benchmark baseline comparison was valid for this edit because no native benchmark run was requested.

Allocation/performance measurements:
- not measured

Measured improvements/regressions:
- not measured

Unmeasured risk and next action:
- Native pointer interaction and window rendering were not exercised. If needed, run the native app and manually load/export a snapshot from the left panel.
