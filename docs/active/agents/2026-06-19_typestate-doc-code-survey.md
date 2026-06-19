# 2026-06-19 Typestate Documentation and Code Survey

Scope: documentation and code related to the Prototype 1 typestate driver, `walk` server/operator surface, durable reconstruction, historical replay, and R13b/R14b handoff work recently implemented on `feature/ploke-loop`.

Status: review/report. This is a source/code survey, not an authority document. Verify current behavior against code and run artifacts before using it to drive live handoff work.

## Sources surveyed

Active docs:

- `docs/active/agents/2026-06-17_typestate-loop-driver-plan.md`
- `docs/active/agents/2026-06-17_typestate-loop-driver-worklog/README.md`
- `docs/active/agents/2026-06-16_walk-server.md`
- `docs/active/agents/2026-06-17_walk-command-guide.md`
- `docs/active/agents/2026-06-18_walk-cli-help-audit.md`
- `docs/active/agents/2026-06-18_walk-summary-verbose-next-slice.md`
- `docs/active/agents/2026-06-02_prototype1-state-loop-walkthrough/README.md`

Crate docs:

- `crates/ploke-eval/docs/prototype1-loop-operator.md`
- `crates/ploke-eval/docs/guides/prototype1-step-execution-path.md`
- `crates/ploke-eval/src/cli/prototype1_state/PROTOTYPE1_LOOP_OPERATOR.md`
- `crates/ploke-eval/src/cli/prototype1_state/typestate/IMPLEMENTATION_PLAN.md`

Code surfaces:

- `crates/ploke-eval/src/cli/prototype1_state/typestate/`
- `crates/ploke-eval/src/cli/prototype1_state/live_edges.rs`
- `crates/ploke-eval/src/cli/prototype1_state/driver/{advance.rs,reconstruct.rs,replay.rs}`
- `crates/ploke-eval/src/cli/prototype1_state/walk/{args.rs,client.rs,controller.rs,phase.rs,protocol.rs,replay.rs,server.rs,summary.rs}`
- `crates/ploke-eval/src/cli/prototype1_state/cli_facing.rs`
- walk/typestate-related tests in `crates/ploke-eval/src/cli/tests.rs`, `typestate/tests.rs`, and `walk/phase.rs`.

GitNexus exploration:

- Queried `Prototype1 typestate walk driver replay reconstruction live_edges`.
- Inspected contexts for `run_to_terminal`, `WalkController.step_once`, and `ReplayCursor.render` using exact indexed UIDs where needed.

## Current implementation snapshot

### Batch typed driver

`prototype1-state` now enters the typed batch driver:

- `Prototype1StateCommand::run` in `run/state_cmd.rs`
- `run_prototype1_state_turn(...)` in `cli_facing.rs`
- `driver::advance::run_to_terminal(...)`

`run_to_terminal` is the canonical R0 -> R14 batch path over live direct edges:

```text
R0 -> R1 -> R3/R2a -> R4a -> R4b/R4c -> R5 -> R6 -> R7 -> R8 -> R9 -> R10 -> R11 -> R12 -> R13 -> R14
```

The old large `run_prototype1_state_turn` body has been reduced to a wrapper. This is a major implementation milestone and should be reflected more prominently in docs that still describe `cli_facing.rs` as the live parent-turn body.

### Typestate module

`typestate/` defines the structural carrier:

```rust
Runtime<Phase, Role, Context, Plan, Children, History, Evidence, Continuation, Report>
```

and value-first transition ergonomics:

```rust
r0.advance(r0_to_r1)?;
r5.advance_async(r5_to_r6).await?;
r8.advance(r8_to_r9.then(r9_to_r10))?;
```

The aliases now cover R0 through R14b, including:

- `R11aRejectedOnly`
- `R11FanoutComplete`
- `R12`
- `R13aStopped`
- `R13bHandoffCommitted`
- `R14aFinalStopped`
- `R14bFinalHandoff`

Important stale comment: `typestate/mod.rs` still says the module is "intentionally not wired into the live controller yet." That is no longer true for the batch driver or `walk` surface.

### `walk` controller

`WalkController` is now a real operator frontend over typed direct edges plus durable reconstruction. Current behavior in code:

- `refresh_from_disk()` reconstructs when server state is empty.
- `R7 -> R8` requires `--watch`.
- `R10 -> R11a | R11` requires `--watch`.
- `R12 -> R13b` is admitted only when selected-successor evidence exists and the operator passes both:
  - `--watch`
  - `--allow git-changes`
- `R12 -> R13a` is rejected when selected-successor evidence exists.
- `R13b -> R14b` is a normal `r13_to_r14` final-report step once R13b has been committed.

The current controller therefore no longer matches older docs that say selected-successor handoff is entirely blocked by the debug server slice.

### Durable reconstruction

`driver/reconstruct.rs` now reconstructs much more than "early" state:

