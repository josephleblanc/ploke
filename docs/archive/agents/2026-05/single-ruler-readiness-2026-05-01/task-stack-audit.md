# Task Stack Audit: Single-Ruler Readiness

Date: 2026-05-01

Scope:

- `.codex/skills/task-stack/SKILL.md`
- `.codex/task-stack.jsonl`
- `crates/ploke-eval/src/cli/prototype1_state/mod.rs`
- `crates/ploke-eval/src/cli/prototype1_state/history.rs`
- Targeted implementation checks in `parent.rs`, `inner.rs`, and `prototype1_process.rs`

No stack rewrite was performed.

## 1. Executive Summary

The stack is stale in the Crown/History area. Several high-priority entries from
2026-04-29 describe gaps that the current implementation now partially or fully
addresses: live handoff seals and appends a History block before successor
spawn, `Crown<Locked>` carries seal material into `Crown<Locked>::seal`,
successor startup verifies the sealed head's current Artifact tree and surface,
and `FsBlockStore::append` compares the observed `LineageState` before
advancing a lineage head.

The remaining single-ruler readiness blockers are narrower:

- the new sparse state-map adapter needs audit against the documented
  Crown/Block invariants;
- ready acknowledgements are still transport files whose loaded contents appear
  to be ignored by the predecessor wait loop;
- fallible before/after transitions still need an audit and outcome carrier so
  long runs do not leave ambiguous in-flight projections;
- open block and admission paths are Crown-mediated, but still accept actor
  identity as caller data rather than deriving it from a `Parent<Ruling>`-like
  carrier;
- `SealBlock::from_handoff` remains a compatibility seam where header material
  is assembled by the caller before the Crown is locked.

The conceptual correction matters for prioritization: Artifact/tree-key identity
is not exclusive to one lineage. Multiple lineage heads may reference the same
Artifact/tree key under policy. Authority conflicts are conflicts over advancing
the same History lineage head without the required Crown, lock/lease, consensus,
or fork-choice rule. Do not turn any of the remaining tree-key tasks into a
cross-lineage artifact uniqueness constraint unless the task is explicitly
single-ruler-local.

## 2. Open Items By Recommended Status

### Keep Focus

- line 39, `history-state-map-adapter-audit`, index `1.22`: keep focus.
  `history.rs` now documents and implements `HistoryStateRoot`, `LineageState`,
  `StoreHead`, and sparse proof plumbing. This is new enough that it should be
  audited before relying on long single-ruler runs. The expected claim is local
  configured-store absence/presence, not consensus or process uniqueness.

- line 20, `history-ready-ack-validation`, index `1.5`: keep focus.
  `wait_for_prototype1_successor_ready` still loads the ready record and then
  discards it. A long run should not treat mere path existence and syntactic JSON
  as proof that the intended successor runtime acknowledged handoff. This is a
  practical single-ruler blocker even though it is transport/debug evidence, not
  Crown authority.

- line 17, `failure-record-audit-before-after-gaps`, index `4.2`: keep focus.
  Long runs need unambiguous terminal or in-flight evidence when a process dies
  between before/after records. Audit first; do not design a broad carrier until
  the real gaps are known.

- line 16, `failure-record-transition-outcomes`, index `4.1`, and line 18,
  `failure-record-mixed-inflight-tests`, index `4.3`: keep focus behind `4.2`.
  These should follow the audit: first define the smallest outcome carrier, then
  lock in monitor/report inference tests for mixed in-flight states.

### Keep Next

- line 33, `history-open-block-admission-authority-gate`, index `1.17`: keep next.
  Current code routes `open_block`, `open_successor`, and `admit_entry` through
  `Crown<Ruling>`, and raw `Block<Open>` construction is private. The remaining
  gap is that `opened_by`, `ruling_authority`, and `admitting_authority` are
  still supplied as data. The next slice should derive these from a structural
  parent/ruler carrier.

- line 32, `history-crown-lock-carries-seal-material`, index `1.16`: keep next
  but reframe the title. The literal title is mostly implemented:
  `Crown<Locked>` carries `SealBlock`, and sealing consumes the locked Crown.
  The remaining issue is the compatibility constructor and caller-assembled
  handoff material. Focus on replacing `SealBlock::from_handoff` with a concrete
  handoff transition/material carrier.

- line 23, `history-genesis-authority-block`, index `1.8`: keep next but reframe.
  Genesis startup now validates local configured-store absence, and handoff
  block opening records `OpeningAuthority::Genesis` with bootstrap procedure,
  tree key, and parent identity reference. The remaining gap is a uniform
  startup/admission carrier and clearer bootstrap wording. Do not phrase this as
  "no other lineage uses this Artifact"; phrase it as "no valid head for this
  configured lineage in this configured store/root."

