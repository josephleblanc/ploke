# Main-Thread Report: CLI Final Admission Integration

Task: unblock live deterministic TUI edit-surface admission.

Changed file:

- `crates/ploke-eval/src/cli/prototype1_state/cli_facing.rs`

Summary:

- Production deterministic TUI candidate generation now builds
  `EditSurfaceAdmission` before backend validation.
- The admission coordinate uses `parent.runtime_id()` and a base text-file
  Artifact derived from the parent checkout content.
- Successor handoff parent loading injects the invocation runtime id through
  `Parent::load_with_runtime_id(...)`; genesis still mints a fresh typed
  runtime id through `Parent::load(...)`.
- Provisional deterministic TUI child nodes now carry the typed parent runtime
  id string, not `ParentIdentity.instance_id`.
- Surface-evidence validation now expects the grant policy id
  `surface-policy:tool-surface-v1`, not the UI surface enum name.
- Test helpers were updated to construct typed `CheckedSurface` evidence and
  authority-bearing admissions.

Verification:

- `cargo fmt --all`
- `cargo check -p ploke-eval 2>&1 | tail -n 80`
- `cargo test -p ploke-eval edit_surface 2>&1 | tail -n 100`
- Focused tests for backend bridge, History checked evidence, parent runtime,
  and deterministic TUI producer passed before the full filter.
