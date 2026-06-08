# Changelog

## 2026-06-07

- Traced the failed long-ish Prototype 1 run to successor artifact preparation,
  not provider behavior, protocol adjudication, child admissibility, or
  child-observe timing.
- Added a source trace from historical artifact selection through successor
  installation and the missing broad-harness workspace error.
- Added regression coverage for selecting the active parent from historical
  traversal candidates.
- Added regression coverage for installing a committed broad-harness successor
  artifact after its provisional edit-harness checkout is gone.
- Added a successor-install-only fallback for missing campaign-owned
  broad-harness workspaces that preserves strict child artifact validation.
- Updated the repo-local `prototype1-loop-run-status` skill with successor
  failure checks and installed the read-only run summary helper.
- Abandoned `p1-handofffix-5g1x2-a2-20260607-190702` for loop progress after
  its baseline eval persisted a failed embedding preflight on the default
  `mistralai/codestral-embed-2505` route.
- Fixed the source path that dropped `prototype1-setup` embedding model/provider
  overrides before baseline agent-batch execution.
- Added focused coverage for setup manifest persistence, batch-to-single
  embedding forwarding, setup CLI parsing, and eval-set identity separation.