- line 25, `history-block-transaction-manifest-slice`, index `1.10`: keep next.
  Still useful after current state-map and surface work. It should now focus on
  admitted transactions/relations, artifact-local provenance manifests, and
  addressable evidence without turning local single-ruler policy into global
  artifact ownership.

- line 28, `history-v2-minimal-type-slice`, index `1.13`: keep next, narrowed.
  Surface commitments and local lineage-state roots have landed. Prune that
  part of the next action mentally; remaining value is artifact commitment/
  manifest refs, evidence/sample/risk refs or roots, and head-state/finality
  placeholders that do not overclaim consensus.

- line 8, `history-live-entry-append-minimal`, index `2.1`, and line 9,
  `history-mark-mutable-records-as-projections`, index `2.2`: keep next after
  single-ruler blockers. They are not the first gate for running, but they are
  needed before reports stop depending on sidecar records that look more
  authoritative than they are.

- line 19, `history-successor-doc-semantics-audit`, index `1.4`: keep next.
  Current canonical docs now prefer incoming Runtime plus verified handoff/Crown
  semantics over a literal `Successor<Admitted>` state. Clean stale comments
  before more agents copy the old term.

- line 40, `history-process-seam-structural-naming`, index `1.23`: keep next
  after the readiness blockers. The selected-successor admission check has
  largely landed, so this can become a small module/context refactor around the
  handoff/startup boundary.

### Reframe

- line 3, `history-live-crown-authority`, index `1`: reframe the group.
  "Wire live Crown-gated History mutation" is no longer an accurate focus:
  live handoff now seals/appends through the Crown path. Rename conceptually to
  remaining single-ruler Crown/History hardening: state-map audit, ready ack,
  actor identity in authority carriers, compatibility handoff material, and
  bootstrap admission.

- line 10, `history-normalize-legacy-record-labels`, index `2.3`: reframe as a
  boundary for importing legacy projections into structural History facts. Do
  not invent flattened names like `ChildArtifactCommittedEntry` as new ontology.

- line 24, `history-key-commitment-naming-review`, index `1.9`: reframe with the
  Tree vs lineage correction. `TreeKeyHash`/`TreeKeyCommitment` should mean an
  Artifact/backend-tree commitment. They must not imply the Artifact/tree key is
  owned by one lineage.

- line 35, `history-locator-artifact-tree-context`, index `1.19`: reframe as a
  lineage plus Artifact commitment recovery problem, not as an Artifact
  uniqueness problem. It can follow the manifest/transaction slice.

- line 14, `report-authority-aware-labels`, index `3.3`: reframe labels around
  projection strength versus sealed authority. This remains useful, but should
  avoid making report vocabulary sound like History authority.

### Close Candidate

- line 1, `prototype1-report-detail-mode`: close candidate as duplicate of
  line 13, `report-compact-default-details-flag`, index `3.2`.

- line 2, `prototype1-inflight-transition-test`: close candidate as duplicate
  of line 18, `failure-record-mixed-inflight-tests`, index `4.3`.

- line 4, `history-identify-live-handoff-boundary`, index `1.1`: close
  candidate. The live boundary is now visible in
  `spawn_and_handoff_prototype1_successor`: prepare/install selected Artifact,
  build successor, seal/append History block, then spawn successor.

- line 5, `history-seal-through-crown`, index `1.2`: close candidate. Sealing
  is routed through `Parent<Selectable>::seal_block_with_artifact`,
  `Crown<Ruling> -> Crown<Locked>`, and `Crown<Locked>::seal`.

- line 6, `history-successor-verifies-block`, index `1.3`: close candidate or
  supersede with the narrower current tasks. Successor startup now loads and
  verifies the sealed head and checks current Artifact tree and surface before
  entering the parent path.

- line 21, `history-sealed-block-successor-admission`, index `1.6`: close
  candidate for the single-ruler-local form. Predecessor handoff appends the
  sealed block before spawn, and successor startup verifies the current sealed
  head. Remaining transport identity/ready-file concerns belong to line 20.

- line 22, `history-tree-key-successor-admission`, index `1.7`: close
  candidate. The successor path now derives the clean tree key from the active
  checkout and checks it against the sealed History head. Keep the conceptual
  correction: this gates entry for the lineage; it does not make tree keys
  globally exclusive across lineages.

- line 34, `history-store-head-transition-proof`, index `1.18`: close candidate
  as stale duplicate. A later entry with the same id at line 36 is already
  closed, and current `append` consumes an expected `LineageState`, compares the
  current state, verifies the sparse proof, and checks genesis/predecessor
  append rules.

