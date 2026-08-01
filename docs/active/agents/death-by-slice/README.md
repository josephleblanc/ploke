# Death By Slice

Ledger for failures caused by earlier narrow implementation slices that made
one command or one generation work while failing to preserve the durable shape
needed by later runtime steps.

This is not a general bug list. Add an entry here when all of these are true:

- a prior implementation was accepted as a bounded vertical slice;
- the slice encoded policy, identity, authority, or state only at the local
  command/projection boundary;
- a later run failed, became unsafe to launch, or silently changed behavior
  because the missing structure did not propagate across the runtime boundary.

## Entries

- [`2026-05-09-prototype1-successor-run-shape-not-propagated.md`](2026-05-09-prototype1-successor-run-shape-not-propagated.md)
  Prototype 1 parent CLI flags for edit-surface generation and successor
  metrics are parent-local and are not serialized into the successor parent
  invocation.
- [`2026-05-09-prototype1-run-profile-toml-plan.md`](2026-05-09-prototype1-run-profile-toml-plan.md)
  Plan to replace parent-local Prototype 1 run-shape flags with a campaign-
  admitted TOML profile under `~/.ploke-eval/`.

## Entry Shape

Each entry should record:

- **Local slice:** the narrow change that looked complete at the time.
- **Missing structure:** the durable object or transition that should have
  carried the semantics.
- **How it came back:** the later run, command, or invariant that exposed the
  gap.
- **Required fix:** the structural change needed to prevent recurrence.
