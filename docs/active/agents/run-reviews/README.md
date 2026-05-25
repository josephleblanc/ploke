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
- [`2026-05-25-p1-gemini35-flash-direct-fresh-20260524-190632-eval.md`](2026-05-25-p1-gemini35-flash-direct-fresh-20260524-190632-eval.md)
  Eval-step review for the direct Google rerun after env-cwd repair, covering
  clean route/preflight setup, non-empty patch output, target-test success,
  rustfmt failure, weak cargo-check resolution, and protocol follow-up.
- [`2026-05-25-p1-gemini35-flash-direct-fresh-20260524-193515-eval.md`](2026-05-25-p1-gemini35-flash-direct-fresh-20260524-193515-eval.md)
  Eval and protocol review for the next direct Google baseline, covering
  completed baseline protocol after the reasoning-default fix, protocol
  over-crediting, stale content/hash tool failures, suspect regression-test
  repair, absent fmt, and weak final validation scope.
- [`2026-05-25-p1-gemini35-flash-direct-fresh-20260524-193515-child-attempt-1.md`](2026-05-25-p1-gemini35-flash-direct-fresh-20260524-193515-child-attempt-1.md)
  First child-plan broad-harness attempt review for `node-b19077fc35c373b5`,
  covering the applied `ploke-core` canonicalization-cache patch, tool-failure
  recovery, staged/apply lifecycle, validation drift, and adjudication
  candidates.
- [`2026-05-25-p1-gemini35-flash-direct-fresh-20260524-193515-child-attempt-2.md`](2026-05-25-p1-gemini35-flash-direct-fresh-20260524-193515-child-attempt-2.md)
  Second child-plan broad-harness attempt review for `node-b19077fc35c373b5`,
  covering the applied `syn_parser` performance patch, stale same-file
  content/hash recovery, failed final validation, and adjudication candidates.
- [`2026-05-25-p1-gemini35-flash-direct-fresh-20260524-193515-child-attempt-3.md`](2026-05-25-p1-gemini35-flash-direct-fresh-20260524-193515-child-attempt-3.md)
  Third child-plan broad-harness attempt review for `node-b19077fc35c373b5`,
  covering the committed `ploke-transform` batching patch, timeout-after-stage
  lifecycle gap, missing post-final validation, and adjudication candidates.
- [`2026-05-25-p1-gemini35-flash-direct-fresh-20260525-035030-eval.md`](2026-05-25-p1-gemini35-flash-direct-fresh-20260525-035030-eval.md)
  Eval and protocol review for the fresh direct Google baseline, covering the
  non-empty replacement-boundary patch, protocol completion to child planning,
  validation-audit improvements and blind spots, recoverable content/hash
  failures, weak test evidence, final cargo-scope caveats, and protocol
  adjudication candidates.
- [`2026-05-25-p1-gemini35-flash-direct-fresh-20260525-035030-child-attempt-1.md`](2026-05-25-p1-gemini35-flash-direct-fresh-20260525-035030-child-attempt-1.md)
  First child-plan broad-harness attempt review for
  `node-dfbca03c896b03ae`, covering the committed seven-file performance/setup
  patch, cargo-failure feedback and retry chain, stale semantic-tool recovery,
  weak focused-`xtask` validation, and the changed-path accounting mismatch
  between terminal/result artifacts and the committed patch.
- [`2026-05-25-p1-gemini35-flash-direct-fresh-20260525-035030-child-attempt-2.md`](2026-05-25-p1-gemini35-flash-direct-fresh-20260525-035030-child-attempt-2.md)
  Second child-plan broad-harness attempt review for
  `node-dfbca03c896b03ae`, covering the r2 `ploke-ty-mcp` startup-lock patch,
  recoverable edit-tool failures, model-visible focused cargo validation,
  missing request-contract validation, and the unmeasured health-check skip
  tradeoff.
- [`2026-05-25-p1-gemini35-flash-direct-fresh-20260525-035030-child-attempt-3.md`](2026-05-25-p1-gemini35-flash-direct-fresh-20260525-035030-child-attempt-3.md)
  Third child-plan broad-harness attempt review for
  `node-dfbca03c896b03ae`, covering the committed r3 `ploke-db` query-ordering
  patch, stale same-file edit recovery, model-visible cargo failure and syntax
  repair, timeout terminal state, missing post-repair validation, and request
  contract validation gap.
