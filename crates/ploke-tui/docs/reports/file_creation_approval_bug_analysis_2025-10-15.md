# File Creation Approval Bug: Root Cause Analysis

Scope
- Crate: `crates/ploke-tui`
- Symptom: After staging a Create File proposal and approving it in the Approvals overlay, a SysInfo message claims the edit/creation was applied, but the file is not created on disk.

Summary (Root Cause)
- The creation-approval handler `rag::editing::approve_creations` emits a success SysInfo and marks the proposal `Applied` unconditionally, even when the underlying I/O operation fails. This yields a false-positive success message while no file is actually created.

Key Findings
- Approval handler exists for file creations and calls the I/O layer correctly:
  - `crates/ploke-tui/src/rag/editing.rs` exposes `approve_creations(...)` and calls `state.io_handle.create_file(req).await` for each staged request.
- However, regardless of per-file results, the function always sets the proposal status to `Applied` and emits a success SysInfo:
  - Code excerpt (behavior):
    - Iterates over `proposal.creates`, collecting successes and errors into `results_json`, tracking a local `applied` counter.
    - Builds a content JSON with `{ ok: applied > 0, applied, results }`.
    - Unconditionally sets `proposal.status = EditProposalStatus::Applied;`.
    - Emits `ToolCallCompleted` and a SysInfo: `"Applied file creations for request_id ..."`.
  - Effect: If `applied == 0` (i.e., all create operations failed), the UI still surfaces a success-like message and the proposal is marked `Applied`, despite no file being created.
- Conditions that cause `IoManagerHandle::create_file` to fail legitimately:
  - Absolute path requirement enforced at I/O layer (`ploke-io/src/create.rs`): relative paths are rejected.
  - Path outside configured roots (if roots are configured) or violating symlink policy.
  - Non-`.rs` extension (restricted to Rust files).
  - Parent directory missing when `create_parents == false` (default); this returns NotFound.
  - Target already exists while `on_exists == error` (default).
- UI overlay event routing is implemented and distinguishes Edit vs Create:
  - `crates/ploke-tui/src/app/mod.rs` builds a unified items list and, on Enter, maps `ProposalKind::Edit` to `ApproveEdits` and `ProposalKind::Create` to `ApproveCreations`.
  - `crates/ploke-tui/src/app/view/components/approvals.rs` correctly renders both kinds with tags `[E]` and `[C]` and uses a sorted-by-id unified view. The key handler also sorts by id to match index selection. This mapping appears correct.
- Staging pipeline for Create File is implemented and uses crate root when available, otherwise falls back to `current_dir()`:
  - `crates/ploke-tui/src/tools/create_file.rs` resolves relative paths against `state.system.crate_focus` (if set), otherwise `std::env::current_dir()` and enforces `.rs` extension early. It stages a `CreateProposal` in `state.create_proposals` and optionally auto-approves based on config.
  - This fallback can direct a relative path to the workspace root instead of the crate root if `crate_focus` isn’t set; if parents don’t exist and `create_parents` is false, the I/O call fails.

Why this matches the observed behavior
- The user sees “Applied …” SysInfo after approval (success message printed unconditionally), but the file is not created (I/O returned an error, e.g., parent doesn’t exist or path invalid). Because status/message are set to success regardless of results, the UI masks the failure.

Secondary observations
- `approve_edits` exhibits a similar unconditional `Applied` marking and success SysInfo even when some/all edit writes fail (it sets `Applied` on Ok(response) without verifying `applied > 0`). Less severe in practice, but inconsistent with evidence-based reporting.
- Overlay key handling and item-to-command mapping are consistent with the rendered list and unlikely to be the culprit.

Evidence Pointers
- Creation approval handler:
  - `crates/ploke-tui/src/rag/editing.rs` (functions: `approve_creations`, `approve_edits`).
- Create-file tool staging and preview:
  - `crates/ploke-tui/src/tools/create_file.rs`.
- Overlay selection and mapping to `ApproveCreations` vs `ApproveEdits`:
  - `crates/ploke-tui/src/app/view/components/approvals.rs` (unified list rendering)
  - `crates/ploke-tui/src/app/mod.rs` (Enter key: sends `StateCommand::ApproveCreations` for `[C]`).
- I/O layer enforcing absolute paths, roots, and atomic create:
  - `crates/ploke-io/src/handle.rs` (`create_file` API)
  - `crates/ploke-io/src/actor.rs` (handles `IoRequest::CreateFile`)
  - `crates/ploke-io/src/create.rs` (absolute path, `.rs` restriction, parent handling, atomic write).

Recommendations (no code changes yet)
- Correct the success path in `approve_creations`:
  - If `applied == 0`, set status to `Failed(err_summary)` and emit `ToolCallFailed` (or at minimum emit a SysInfo that reflects zero applied and show error reasons). Only emit “Applied …” when `applied > 0`.
  - Include a brief per-file result summary in the SysInfo (N applied, M failed) and the resolved absolute path. This aligns with “Evidence-based changes”.
- Align `approve_edits` with the same evidence discipline (avoid unconditional success when `applied == 0`).
- Consider defaulting `create_parents = true` for CreateFile unless explicitly set to `false` to reduce typical failures for new module paths, or ensure the model/tooling knows to set it.
- Add tests:
  - Unit: exercise `approve_creations` with a temp dir and a missing parent when `create_parents=false` → expect failure status and no success SysInfo.
  - E2E (gated): stage a creation under a temp workspace, approve, assert the file exists and DB rescan updates indexes; record artifacts under `target/test-output/...`.
- Optional: When `crate_focus` is unset, warn in SysInfo that relative paths resolve against `current_dir()` and may not match expected crate layout.

Conclusion
- The proximate cause of “file not created but success shown” is the approval handler reporting success unconditionally, masking I/O failures. Fixing the status/message gating and surfacing per-file errors will make failures visible and actionable; in parallel, ensure path/parent handling is configured so the intended create operations succeed under normal workflows.
