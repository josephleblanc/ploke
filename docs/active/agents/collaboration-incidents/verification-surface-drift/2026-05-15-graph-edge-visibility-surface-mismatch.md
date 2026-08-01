# 2026-05-15 Graph Edge Visibility Surface Mismatch

- Trigger:
  User said the newly claimed `P_O` edges were still not visible and pointed
  out that the left panel already had a way to prove that.
- User-visible failure:
  The agent reported that the edges were visible because contract diagnostics
  showed `visible_edges=16` and `P_O=2`, but that only proved the projected
  edge payloads were marked visible. It did not prove that the current
  renderer/readability path could actually draw those edges.
- Touched code surface:
  - `crates/ploke-egui/src/ui/view/projection.rs`
  - `crates/ploke-egui/src/ui/view/diagnostics.rs`
  - `crates/ploke-egui/src/diagnostics/default_view/*`
- What the agent did:
  - checked graph/contract edge counts
  - failed to check the drawable edge path used by the view diagnostics
  - missed that `readability_diagnostics` drops self-loop edges with
    `if start == end { return None; }`
  - therefore treated “present in the projected graph” as “visibly rendered”
- Skipped docs / skills / instructions:
  - skipped the practical consequence of `Verification Surface Honesty` for
    graph rendering surfaces
  - skipped checking the left-panel/readability path that already separated
    visible payload counts from drawable edge geometry
- Why the behavior was risky:
  It makes graph fixes sound complete when they only change inventory counts.
  In this case it hid the real bug: self-loop `P_O` edges were admitted and
  counted, but the current drawable geometry path dropped them.
- Concrete prevention rule:
  For graph or edge rendering work, do not stop at `edge_count` or contract
  inventory. Verify the drawable/readability path too. If an edge class is a
  self-loop or other special geometry, explicitly check whether the renderer
  constructs a visible shape for it before claiming the edge is visible.
