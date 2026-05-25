# Eval Patch Artifact Collision And Empty Diff

`ploke-eval` currently produces untrustworthy patch artifacts for some runs.
This is not just low agent success. We have evidence of output corruption,
cross-run artifact collision, and successful patch-tool calls that do not
survive to final diff capture.

## Severity

High. This undermines:

- campaign patch-rate metrics
- benchmark submission exports
- any analysis that treats `multi-swe-bench-submission.jsonl` as authoritative
- comparisons between control/setup-only and treatment/agent runs

## Confirmed Evidence

### 1. Setup-only runs can have non-empty submission patches

Two runs currently show a non-empty `fix_patch` even though their
`record.json.gz` says they were `setup-only` runs with `0` agent turns:

- `clap-rs__clap-3311`
- `clap-rs__clap-2648`

For `clap-rs__clap-3311`:

- [record.json.gz](/home/brasides/.ploke-eval/runs/clap-rs__clap-3311/record.json.gz)
  records `run_arm.execution = "setup-only"` and `0` turns.
- [execution-log.json](/home/brasides/.ploke-eval/runs/clap-rs__clap-3311/execution-log.json)
  shows the shell-only control flow and still includes `write_msb_submission`.
- [multi-swe-bench-submission.jsonl](/home/brasides/.ploke-eval/runs/clap-rs__clap-3311/multi-swe-bench-submission.jsonl)
  contains a huge unrelated diff.

That artifact combination should be impossible if run outputs are isolated and
internally coherent.

### 2. Most empty submissions never reached editing

Across the `rust-baseline-grok4-xai` campaign:

- `221` complete runs have submission files
- `49` have non-empty `fix_patch`
- `172` have empty `fix_patch`

Breakdown:

- `136` empty patch, no `non_semantic_patch` or `apply_code_edit` call recorded
- `25` empty patch, patch tools used but all attempts failed
- `11` empty patch, at least one patch tool call reported success
- `40` non-empty patch, patch tool used
- `9` non-empty patch, no patch tool recorded

So the low patch count is not just export loss. Most runs never land an edit.
But there is also a smaller integrity problem around patch capture itself.

### 3. Successful patch-tool calls can still yield an empty final diff

`tokio-rs__tokio-5781` is a concrete example:

- [multi-swe-bench-submission.jsonl](/home/brasides/.ploke-eval/runs/tokio-rs__tokio-5781/multi-swe-bench-submission.jsonl)
  has `fix_patch: ""`
- `inspect turn 1 --show tool-calls` shows a successful `non_semantic_patch`
  call after an earlier malformed one
- `inspect turn 1 --show tool-result --index 11` reports:
  - `ok: true`
  - `staged: 1`
  - `applied: 0`
  - file `tokio/src/io/ready.rs`
- the run record shows expected target files unchanged in the final patch artifact

So a reported successful patch-tool call is not enough to trust final patch
capture.

### 3a. The suspicious empty-patch bucket is a coherent artifact class

Using the persisted `~/.ploke-eval` run artifacts, the
`edit_reported_success_but_empty_final_patch` bucket resolves to `11` runs
with the same local signature:

- `multi-swe-bench-submission.jsonl` has `fix_patch: ""`
- `agent-turn-trace.json` has `terminal_record.summary == "Request summary:
  [success]"`
- `patch_artifact.edit_proposals` is non-empty

The matching runs are:

- `clap-rs__clap-3874`
- `clap-rs__clap-4032`
- `clap-rs__clap-4081`
- `clap-rs__clap-4151`
- `clap-rs__clap-4408`
- `clap-rs__clap-4474`
- `clap-rs__clap-4635`
- `clap-rs__clap-5080`
- `sharkdp__bat-1518`
- `tokio-rs__tokio-4789`
- `tokio-rs__tokio-5781`

Representative shape:

- `tokio-rs__tokio-5781`, `tokio-rs__tokio-4789`, `clap-rs__clap-3874`, and
  `sharkdp__bat-1518` all show:
  - a staging-style success payload such as `{"ok":true,"staged":1,"applied":0}`
  - then `Failed to apply edits across 1 files`
  - then `No edits were applied`
  - then empty final `fix_patch`
