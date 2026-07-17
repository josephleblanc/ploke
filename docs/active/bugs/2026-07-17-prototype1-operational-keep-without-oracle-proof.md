# Prototype 1 Operational Keep Without Oracle Proof

Status: source repaired; historical replay passes; fresh strict live handoff
validation pending.

## Broken Contract

A self-edit candidate must not become successor-handoff authority merely
because operational comparison returns `keep` when the admitted policy requires
benchmark-resolution evidence. Ranking policy, evidence completeness, and
handoff admission are separate decisions.

When strict oracle admission is configured, every admitted target must have
matching, internally consistent oracle evidence in the sealed evaluation and
selection-input carriers, and every verdict must be resolved. Missing or
unresolved evidence must stop admission without weakening History, digest,
epoch, checkout, or successor validation.

## Preserved Incident

Campaign
`p1-v15-scanbarrier-livehandoff-keeponly-g35f-oropenai-3g1x3-p3-20260716-234429`
reached R12 and selected child `node-6bae782006db482c` / branch
`branch-68afac57e92d5ebd`.

The committed profile set:

```toml
[selection.oracle]
mode = "record-only"
require_evidence = true

[execution.mbe]
enabled = false
```

The branch evaluation records `oracle_eligible = true` for both baseline and
treatment metrics, but `oracle_evaluation = null`, while the operational
disposition is `keep`. Record-only mode ignored `require_evidence`, so the
candidate entered traversal without oracle proof.

The selected candidate commit
`b5e1c2da4c5a9ca07aa010cafe7f75e78ab43aa5` added malformed-JSON recovery code
whose new test did not exercise the added branch. There is therefore neither
oracle proof nor a convincing source-level causal argument for handoff.

The campaign remains preserved at R12. No digest or committed profile was
rewritten and no successor checkout was installed.

## Source Boundary

Before repair:

1. Profile validation coupled required evidence to MBE only for
   `relative-score`.
2. `record-only` returned no oracle score and imposed no admission rule.
3. Traversal ranked candidates on operational/protocol evidence and could
   select an oracle-empty `keep`.
4. Handoff correctly trusted the resulting sealed selection; the missing
   contract was before sealing, not inside digest or successor verification.

The fix therefore belongs at profile admission and traversal admission. The
handoff verifier must remain unchanged.

## Repair

The canonical run-profile carrier now owns `OracleGate`:

- `disabled` is the serde default and preserves historical behavior;
- `all-resolved` is a fail-closed admission rule independent of oracle ranking
  mode.

`all-resolved` requires `require_evidence = true`, MBE enabled, and a nonempty
admitted target set. Traversal binds the exact target set from the admitted
profile, requires matching sealed and selection-input evidence, validates
verdict/diagnostic/usability consistency, and excludes valid unresolved
candidates before ranking. Malformed or incomplete evidence remains a hard
error.

The gate and target set are carried through live run shape, sealed traversal
evidence, passive DTOs, ScoreChildProp formula/replay, an additive
`eval_oracle_gate` DB relation, CLI replay output, and the egui selection
summary. The additive relation avoids changing the stored shape of
`eval_run_profile_policy` in older eval databases. A separate additive
`eval_selection_oracle` row binds each persisted selection decision to its
sealed oracle mode, evidence requirement, gate, target set, and formula id.

## Docs Expectation

`crates/ploke-eval/docs/prototype1/run-profile.md` now documents oracle `mode`
and `gate` as separate axes. The strict live profile must use:

```toml
[selection.oracle]
mode = "record-only"
require_evidence = true
gate = "all-resolved"

[execution.mbe]
enabled = true
```

No operator or test should retrofit these settings into an admitted run. A
changed profile or source epoch requires a fresh campaign.

## Current Repro Coverage

The immutable fixture
`crates/ploke-eval/src/tests/fixtures/prototype1-v15-missing-oracle-20260717/`
contains SHA-256-pinned copies of the production carriers. The historical replay
drives `select_successor_for_profile` and proves the original profile selects
while `all-resolved` rejects the same evidence.

Additional tests cover:

- resolved candidate selection over a higher-performing unresolved candidate;
- all-unresolved no-selection behavior;
- missing and wrong-target evidence rejection;
- legacy RelativeScore sealed-evidence precedence under `disabled`;
- profile admission constraints;
- passive record and traversal target round trips; and
- additive eval DB schema installation.

## Remaining Repro / Validation

Run a fresh campaign from the repaired source with MBE enabled and
`gate = "all-resolved"`. A successful proof must show:

1. the selected candidate has resolved oracle evidence for every committed
   target in both carriers;
2. the selection formula and DB rows expose the gate and target set;
3. the exact selected Artifact passes the existing History and digest checks;
4. the predecessor hands control to the verified successor runtime; and
5. the successor advances another generation without rewriting prior evidence.

If every candidate is unresolved, preserve the run and add a typed persisted
no-selection outcome before treating the negative gate behavior as fully
observable.
