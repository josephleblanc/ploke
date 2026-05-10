# Self-Improvement Loop Handoffs

Use this as the shared restart list for the records/playback/frontend track. The dated handoffs remain the detailed source docs; this file is the routing table.

## Current Spine

- [`typed-persistence-spine/2026-05-10-survey-inventory-handoff.md`](typed-persistence-spine/2026-05-10-survey-inventory-handoff.md)
  Current handoff for the completed typed-persistence-spine survey inventory and next implementation slices.
- [`../../agents/2026-05-09_run-playback-coarse-history-handoff.md`](../../agents/2026-05-09_run-playback-coarse-history-handoff.md)
  Current operational handoff for the coarse/fine sealed-History playback slice and real-run record compatibility.
- [`../../agents/2026-05-09_egui-wasm-observability-handoff.md`](../../agents/2026-05-09_egui-wasm-observability-handoff.md)
  Current frontend observability handoff for native/WASM egui work over playback models. Treat this as benchmark-evidence-first observability, not a generic run dashboard.

## Supporting Contracts

- [`baseline-evidence-authority-plan.md`](baseline-evidence-authority-plan.md)
  Current plan for fixing the missing baseline record failure by making `Parent<P>` require complete parent-local baseline evidence before child fanout.
- [`../../agents/2026-05-09_run-playback-typed-observability-plan.md`](../../agents/2026-05-09_run-playback-typed-observability-plan.md)
  Design contract for typed `RunPlayback` / `RunPlaybackRef` projections, evidence strength, causal order, and projection authority.
- [`../../agents/2026-05-09_ploke-records-protocol-handoff.md`](../../agents/2026-05-09_ploke-records-protocol-handoff.md)
  Shared `ploke-records` / `ploke-tree` passive schema and record-store handoff.
- [`../../agents/2026-05-09_records-emission-clean-sweep-handoff.md`](../../agents/2026-05-09_records-emission-clean-sweep-handoff.md)
  Producer-side record emission normalization handoff.

## Background

- [`../../../workflow/evalnomicon/drafts/prototype1-history-handoff-2026-04-29.md`](../../../workflow/evalnomicon/drafts/prototype1-history-handoff-2026-04-29.md)
  Older History/Crown background. Use for conceptual continuity, not current task status.
- [`../../../workflow/evalnomicon/drafts/prototype1-timing-projection-handoff-2026-05-02.md`](../../../workflow/evalnomicon/drafts/prototype1-timing-projection-handoff-2026-05-02.md)
  Older timing/projection context.
