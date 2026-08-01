# ploke-eval CLI audit: post-patch cold-start read

1. If I were dropped cold into this repo, for one instance I would run:
   `./target/debug/ploke-eval list-msb-datasets`
   `./target/debug/ploke-eval fetch-msb-repo --dataset-key <dataset-key>`
   `./target/debug/ploke-eval model current`
   `./target/debug/ploke-eval model providers [MODEL_ID]`
   `./target/debug/ploke-eval prepare-msb-single --dataset-key <dataset-key> --instance <instance-id>`
   `./target/debug/ploke-eval run-msb-agent-single --instance <instance-id> [--provider <slug>]`

   For campaign/family progress I would run:
   `./target/debug/ploke-eval campaign show --campaign <campaign>`
   `./target/debug/ploke-eval closure status --campaign <campaign>`
   `./target/debug/ploke-eval closure advance eval --campaign <campaign> --dry-run`
   `./target/debug/ploke-eval closure advance eval --campaign <campaign>`

   For export I would run:
   `./target/debug/ploke-eval campaign export-submissions --campaign <campaign>`

2. Yes. The help now makes these locations explicit:
   typed target registry: `~/.ploke-eval/registries/multi-swe-bench-rust.json`
   closure state: `~/.ploke-eval/campaigns/<campaign>/closure-state.json`
   default campaign export file: `~/.ploke-eval/campaigns/<campaign>/multi-swe-bench-submission.jsonl`
   with `--nonempty-only`: `~/.ploke-eval/campaigns/<campaign>/multi-swe-bench-submission.nonempty.jsonl`

3. Yes. The trust story is materially clearer now.
   Trust most: per-run artifacts for the chosen run, especially the per-run `multi-swe-bench-submission.jsonl`.
   Next: `campaign export-submissions` output.
   Then: closure state for campaign progress.
   Do not treat the raw batch aggregate `multi-swe-bench-submission.jsonl` as authoritative if batches may have been rerun, interrupted, or overlapped.
   Do not treat terminal summary strings as proof.
   Also, local `ploke-eval` artifacts are telemetry and candidate patch artifacts, not the official benchmark verdict; official pass/fail still comes from the external evaluator.

4. Remaining CLI ambiguities after the help patch:
   It is still not fully crisp whether `run-msb-single` should normally be run before `run-msb-agent-single`, or whether `run-msb-agent-single` is the standalone default happy path. The top-level recommended workflow shows both, while the operator stance and command description imply the agent command is the normal path.
   "Per-run artifacts" is clearer than before, but there is still a small location ambiguity between `~/.ploke-eval/runs/<instance>` and the nested agent run directories under `~/.ploke-eval/runs/<instance>/runs/run-<timestamp>-<arm>-<suffix>`, which is where `run-msb-agent-single` says the key agent-run files live.
   Campaign/family bootstrap is still somewhat implicit: the help now explains the state files and the steady-state commands, but not how a cold operator should discover valid campaign names or decide whether to use an existing campaign versus `campaign init`.
   `campaign export-submissions` says it exports from completed runs in closure state and reads per-run submission artifacts, but the help does not say how it resolves multiple completed runs for the same instance if more than one exists.
