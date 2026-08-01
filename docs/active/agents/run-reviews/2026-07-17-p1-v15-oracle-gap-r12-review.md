# R12 Review: p1-v15-scanbarrier-livehandoff-keeponly-g35f-oropenai-3g1x3-p3-20260716-234429

Date: 2026-07-17

Campaign: `p1-v15-scanbarrier-livehandoff-keeponly-g35f-oropenai-3g1x3-p3-20260716-234429`

Status: preserved at R12; no successor handoff attempted

## Verdict

V15 proved that the repaired post-edit scan barrier can complete inside the
live walk path and that a built child runtime can run through treatment
evaluation, protocol, parent comparison, and successor selection. The control
session committed every phase from R3 through R12, and the selected child
runtime terminated cleanly after writing its treatment result.

V15 must not be used as a handoff proof. Its sole selected child received an
operational `keep`, but the admitted profile had MBE disabled and the branch
evaluation contained no oracle evaluation. The candidate edit itself added a
second malformed-JSON recovery branch in `ploke-protocol`; its only new test
did not exercise that branch. Operational selection therefore produced a
continuable-looking candidate without proof that the benchmark target was
resolved or that the self-edit caused an improvement.

The run remains byte-for-byte preserved at R12. No profile, History, epoch,
checkout, or digest was rewritten to force handoff. A checked-in historical
fixture copies the production carriers and proves that the new strict policy
fails closed on the missing oracle evidence.

## Evidence Roots

- Campaign root:
  `/home/brasides/.ploke-eval/campaigns/p1-v15-scanbarrier-livehandoff-keeponly-g35f-oropenai-3g1x3-p3-20260716-234429`
- Seed checkout:
  `/home/brasides/.ploke-eval/setup-seeds/p1-v15-scanbarrier-livehandoff-keeponly-g35f-oropenai-3g1x3-p3-20260716-234429`
- Control journal:
  `prototype1/control/sessions/d789cb19e6082290577a7fe51b1b3ceb4f5f4832519444dacaf61488350a778c/control-journal.jsonl`
- Transition journal: `prototype1/transition-journal.jsonl`
- Parent: `node-9c9dcbeeb3a4d400`
- Selected child: `node-6bae782006db482c`
- Selected branch: `branch-68afac57e92d5ebd`
- Candidate workspace:
  `prototype1/workspaces/edit-harness/node-9c9dcbeeb3a4d400-r3`
- Candidate commit: `b5e1c2da4c5a9ca07aa010cafe7f75e78ab43aa5`
- Immutable regression fixture:
  `crates/ploke-eval/src/tests/fixtures/prototype1-v15-missing-oracle-20260717/`

The fixture README records SHA-256 values for every imported artifact. The
production replay test verifies those hashes before decoding the typed
carriers. No sqlite file was opened directly for this review.

## Walk Proof

The step-mode session committed this path:

```text
R3 -> R4a -> R4b -> R4c -> R5 -> R6 -> R7
   -> R8 -> R9 -> R10 -> R11 -> R12
```

The R12 control cursor is
`fa885afe65042e157da06da5712a8bd1516e37daa8c03e914e8436b34fcd6291`.
The long live edges completed rather than remaining nonterminal:

| Edge | Elapsed wall time | Result |
| --- | ---: | --- |
| R5 to R6 | 739.869 s | committed |
| R7 to R8 | 1038.177 s | committed |
| R10 to R11 | 546.948 s | committed |

The R7-to-R8 path includes the bounded post-edit scan barrier that replaced the
v14 unbounded-stall behavior. The child transition journal records runtime
`2da0bb10-fe35-4e43-a773-77e0145acfff` as result-written, then observed with
`child_lifecycle = terminated` and a complete treatment campaign.

No v15 walk server or child runtime remains live. The unrelated v12 walk server
is intentionally preserved and continues to own the main workspace binary.

## Admitted Policy Gap

The committed profile SHA-256 is
`cfda200a2ad71803cb405c6e109bca8c80dd6b2101d4ec4a12abe9eedf1c4f95`.
Its relevant settings were:

