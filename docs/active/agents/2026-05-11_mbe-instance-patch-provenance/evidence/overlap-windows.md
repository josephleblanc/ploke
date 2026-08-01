# Direct Answer
Yes. The same benchmark repo root, `/home/brasides/.ploke-eval/repos/BurntSushi/ripgrep`, was used by runs that clearly overlapped in time. I also found at least one evaluated baseline/treatment pair that was serialized, so overlap was not universal, but it was real.

# Bounded Commands Used
- `rg --files` on the campaign and instance roots to locate typed artifacts.
- `ls -lh` and `wc -l` before reading unknown-size files.
- `stat -c '%y'` on runner-result, node, registry, and msb submission files.
- `jq` on small JSON projections (`run.json`, `node.json`, `runner-result.json`, `agent-turn-summary.json`, branch evaluation JSON).
- `gzip -cd ... | jq` on `record.json.gz`, with output narrowed to timing fields only.
- `sed -n` on the branch registry JSONL, with width/record bounds.

# Runs Compared
## Same-parent batch that overlaps
All rows below share `parent_branch_id = prototype1-parent-p1-history-traversal-20260511-2-gen0` in `branches.json` and the same repo root in `record.json.gz`.

| branch_id | run | started_at (UTC) | ended_at (UTC) | relation |
|---|---|---:|---:|---|
| `branch-f2b6d7ed1e5c5c39` | `run-1778502757642-structured-current-policy-07140f90` | `2026-05-11T12:32:36.848355324+00:00` | `2026-05-11T12:36:27.783205936+00:00` | overlaps later siblings |
| `branch-cd5d2f57506bfd16` | `run-1778502760411-structured-current-policy-a221dbb1` | `2026-05-11T12:32:39.619524751+00:00` | `2026-05-11T12:42:09.738222873+00:00` | started before row 1 ended |
| `branch-cfd7c73a8d27c96e` | `run-1778502762074-structured-current-policy-cf9db0f0` | `2026-05-11T12:32:41.555480523+00:00` | `2026-05-11T12:37:25.296642862+00:00` | started before row 1 ended |
| `branch-e6f76ed7620b3802` | `run-1778502762263-structured-current-policy-ec03c38a` | `2026-05-11T12:32:41.611687753+00:00` | `2026-05-11T12:43:03.615019059+00:00` | started before row 1 ended |
| `branch-ed5fea2d78b270c6` | `run-1778502765581-structured-current-policy-5e838b51` | `2026-05-11T12:32:41.615300302+00:00` | `2026-05-11T12:43:14.570434118+00:00` | started before row 1 ended |

## Serialized evaluated pair
This is a direct baseline/treatment comparison from `branch-744fd327c760827d`'s evaluation report.

| role | branch_id | run | started_at (UTC) | ended_at (UTC) | relation |
|---|---|---:|---:|---:|---|
| baseline | `branch-8e1c1765c37d9dad` | `run-1778505878234-structured-current-policy-1d09ca9d` | `2026-05-11T13:24:37.444819571+00:00` | `2026-05-11T13:28:55.404169211+00:00` | finished before treatment started |
| treatment | `branch-744fd327c760827d` | `run-1778507241202-structured-current-policy-bbed25a2` | `2026-05-11T13:47:20.559646813+00:00` | `2026-05-11T14:02:54.678400708+00:00` | serialized after baseline |

# Overlap Evidence
- The five-run batch above is the strongest proof: the second through fifth runs all started within about 3 seconds of the first run and while the first was still active.
- Example: `run-1778502760411...` began `2.77s` after `run-1778502757642...` and still ran for another `~9m30s`.
- Their `multi-swe-bench-submission.jsonl` mtimes also line up with the same ordering, which is consistent with the record timing.
- `agent-turn-summary.json` shows successful completion for the sampled runs, so these are ordinary finished child runs, not aborted placeholders.

# No-Overlap / Disproof Evidence
- The evaluated baseline/treatment pair for `branch-744fd327c760827d` did **not** overlap: the baseline ended at `13:28:55.404 UTC`, and the treatment did not start until `13:47:20.560 UTC`.
- `run.json` / registry projections are too sparse for overlap proof here; the `record.json.gz` timing fields are the authoritative source.
- Campaign node projections are coarse completion markers only: sampled `runner-result.json` files show `generation`, `status`, and `recorded_at`, while the paired `node.json` files have `recorded_at: null`.

# Caveats
- `record.json.gz` is the only file family here that gives both start and end windows for the run itself.
- `stat` mtimes are file-write times, not run-start times.
- `branches.json` confirms same-parent lineage for the overlapping batch, but the overlap conclusion still comes from the run record windows.
- I sampled only the most relevant pairs and one clear same-parent batch; I did not exhaustively compare all 51 runs in the instance tree.

# Next Exact Question
Should I extend this to a full overlap matrix for all `BurntSushi__ripgrep-2209` runs, or trace one specific branch pair back to its registry and msb submission artifacts end to end?
