# Prototype 1 Score Grounding Handoff - 2026-05-06

This handoff is for a cold restart after the score/evidence discussion and the
fresh 5-generation loop setup.

## Current Goal

We are still on long-horizon item 1 from
`docs/active/todo/2026-05-05_long-horizon.md`:

> Inventory and unify eval evidence. Find all run/eval/protocol/report/log data
> we already generate, preserve provenance, and decide what can count as
> admissible evidence for child selection.

The immediate grounding question was: can we print a score, and why is the
printed score not yet useful as a decision-grade successor signal?

## Important User Direction

- Use `History` as the durable spine for intervention, evaluation, and selection
  evidence. Do not create a shadow archive/selection evidence layer outside
  `History`.
- Do not reintroduce `archive_candidate.rs` or the discarded semantic edit
  staging in `ploke-eval`.
- Do not reimplement `ploke-tui` semantic edit correctness in `ploke-eval`.
- Avoid "projection" language unless it is precise. The user is explicitly
  frustrated by projection-shaped wrappers becoming fake domain objects.
- Structural naming matters. The recent failure mode was renaming flattened
  structures instead of separating their parts.
- Large file growth in `ploke-eval` is an active concern. Do not continue blob
  growth. Split by real structure, not by cosmetic naming.

## Current Main Checkout Status

Main checkout:

```text
/home/brasides/code/ploke
branch: prototype1-loop-test-20260426
```

Dirty state at handoff:

```text
 M crates/ploke-eval/src/cli/prototype1_state/cli_facing.rs
 D crates/ploke-eval/src/cli/prototype1_state/score.rs
?? crates/ploke-eval/src/cli/prototype1_state/score/
```

This is the in-progress score-module split. The user started moving code out of
`score.rs`; prior workers split some parts into:

```text
crates/ploke-eval/src/cli/prototype1_state/score/component.rs
crates/ploke-eval/src/cli/prototype1_state/score/operational.rs
crates/ploke-eval/src/cli/prototype1_state/score/profile.rs
crates/ploke-eval/src/cli/prototype1_state/score/protocol.rs
crates/ploke-eval/src/cli/prototype1_state/score/select.rs
crates/ploke-eval/src/cli/prototype1_state/score/tests.rs
crates/ploke-eval/src/cli/prototype1_state/score/mod.rs
```

Recent verification before handoff:

```text
cargo fmt --all --check
cargo check -p ploke-eval
cargo test -p ploke-eval --lib prototype1_state::score
```

These passed after the split and after a cosmetic rename pass. Warnings existed.

Do not assume the current score module structure is good. The user explicitly
stopped editing after pointing out that `ScoreReport`/`ScoreSnapshot` renames
did not fix the structure.

## Fresh 5-Generation Loop Setup

A new parent worktree was created and setup succeeded:

```text
campaign_id: p1-5gen-score-grounding-20260506-1
worktree: /home/brasides/.ploke-eval/worktrees/p1-5gen-score-grounding-20260506-1
instance_id: BurntSushi__ripgrep-2209
dataset_key: ripgrep
parent_id/node_id: node-7ab22f00d4824713
branch_id: prototype1-parent-p1-5gen-score-grounding-20260506-1-gen0
```

Search policy from setup:

```text
generations<=5
nodes<=32
children=2..=6
stop_on_first_keep=no
require_keep_for_continuation=yes
explore_from_rejected=yes
```

Setup command that succeeded:

```bash
cd /home/brasides/.ploke-eval/worktrees/p1-5gen-score-grounding-20260506-1
/home/brasides/code/ploke/target/debug/ploke-eval loop prototype1-setup \
  --campaign p1-5gen-score-grounding-20260506-1 \
  --dataset-key ripgrep \
  --instance BurntSushi__ripgrep-2209 \
  --max-generations 5 \
  --max-total-nodes 32
```

Note: an earlier setup attempt with `--dataset-key multi-swe-bench` failed
harmlessly because that registry key was not found. It did not create the
campaign directory or parent branch.