- `clap-rs__clap-4032` is the clearest misleading-status exemplar:
  - `apply_code_edit` reports `applied: 0`, `ok: false`
  - the UI summary still says `Applied 0 edits across 1 files`
  - the payload still carries `status: "applied"`
  - the final submission patch is empty

So the issue is not just "some runs happened to end empty". We have a stable
bucket where run/session summaries read as success while the durable patch
artifact is empty.

### 4. Repo cleanup is incomplete before runs

`checkout_repo_to_base()` at
[runner.rs](/home/brasides/code/ploke/crates/ploke-eval/src/runner.rs:2541)
does:

- `git reset --hard`
- `git checkout --detach <base_sha>`

It does not run `git clean`. At least one recorded setup phase still shows
untracked files in `git_status_porcelain`, which increases contamination risk
for shared repo checkouts.

## Current Root-Cause Hypotheses

### A. Run output identity is too coarse

`prepare_record()` writes every run to `runs_root/<instance_id>`:
[msb.rs](/home/brasides/code/ploke/crates/ploke-eval/src/msb.rs:140)

That means:

- control/setup-only runs
- treatment/agent runs
- retries / reruns for the same instance

all target the same output directory and the same submission filename:

- `run.json`
- `record.json.gz`
- `execution-log.json`
- `multi-swe-bench-submission.jsonl`

This is the most likely explanation for setup-only records coexisting with
non-empty submission patches.

### B. Submission writing is too permissive

Both run flows write `multi-swe-bench-submission.jsonl`:

- setup-only/control path:
  [runner.rs](/home/brasides/code/ploke/crates/ploke-eval/src/runner.rs:1387)
- agent/treatment path:
  [runner.rs](/home/brasides/code/ploke/crates/ploke-eval/src/runner.rs:1787)

Writing submission artifacts from setup-only runs is conceptually wrong even
without the directory-collision bug.

### C. Patch-tool success semantics are weaker than “repo changed”

At least some successful `non_semantic_patch` calls appear to mean “edit staged
in tool bookkeeping” rather than “change persisted in the working tree and final
diff”.

That gap makes final patch metrics under-explain where edits are disappearing.

## Traced Failure Modes

### 1. Tool success currently means “proposal staged”, not “repo changed”

`apply_code_edit` stages an `EditProposal` and emits a successful tool
completion before any durable repo write has happened:

- proposal staged in
  [crates/ploke-tui/src/rag/tools.rs](/home/brasides/code/ploke/crates/ploke-tui/src/rag/tools.rs:780)
- result payload built as `ApplyCodeEditResult { ok: true, staged, applied: 0 }`
  in
  [crates/ploke-tui/src/rag/tools.rs](/home/brasides/code/ploke/crates/ploke-tui/src/rag/tools.rs:837)
- completion emitted in
  [crates/ploke-tui/src/rag/tools.rs](/home/brasides/code/ploke/crates/ploke-tui/src/rag/tools.rs:869)

`ns_patch` follows the same staging-first pattern through
[crates/ploke-tui/src/tools/code_edit.rs](/home/brasides/code/ploke/crates/ploke-tui/src/tools/code_edit.rs:134)
and the generic dispatcher in
[crates/ploke-tui/src/tools/mod.rs](/home/brasides/code/ploke/crates/ploke-tui/src/tools/mod.rs:275).

This means the session layer can legitimately record tool success while no file
change has landed yet.

### 2. Semantic apply can mark proposals `Applied` with `applied == 0`

The strongest current bug is in the semantic apply path:

- `approve_edits()` dispatches semantic proposals to `apply_semantic_edit()` in
  [crates/ploke-tui/src/rag/editing.rs](/home/brasides/code/ploke/crates/ploke-tui/src/rag/editing.rs:73)
- `apply_semantic_edit()` counts successful per-file writes into `applied` in
  [crates/ploke-tui/src/rag/editing.rs](/home/brasides/code/ploke/crates/ploke-tui/src/rag/editing.rs:294)
- the JSON payload honestly sets `"ok": applied > 0` in
  [crates/ploke-tui/src/rag/editing.rs](/home/brasides/code/ploke/crates/ploke-tui/src/rag/editing.rs:311)
- but the function then unconditionally sets
  `proposal.status = EditProposalStatus::Applied` in
  [crates/ploke-tui/src/rag/editing.rs](/home/brasides/code/ploke/crates/ploke-tui/src/rag/editing.rs:319)
