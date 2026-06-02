Findings:
- Current authoritative agent-turn files are under each run output directory:
  `<prepared.output_dir>/runs/run-<utc_millis>-<run_arm.id>-<uuid8>/agent-turn-trace.json`
  and `agent-turn-summary.json`.
- Prefer registered artifact paths when available:
  `RunRegistration.artifacts.turn_trace` and `turn_summary`.
- Fallback should be direct files under a concrete `runs/run-*` directory.
- Recursive discovery from the broader run root is too broad and can enter
  worktrees, target directories, caches, logs, or unrelated files.
- Legacy top-level instance-root agent-turn files are explicitly
  non-authoritative and should not drive current loader behavior.

Changed files: none.
