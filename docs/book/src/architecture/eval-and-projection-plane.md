# Eval and Projection Plane

Ploke has an internal evaluation/protocol plane that should stay conceptually separate from the release-facing TUI runtime.

## Purpose

The eval/projection plane records and inspects runs, artifacts, patches, reviews, and playback state. It supports Prototype 1 evaluation and browser/egui surfaces.

## Main crates

- `ploke-protocol` defines typed procedure and artifact abstractions.
- `ploke-records` defines passive persisted record schemas.
- `ploke-tree` loads records into read-only projection structures such as run forests and history graphs.
- `ploke-eval` contains internal prototype and campaign tooling.
- `ploke-egui` and related crates render inspection surfaces.

## Authority rule

Passive records are evidence, not authority. Loading a record into a projection should not imply permission to advance scheduler state, mutate history, or apply patches.

This rule keeps debug/replay/read-side tooling from becoming an accidental control plane.