- and emits UI `status = "applied"` in
  [crates/ploke-tui/src/rag/editing.rs](/home/brasides/code/ploke/crates/ploke-tui/src/rag/editing.rs:336)

So `Ok(Vec<Result<...>>)` from the write layer can contain only per-file
failures, leaving `applied == 0`, while the proposal and UI still say
`Applied`.

### 3. `ploke-eval` trusts proposal status more than durable mutation

Patch capture is currently split:

- `patch_artifact.applied` is inferred from proposal status strings in
  [crates/ploke-eval/src/runner.rs](/home/brasides/code/ploke/crates/ploke-eval/src/runner.rs:801)
- expected-file change detection is computed separately in
  [crates/ploke-eval/src/runner.rs](/home/brasides/code/ploke/crates/ploke-eval/src/runner.rs:807)
- final benchmark submission truth comes only from `git diff` in
  [crates/ploke-eval/src/runner.rs](/home/brasides/code/ploke/crates/ploke-eval/src/runner.rs:860)

That means a proposal can be recorded as `Applied`, `patch_artifact.applied`
can become `true`, and the final `fix_patch` can still be empty.

### 4. Historical setup-only leakage was real and had two causes

The earlier setup-only corruption was not hypothetical:

- both setup-only and treatment runs wrote into the same
  `runs/<instance_id>` root
- setup-only also wrote `multi-swe-bench-submission.jsonl` itself

This is why runs like `clap-rs__clap-3311` can show:

- `run_arm.execution = "setup-only"`
- zero agent turns
- `write_msb_submission` in `execution-log.json`
- non-empty submission diff

### 5. Current nested run dirs fix write collision, but read-side lookup is still arm-agnostic

The current worktree already fixes the direct write collision:

- nested per-run dirs under `runs/<instance>/runs/run-...`
- treatment-only submission writing via
  `maybe_build_msb_submission_record()`

But read-side selection is still "latest run wins", regardless of arm:

- `latest_run_dir_for_instance()` in
  [crates/ploke-eval/src/run_history.rs](/home/brasides/code/ploke/crates/ploke-eval/src/run_history.rs:99)
- campaign submission lookup in
  [crates/ploke-eval/src/cli.rs](/home/brasides/code/ploke/crates/ploke-eval/src/cli.rs:2981)
- `--instance` record resolution in
  [crates/ploke-eval/src/cli.rs](/home/brasides/code/ploke/crates/ploke-eval/src/cli.rs:7831)
- closure artifact selection in
  [crates/ploke-eval/src/closure.rs](/home/brasides/code/ploke/crates/ploke-eval/src/closure.rs:566)

So a newer setup-only run can still hide an older treatment run on the read
side even though the files are now isolated on disk.

## Immediate Risks

- campaign patch-rate summaries are inflated or deflated by mixed run artifacts
- exported submission files may be attributed to the wrong execution arm
- setup-only runs can look like they generated benchmark patches
- patch-tool success metrics can overstate actual repository mutation
- semantic edit status can claim `Applied` while `applied == 0`
- session-level `success` summaries can coexist with empty final `fix_patch`

## Recommended Fix Order

1. Fix semantic apply status:
   - only set `Applied` when `applied > 0`
   - emit failed/non-applied UI state otherwise
2. Tighten tool/session semantics:
   - distinguish proposal staged from patch landed
   - do not let `ToolCallCompleted` imply durable mutation
3. Make `patch_artifact.applied` depend on real mutation evidence:
   - successful apply count
   - expected-file change
   - or final repo diff
4. Introduce unique run output identity beyond bare `instance_id`.
5. Stop writing benchmark submission artifacts for setup-only/control runs.
6. Add a run-integrity check:
   - setup-only run with non-empty `fix_patch` => hard error
   - setup-only run with any agent turns => hard error
7. Make lookup arm-aware:
   - campaign export should prefer treatment runs
   - `--instance` inspection should not silently choose the wrong arm
8. Consider stricter repo scrubbing before checkout, including untracked files.

## Minimal Repro Cases To Preserve

- `clap-rs__clap-3311`
- `clap-rs__clap-2648`
- `tokio-rs__tokio-5781`

These should be used as regression checks when the output-isolation fix lands.
