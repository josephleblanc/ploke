# Claim
Concurrent child self-validation runs did share the same benchmark instance root/config and did overlap in time, but the persisted records do **not** show a shared mutable benchmark checkout. The stronger reading is that patch provenance was ambiguous or repeated across children, not that one mutable checkout was definitively shared.

# Evidence Supporting
- The sampled BurntSushi children point at the same benchmark repo root / `repo_dir` tuple and the same benchmark config values.
- The overlapping run windows show multiple child runs starting within seconds of each other while earlier runs were still active.
- Patch attribution evidence shows non-empty `fix_patch` values that are not cleanly explained by applied proposal evidence alone.
- Repeated patch hashes recur across distinct nodes and branches, which is consistent with sliced/repeated patch export.
- Some sampled runs show `Content changed` markers even when `PatchArtifact.applied = false`, reinforcing attribution drift.

# Evidence Against / Caveats
- Each sampled child has its own `workspace_root`, which argues against a single shared mutable checkout path.
- The persisted config caps `max_workers_run_instance` at `1`, which weakens the case for concurrent same-folder mutation.
- One evaluated baseline/treatment pair was serialized, so overlap was real but not universal.
- Repeated hashes can still arise from independent rediscovery of the same edit in sibling fanout.
- The records do not expose a per-run lock, lease, or checkout-ownership artifact for `repo_dir`.

# Current Confidence
Moderate on overlap plus attribution ambiguity. Low on the stronger claim that a mutable benchmark checkout was shared.

# Strongest Next Check
Trace checkout creation/cleanup for the BurntSushi batch and look for a per-run lock or lease record tied to `repo_dir` or the generated worktree.

# Operational Consequence
Treat repeated or sliced `fix_patch` output as a provenance problem until checkout ownership is explicit. Do not infer shared mutable checkout from overlap alone.
