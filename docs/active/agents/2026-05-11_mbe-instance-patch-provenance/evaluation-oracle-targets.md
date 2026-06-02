# Scope

Discovery/report only for the typed-persistence inventory family `evaluation-oracle-targets`, focused on how the Multi-SWE-bench submission artifact is produced and how its provenance can be traced without reading run logs or JSONL streams wholesale.

Primary question answered: `multi-swe-bench-submission.jsonl` is written from a typed `MultiSweBenchSubmissionRecord`; its `fix_patch` is collected by running `git diff --no-ext-diff --binary <prepared.base_sha> --` in the prepared run repo when `base_sha` exists, otherwise `git diff --no-ext-diff --binary HEAD --`.

# Inventory Rows Used

- `eval.instance.registry`
  - Accepted row: `docs/active/plans/self-improvement-loop/typed-persistence-spine/inventory.jsonl:48`
  - Survey row: `docs/active/plans/self-improvement-loop/typed-persistence-spine/reports/2026-05-10-evaluation-oracle-targets.survey-g.jsonl:7`
  - Typed records: `TargetRegistry`, `RegistryEntry`, `RegistrySource`, `RegistryEntryState`
  - Joins: `instance_id`, `dataset_label`, `org`, `repo`, `number`, `base_sha`

- `eval.run_record.metadata_setup`
  - Accepted row: `docs/active/plans/self-improvement-loop/typed-persistence-spine/inventory.jsonl:50`
  - Survey row: `docs/active/plans/self-improvement-loop/typed-persistence-spine/reports/2026-05-10-evaluation-oracle-targets.survey-g.jsonl:9`
  - Typed records: `RunMetadata`, `BenchmarkMetadata`, `SetupPhase`, `IndexedCrateSummary`, `ParseFailureRecord`
  - Joins: `manifest_id`, `benchmark.instance_id`, `repo_root`, `base_sha`, `db_timestamp_micros`

- `eval.run_record.patch_packaging`
  - Accepted row: `docs/active/plans/self-improvement-loop/typed-persistence-spine/inventory.jsonl:51`
  - Survey row: `docs/active/plans/self-improvement-loop/typed-persistence-spine/reports/2026-05-10-evaluation-oracle-targets.survey-g.jsonl:10`
  - Typed records: `PatchPhase`, `PatchArtifact`, `ProposalSnapshotRecord`, `ExpectedFileChangeRecord`, `SubmissionArtifactState`, `PackagingPhase`
  - Joins: `request_id`, `call_id`, `files`, patch proposal ids, `msb_submission_path`

# Source Code Ranges Worth Reading

- `crates/ploke-eval/src/spec.rs:68-107`
  - `PrepareSingleRunRequest`, `RunSource::MultiSweBench`, `MultiSweBenchSource`, and `PreparedSingleRun` carry `task_id`, `repo_root`, `base_sha`, issue, output directory, and benchmark source identity.

- `crates/ploke-eval/src/target_registry.rs:42-84`
  - Typed target registry shape, including `RegistrySource { dataset_path, org, repo, number, base_sha }`.

- `crates/ploke-eval/src/target_registry.rs:470-483`
  - Dataset record is projected into `RegistrySource`; `base_sha` comes from `record.base.sha`.

- `crates/ploke-eval/src/record.rs:575-767`
  - `RunMetadata::from_manifest` copies `manifest.task_id`, `manifest.repo_root`, `manifest.base_sha`, issue, and budget into the run record.

- `crates/ploke-eval/src/record.rs:1359-1438`
  - `PatchPhase`, `SubmissionArtifactState`, and `PackagingPhase`; `PackagingPhase.msb_submission_path` records the submission artifact path.

- `crates/ploke-eval/src/runner.rs:164-184`
  - Run registration pre-populates `RunArtifactRefs.msb_submission` for Multi-SWE-bench treatment runs.

- `crates/ploke-eval/src/runner.rs:1015-1068`
  - `maybe_build_msb_submission_record`, `write_msb_submission_artifact`, and `collect_submission_fix_patch`; this is the core submission writer path.

- `crates/ploke-eval/src/runner.rs:1688-1716` and `crates/ploke-eval/src/runner.rs:2128-2150`
  - Both runner paths reset/checkout to `prepared.base_sha` before setup/agent work, then persist `RepoStateArtifact`.

- `crates/ploke-eval/src/runner.rs:1938-1982` and `crates/ploke-eval/src/runner.rs:2448-2486`
  - Packaging phase calls `write_msb_submission_artifact`, updates lifecycle submission status, and writes `PackagingPhase`.

