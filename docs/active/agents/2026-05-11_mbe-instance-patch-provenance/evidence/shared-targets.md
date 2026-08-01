# Direct Answer
Supported for shared benchmark target reuse within a parent batch: the sampled BurntSushi children all point at the same benchmark repo root / repo_dir tuple and the same benchmark config values. Not supported for concurrent same-folder mutation: each child has its own `workspace_root`, and the persisted config caps `max_workers_run_instance` at `1`.

# Bounded Commands Used
- `ls -lah /home/brasides/.ploke-eval/campaigns/p1-history-traversal-20260511-2/prototype1`
- `find /home/brasides/.ploke-eval/campaigns/p1-history-traversal-20260511-2/prototype1 -maxdepth 2 -type f ...`
- `rg -n 'BurntSushi__ripgrep-2209|repo_dir|repo_root|workdir|output_dir|base_sha|workspace_root|parent_node_id|node_id|generation' ... | head -n 120 | cut -c 1-400`
- `jq` on sampled `node.json`, `runner-request.json`, `scheduler.json`, `run-profile.toml`, and compressed `record.json.gz` files
- `gzip -dc <record.json.gz> | jq ...` with width-capped output

# Sample Rows
- `node-00bd13689a1a4205` -> parent `node-deef8d456a167165`, gen `4`, `workspace_root=/home/brasides/.ploke-eval/campaigns/p1-history-traversal-20260511-2/prototype1/nodes/node-00bd13689a1a4205/worktree`
- `node-10406bef19377de1` -> parent `node-deef8d456a167165`, gen `4`, same batch as above, different `workspace_root`
- `node-144b736bf29f5243` -> parent `node-c7ee621e529f17a9`, gen `4`, `workspace_root` isolated to that node
- `node-3921c9725a638c09` -> parent `node-c7ee621e529f17a9`, gen `4`, same batch as above, different `workspace_root`

Benchmark config sampled from three child worktrees was identical:
- `workdir=/home/brasides/.ploke-eval/msb/workdir/clap-5075`
- `repo_dir=/home/brasides/.ploke-eval/msb/repos/clap-5075`
- `output_dir=/home/brasides/.ploke-eval/msb/output/clap-5075`
- `max_workers_run_instance=1`

Run records for BurntSushi__ripgrep-2209 carried the same benchmark root:
- `repo_root=/home/brasides/.ploke-eval/repos/BurntSushi/ripgrep`
- `base_sha=4dc6c73c5a9203c5a8a89ce2161feca542329812`

# Grouping Evidence
- `run-profile.toml` says target instance `BurntSushi__ripgrep-2209`, `children = { min = 6, max = 6 }`, `schedule = "full-batch"`.
- `scheduler.json` groups the root into parent-batch planning for the same instance.
- `jq` over `node.json` files found two 6-child sibling groups for BurntSushi at generation 4: one under `node-deef8d456a167165`, one under `node-c7ee621e529f17a9`.
- In the sampled sibling groups, each child had a distinct `target_relpath` and distinct `workspace_root`, but the benchmark config file content stayed identical across worktrees.

# Disproof Evidence Sought
- Looked for a child-specific `repo_dir` or `repo_root` split across siblings. None found in the sampled records.
- Looked for `max_workers_run_instance > 1` or any record implying simultaneous same-instance worker slots. None found.
- Looked for shared child `workspace_root` values. None found; every sampled child had its own worktree path.
- I did not find a BurntSushi-specific `.tmp` filename; the per-child `.tmp` config found in these worktrees is the shared benchmark harness config.

# Caveats
- The persisted record evidence supports shared benchmark root/config, but it does not by itself prove that two child processes were active at the exact same instant.
- The strongest anti-concurrency signal is the explicit worker cap of `1`, not a lock record.
- The config file name is `msb-clap-5075-config.json` even inside BurntSushi child worktrees; I treated its fields as the benchmark harness projection, not as a BurntSushi-specific artifact name.

# Next Exact Question
Which component owns checkout creation and cleanup for the BurntSushi batch, and does its record stream expose per-run lock or lease evidence for `repo_dir`?
