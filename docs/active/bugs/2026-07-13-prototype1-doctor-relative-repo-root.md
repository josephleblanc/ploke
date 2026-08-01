# Prototype 1 doctor passed a relative parent root into absolute-path indexing

Status: fixed in source and live verified

Discovered: 2026-07-13

## Broken Contract

Prototype 1 doctor, prompt, step, and continue accept `--repo-root PATH`, and
doctor itself suggests commands using `--repo-root .` after changing into the
parent checkout. The shared runtime context therefore must normalize that
accepted path before passing it to subsystems whose file-operation contracts
require an absolute workspace root.

Before the fix, ordinary doctor accepted `--repo-root .`, but
`--headless-tui-setup-preflight` failed immediately:

```text
phase: sparse_workspace_resolve_index_target
detail: Failed to normalize target path: File operation read failed for ./.: path must be absolute
```

Using the canonical absolute path against the same checkout passed after the
expected workspace ingestion, proving this was path propagation rather than a
RAG, database, profile, or campaign failure.

## Source Boundary

```text
prototype1-doctor --repo-root .
-> run::resolve_context
-> RuntimeContext.repo_root = "."
-> run_headless_tui_setup_preflight
-> setup_workspace_tui_runtime_with_read_roots
-> sparse workspace target normalization rejects relative path
```

`resolve_context` is shared by doctor, prompt, step, continue, and diagnosis.
It now canonicalizes the already-required existing parent checkout once, before
loading parent identity or constructing `RuntimeContext`. This keeps every
control path on one absolute checkout coordinate and does not make a missing or
invalid checkout permissible.

## Verification

`control_repo_root_is_canonical` covers the shared normalization helper. A live
rerun of the original relative command completed workspace ingestion and
reported:

```text
headless_tui_setup_preflight.outcome = passed
phase = baseline_eval
blockers = []
```

The same admitted campaign also passed its live Direct Google protocol canary.
The later OpenRouter embedding preflight failed independently with the already
documented key-specific monthly-limit blocker.