```toml
[selection.oracle]
mode = "record-only"
require_evidence = true

[execution.mbe]
enabled = false
```

Because the profile predated an admission gate, the missing `gate` field
defaulted to `disabled`. In record-only mode, `require_evidence` did not affect
successor admission. That combination was valid but misleading: it looked
strict in the profile while permitting an operationally kept candidate with no
oracle result.

The branch evaluation proves the missing evidence rather than a provider or
serialization failure:

- eval-set identity: `BurntSushi__ripgrep-2209`;
- baseline metrics: `oracle_eligible = true`;
- treatment metrics: `oracle_eligible = true`;
- `oracle_evaluation = null`;
- overall disposition: `keep`.

## Candidate Review

The selected commit added 33 lines to
`crates/ploke-protocol/src/llm.rs`. It introduced a general-looking recovery
path that searches for the last `}` and retries parsing a prefix. The added
test passes truncated JSON that has no closing brace, so it exercises an
earlier recovery path rather than the new last-brace branch. The change is
therefore not supported by the test it added and overlaps existing malformed
JSON recovery behavior.

This makes v15's operational `keep` especially unsuitable as handoff authority:
the candidate source change was not shown to repair the benchmark target, and
the benchmark evaluation supplied no MBE verdict.

## Source Repair

The follow-up source adds a typed `OracleGate` with two policies:

- `disabled`: preserve existing record-only and relative-score behavior;
- `all-resolved`: require exact admitted target coverage and admit only
  candidates whose oracle verdicts are all resolved.

For `all-resolved`, selection now requires:

1. `require_evidence = true`;
2. MBE enabled with a nonempty target set;
3. sealed eval-set identity equal to the admitted profile targets;
4. selection-input comparisons equal to the same targets;
5. exactly one oracle evaluation per target in both carriers;
6. byte-equivalent typed evaluations across the two carriers; and
7. verdict, diagnostic, and `usable_for_selection` consistency.

Valid negative oracle evidence excludes a candidate before ranking. Missing,
duplicate, unknown, mismatched, or internally inconsistent evidence is a hard
selection error. The policy and exact target list are sealed in traversal
evidence, persisted in passive records, mirrored into an additive
`eval_oracle_gate` campaign-policy relation and a decision-linked
`eval_selection_oracle` relation, and shown in replay JSON, CLI table output,
and the egui formula summary.

The implementation does not alter successor digest verification, History
verification, epoch checks, checkout validation, or handoff authority.

## Regression Coverage

The production replay reconstructs the v15 candidate from its real campaign,
profile commitment, parent identity, child plan, node, runner result, terminal
channel, and evaluation report. It proves:

- the original disabled policy still selects the v15 candidate;
- the same typed evidence under `all-resolved` fails closed with missing oracle
  evidence;
- the fixture remains read-only; and
- no History or eval DB is created beside the fixture.

Focused coverage also proves resolved-vs-unresolved filtering under both
selection strategies, exact target matching, missing-evidence rejection,
legacy RelativeScore compatibility, passive serde round trips, additive DB
installation, and CLI/UI projection.

Oracle-excluded ScoreChildProp rows retain their pre-gate performance values
while remaining non-selectable, so operators can distinguish exclusion from
missing performance evidence.

## Remaining Observability Gaps

These do not justify weakening the gate, but remain follow-up work:

1. When every candidate has valid negative oracle evidence, traversal returns
   no selection before sealing a decision. The exclusion assessments are not
   yet persisted as a no-selection History/formula/DB artifact.
2. FrontierMax has no strategy formula. Its database candidate projection can
   therefore lack the ScoreChildProp `selectable` and exclusion fields.
3. Existing historical policy rows have no `eval_oracle_gate` row; readers
   must interpret absence as the historical `disabled` default.

The fresh proof campaign should use HistoryScoreChildProp, MBE enabled, and
`gate = "all-resolved"`. If all children are unresolved, preserve that run as
negative evidence and repair the no-selection observability gap before claiming
an inspectable rejection proof.
