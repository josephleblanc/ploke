# Prototype 1 History v2 Startup Authority Audit C

Recorded: 2026-04-29

Scope: cross-document consistency for History as authority versus imported JSON
evidence/projections, with emphasis on descriptive versus prescriptive claims.
No source or design docs were modified.

## Summary

The current documents mostly preserve the correct authority split: sealed History
is the intended authority surface, while scheduler, branch, runner, preview,
metrics, and legacy journal records remain evidence or projections until admitted
into sealed blocks. The clearest residual risk is wording drift around startup:
some source docs describe the artifact/head gate as a "current claim" even though
live startup still validates parent identity, scheduler/invocation state, and
successor files rather than a sealed History head.

The v2 concepts `PolicyRef`/`PolicyScope`/`Surface`, artifact-local manifest
commitments, stochastic evidence refs, and head-state/finality are consistently
presented as intended or next-slice material in the conceptual docs. The current
code implements only narrower placeholders: `ProcedureRef`, `ArtifactRef`,
`TreeKeyHash`, local block hashes, append storage, and provisional preview
grouping.

## Checked Claims

| Claim | Classification | Evidence |
| --- | --- | --- |
| History is not scheduler/branches/report/preview/metrics/Cozo and existing JSON does not become authority by being read. | Implemented/descriptive | `crates/ploke-eval/src/cli/prototype1_state/history.rs:30`, `crates/ploke-eval/src/cli/prototype1_state/history.rs:130`, `docs/workflow/evalnomicon/drafts/history-blocks-and-crown-authority.md:27`, `docs/reports/prototype1-record-audit/history-admission-map.md:3` |
| The read-only preview imports evidence and emits History-shaped provisional groups, not sealed authority blocks. | Implemented/descriptive | `docs/reports/prototype1-record-audit/history-admission-map.md:108`, `docs/reports/prototype1-record-audit/history-admission-map.md:127`, `crates/ploke-eval/src/cli/prototype1_state/history_preview.rs:1`, `crates/ploke-eval/src/cli/prototype1_state/history_preview.rs:253`, `crates/ploke-eval/src/cli/prototype1_state/history_preview.rs:1564` |
| The metrics command is a projection, and its selection labels are source-strength labels rather than Crown authority. | Implemented/descriptive with naming risk | `docs/reports/prototype1-record-audit/history-admission-map.md:184`, `crates/ploke-eval/src/cli/prototype1_state/metrics.rs:1`, `crates/ploke-eval/src/cli/prototype1_state/metrics.rs:1009`, `crates/ploke-eval/src/cli/prototype1_state/metrics.rs:1129`, `crates/ploke-eval/src/cli/prototype1_state/metrics.rs:1828` |
| `Block<Open> -> Block<Sealed>` is now structurally gated by `Crown<Locked>` and same-lineage checking. | Implemented/descriptive | `crates/ploke-eval/src/cli/prototype1_state/inner.rs:42`, `crates/ploke-eval/src/cli/prototype1_state/inner.rs:59`, `crates/ploke-eval/src/cli/prototype1_state/history.rs:1121`, `crates/ploke-eval/src/cli/prototype1_state/history.rs:1455`, `crates/ploke-eval/src/cli/prototype1_state/history.rs:1466` |
| Sealed blocks are locally tamper-evident by deterministic entry roots and block hashes. | Implemented/descriptive | `crates/ploke-eval/src/cli/prototype1_state/history.rs:1318`, `crates/ploke-eval/src/cli/prototype1_state/history.rs:1385`, `crates/ploke-eval/src/cli/prototype1_state/history.rs:1394`, `crates/ploke-eval/src/cli/prototype1_state/history.rs:1405`, `crates/ploke-eval/src/cli/prototype1_state/history.rs:1418` |
| `BlockStore` appends sealed blocks and maintains rebuildable indexes, but `heads.json` is not an authenticated head proof or finality mechanism. | Implemented/descriptive plus intended limitation | `crates/ploke-eval/src/cli/prototype1_state/history.rs:396`, `crates/ploke-eval/src/cli/prototype1_state/history.rs:409`, `crates/ploke-eval/src/cli/prototype1_state/history.rs:536`, `crates/ploke-eval/src/cli/prototype1_state/history.rs:552`, `crates/ploke-eval/src/cli/prototype1_state/history.rs:563` |
| Live startup must prove `ProducedBy` and `AdmittedBy` before `Parent<Ruling>`. | Intended/prescriptive, not implemented | `docs/workflow/evalnomicon/chat-history/history-blocks-v2.md:81`, `docs/workflow/evalnomicon/chat-history/history-blocks-v2.md:90`, `crates/ploke-eval/src/cli/prototype1_state/history.rs:52`, `crates/ploke-eval/src/cli/prototype1_state/mod.rs:85`, `crates/ploke-eval/src/cli/prototype1_state/mod.rs:93` |
| The current live startup path is History-gated. | Ambiguous/overclaiming where phrased as current | `crates/ploke-eval/src/cli/prototype1_state/history.rs:117` says "Current claim" and "may enter ... only if"; the same file says successor verification through sealed History is not enforced at `crates/ploke-eval/src/cli/prototype1_state/history.rs:159`, `crates/ploke-eval/src/cli/prototype1_state/history.rs:250`; live startup loads parent identity and checks it through `Parent::<Unchecked>::load(...).check(...)` at `crates/ploke-eval/src/cli/prototype1_state/cli_facing.rs:3208`, then acknowledges successor handoff via invocation/ready records at `crates/ploke-eval/src/cli/prototype1_state/cli_facing.rs:3056`, `crates/ploke-eval/src/cli/prototype1_state/cli_facing.rs:3110`. |
| Live handoff locks a Crown carrier before successor spawn. | Implemented/descriptive, but not yet sealed History authority | `crates/ploke-eval/src/cli/prototype1_process.rs:861`, `crates/ploke-eval/src/cli/prototype1_process.rs:875`, `crates/ploke-eval/src/cli/prototype1_state/parent.rs:423`; the missing persistence/verification is explicitly recorded at `crates/ploke-eval/src/cli/prototype1_state/history.rs:250` and `crates/ploke-eval/src/cli/prototype1_state/mod.rs:664`. |
| `PolicyRef`/`PolicyScope` should be defined through `Surface`. | Intended/prescriptive | `docs/workflow/evalnomicon/chat-history/history-blocks-v2.md:143`, `docs/workflow/evalnomicon/chat-history/history-blocks-v2.md:159`, `docs/workflow/evalnomicon/drafts/history-blocks-and-crown-authority.md:278`; current History still uses `ProcedureRef` as `policy_ref` and explicitly lacks first-class `PolicyRef` at `crates/ploke-eval/src/cli/prototype1_state/history.rs:1088`, `crates/ploke-eval/src/cli/prototype1_state/history.rs:1102`. `Surface` exists in intervention algebra, not History policy, at `crates/ploke-eval/src/intervention/algebra/mod.rs:68`. |
| Artifact commitments need backend tree key plus artifact-local manifest digest/ref. | Intended/prescriptive, partially scaffolded | `docs/workflow/evalnomicon/chat-history/history-blocks-v2.md:219`, `docs/reports/prototype1-record-audit/history-admission-map.md:92`; code has `ArtifactRef` and private `TreeKeyHash`/`TreeKeyCommitment` at `crates/ploke-eval/src/cli/prototype1_state/history.rs:671`, `crates/ploke-eval/src/cli/prototype1_state/history.rs:685`, `crates/ploke-eval/src/cli/prototype1_state/history.rs:707`, but `OpenBlock` still records `opened_from_artifact: ArtifactRef` and `policy_ref: ProcedureRef` without manifest digest at `crates/ploke-eval/src/cli/prototype1_state/history.rs:1093`. |
| Stochastic evidence refs, rejected/failure evidence, uncertainty/risk refs, rollback/fork/finality/head-state are v2 block concerns. | Intended/prescriptive | `docs/workflow/evalnomicon/chat-history/history-blocks-v2.md:136`, `docs/workflow/evalnomicon/chat-history/history-blocks-v2.md:188`, `docs/workflow/evalnomicon/chat-history/history-blocks-v2.md:234`; code marks these as design target at `crates/ploke-eval/src/cli/prototype1_state/history.rs:1184`, `crates/ploke-eval/src/cli/prototype1_state/history.rs:1205`, `crates/ploke-eval/src/cli/prototype1_state/history.rs:1209`, while implemented `SealedBlockHeader` stops at successor/artifact/entry-root/hash fields at `crates/ploke-eval/src/cli/prototype1_state/history.rs:1158`. |
| Ingress is the route for late/backchannel observations after a Crown lock. | Intended with partial core implementation | Draft policy says late events can affect the next epoch only after import at `docs/workflow/evalnomicon/drafts/history-blocks-and-crown-authority.md:313`, `docs/workflow/evalnomicon/drafts/history-blocks-and-crown-authority.md:334`, `docs/workflow/evalnomicon/drafts/history-blocks-and-crown-authority.md:358`. Core typestate exists at `crates/ploke-eval/src/cli/prototype1_state/history.rs:1485`, `crates/ploke-eval/src/cli/prototype1_state/history.rs:1537`, `crates/ploke-eval/src/cli/prototype1_state/history.rs:1547`, but live capture/import is still listed as missing at `crates/ploke-eval/src/cli/prototype1_state/history.rs:165`. Preview labels successor ready/completion as degraded pre-History or ingress-dependent at `crates/ploke-eval/src/cli/prototype1_state/history_preview.rs:1301`. |
| Legacy flattened journal names must not become the History ontology. | Implemented/descriptive guidance; residual import-surface drift | `AGENTS.md:5`, `AGENTS.md:21`, `crates/ploke-eval/src/cli/prototype1_state/history.rs:265`, `crates/ploke-eval/src/cli/prototype1_state/journal.rs:214`, `crates/ploke-eval/src/cli/prototype1_state/journal.rs:235`, `crates/ploke-eval/src/cli/prototype1_state/journal.rs:252`. Preview still consumes legacy variants directly at `crates/ploke-eval/src/cli/prototype1_state/history_preview.rs:724`, so keep the current `authority_status`/normalization labels load-bearing until those variants are deprecated at the source. |

