# 2026-05-15: History Claims Overreach

## Trigger

While tracing artifact identity for the `ploke-egui` inspector, the discussion
reached `ClaimsRecord` / `block::Claims` in Prototype 1 History. The agent
treated that as a likely cleanup target and began editing `ploke-eval`
authority code.

## User-Visible Failure

The user read this as casual interference with one of the most sensitive
control surfaces in the repo: the History/Crown machinery that constrains loop
authority, successor selection, and process fanout.

## What Went Wrong

- The active thread was UI archaeology and trust in debugger claims.
- The agent jumped from diagnosis into implementation on
  `crates/ploke-eval/src/cli/prototype1_state/history.rs`.
- The agent did not first re-read the file-level docs and surrounding
  operational docs that explain History as a central authority/control
  mechanism.
- The agent failed to treat "this looks structurally weak" as a reason to stop
  and classify the boundary before editing.

## Why This Was Risky

- Prototype 1 History is not just storage shape. It is part of the control
  mechanism that keeps the loop from drifting into unsafe runtime behavior.
- Casual edits in this area can damage successor authority, admission shape, or
  the control path that prevents runaway child spawning and disk/process abuse.
- Even proposing the change lightly signaled that the agent was not respecting
  the centrality of the type surface or the prior user guidance around it.

## Sources That Should Have Been Respected First

- `crates/ploke-eval/src/cli/prototype1_state/history.rs` file-level docs
- `docs/workflow/evalnomicon/drafts/`
- `AGENTS.md` Prototype 1 / History / structural carrier guidance
- prior user walkthroughs establishing History as a central authority surface

## Correct Behavior Next Time

1. If a UI/debugger thread reaches Prototype 1 History, stop and classify the
   move as diagnosis only unless the user explicitly asks for History edits.
2. Before touching `history.rs`, re-read the file-level docs and the relevant
   repo guidance for authority/control surfaces.
3. Use structural-carrier / persistence / authority discipline before editing
   any claim, admission, or handoff path in `ploke-eval`.
4. If the problem can be recorded as archaeology, report wording, or graph-path
   separation, do that first and leave runtime authority code alone.
5. If the user expresses alarm about this boundary, revert the partial work and
   log the incident.

## Memory Hypothesis

Useful memory to retain:

- the user treats Prototype 1 History/Crown as one of the most important and
  most dangerous authority surfaces in the codebase
- casual edits there are interpreted as a major trust failure
- when a thread starts in UI archaeology, the default should be to document or
  trace authority boundaries, not to patch them
