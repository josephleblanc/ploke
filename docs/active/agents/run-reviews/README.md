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
- [`2026-05-25-p1-gemini35-flash-direct-15g2x3-20260525-035000-node-15006265e24b3b9b-run-1779711015972-treatment-eval.md`](2026-05-25-p1-gemini35-flash-direct-15g2x3-20260525-035000-node-15006265e24b3b9b-run-1779711015972-treatment-eval.md)
  Treatment child eval review for `node-15006265e24b3b9b`, reconstructing
  non-empty patch projection, cargo evidence versus the final claim, tool
  failure recovery, and the observe-child sidecar evidence-loss race.
- [`2026-05-25-p1-gemini35-flash-direct-15g2x3-isolated-20260525-113746-burntsushi-ripgrep-2209-run-1779709154252-eval.md`](2026-05-25-p1-gemini35-flash-direct-15g2x3-isolated-20260525-113746-burntsushi-ripgrep-2209-run-1779709154252-eval.md)
  Isolated baseline eval review for `BurntSushi__ripgrep-2209`, separating
  non-empty patch export from an aborted agent turn, final red validation,
  stale same-file context failures, protocol accounting mismatch, and unsupported
  benchmark usefulness.
- [`2026-05-25-p1-gemini35-flash-direct-15g2x3-clean-20260525-120225-burntsushi-ripgrep-2209-run-1779710592442-eval.md`](2026-05-25-p1-gemini35-flash-direct-15g2x3-clean-20260525-120225-burntsushi-ripgrep-2209-run-1779710592442-eval.md)
  Clean baseline eval review for `BurntSushi__ripgrep-2209`, reconstructing
  stale same-file edit recovery, validation-driven production repair, final
  cargo coverage, summary projection gaps, weak test proof, and patch usefulness
  against the issue/gold shape while separating later protocol artifacts.
- [`2026-05-25-p1-gemini35-flash-direct-15g2x3-20260525-035000-node-e803fd3e8d51dde6-run-1779711015527-structured-current-policy-1880bedc.md`](2026-05-25-p1-gemini35-flash-direct-15g2x3-20260525-035000-node-e803fd3e8d51dde6-run-1779711015527-structured-current-policy-1880bedc.md)
  Selected-child handoff review for `branch-821e452418987122`, reconstructing
  treatment patch export, cargo validation, metric-only `keep` selection,
  same-file edit recovery, and future LLM adjudication signals.
- [`2026-05-25-p1-gemini35-flash-direct-15g2x3-fixed-20260525-123824-baseline-run-1779712736739.md`](2026-05-25-p1-gemini35-flash-direct-15g2x3-fixed-20260525-123824-baseline-run-1779712736739.md)
  Baseline eval review for `node-57e8487f70ce4abc`, reconstructing non-empty
  patch/submission output, failed-but-recovered `apply_code_edit` behavior,
  cargo scope evidence, weak issue-shaped test proof, protocol incompleteness,
  and operator-status accounting gaps.
- [`2026-05-25-p1-gemini35-flash-direct-15g2x3-fixed-20260525-123824-baseline-protocol-run-1779712736739.md`](2026-05-25-p1-gemini35-flash-direct-15g2x3-fixed-20260525-123824-baseline-protocol-run-1779712736739.md)
  Protocol follow-up for the same baseline run, proving all three required
  protocol stages completed, classifying four repaired malformed adjudicator
  JSON outputs, preserving useful LLM-adjudication examples, and separating
  protocol completion from stale operator projections.
- [`2026-05-25-p1-gemini35-flash-direct-15g2x3-fixed-20260525-123824-node-57e8487f70ce4abc-r10-r11-broad-harness.md`](2026-05-25-p1-gemini35-flash-direct-15g2x3-fixed-20260525-123824-node-57e8487f70ce4abc-r10-r11-broad-harness.md)
  Broad-harness child review for `node-57e8487f70ce4abc-r10` and `r11`,
  reconstructing prompt, tool-output visibility, edit staging/application,
  cargo evidence, missing transition authority, and positive/negative
  adjudication examples while keeping both attempts separate.
- [`2026-05-25-p1-gemini35-flash-direct-15g2x3-fixed-20260525-123824-node-57e8487f70ce4abc-r12-broad-harness.md`](2026-05-25-p1-gemini35-flash-direct-15g2x3-fixed-20260525-123824-node-57e8487f70ce4abc-r12-broad-harness.md)
  Broad-harness child review for `node-57e8487f70ce4abc-r12`, reconstructing
  the prompt, model-visible tool lifecycle, applied `ploke-protocol` edit,
  focused cargo evidence, missing ploke-eval validation, transition-authority
  gap, and adjudication examples.
- [`2026-05-25-p1-gemini35-flash-direct-15g2x3-20260525-140904-baseline-eval-protocol-run-1779743381178.md`](2026-05-25-p1-gemini35-flash-direct-15g2x3-20260525-140904-baseline-eval-protocol-run-1779743381178.md)
  Combined baseline eval and protocol review for `BurntSushi__ripgrep-2209`,
  covering non-empty patch/submission output, model-visible cargo results,
  final cargo-scope mismatch, completed protocol artifacts, repaired malformed
  adjudicator JSON, and child-plan record gaps.