## Critiques And Corrections

1. Reword present-tense startup authority claims in source docs.
   `crates/ploke-eval/src/cli/prototype1_state/history.rs:117` reads like an
   implemented gate, but `crates/ploke-eval/src/cli/prototype1_state/history.rs:159`,
   `crates/ploke-eval/src/cli/prototype1_state/history.rs:250`, and
   `crates/ploke-eval/src/cli/prototype1_state/cli_facing.rs:3208` show the live
   gate is not sealed-History based. Replace "Current claim" there with
   "Intended local authority claim once startup admission is wired" or add an
   adjacent explicit "not implemented in live startup" sentence.

2. Rename or qualify metrics `selection_authority`.
   `crates/ploke-eval/src/cli/prototype1_state/metrics.rs:239`,
   `crates/ploke-eval/src/cli/prototype1_state/metrics.rs:1017`, and
   `crates/ploke-eval/src/cli/prototype1_state/metrics.rs:1136` use "authority" for
   `transition_journal` versus `mutable_projection`. That is clear in the map at
   `docs/reports/prototype1-record-audit/history-admission-map.md:224`, but the
   field name can be mistaken for Crown/History authority. Prefer
   `selection_source_strength`, `selection_source_class`, or a documented enum
   whose variants say they are not sealed authority.

