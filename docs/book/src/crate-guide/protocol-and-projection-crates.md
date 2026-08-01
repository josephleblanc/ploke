# Protocol and Projection Crates

These crates support evaluation, records, replay, and inspection. They are important, but they are not the default release-facing path.

## `ploke-protocol`

Typed procedures, steps, artifacts, and adjudication-facing structures.

## `ploke-records`

Passive persisted schemas. Prefer typed records over opaque JSON when a relation is part of the durable protocol surface.

## `ploke-tree`

Read-only projection layer over records. It builds run forests, graph projections, playback surfaces, and artifact views.

## `ploke-eval`

Internal evaluation and Prototype 1 tooling. This crate can evolve faster than the main TUI path.

## UI projection crates

`ploke-egui`, `ploke-tree-browser`, and `ploke-tree-egui` are inspection surfaces. They should render typed projections rather than taking ownership of protocol authority.