### Defer

- line 7, `history-consolidation-surface`, index `2`: defer as a broad group
  until the single-ruler blockers are cleared. Keep its children available as
  next work, but full consolidation can wait.

- line 11, `report-footprint-reduction`, index `3`, line 12,
  `report-shared-evidence-inventory`, index `3.1`, line 13,
  `report-compact-default-details-flag`, index `3.2`: defer until the run is
  semantically reliable. Operator ergonomics matter, but not ahead of authority
  and failure-record correctness.

- line 15, `uniform-failure-recording`, index `4`: keep as umbrella but do not
  make the group itself the focus. Work the concrete children in order.

- line 29, `metrics-selection-source-naming`, index `3.4`: defer until
  report/metrics cleanup. This is a naming/claim-boundary issue, not a blocker
  for long single-ruler execution.

- line 35, `history-locator-artifact-tree-context`, index `1.19`: can wait
  until after the manifest/transaction slice unless new evidence shows locator
  misuse in the live handoff path.

## 3. Tentative Priority-Ordered Next Items

1. Audit `history-state-map-adapter-audit` (`1.22`, line 39) against the
   documented local claims: sparse proof correctness, absence/presence wording,
   append root checks, and no consensus/process-uniqueness overclaim.

2. Fix or specify `history-ready-ack-validation` (`1.5`, line 20): validate
   ready record campaign/node/runtime/pid/path against the exact invocation and
   use exclusive creation semantics where practical.

3. Run `failure-record-audit-before-after-gaps` (`4.2`, line 17) over
   materialize, build, spawn, observe, evaluate, select, handoff, and successor
   startup.

4. Implement the smallest outcome/evidence carrier for
   `failure-record-transition-outcomes` (`4.1`, line 16), then add the mixed
   in-flight tests from `failure-record-mixed-inflight-tests` (`4.3`, line 18).

5. Tighten `history-open-block-admission-authority-gate` (`1.17`, line 33) so
   actor/ruler identity is supplied structurally rather than by caller data.

6. Rework `history-crown-lock-carries-seal-material` (`1.16`, line 32) into a
   concrete handoff transition/material carrier that replaces the compatibility
   `SealBlock::from_handoff` seam.

7. Reframe and finish `history-genesis-authority-block` (`1.8`, line 23) as a
   uniform local startup/admission carrier for configured lineage absence, not
   as an Artifact uniqueness rule.

8. Narrow and continue `history-block-transaction-manifest-slice` (`1.10`, line
   25) plus `history-v2-minimal-type-slice` (`1.13`, line 28): artifact
   manifests, admitted relations/transactions, evidence refs, and head-state
   placeholders after removing already-landed surface/state-map work from the
   mental backlog.

9. Start the minimal consolidation pair: `history-live-entry-append-minimal`
   (`2.1`, line 8) and `history-mark-mutable-records-as-projections` (`2.2`,
   line 9).

10. Clean terminology drift in `history-successor-doc-semantics-audit` (`1.4`,
    line 19), `history-key-commitment-naming-review` (`1.9`, line 24), and
    `history-process-seam-structural-naming` (`1.23`, line 40).

## 4. Notes On Terminology/Naming Drift

- The stack has stale parent ids: lines 39 and 40 use
  `parent_id:"history-live-authority"`, while the open group is
  `history-live-crown-authority`. This is an indexing hygiene issue, not a
  semantic blocker.

- `history-live-crown-authority` is now too broad and partly historical. The
  live Crown-gated path exists; remaining work is hardening and removing
  compatibility seams.

- `Successor<Admitted>` should not be propagated as a literal state unless the
  implementation grows that carrier. Current docs point toward incoming Runtime
  plus verified sealed-head handoff into the parent path.

- `TreeKeyHash` and `TreeKeyCommitment` need careful language. They should read
  as Artifact/backend-tree commitments used in a lineage admission check, not as
  globally exclusive Artifact ownership.

- `selected_successor` is transport/debug identity in the sealed header unless
  and until the authority rule is modeled as `MayEnterRuling(H, L, P, R_i)`.
  The authority coordinate is the History lineage, under the current
  policy-bearing runtime surface and sealed Artifact/surface commitments.

- Report and metrics fields such as `selection_authority`, `block_height`, and
  `lineage_id` should distinguish projection labels from sealed History
  authority. A report can mention lineage-shaped data without claiming Crown
  authority.

- Long process-seam names like `validate_prototype1_successor_continuation` and
  `spawn_and_handoff_prototype1_successor` are symptoms of a missing
  handoff/startup context or module. Do this refactor after the remaining
  readiness blockers, so naming follows the actual structure rather than
  anticipating it.
