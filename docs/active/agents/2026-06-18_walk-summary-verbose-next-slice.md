# 2026-06-18 Walk Summary Verbose Context Slice

Short restart note for the next operator-discovery slice on Prototype 1 `walk`.

Related files:
- `docs/active/agents/2026-06-17_typestate-loop-driver-plan.md`
- `docs/active/agents/2026-06-17_walk-command-guide.md`
- `crates/ploke-eval/src/cli/prototype1_state/walk/summary.rs`
- `crates/ploke-eval/src/cli/prototype1_state/history_preview.rs`
- `crates/ploke-eval/src/cli/prototype1_state/evidence.rs`

## Goal

Make `ploke-eval loop walk summary` more self-explaining for operator/debugger use without adding a broad, magical `walk why` command.

## Current issue

Compact summary rows can show values such as:

```text
gen1 ... selection=Stop handoff=acknowledged
```

That is technically derived from the candidate-local successor decision, but it is easy to misread as "the campaign stopped here." The actual durable evidence for the 5x3 run showed:

- selected candidate branch disposition: `reject`
- candidate-local selection outcome: `Stop`
- continuation disposition: `continue_explore_from_rejected`
- handoff: `acknowledged`

So the run continued from a rejected candidate because policy allowed rejected-branch exploration.

## Next implementation slice

1. Add `--verbose` / `-v` to `walk summary`.
2. Change compact generation rows to distinguish:
   - `decision` / candidate-local selection outcome
   - `branch` / selected branch disposition when available
   - `continuation` / continuation disposition when available
   - `handoff`
3. In verbose mode, add field explanations/glossary, especially:
   - `decision=Stop` does not necessarily mean the campaign stopped.
   - `continuation=continue_explore_from_rejected` means policy admitted a rejected selected branch as the next exploration parent.
4. Add next-command hints to the existing typed inspection surface, e.g.:

```bash
ploke-eval history --repo-root <root> selection-show --row <row> --replay
ploke-eval history --repo-root <root> score-selection-review --generation <N>
```

## Storage/authority direction

Do not grow raw file parsing in `walk` long-term. The richer inspection path should be backed by the existing typed evidence adapter:

- `EvidenceStore`
- `FsEvidenceStore`
- `ChildEvidenceSet`
- `ScoreSelectionReview`
- sealed selection projection from `history_preview`

`FsBlockStore` / `BlockStore` should be used for sealed History head/authority inspection, not as the primary child-evidence explanation source.

## Verification targets

- `cargo fmt --all`
- `cargo check -p ploke-eval --all-targets`
- `cargo test -p ploke-eval loop_walk_summary_command_parses --all-targets`
- Smoke `walk summary` and `walk summary -v` against:
  `/home/brasides/.ploke-eval/worktrees/p1-walk5x3local2295-det-g25p-p25f-20260618-122326`