- R1/R3/R4a/R4b/R4c/R5 from parent identity/startup/journal evidence.
- R6 from durable parent baseline evidence.
- R7 from run policy and child budget.
- R8 from existing child-plan authority.
- R9/R10 through pure typed edges.
- R11/R12 from channel-derived child outcomes or rejected-only evidence.
- R13a/R14a stopped path from continuation/report/resource evidence.
- R13b/R14b handoff path from durable handoff/ready and parent-complete evidence.

Important stale names/comments:

- module doc says "Side-effect-free reconstruction for early Prototype 1 parent typestates";
- `EarlyState` / `EarlySnapshot` / `reconstruct_early` now represent broad durable reconstruction through R14b.

This is not just cosmetic. The names make it easy for future agents to underestimate how much authority-sensitive reconstruction currently happens here.

### Historical replay

`driver/replay.rs` implements read-only replay cursor movement over transition-journal projections:

- `walk replay`
- `walk back`
- `walk forward`
- `walk branch-live --reason ... --allow provenance-record`

Recent UX updates:

- default recent window is now 3 entries;
- output hints `--tail N` for expansion;
- cursor movement remains in-memory/server-local and read-only.

Known limitation confirmed during use-testing: `--index` moves the current cursor, but the `recent entries` list is still journal-tail-based, not cursor-centered. This is documented in `2026-06-18_walk-cli-help-audit.md` as a remaining follow-up.

### `walk summary`

`walk summary` is serverless/read-only and provides useful run discovery:

- active parent identity;
- policy/fanout/node count progress;
- journal tail cursor;
- per-generation rows with `branch`, `decision`, `handoff`;
- verbose field guide and pointers to typed history inspection commands.

Current caveat: `walk summary` still directly parses several TOML/JSON artifacts. The intended direction, documented in `2026-06-18_walk-summary-verbose-next-slice.md`, is to migrate richer inspection behind typed read adapters:

- `EvidenceStore`
- `FsEvidenceStore`
- `ChildEvidenceSet`
- sealed History selection projection
- `FsBlockStore` for History head/authority inspection

## Documentation freshness review

### Good/current docs

`2026-06-18_walk-cli-help-audit.md`

- Current for replay default-tail/help changes.
- Correctly lists remaining replay UX follow-ups.

`2026-06-18_walk-summary-verbose-next-slice.md`

- Good design note for the summary/typed-evidence direction.
- Partly superseded by implementation: `branch`/`decision`/`handoff` and verbose field guide are implemented. `continuation` is still not surfaced directly by `walk summary`.

`crates/ploke-eval/docs/prototype1-loop-operator.md`

- Still a good broader operator map for parent checkout/campaign root separation and persisted files.
- It does not yet include the new `walk` command surface as a first-class operator/debugger path.

### Partly stale docs

`2026-06-17_typestate-loop-driver-plan.md`

- Strong conceptual authority/step/replay model.
- Current phase table and success criteria are partly stale because R13b/R14b have since been implemented in code under gates.
- Open design checkpoints are still useful, especially:
  - read-only back semantics;
  - server lifecycle at handoff;
  - long-running `--watch`/progress semantics;
  - coarse-first, fine-grained replay later.

`2026-06-17_typestate-loop-driver-worklog/README.md`

- Excellent slice history through Slice 15 and channel-ref tightening.
- Missing a final current-status addendum for the later commits in this session:
  - read-only replay cursor;
  - R13b/R14b reconstruction/handoff implementation;
  - 5x3 generation-local smoke proof;
  - `walk summary` and help UX improvements.

`2026-06-16_walk-server.md`

- Good cold-start module map and guardrails.
- Stale around current supported path. It still says R13b/R14b handoff/final report are not exposed or not yet admitted. Code now exposes them behind explicit gates.
- Does not mention `walk summary`, `walk replay`, `walk back`, `walk forward`, or `branch-live` in the operator command list.

`2026-06-17_walk-command-guide.md`

- Good practical guide through R14a/stopped path and safety notes.
- Stale around selected-successor handoff: code now admits R13b with `--watch --allow git-changes`.
- Missing the new summary/replay/help behavior.

`2026-06-02_prototype1-state-loop-walkthrough/README.md`

- Useful glossary and historical source-grounded walkthrough.
- It predates the typed driver extraction and should be treated as background, not current implementation authority for the parent turn body.

`crates/ploke-eval/docs/guides/prototype1-step-execution-path.md`

- Useful for the older `prototype1-step` diagnosed-phase controller.
- Separate from `walk`/typed-driver work; should not be read as describing the new typestate driver.

### Stale source comments/docs

`typestate/mod.rs`

- Says typestate map is not wired into the live controller. This is stale.

`driver/reconstruct.rs`

- Says early/R0-R5 reconstruction, but reconstructs through R14b.

`driver/reconstruct.rs` naming

- `EarlyState`, `EarlySnapshot`, `reconstruct_early` are now misleading.
- Rename would be a real code change and should be done separately with GitNexus impact analysis.
- If not renaming immediately, update module/function docs to avoid future confusion.

## Code review observations

### Strengths