The runtime was built inside the parent worktree:

```bash
cd /home/brasides/.ploke-eval/worktrees/p1-5gen-score-grounding-20260506-1
cargo build -p ploke-eval
```

Build finished successfully. The parent worktree was clean after build.

Monitor check succeeded:

```bash
cd /home/brasides/.ploke-eval/worktrees/p1-5gen-score-grounding-20260506-1
./target/debug/ploke-eval loop prototype1-monitor \
  --campaign p1-5gen-score-grounding-20260506-1 \
  --repo-root . \
  report
```

It showed only gen0 planned, no evaluations yet.

## Live Loop Handoff

Codex did not start the live loop. The user should run it from a normal
terminal, inside the parent worktree:

```bash
cd /home/brasides/.ploke-eval/worktrees/p1-5gen-score-grounding-20260506-1
PLOKE_PROTOTYPE1_TRACE_JSONL=auto \
./target/debug/ploke-eval --debug-tools loop prototype1-state --repo-root .
```

Do not run `prototype1-state` from Codex. Observe with monitor commands only.

Useful observation commands after the user starts it:

```bash
cd /home/brasides/.ploke-eval/worktrees/p1-5gen-score-grounding-20260506-1
./target/debug/ploke-eval loop prototype1-monitor \
  --campaign p1-5gen-score-grounding-20260506-1 \
  --repo-root . \
  report

./target/debug/ploke-eval loop prototype1-monitor \
  --campaign p1-5gen-score-grounding-20260506-1 \
  --repo-root . \
  history-scores --rows 20

./target/debug/ploke-eval loop prototype1-monitor \
  --campaign p1-5gen-score-grounding-20260506-1 \
  --repo-root . \
  score-selection-review --rows 50
```

Prefer compact monitor/report commands. Do not dump full runner results,
transition journals, stream logs, full provider responses, patches, or
`slice.jsonl` into context.

## What We Learned From The Previous Score Print

Command run from main checkout:

```bash
cargo run -p ploke-eval -- history scores --rows 5
```

Resolved campaign:

```text
p1-3gen-selection-evidence-20260506-1
```

It printed four child rows, but no comparable numeric score:

```text
gen | node | branch | state | score | evaluations | comparable | missing_metrics | diagnostics | eval_set
1 | node-0ec4e8a0a155a0ee | branch-a6c6dafb70654d7b | incomplete | - | 0 | 0 | 0 | 1 | -
1 | node-93cfcfecb1494055 | branch-8680aa095ad2df73 | incomplete | - | 0 | 0 | 0 | 1 | -
1 | node-e9a220315c7400c8 | branch-64c686b8f50bab29 | invalid | - | 1 | 0 | 1 | 4 | compared_instance_ids ids=1 missing=0
0 | node-f977bac6fedf8795 | prototype1-parent-p1-3gen-selection-evidence-20260506-1-gen0 | incomplete | - | 0 | 0 | 0 | 1 | -
```

Diagnostics said:

- Most children had no evaluation reports to score.
- The one evaluated child lacked `evaluation_procedure_id`,
  `evaluator_identity`, and `eval_set_identity`.
- One typed baseline run had role `Treatment`, expected `Control`.

## Sub-Agent Findings

All sub-agents were closed before handoff.

### Missing Evaluations

Most `evaluations = 0` rows in `p1-3gen-selection-evidence-20260506-1` are not
evidence-discovery bugs. There was only one evaluation artifact in that
campaign, for:

```text
node-e9a220315c7400c8 / branch-64c686b8f50bab29
```

The other gen1 nodes were still `planned` and had no `runner-result.json`.

Relevant paths:

```text
/home/brasides/.ploke-eval/campaigns/p1-3gen-selection-evidence-20260506-1/prototype1/scheduler.json
/home/brasides/.ploke-eval/campaigns/p1-3gen-selection-evidence-20260506-1/prototype1/evaluations/branch-64c686b8f50bab29.json
/home/brasides/.ploke-eval/campaigns/p1-3gen-selection-evidence-20260506-1/prototype1/nodes/node-e9a220315c7400c8/runner-result.json
```