- [`2026-06-01-p1-gemini35-flash-direct-15g2x3-par2-20260601-173956-baseline-eval-protocol.md`](2026-06-01-p1-gemini35-flash-direct-15g2x3-par2-20260601-173956-baseline-eval-protocol.md)
  Baseline eval and protocol review for `BurntSushi__ripgrep-2209`, covering
  applied patch/submission output, successful issue-linked tests, empty
  successful read defects, formatting/doc-comment patch-quality gaps, protocol
  edit-state projection drift, and current child-plan readiness.
- [`2026-06-01-p1-gemini35-flash-direct-15g2x3-par2-20260601-173956-node-552c19a55f53dbe6-broad-harness.md`](2026-06-01-p1-gemini35-flash-direct-15g2x3-par2-20260601-173956-node-552c19a55f53dbe6-broad-harness.md)
  Broad-harness child review for parent slot `node-552c19a55f53dbe6`, covering a
  900s timeout with a verified one-file `ploke-error` diff, focused validation
  only, absent submitted result, and no benchmark/admission usefulness.
- [`2026-06-01-p1-gemini35-flash-direct-15g2x3-par2-20260601-173956-node-552c19a55f53dbe6-r2-broad-harness.md`](2026-06-01-p1-gemini35-flash-direct-15g2x3-par2-20260601-173956-node-552c19a55f53dbe6-r2-broad-harness.md)
  Broad-harness r2 review, classifying a timed-out/no-edit attempt with no
  submitted result, no proposal events, clean workspace state, empty successful
  read defects, and benchmark-useless outcome.
- [`2026-06-01-p1-gemini35-flash-direct-15g2x3-par2-20260601-173956-node-552c19a55f53dbe6-r3-broad-harness.md`](2026-06-01-p1-gemini35-flash-direct-15g2x3-par2-20260601-173956-node-552c19a55f53dbe6-r3-broad-harness.md)
  Broad-harness r3 review, covering a timed-out trace-bearing attempt with a
  verified dirty `ploke-db/src/helpers.rs` diff, missing submitted result,
  non-final validation, and no descendant-performance proof.
- [`2026-06-01-p1-gemini35-flash-direct-15g2x3-par2-20260601-173956-node-552c19a55f53dbe6-r4-broad-harness.md`](2026-06-01-p1-gemini35-flash-direct-15g2x3-par2-20260601-173956-node-552c19a55f53dbe6-r4-broad-harness.md)
  Broad-harness r4 review, covering a timed-out trace-bearing attempt with a
  verified two-file `ploke-db` diff, incomplete ordered apply projection for the
  second proposal, focused validation gaps, and absent submitted result.
- [`2026-06-01-p1-gemini35-flash-direct-15g2x3-par2-20260601-173956-node-552c19a55f53dbe6-r5-provider-unavailable.md`](2026-06-01-p1-gemini35-flash-direct-15g2x3-par2-20260601-173956-node-552c19a55f53dbe6-r5-provider-unavailable.md)
  Broad-harness r5 provider-unavailable review, separating Google/Vertex
  `HTTP_429 RESOURCE_EXHAUSTED` from model/patch failure and verifying no
  submitted result, no proposal events, and no workspace diff.
- [`2026-06-01-p1-gemini35-flash-direct-15g2x3-par2-20260601-173956-node-552c19a55f53dbe6-r6-broad-harness.md`](2026-06-01-p1-gemini35-flash-direct-15g2x3-par2-20260601-173956-node-552c19a55f53dbe6-r6-broad-harness.md)
  Broad-harness r6 applied-candidate review, covering the `FanOut` concurrency
  edit in `ploke-protocol/src/procedure.rs`, focused cargo validation, request
  contract mismatch, unmeasured benchmark value, and missing authority/admission
  artifacts.
- [`2026-06-01-p1-gemini35-flash-direct-15g2x3-par2-20260601-173956-node-552c19a55f53dbe6-r7-r9-incomplete-child-state.md`](2026-06-01-p1-gemini35-flash-direct-15g2x3-par2-20260601-173956-node-552c19a55f53dbe6-r7-r9-incomplete-child-state.md)
  Incomplete child-state note, not a durable successful run review: r7 is
  workspace-only with no trace/result/diff, and r8-r9 are request-only.
- [`2026-06-01-p1-gemini35-flash-direct-15g2x3-par2-20260601-173956-child-run-review-fanin.md`](2026-06-01-p1-gemini35-flash-direct-15g2x3-par2-20260601-173956-child-run-review-fanin.md)
  Fan-in synthesis for the child run-review cards, separating timed-out attempts,
  provider-unavailable r5, applied-but-unadmitted r6, and incomplete r7-r9 state
  while mapping findings to alive bugs, blocker-repair work, and adjudication
  signals.
