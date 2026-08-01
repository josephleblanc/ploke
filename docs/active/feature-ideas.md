# Feature Ideas

Ideas for features that are non-trivial to implement, briefly described.

## Features

### Recover from immutable-surface write attempts

When a model attempts to edit a protected path such as `Cargo.toml`, the write
preflight must continue to deny the tool call before execution. This invariant is
intentional: long Prototype 1 loops need a small, explicit mutable surface so a
candidate cannot accidentally or intentionally change dependency/configuration
authority.

The failure should still be recoverable when the workspace remains clean. Instead
of letting one denied protected-path edit become an automatic `completed_without_edit`,
the headless/eval loop should be able to spend a small recovery budget and send a
stronger correction back to the model:

- the attempted path is immutable for this run;
- manifest/config edits are not valid candidate patches;
- the model must choose an allowed source-code edit or explicitly report that no
  mutable-surface solution exists;
- repeated attempts against the same protected path should still fail fast.

Initial scope:

1. Add a single protected-write recovery retry for Prototype 1 headless/eval
   attempts when the terminal outcome is no-edit and the last material failure is
   a write-policy denial.
2. Keep the existing tool preflight strict; do not relax `Cargo.toml`,
   `Cargo.lock`, `.cargo/`, `.ploke/`, `crates/ploke-eval`, or other protected
   surfaces.
3. Gate the recovery by cleanliness and budget: only retry if the workspace is
   clean and provider/tool budgets remain.

### Search mutable surface

Add a tool or query mode that searches only paths inside the mutable surface.
This can start coarse-grained: point the model at crates/files that are writable
under the active `SurfacePolicy`, excluding protected roots and protected
filenames.

This should use the same policy data as write preflight, not a separate allowlist,
so the guidance cannot drift from enforcement. Possible shapes:

- `search_writable_surface { query, top_k }`
- `list_writable_surface { max_entries, kind }`
- `request_code_context { ..., surface_filter: "writable_only" }`

The immediate recovery use case is a model trying to mutate `Cargo.toml`. After
the denial, the recovery prompt can direct the model to search the writable
surface for an alternate implementation change instead of retrying the manifest
edit.

### Replay/live-branch debugger value

The recent live tool-loop checkpoint demonstrates why replayability matters. A
Direct-Google run attempted a protected `Cargo.toml` edit, received a structured
tool denial, and then terminated with prose instead of finding an allowed source
edit. With durable tool-loop checkpoints, we can:

1. inspect the exact provider response, tool request, denial payload, and terminal
   response;
2. change the recovery language or mutable-surface search tool in the current
   workspace;
3. replay the same checkpoint through the updated tool handling; and
4. branch into a new live continuation to see whether the model trajectory changes
   toward a valid patch or exposes a different blocker.

That turns a one-off live failure into a reproducible debugger fixture for the
hybrid provider/tool loop.

## Shelved

### Structured immutable-surface blocked outcome

A richer outcome such as `BlockedByImmutableSurface { path, intended_change,
rationale }` would be useful evidence, but it is larger than the immediate
recovery slice. Defer until protected-write recovery and mutable-surface search
are proven.
