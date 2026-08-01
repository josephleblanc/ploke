# Source of Truth

Use this split to avoid duplicate authority between the `ploke-eval` manual,
Evalnomicon, active workflow docs, and raw run data.

| Claim type | Canonical home |
| --- | --- |
| Current CLI behavior | `crates/ploke-eval/docs/` plus `crates/ploke-eval/src/` |
| Current file and artifact layout | `crates/ploke-eval/docs/` |
| Runtime knobs and admitted profile fields | `crates/ploke-eval/docs/reference/` and source schemas |
| Prototype 1 operator/debugging truth | `crates/ploke-eval/docs/prototype1/` |
| Prototype 1 conceptual rationale | `docs/workflow/evalnomicon/src/prototype1/` |
| Eval methodology and protocols | `docs/workflow/evalnomicon/src/core/` and `src/protocols/` |
| Active handoffs and rolling current state | `docs/active/` |
| Raw run data and generated artifacts | `~/.ploke-eval/` |

When two docs disagree, prefer current source for implementation behavior, then
this manual for operator-facing descriptions, then Evalnomicon for conceptual
framing, then historical drafts and logs.