- `crates/ploke-eval/src/runner.rs:2611-2668`, `crates/ploke-eval/src/runner.rs:2710-2729`, and `crates/ploke-eval/src/runner.rs:2763-2796`
  - Batch runner initializes aggregate `multi-swe-bench-submission.jsonl`, appends per-run submission blobs, and returns `BatchRunArtifactPaths.msb_submission` only when the aggregate file is nonempty.

- `crates/ploke-eval/src/runner.rs:3698-3718`
  - `write_jsonl_line` writes one serialized typed record plus newline with `fs::write`; `append_jsonl_blob` starts immediately after for aggregate appends.

- `crates/ploke-eval/src/runner.rs:4670-4848`
  - Tests demonstrate the submission file path, JSONL shape, and that `fix_patch` is the actual repo diff, including the partial-apply case where expected-but-unchanged files are not invented.

- `crates/ploke-eval/src/cli.rs:4448-4510`
  - Campaign export command writes campaign-level `multi-swe-bench-submission.jsonl` from collected typed records.

- `crates/ploke-eval/src/cli.rs:5260-5335`
  - Campaign export reader prefers `row.artifacts.msb_submission`, otherwise selects `PreferTreatmentWithSubmission` and reads that run's `multi-swe-bench-submission.jsonl`.

- `crates/ploke-eval/src/closure.rs:155-197` and `crates/ploke-eval/src/closure.rs:575-595`
  - `ClosureInstanceRow` and `ClosureArtifactRefs` carry `msb_submission`; closure recompute copies it from the preferred treatment registration.

- `crates/ploke-eval/src/run_registry.rs:166-202` and `crates/ploke-eval/src/run_registry.rs:285-306`
  - Preferred run selection returns the first matching registration; `PreferTreatmentWithSubmission` requires a treatment run with `NonemptyPatch` or `EmptyPatch`.

- `crates/ploke-eval/src/inner/core.rs:58-122`
  - `RunIntent` and `FrozenRunSpec` persist run `task_id`, `repo_root`, `base_sha`, storage roots, batch/campaign ids, arm id, and role.

- `crates/ploke-eval/src/inner/registry.rs:45-68`, `crates/ploke-eval/src/inner/registry.rs:119-130`, and `crates/ploke-eval/src/inner/registry.rs:380-405`
  - `RunRegistration` and `RunArtifactRefs` persist the run manifest path, run root, record path, and optional `msb_submission` path.

# Persisted Records / Artifact Paths Involved

- Registry: `~/.ploke-eval/registries/multi-swe-bench-rust.json`
  - Typed as `TargetRegistry`.
  - Identifies benchmark target and base SHA through `RegistryEntry.source`.

- Per-instance run manifest: `~/.ploke-eval/instances/<instance>/run.json`
  - Typed as `PreparedSingleRun`.
  - Carries `task_id`, `repo_root`, `base_sha`, `issue`, output directory, and `RunSource::MultiSweBench`.

- Run registration: registry-managed per-run JSON under the run registry root.
  - Typed as `RunRegistration`.
  - Carries `RunIntent`, `FrozenRunSpec`, lifecycle/submission status, and `RunArtifactRefs`.

- Run record: `~/.ploke-eval/instances/<instance>/runs/run-*/record.json.gz`
  - Typed as `RunRecord`.
  - Carries `RunMetadata.benchmark`, `SetupPhase.repo_state`, `PatchPhase.patch_artifact`, and `PackagingPhase`.

- Repo state artifact: `~/.ploke-eval/instances/<instance>/runs/run-*/repo-state.json`
  - Typed as `RepoStateArtifact`.
  - Records `repo_root`, `requested_base_sha`, checked-out head, and porcelain status after checkout.

- Per-run submission artifact: `~/.ploke-eval/instances/<instance>/runs/run-*/multi-swe-bench-submission.jsonl`
  - Typed line shape: `MultiSweBenchSubmissionRecord { org, repo, number, fix_patch }`.
  - Written only for `RunArmRole::Treatment` with `RunSource::MultiSweBench`.

- Batch aggregate submission: `<prepared.output_dir>/multi-swe-bench-submission.jsonl`
  - Built by appending per-run submission blobs.
  - Convenience aggregate, not the source per-instance patch record.

- Campaign aggregate submission: `~/.ploke-eval/campaigns/<campaign>/multi-swe-bench-submission.jsonl`
  - Built by reading completed closure rows and serializing typed `MultiSweBenchSubmissionRecord` values.

# Provenance Chain Contribution

1. Target identity starts in the registry row: `TargetRegistry.entries[].source` gives `org`, `repo`, `number`, and benchmark `base_sha`.

