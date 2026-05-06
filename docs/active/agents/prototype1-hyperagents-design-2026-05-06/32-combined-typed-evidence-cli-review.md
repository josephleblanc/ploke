# Reviewer C: Combined Typed Evidence CLI Review

Date: 2026-05-06

## Findings

No blocking findings.

Follow-up: `EvidenceParseStatus::JsonFallback` is doing two jobs in
`evidence.rs`: it marks true fallback after a known typed decode fails, and it
also marks valid JSON for classes that do not yet have a typed decoder
(`Invocation`, `AttemptResult`, `SuccessorReady`, `SuccessorCompletion`,
`Scheduler`, and `BranchRegistry`). That is documented in report 30 and does not
break the current projection, but it is slightly imprecise for future consumers
that may read `json_fallback` as "typed parse was attempted and failed." A later
status such as `untyped_json` or `json_projection` would make the provenance
surface clearer without changing selection behavior.

## Review Questions

1. Typed decoding preserves the read-only evidence/projection boundary. The new
   evidence path decodes `Document` payloads inside
   `ChildEvidenceSet::from_sources`, preserves `EvidencePointer`, class,
   treatment, parse status, facts, and diagnostics, and does not call History
   admission, Crown, block write, scoring, or selection APIs. The module docs
   explicitly keep sealed authority on the History/Crown path.
2. Parse status and fact-origin fields are useful and mostly accurate for the
   current operator projection. Typed decodes record the expected source type;
   typed decode failures retain the diagnostic and degrade to JSON extraction;
   path and filename facts are marked separately. The only semantic wrinkle is
   the overloaded `json_fallback` status noted above.
3. Selection projection still fails closed on conflicts after typed decoding.
   `selection_input()` imports child conflict diagnostics before constructing a
   `SelectionInput`, rejects missing required fields, rejects duplicate
   evaluations for a branch, and refuses branch metadata conflicts. Ambiguous
   runtime/branch joins are removed from the join maps and become unplaced
   evidence instead of being reused indirectly.
4. `history child-evidence` and the monitor alias call the existing child
   evidence producer. Both CLI paths route through `run_child_evidence`, which
   calls `history_preview::run_child_evidence`; that constructs an
   `FsEvidenceStore` and calls `store.child_evidence()`. I did not find a
   duplicated join implementation in the CLI layer.
5. Command names and output are bounded and structural. The command is
   `child-evidence`, table output is capped at 20 child/evaluation/unplaced/
   diagnostic rows and 3 compared runs per evaluation, and the wording stays in
   evidence/projection terms. It does not frame the output as scoring or
   authority; the table prints `treatment_note: source treatment labels are not
   sealed authority`.
6. Formatting and compile/test checks passed. Existing warning noise remains in
   `syn_parser` and Prototype 1 History/dead-code areas, but no new compile
   failure appeared in the scoped patch.
7. I did not see naming or structure drift that violates the local AGENTS.md
   constraints. The child evidence layer is a projection over existing child,
   runtime, branch, evaluation, source, and diagnostic carriers. It does not add
   a new authority layer or flatten the Parent/child protocol into new public
   status writers.

## Verification

- `cargo fmt --all`: passed.
- `cargo test -p ploke-eval --lib prototype1_state::evidence`: passed, 10 tests.
- `cargo test -p ploke-eval cli::tests::history_child_evidence_should_parse`:
  passed.
- `cargo test -p ploke-eval cli::tests::prototype1_monitor_child_evidence_should_parse`:
  passed.
- `cargo check -p ploke-eval`: passed.
- `git diff --check`: passed.

## Changed Files

No source files changed by this review. I added this report and updated the
directory README index.

## Recommendation

Accept with follow-up. The combined patch is structurally sound and verified;
the only requested follow-up is to split the overloaded `json_fallback` status
before downstream tooling starts treating parse status as a stronger fact.