Relevant code:

```text
crates/ploke-eval/src/cli/prototype1_state/history_preview.rs:142-202
crates/ploke-eval/src/cli/prototype1_state/evidence.rs:183-255
crates/ploke-eval/src/cli/prototype1_state/score/mod.rs:497-551
crates/ploke-eval/src/cli/prototype1_state/report.rs:76-86
```

### Missing Evaluation Identity

The missing eval identity in the old campaign is stale artifact data, not a
current producer omission.

Current code writes:

- `evaluation_procedure_id`
- `evaluator_identity`
- `eval_set_identity`

Relevant code:

```text
crates/ploke-eval/src/cli/prototype1_state/cli_facing.rs:6466-6538
crates/ploke-eval/src/cli/prototype1_state/evidence.rs:1539-1564
crates/ploke-eval/src/cli/prototype1_state/score/mod.rs:689-807
```

The old artifact at:

```text
/home/brasides/.ploke-eval/campaigns/p1-3gen-selection-evidence-20260506-1/prototype1/evaluations/branch-64c686b8f50bab29.json
```

has only `overall_disposition`, `reasons`, and `compared_instances`; it lacks
the typed identity fields.

### Selection Review

For `p1-3gen-selection-evidence-20260506-1`,
`history score-selection-review --format json --rows 50` produced:

```text
7 rows total
1 projected/joined row
3 failed_projection rows
3 missing_selection rows
local_alpha: null everywhere
```

The only joined row was:

```text
node-e9a220315c7400c8 / branch-64c686b8f50bab29 / gen 1
```

It had a selector result but its child score was invalid, so no local alpha.

Current join behavior is coordinate-based:

```text
node_id + branch_id + generation
```

Relevant code:

```text
crates/ploke-eval/src/cli/prototype1_state/score/mod.rs:434-455
crates/ploke-eval/src/cli/prototype1_state/score/select.rs:171-239
crates/ploke-eval/src/cli/prototype1_state/evidence.rs:101-113
crates/ploke-eval/src/successor_selection/evidence.rs:9-31
```

This is useful as a selection audit, but not yet durable selection evidence.
It is also a known drift point because selection input construction currently
lives in the evidence/report path rather than flowing through `History`.

### Unanswered Due To Switching Tasks

Two explorers were intentionally shut down when we switched to setting up the
fresh 5-gen run. Their questions remain open:

- Where exactly did the bad baseline/control vs treatment `RunRegistration`
  role come from?
- Do any existing campaigns already contain a complete comparable numeric
  score?

The fresh 5-gen campaign is expected to answer the second question more
directly.

## Score Module Structural Warning

Do not continue the current superficial naming cleanup.

The user specifically called out:

- `ScoreReport` is not one concept. It mixes score data with report envelope and
  filtering.
- `ScoreSnapshot` is not one concept. It mixes score data with runtime/report
  context.

The next structural pass should identify real carriers before editing.
Likely decompositions:

- score/assessment model: the actual scored evidence/assessment data
- identity validation: evaluation procedure, evaluator, eval set, run
  registration identity
- report/output envelope: CLI/JSON/table request, generated_at, campaign,
  paths, row limits
- selection review/join: current audit joining score evidence to selection
  inputs

Do not just rename these types again.

## Suggested Cold Restart Sequence

1. Read this handoff.
2. Read `docs/active/todo/2026-05-05_long-horizon.md`.
3. Check whether the user has started the live 5-gen loop.
4. If the loop is running or complete, observe only with compact monitor
   commands from the parent worktree.
5. After it has child evals, run `history-scores` and
   `score-selection-review` for the new campaign.
6. Use the fresh campaign to answer:
   - do current eval artifacts carry typed identity?
   - are baseline/treatment run roles correct now?
   - do any children become `complete` and produce comparable scores?
   - does selection review join cleanly?
7. Only then return to score-module cleanup, and do it structurally rather than
   cosmetically.

