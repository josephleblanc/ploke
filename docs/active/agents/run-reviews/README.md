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