1. **Typed axes stayed structural.** The runtime state uses nested axes instead of flattened state names, matching repo naming guidance and the original design goal.
2. **Live edges remain canonical.** `walk` and `run_to_terminal` call direct functions from `live_edges.rs` instead of duplicating transition semantics.
3. **Reconstruction is strict.** Missing child terminal/channel/evaluation evidence blocks rather than fabricating success.
4. **Handoff is gated in the operator surface.** `walk` requires both `--watch` and `--allow git-changes` before R13b can consume selected-successor R12.
5. **Current-generation handoff selection enforces terminal channel citation.** `select_artifact_for_handoff` rejects current-generation candidates without a terminal child-channel `Result` citation.
6. **Replay/back/forward are read-only.** Current implementation matches the desired non-destructive semantics.
7. **CLI discoverability improved.** Help text now explains common workflows and safety gates directly.

### Risks / gaps

1. **Docs understate current R13b/R14b capability.** A future operator reading the older docs may think handoff remains blocked when it is now admitted with gates.
2. **Names understate reconstruction scope.** `reconstruct_early` and `EarlyState` are no longer early-only and could mislead future edits.
3. **`walk summary` is still artifact-parser based.** Good for discovery, but richer inspection should move to typed `EvidenceStore`/History projections before adding more semantics.
4. **Replay output is not cursor-centered.** The current cursor can be at `#6` while recent entries show `#249..#251`. The hint makes truncation clear, but debugging a local cursor still requires manual `--tail` or future centered rendering.
5. **Long-running edge progress is still blocking `--watch`.** The plan’s desired default "return after ~1s and show progress command" remains future work.
6. **Handoff server lifecycle remains unresolved.** Code can admit R13b/R14b, but docs still prefer parent server stop/read-only and fresh successor server semantics as the long-term model. This needs an explicit current implementation note before relying on very long multi-generation live walks.
7. **Test coverage is strongest at parse/shape/unit level.** The 5x3 smoke is strong evidence, but automated end-to-end coverage for R13b/R14b handoff remains limited by fixture/live-run complexity.

## Recommended next documentation cleanup

1. **Add one current status matrix.** Prefer a compact `walk-current-status.md` or update `2026-06-17_walk-command-guide.md` with a phase table:

```text
R0-R7      reconstruct/step supported
R7->R8     live requires --watch
R8->R10    pure/reconstructable from child-plan evidence
R10->R11   live requires --watch
R11->R12   pure projection
R12->R13a  stopped/no-selection
R12->R13b  selected-successor; requires --watch --allow git-changes
R13a->R14a final stopped report
R13b->R14b final handoff report
replay/back/forward read-only cursor only
branch-live provenance record only
```

2. **Update stale source docs.** Minimal docs-only fixes:
   - `typestate/mod.rs`: remove "not wired into live controller yet".
   - `driver/reconstruct.rs`: replace "early/R0-R5" wording with "durable reconstruction through currently supported phases".

3. **Append latest worklog entry.** Add a short worklog section for:
   - R13b/R14b handoff/reconstruction;
   - final 5x3 smoke;
   - replay/summary/help UX slices.

4. **Mark old walkthroughs as historical.** The 2026-06-02 walkthrough is valuable, but it should explicitly say the batch parent turn is now routed through `driver::advance::run_to_terminal`.

5. **Document `walk summary` storage caveat.** Keep its current serverless value, but state that detailed explanation commands should use typed evidence stores.

## Recommended next code cleanup candidates

These are not urgent correctness fixes, but would reduce future agent mistakes:

1. Rename or re-doc:
   - `EarlyState` -> `ReconstructedState` or `DurableState`;
   - `EarlySnapshot` -> `ReconstructionSnapshot`;
   - `reconstruct_early` -> `reconstruct_durable` or `reconstruct_current`.

2. Add cursor-centered replay rendering:

```bash
ploke-eval loop walk replay --index 42 --around 5
```

or make `--tail` cursor-relative when `--index` is supplied.

3. Move `walk summary` toward a typed read model over:
   - `EvidenceStore` / `FsEvidenceStore`;
   - `ChildEvidenceSet`;
   - `project_sealed_selection_commitments` / selection-show projection;
   - `FsBlockStore` for History head/authority.

4. Add tests/smokes that assert operator gates:
   - selected-successor R12 without `--watch` blocks;
   - selected-successor R12 with `--watch` but no `--allow git-changes` blocks;
   - no-selection R12 cannot target R13b/R14b;
   - R13b -> R14b final report path preserves report evidence.

## Bottom line

The typestate feature is substantially further along than several active docs suggest. The code now has:

- a typed batch driver for `prototype1-state`;
- a `walk` operator surface over the same live edges;
- durable reconstruction through stopped and handoff-final states when evidence exists;
- read-only historical replay cursor movement;
- guarded selected-successor handoff from `walk`.

The main immediate risk is not an obvious code invariant violation; it is documentation drift around R13b/R14b and misleading "early reconstruction" naming. The safest next work is a docs cleanup/status matrix plus small source-comment fixes before more behavior is added.
