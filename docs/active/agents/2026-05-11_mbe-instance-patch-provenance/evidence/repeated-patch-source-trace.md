# Direct Answer
The repeated `fix_patch` hash is `40a13afa29659212ed068e2e51d4d6665f018bf9047849643a1a421df2f63e9e` (`4444` bytes, `123` lines). The recorded path evidence points at the shared benchmark checkout under `/home/brasides/.ploke-eval/repos/BurntSushi/ripgrep`, not at any candidate node's `workspace_root`.

That said, the exact `git diff` working directory is not persisted as a separate field. So the best-supported reading is "shared benchmark checkout", but the command cwd itself remains implicit.

# Repeated Patch Chosen
- Hash: `40a13afa29659212ed068e2e51d4d6665f018bf9047849643a1a421df2f63e9e`
- Sample nodes: `node-e73f3c82afd7f4c3`, `node-d60e06343e095ab0`, `node-aa63876e81dde4f0`
- Patch size: `4444` bytes, `123` lines

# Nodes / Runs Compared
| Node | `workspace_root` | Branch | Target |
|---|---|---|---|
| `node-e73f3c82afd7f4c3` | `/home/brasides/.ploke-eval/campaigns/p1-history-traversal-20260511-2/prototype1/nodes/node-e73f3c82afd7f4c3/worktree` | `branch-f8cef63febe9b993` | `docs/reports/prototype1-loop-introspection-survey-2026-05-02/shared-agent-brief.md` |
| `node-d60e06343e095ab0` | `/home/brasides/.ploke-eval/campaigns/p1-history-traversal-20260511-2/prototype1/nodes/node-d60e06343e095ab0/worktree` | `branch-7119d9388f981fd1` | `docs/reports/prototype1-message-invariant/review-a.md` |
| `node-aa63876e81dde4f0` | `/home/brasides/.ploke-eval/campaigns/p1-history-traversal-20260511-2/prototype1/nodes/node-aa63876e81dde4f0/worktree` | `branch-c4b37c0250843fe0` | `docs/reports/prototype1-message-invariant/README.md` |

All three runs wrote to distinct treatment outputs and distinct submission paths under their own `runs/run-.../multi-swe-bench-submission.jsonl` files.

# Source Path Evidence
- `record.json.gz` setup state says `repo_root=/home/brasides/.ploke-eval/repos/BurntSushi/ripgrep`, with the same `base_sha` and a clean `git_status_porcelain`.
- `agent-turn-summary.json` records the patch artifact file as `/home/brasides/.ploke-eval/repos/BurntSushi/ripgrep/crates/printer/src/util.rs`.
- The same summary also shows `Content changed for "/home/brasides/.ploke-eval/repos/BurntSushi/ripgrep/crates/printer/src/util.rs"`.
- `phases.packaging.msb_submission_path` points to the treatment submission file under the run root, but does not record a separate checkout cwd.

# Workspace vs Repo Dir Comparison
- Candidate `workspace_root` values are per-node and distinct.
- Benchmark config uses shared `repo_dir=/home/brasides/.ploke-eval/repos`, with node-local `workdir` and `output_dir` under `/tmp/ploke-mbe-campaign-p1-history-traversal-20260511-2-v2/...`.
- The persisted repo state anchors to the shared benchmark checkout path `/home/brasides/.ploke-eval/repos/BurntSushi/ripgrep`, which is separate from every candidate node `workspace_root`.
- The recorded patch file path sits under the shared checkout, not under any candidate worktree.

# What Is Proven
- The repeated patch bytes are identical across all three nodes.
- The candidate nodes do not share a `workspace_root`.
- The benchmark repo root is shared and explicitly persisted.
- The patch artifact and related preview text point at a file inside that shared repo root.

# What Is Not Proven
- The exact `git diff` cwd is not stored as a dedicated field.
- The records do not prove whether the bytes were produced by one packaging checkout reused across children or by independent packaging that happened to use the same shared repo root.
- The records do not show a per-child checkout ownership or lease field for this patch path.

# Smallest Verification Commands
```bash
sha256sum \
  /tmp/ploke-mbe-campaign-p1-history-traversal-20260511-2-v2/node-e73f3c82afd7f4c3/workdir/BurntSushi/ripgrep/evals/pr-2209/fix.patch \
  /tmp/ploke-mbe-campaign-p1-history-traversal-20260511-2-v2/node-d60e06343e095ab0/workdir/BurntSushi/ripgrep/evals/pr-2209/fix.patch \
  /tmp/ploke-mbe-campaign-p1-history-traversal-20260511-2-v2/node-aa63876e81dde4f0/workdir/BurntSushi/ripgrep/evals/pr-2209/fix.patch

jq -r '.workspace_root' \
  /home/brasides/.ploke-eval/campaigns/p1-history-traversal-20260511-2/prototype1/nodes/node-{e73f3c82afd7f4c3,d60e06343e095ab0,aa63876e81dde4f0}/runner-request.json

gzip -cd /home/brasides/.ploke-eval/instances/prototype1/p1-history-traversal-20260511-2/treatments/branch-f8cef63febe9b993/instances/BurntSushi__ripgrep-2209/runs/run-1778503913183-structured-current-policy-369aee1d/record.json.gz | jq '.setup.repo_state, .phases.packaging'
```

# Next Exact Check
Read the patch-packaging wrapper that emitted `multi-swe-bench-submission.jsonl` and find the actual `git diff` invocation or cwd binding. That is the missing field that would distinguish "shared checkout" from "same repo root reused in separate packaging steps".