2. Run preparation carries that identity into `PreparedSingleRun`: `task_id`, `repo_root`, `base_sha`, and `RunSource::MultiSweBench` identify the instance and target repository for execution.

3. The run is reset to the benchmark base before execution: runner paths call `checkout_repo_to_base(&prepared.repo_root, prepared.base_sha.as_deref())`, which runs `git reset --hard` and then `git checkout --detach <base_sha>` when a base is present.

4. Agent/self-validation edits happen in the mutable checkout. The submission writer does not read a patch proposal record or History block as source truth; it reads the repository state by shelling out to `git diff`.

5. `collect_submission_fix_patch` diffs the current mutable checkout against `prepared.base_sha` when present. Therefore the exported patch appears to be a benchmark-base diff, measured from the current checkout, not a parent-delta diff and not a direct serialization of a proposal/History artifact.

6. `write_msb_submission_artifact` packages that diff into `MultiSweBenchSubmissionRecord` and writes the per-run `multi-swe-bench-submission.jsonl` with one JSONL line.

7. `PackagingPhase` records `SubmissionArtifactState` plus `msb_submission_path`; `RunRegistration.artifacts.msb_submission` and closure state make that path discoverable for campaign export.

# Smallest Verification Commands

- Inventory rows:
  - `rg -n 'eval\\.instance\\.registry|eval\\.run_record\\.metadata_setup|eval\\.run_record\\.patch_packaging' docs/active/plans/self-improvement-loop/typed-persistence-spine/inventory.jsonl | cut -c 1-1400`

- Accepted survey rows:
  - `rg -n 'eval\\.instance\\.registry|eval\\.run_record\\.metadata_setup|eval\\.run_record\\.patch_packaging' docs/active/plans/self-improvement-loop/typed-persistence-spine/reports/2026-05-10-evaluation-oracle-targets.survey-g.jsonl | cut -c 1-1400`

- Submission writer:
  - `rg -n 'maybe_build_msb_submission_record|write_msb_submission_artifact|collect_submission_fix_patch|write_jsonl_line|append_jsonl_blob' crates/ploke-eval/src/runner.rs | cut -c 1-500`

- Exact source reads:
  - `sed -n '1015,1068p' crates/ploke-eval/src/runner.rs`
  - `sed -n '1938,1982p' crates/ploke-eval/src/runner.rs`
  - `sed -n '2448,2486p' crates/ploke-eval/src/runner.rs`
  - `sed -n '4670,4848p' crates/ploke-eval/src/runner.rs`

- Focused tests, with bounded output:
  - `cargo test -p ploke-eval write_msb_submission_artifact_writes_treatment_submission_into_run_dir 2>&1 | tail -n 40`
  - `cargo test -p ploke-eval write_msb_submission_artifact_uses_repo_diff_only_for_partial_apply_state 2>&1 | tail -n 40`
  - `cargo test -p ploke-eval collect_submission_fix_patch_exports_git_diff_and_jsonl_shape 2>&1 | tail -n 40`

# Avoid Reading Wholesale

- Do not read `transition-journal.jsonl`, `history/blocks/segment-*.jsonl`, observation JSONL, `llm-full-responses.jsonl`, or submission JSONL files wholesale for this question.

- Do not read full `record.json.gz` files unless the exact instance/run has been chosen; prefer metadata first: path, mtime, size, and `RunRegistration.artifacts.msb_submission`.

- Do not broad-search raw run roots for patch text. The source-level chain already shows the submission patch comes from `git diff` against `prepared.base_sha`; raw artifacts should only be used to verify a named run.

- If a concrete run must be inspected, first inspect the registration and artifact metadata, then read at most the one named `multi-swe-bench-submission.jsonl` record with a width cap, for example:
  - `tail -n 1 <run-dir>/multi-swe-bench-submission.jsonl | cut -c 1-400`

# Open Questions

- Which concrete Prototype 1 child run produced the MBE artifact in question? This report identifies the source-level packaging chain, but not a specific `run_id` or child node.

- Was the child checkout still exactly rooted at the benchmark base immediately before packaging? Source code intends this via checkout at run start, but a named run should verify `repo-state.json` and registration metadata before relying on it.

- Is there a typed join from Prototype 1 child self-validation evidence to the MBE run registration, or is the only hard join currently the mutable checkout diff plus `msb_submission_path`? The inspected ranges suggest the latter for submission packaging.

- Should future provenance require `MultiSweBenchSubmissionRecord` or `PackagingPhase` to carry the diff base SHA explicitly? Today the base is recoverable through `PreparedSingleRun`, `RunMetadata.benchmark.base_sha`, `FrozenRunSpec.base_sha`, and `RepoStateArtifact.requested_base_sha`, not from the submission line itself.
