# Run Reviews

Run-scoped review and diagnostic reports for Prototype 1 campaigns. These files
are evidence about named runs; do not generalize them to current implementation
state without checking newer code, History records, and run artifacts.

- [`2026-05-18-tool-failures.md`](2026-05-18-tool-failures.md)
  Review of tool-failure behavior observed in a specific run context.
- [`2026-05-19-p1-smoke-broad-harness-1x3-20260519-1.md`](2026-05-19-p1-smoke-broad-harness-1x3-20260519-1.md)
  Initial review for the `p1-smoke-broad-harness-1x3-20260519-1` campaign.
- [`2026-05-19-p1-smoke-broad-harness-1x3-20260519-1-deep-review.md`](2026-05-19-p1-smoke-broad-harness-1x3-20260519-1-deep-review.md)
  Deep review companion for the same campaign.
- [`2026-05-19-p1-smoke-broad-harness-1x3-20260519-1/`](2026-05-19-p1-smoke-broad-harness-1x3-20260519-1/)
  Run-specific trace inventory, trace review, replay taxonomy, and RF-04
  implementation design notes.
- [`2026-05-23-p1-google-live-multigen-2g3x3-20260523-192017.md`](2026-05-23-p1-google-live-multigen-2g3x3-20260523-192017.md)
  Run review for the first Google live multigen setup, covering eval output,
  empty patch packaging, missing MBE oracle evidence, LLM token stats, and
  trace-level tool-result quality versus transport success.
- [`2026-05-23-p1-gemini35-flash-multigen-2g3x3-20260523-223658.md`](2026-05-23-p1-gemini35-flash-multigen-2g3x3-20260523-223658.md)
  Step review for the Gemini 3.5 Flash baseline eval, covering non-empty patch
  output, focused test success, rustfmt failure, token-heavy debugging, and
  trace/tool-quality issues before protocol adjudication.
- [`2026-05-24-p1-gemini35-flash-multigen-2g3x3-20260523-223658-protocol-followup.md`](2026-05-24-p1-gemini35-flash-multigen-2g3x3-20260523-223658-protocol-followup.md)
  Follow-up review after the baseline protocol step failed, separating the
  usable eval submission and cargo-learning trace from the current protocol
  segmentation JSON trailing-characters blocker.
- [`2026-05-24-p1-gemini35-flash-direct-profile-20260524-151353-eval.md`](2026-05-24-p1-gemini35-flash-direct-profile-20260524-151353-eval.md)
  Focused eval-step review for the direct Google Gemini 3.5 Flash run, covering
  thought-signature preservation, tool-call ledger parity, patch/submission
  output, cargo visibility, weak final validation, and adjudication fields.
- [`2026-05-24-p1-gemini35-flash-direct-fresh-20260524-163447-eval.md`](2026-05-24-p1-gemini35-flash-direct-fresh-20260524-163447-eval.md)
  Eval-step review for the fresh direct Google Gemini 3.5 Flash run, covering
  the non-empty but behaviorally incomplete patch, hidden apply failure,
  cargo-scope mismatch, empty successful reads, and adjudication candidates.
- [`2026-05-25-p1-gemini35-flash-direct-15g2x3-20260525-035000-burntsushi-ripgrep-2209-run-1779706500140-eval.md`](2026-05-25-p1-gemini35-flash-direct-15g2x3-20260525-035000-burntsushi-ripgrep-2209-run-1779706500140-eval.md)
  Baseline eval review for `BurntSushi__ripgrep-2209`, reconstructing useful
  context discovery, stale same-file edit recovery, final package-intended
  cargo check/test visibility, patch export, benchmark usefulness, and protocol
  completion blind spots.
- [`2026-05-25-p1-gemini35-flash-direct-15g2x3-20260525-035000-burntsushi-ripgrep-2209-run-1779706500140-protocol-followup.md`](2026-05-25-p1-gemini35-flash-direct-15g2x3-20260525-035000-burntsushi-ripgrep-2209-run-1779706500140-protocol-followup.md)
  Follow-up review after protocol completion for the same run, reconciling the
  stale missing-protocol finding with full protocol artifacts, malformed
  adjudication JSON repair visibility, same-file edit failure credit, cargo
  scope visibility, and benchmark-usefulness limits.
- [`2026-05-25-p1-gemini35-flash-direct-15g2x3-isolated-20260525-113746-burntsushi-ripgrep-2209-run-1779709154252-eval.md`](2026-05-25-p1-gemini35-flash-direct-15g2x3-isolated-20260525-113746-burntsushi-ripgrep-2209-run-1779709154252-eval.md)
  Isolated baseline eval review for `BurntSushi__ripgrep-2209`, separating
  non-empty patch export from an aborted agent turn, final red validation,
  stale same-file context failures, protocol accounting mismatch, and unsupported
  benchmark usefulness.