3. Keep `PolicyRef`/`PolicyScope`/`Surface` explicitly marked as v2 intended
   until History owns them structurally.
   `docs/workflow/evalnomicon/chat-history/history-blocks-v2.md:143` and
   `docs/workflow/evalnomicon/drafts/history-blocks-and-crown-authority.md:278`
   are correctly prescriptive. Do
   not let `crates/ploke-eval/src/cli/prototype1_state/history.rs:1102`'s
   `ProcedureRef policy_ref` become the claimed policy implementation without a
   separate policy/surface type.

4. Keep artifact commitments described as partial until manifest digests are in
   the block header or referenced evidence. `TreeKeyHash` is a useful barrier at
   `crates/ploke-eval/src/cli/prototype1_state/history.rs:685`, but
   `ArtifactRef` at `crates/ploke-eval/src/cli/prototype1_state/history.rs:671`
   is still only a recoverable reference, and
   `docs/reports/prototype1-record-audit/history-admission-map.md:92` correctly
   records the missing artifact-local provenance manifest.

5. Preserve the authority/projection distinction in examples and operator-facing
   docs. The preview is labeled well in code
   (`crates/ploke-eval/src/cli/prototype1_state/history_preview.rs:1`,
   `crates/ploke-eval/src/cli/prototype1_state/history_preview.rs:1569`) and the
   metrics module is labeled well
   (`crates/ploke-eval/src/cli/prototype1_state/metrics.rs:1`). Any new examples under the `history` command should repeat
   that these outputs are read-only projections unless they are backed by
   `Block<Sealed>` appended through `BlockStore`
   (`crates/ploke-eval/src/cli/prototype1_state/history.rs:422`,
   `crates/ploke-eval/src/cli/prototype1_state/history.rs:536`).
