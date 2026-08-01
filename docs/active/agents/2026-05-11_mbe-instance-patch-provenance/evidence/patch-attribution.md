# Patch Attribution Evidence

## Direct Answer

The persisted records support patch misattribution / stale-state export, not a clean one-to-one link between `PatchArtifact` evidence and the exported MBE `fix_patch`.

In this campaign, 23/51 treatment runs with submission artifacts had non-empty `fix_patch`, and 11 of those 23 had `PatchArtifact.applied = false`. Ten non-empty submissions had no proposal statuses at all. I also found 5 runs with `Content changed` markers in the summary/trace, plus 7 repeated patch-hash groups across different nodes.

That is enough to say the submission patch is often not attributable to applied proposal evidence alone. It does not prove every repeated patch is contamination, but it does show the exporter can emit non-empty benchmark patches from states that the patch artifact does not explain.

## Bounded Commands Used

- `ls -lah` / `wc -l` on campaign directories before opening files.
- `jq` on `campaign.json`, `prototype1/nodes/*/node.json`, `runner-result.json`, `agent-turn-summary.json`, and one-line `multi-swe-bench-submission.jsonl`.
- `find` to resolve the treatment run root for each sampled node by `branch_id`.
- `rg -n "Content changed"` on the named run roots only.
- A bounded `/tmp` helper script to aggregate 51 submission artifacts and summarize patch hashes, byte counts, proposal states, and content-changed markers.

## Aggregate Signals

- Total treatment runs with submission artifacts sampled: `51`
- Non-empty `fix_patch`: `23`
- Non-empty `fix_patch` with `PatchArtifact.applied = false`: `11`
- Non-empty `fix_patch` with no proposal statuses: `10`
- Runs with `Content changed` marker in summary/trace: `5`
- Repeated patch-hash groups across distinct nodes: `7`

Repeated hashes of note:

- `97a7770193133478b5c84d2a37987493e73b4f1d6ed2c9e4112ba2c1516e38ab`
  - `node-aa63876e81dde4f0`
  - `node-d60e06343e095ab0`
  - `node-e73f3c82afd7f4c3`
  - 4444 bytes, `crates/printer/src/util.rs`
- `d0f7e271f92b89a82e81adf689fa3f5dbda5d03b695348fe838546ea455aab60`
  - `node-28b9040eae0e1e82`
  - `node-657049136fa4a9b7`
  - 9067 bytes, `crates/printer/src/standard.rs` + `crates/printer/src/util.rs`
- `5d934be3307c9277a7291a9bbcb9edcaab4de0ca00b8ed58717ffc5fd771d88f`
  - `node-c7ee621e529f17a9`
  - `node-301d47d579077fb0`
  - `node-b48ba42fe5ce1c86`
  - 3172 bytes, `crates/printer/src/util.rs`

## Concrete Examples

- `node-e73f3c82afd7f4c3`
  - parent `node-a1b9e2b8ec1296c2`
  - generation `2`
  - treatment run root: `/home/brasides/.ploke-eval/instances/prototype1/p1-history-traversal-20260511-2/treatments/branch-f8cef63febe9b993/instances/BurntSushi__ripgrep-2209/runs/run-1778503913183-structured-current-policy-369aee1d`
  - `fix_patch`: `4444` bytes, hash `97a7770193133478b5c84d2a37987493e73b4f1d6ed2c9e4112ba2c1516e38ab`
  - modified file: `crates/printer/src/util.rs`
  - `PatchArtifact.applied = false`
  - `PatchArtifact.all_proposals_applied = false`
  - proposal statuses: `Failed`
  - expected file change: `crates/printer/src/util.rs`
  - `Content changed` hits: `4`

- `node-fc481e444bb4327b`
  - parent `node-58dade5cbc266759`
  - generation `2`
  - treatment run root: `/home/brasides/.ploke-eval/instances/prototype1/p1-history-traversal-20260511-2/treatments/branch-070ccf60f5467cce/instances/BurntSushi__ripgrep-2209/runs/run-1778505862674-structured-current-policy-4ecb95e4`
  - `fix_patch`: `681` bytes, hash `68ad4ba70aab4f6f054a3e58e9f9447971f5b748b94aecd67c6c29b7bb1d5b94`
  - modified file: `crates/printer/src/util.rs`
  - `PatchArtifact.applied = false`
  - `PatchArtifact.all_proposals_applied = false`
  - proposal statuses: `[]`
  - expected file change: `crates/printer/src/util.rs`
  - `Content changed` hits: `0`

- `node-28b9040eae0e1e82`
  - parent `node-bab64f5af0c6b158`
  - generation `4`
  - treatment run root: `/home/brasides/.ploke-eval/instances/prototype1/p1-history-traversal-20260511-2/treatments/branch-fb0714a91b3a85e9/instances/BurntSushi__ripgrep-2209/runs/run-1778517876168-structured-current-policy-b7788ed4`
  - `fix_patch`: `9067` bytes, hash `d0f7e271f92b89a82e81adf689fa3f5dbda5d03b695348fe838546ea455aab60`
  - modified files: `crates/printer/src/standard.rs`, `crates/printer/src/util.rs`
  - `PatchArtifact.applied = false`
  - `PatchArtifact.all_proposals_applied = false`
  - proposal statuses: `[]`
  - expected file change: `crates/printer/src/util.rs`
  - `Content changed` hits: `0`

- `node-cad5feff658ac025`
  - parent `node-df03c2dc4dc98f9d`
  - generation `3`
  - treatment run root: `/home/brasides/.ploke-eval/instances/prototype1/p1-history-traversal-20260511-2/treatments/branch-a10725afba34e944/instances/BurntSushi__ripgrep-2209/runs/run-1778508694333-structured-current-policy-b0ffab46`
  - `fix_patch`: `2747` bytes, hash `5bb1d22ef8e2ae64a334e724034db7cf7fdb426ca86db28dedce86e22f6ec048`
  - modified file: `crates/printer/src/util.rs`
  - `PatchArtifact.applied = true`
  - `PatchArtifact.all_proposals_applied = false`
  - proposal statuses: `Failed,Applied,Applied`
  - expected file change: `crates/printer/src/util.rs`
  - `Content changed` hits: `7`

## Disproof Evidence Sought

I looked for the opposite pattern: a clean alignment where non-empty `fix_patch` only appears when `PatchArtifact.applied` is true and the proposal list explains the edit. That did not hold.

I also looked for unique patch hashes per node. That did not hold either: the same exact `fix_patch` bytes recur across multiple nodes and branches.

I did not find a strong disproof of stale or misattributed patch export in this sample set.

## Caveats

- Repeated patch hashes are suggestive, not proof of contamination. A sibling fanout can legitimately rediscover the same edit.
- `PatchArtifact` is summary evidence, not the benchmark diff itself. A mismatch proves attribution failure, but not necessarily the exact mechanism.
- This pass only sampled the treatment runs that had submission artifacts available under the campaign. It did not inspect raw logs wholesale.

## Next Exact Question

For the repeated-hash groups, did those nodes share a mutable checkout/worktree during child self-validation, or were the identical `fix_patch` bytes independently produced from separate clean checkouts?

