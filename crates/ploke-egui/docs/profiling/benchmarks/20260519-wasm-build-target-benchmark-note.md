`cargo build` for `wasm32-unknown-unknown`, native `cargo check`, and focused renderer/cache tests were the verification surfaces.

Change summary:
- Split passive `ploke-records` tool carriers from the native `ploke-tui` decoder dependency so `ploke-egui` can compile for wasm.
- Kept decoded tool argument/result rendering native-only and used raw persisted payload rendering on wasm.
- Added wasm `uuid/js` feature activation at the `ploke-records` target dependency edge.

Commands:
- `cargo build -p ploke-egui --target wasm32-unknown-unknown`
- `cargo check -p ploke-records`
- `cargo check -p ploke-egui`
- `cargo test -p ploke-records`
- `cargo test -p ploke-egui tool_decoded_payload_cache_reuses_stable_records`
- `cargo test -p ploke-egui benchmark`
- `cargo test -p ploke-egui --features "dev native-benchmark" benchmark`
- `cargo check -p ploke-egui --features "dev native-benchmark"`

Baseline comparison:
- No benchmark baseline comparison was valid. This was a compile-target dependency split, not a rendered native performance change.

Allocation churn:
- Not measured. No native benchmark suite or allocator report was run.
- Callsite attribution was not captured.

Measured improvements:
- The wasm build now reaches `Finished dev profile` instead of failing in `mio` through `ploke-tui` or in `uuid` missing wasm randomness.

Measured regressions:
- None measured. Native checks and the focused decoded-tool cache test passed.

Remaining allocation debt:
- Unchanged. Existing native allocation reports remain the source of truth for UI churn.

Unmeasured risk and next action:
- The wasm UI compiles but was not loaded in a browser or checked through a canvas render path.
- The wasm path shows raw tool payloads rather than decoded native tool DTOs; a future wasm-safe owned DTO split can restore decoded drilldown without depending on `ploke-tui`.
