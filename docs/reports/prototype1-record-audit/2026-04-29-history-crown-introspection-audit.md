# Prototype 1 History/Crown Introspection Audit

Date: 2026-04-29

Scope: current Prototype 1 persisted-record and introspection surfaces that present History-shaped data: `history.rs`, `history_preview.rs`, `metrics.rs`, `report.rs`, and the CLI aliases that expose them. This audit did not inspect unrelated live controller internals except where needed to evaluate report/preview/metrics authority claims.

## Executive Finding

The current implementation mostly preserves the documented claim boundary in prose and source labels: History is not yet live Crown authority, and the preview, report, and metrics surfaces are explicitly read-only projections over pre-History records. That is correct.

The remaining architectural risk is presentation drift. Several introspection surfaces expose History-shaped terms (`block_height`, `lineage_id`, `selected_by_generation`, `selection_authority`, `JournalView`, `EvaluationView`) while deriving them from mutable scheduler/registry/node records, legacy transition journal entries, or heuristic dashboard scoring. The code generally labels these as degraded/provisional, but the JSON/table shapes still make it easy for future code or operators to consume them as if they were sealed History or Crown decisions.

## Intended Model Baseline

The design documents define the stronger object as:

- `History = chain of sealed Blocks`.
- `Block = authority epoch for one active lineage`.
- `Entry = provenance-bearing fact inside an epoch`.
- `Ingress = append-only late/backchannel observations outside the sealed epoch`.
- `Projection = disposable view or index derived from History`.

The Crown boundary is the important authority cut: `Parent<Ruling>` records entries, locks `Crown<Locked>`, sealing `Block<Sealed>`; `Successor<Admitted>` verifies the sealed block before becoming the next `Parent<Ruling>` (`docs/workflow/evalnomicon/drafts/history-blocks-and-crown-authority.md:36`). The type-safety claim is deliberately narrow and requires private or sealed state markers, move-only transitions, records emitted as transition projections, and validation at authoritative transition boundaries (`docs/workflow/evalnomicon/drafts/history-blocks-and-crown-authority.md:108`).

## What The Code Currently Enforces

`history.rs` is an invariant core, not live authority. Its module docs correctly state that live handoff still uses transition journals, invocation files, ready files, and mutable scheduler/branch projections, and that those are not sealed History blocks (`crates/ploke-eval/src/cli/prototype1_state/history.rs:6`). It also records that `Parent<Ruling>`, `Crown<Locked>`, and `Successor<Admitted>` are not live gates and that successor validation still consults mutable state rather than a sealed block hash (`history.rs:71`).

Inside the core, `Block<block::Open>`, `Block<block::Sealed>`, `Entry<Draft/Observed/Proposed/Admitted>`, and `Ingress<Open/Imported>` preserve useful typestate structure. Entry admission stores distinct observer, recorder, proposer, admitting authority, ruling authority, lineage, block id, and block height (`history.rs:721`). Block sealing commits entry hashes, entries root, selected successor, active artifact, Crown lock transition reference, and block hash (`history.rs:763`). Sealed block verification recomputes the entries root and block hash (`history.rs:830`).

The key gap is also documented in code: `SealBlock` commits a `crown_lock_transition` reference but does not require a live `Crown<Locked>` authority carrier (`history.rs:620`). `Block<Open>::open` and `Block<Open>::seal` are crate-visible constructors/transitions, so within the crate they can still be used as direct History construction APIs rather than as projections of live Crown transitions. This is acceptable for the current self-contained core only because it is not wired into live handoff.

## Introspection Surfaces

### `history preview`

`history_preview.rs` correctly declares that it does not write sealed History blocks and only emits a History-shaped preview before live Crown handoff wiring exists (`crates/ploke-eval/src/cli/prototype1_state/history_preview.rs:1`). It uses a narrow `EvidenceStore`, hashes source records, and imports known evidence classes from transition journal lines and adjacent JSON files (`history_preview.rs:31`).

Preserved invariants:

- Every preview entry carries a source ref and payload hash.
- Journal entries are marked `degraded_pre_history` and explicitly described as preview projections, not admitted History entries (`history_preview.rs:683`).
- Evaluation, invocation, attempt result, latest runner result, successor ready/completion, runner request, and node record documents each carry degraded/projection authority statuses where appropriate (`history_preview.rs:1210`, `history_preview.rs:1242`, `history_preview.rs:1281`, `history_preview.rs:1331`, `history_preview.rs:1352`).
- Scheduler and branch registry documents are deferred rather than admitted as entries; the deferred reasons name them as mutable projection/catalog records (`history_preview.rs:1513`).
- Provisional blocks are explicitly marked `provisional_unsealed` with notes that they group records by observed generation and are not sealed by `Crown<Locked>` (`history_preview.rs:1543`).

Risks:

- `block_height` is derived from `generation.unwrap_or_default()` for both journal and document entries (`history_preview.rs:693`, `history_preview.rs:1074`). The admission map explicitly says generation should be block height only where it matches a Crown epoch and should not be assumed equivalent after branching or merge (`docs/reports/prototype1-record-audit/history-admission-map.md:52`). Defaulting unknown generation to block height `0` creates a false genesis-shaped bucket for records whose epoch is unknown.
- `PreviewBlock` exposes `lineage_id: "prototype1-preview-lineage"` (`history_preview.rs:1565`). The note says provisional, but the field name is the same semantic coordinate expected in real sealed History. This is a presentation hazard for downstream JSON consumers.
- The bounded table printer highlights `block_height` and hides the nested authority status/notes for entry slices (`cli_facing.rs:1637`). That makes the quick inspection path less authority-honest than the full JSON.
- Successor ready/completion are marked `degraded_pre_history_or_ingress`, which is correct, but without a live Crown lock boundary the preview cannot decide whether they belong in a sealed epoch or ingress. This should remain a gap, not an inferred History fact.

### `history metrics`

`metrics.rs` correctly declares itself a read-only metrics projection and says it does not strengthen mutable scheduler, registry, or node-local authority (`crates/ploke-eval/src/cli/prototype1_state/metrics.rs:1`). The command output is labeled `prototype1 metrics projection`, and the dashboard has a derivation id (`metrics.rs:22`).

Preserved invariants:

- Selection source is tracked as either `transition_journal` or `mutable_projection`; mutable scheduler/registry selections are not silently upgraded (`metrics.rs:1129`, `metrics.rs:1828`).
- Trajectory state records degraded lineage and ambiguity/incompleteness instead of pretending the lineage coordinate exists (`metrics.rs:542`).
- Cohorts group by `(parent_node_id, generation)` with lineage explicitly unavailable (`metrics.rs:1261`).
- Dashboard scoring is implemented as a local heuristic projection, not as policy truth (`metrics.rs:339`).

Risks:

- `selected_by_generation`, generation summaries, and `selection_authority` are compelling names. They are useful projections, but they can be consumed as the historical selection policy. This is especially risky because `strongest_selection_authority` prefers any transition journal source over mutable projection (`metrics.rs:1828`), while transition journal evidence is still degraded until sealed by real History.
- `collect_selected_branches` recursively treats any object containing `selected_branch_id` and `branch_id` as selection evidence. That is intentionally broad for current records, but it makes projection selection depend on JSON field names rather than a typed decision record.
- The dashboard score has a derivation id, but no source digest bundle for the full input set. Rows carry source refs, yet the dashboard as a whole is not a verifiable projection artifact.

### `loop prototype1-monitor report`

`report.rs` is correctly documented as a provisional aggregate, not a sealed History value (`crates/ploke-eval/src/cli/prototype1_state/report.rs:1`). It also reports major weak fields directly, including missing `sealed_by`, missing real `Crown<Locked>` authority, missing predecessor block verification, distributed tool/model/prompt evidence, and inconsistent metrics derivation/source digests (`report.rs:107`).

Risks:

- `JournalView` and `EvaluationView` are explicitly called out by the admission map as names to avoid in the next implementation pass (`docs/reports/prototype1-record-audit/history-admission-map.md:236`). They remain in the report implementation (`report.rs:304`, `report.rs:357`). Since this is still a read-only report, this is not a correctness break, but it is structural naming debt.
- `JournalView` counts flattened legacy variants such as `ChildArtifactCommitted`, `SuccessorHandoff`, and stringified journal kinds (`report.rs:313`, `report.rs:718`). The History module explicitly says these should stay storage labels and be normalized into role/state facts before becoming History ontology (`history.rs:103`).
- `selected_trajectory` starts from scheduler `last_continuation_decision`, otherwise falls back to the max-generation node (`report.rs:656`). That is acceptable as a report heuristic, but it must not be treated as a Crown-selected lineage.

### CLI aliases and monitor output

The `history` command exposes only `metrics` and `preview`, and both subcommand docs say read-only/projection (`crates/ploke-eval/src/cli.rs:465`). The older monitor aliases route to the same implementations (`cli_facing.rs:1458`). This is structurally reasonable.

The monitor watch/journal output still prints flattened labels such as `child_artifact_committed`, `active_checkout_advanced`, `successor_handoff`, `child_ready`, and `observe_child:*` (`cli_facing.rs:2168`, `cli_facing.rs:2273`). That is acceptable for legacy monitoring, but these labels should not leak into History entry kinds or future typed transition APIs.

## Correctness Of Current Claims

Claims that are currently correct:

- Current live records are not sealed History blocks.
- Current preview/report/metrics surfaces are read-only projections or degraded evidence.
- The `history.rs` typestate core can represent admitted entries, sealed blocks, ingress imports, block hashes, and successor block opening, but is not live handoff authority.
- The current local claim is tamper-evident only for constructed `Block<Sealed>` values, not for all Prototype 1 persisted JSON.

Claims that would be too strong today:

- That `history preview` block heights are Crown epochs.
- That preview `lineage_id` identifies a real active lineage.
- That `transition_journal` selection authority is equivalent to sealed Crown authority.
- That successor ready/completion files can be classified without a Crown lock boundary.
- That dashboard rank/score reflects the Parent selection policy.
- That any CLI report or monitor output is authoritative History.

## Structural Naming Risks

The strongest naming risks are not just long identifiers; they are flattened role/state records masquerading as durable ontology:

- Legacy journal variants: `ChildArtifactCommittedEntry`, `ActiveCheckoutAdvancedEntry`, `SuccessorHandoffEntry`, `ChildReady`, and `ObserveChild` remain visible in reports and monitor output. They should remain storage compatibility labels until normalized as `Artifact<Committed>`, `Checkout<Advanced>`, `Successor<Ready>`, `Child<Ready>`, or equivalent typed carriers.
- Generic view names: `JournalView` and `EvaluationView` carry source-specific aggregates rather than the algebraic projection vocabulary already documented for the next pass.
- Authority labels: `transition_journal` is a source class, not Crown authority. In metrics output, `selection_authority` should eventually distinguish source strength from authority role, e.g. source evidence versus admitted decision authority.
- Preview blocks: `PreviewBlock.lineage_id` and `PreviewEntry.block_height` reuse real History field names for provisional grouping. That preserves shape but also imports too much semantic confidence into the projection.

## Authority And Provenance Gaps

1. Live authority carriers are still absent from the handoff gate. `history.rs` records this gap, and report output repeats it.
2. Block sealing records a Crown lock transition reference, not proof that a live `Crown<Locked>` existed.
3. `sealed_by` / committer actor is still missing from the sealed header model and from current records.
4. Successor admission is not gated on verifying a sealed block hash.
5. Preview grouping collapses unknown generation into block height `0`.
6. Metrics can identify source refs for row-level facts but does not produce a verifiable dashboard input digest/root.
7. CLI table slices for preview entries omit authority status and notes even when showing block height.
8. Current records still lack stable lineage ids, so trajectory and cohort projections are necessarily degraded.
9. Operational environment remains partial for preview entries; tool/model/prompt/full-response evidence is still distributed.
10. Late/backchannel successor evidence cannot be separated from in-epoch evidence until Crown lock timing is represented in records.

## Recommendations

1. Keep the current documentation claim boundary. Do not upgrade the wording beyond "read-only projection/degraded evidence" until live `Parent<Ruling> -> Crown<Locked> -> Block<Sealed> -> Successor<Admitted>` gates exist.
2. Rename or wrap preview JSON fields that are synthetic, or add explicit sibling fields: `provisional_block_height`, `generation_bucket`, and `preview_lineage_id`. Avoid presenting them as real `block_height` / `lineage_id` without the authority qualifier.
3. Do not default unknown generation to block height `0`. Use `Option<u64>` for preview grouping, or a separate `unknown_epoch` bucket.
4. Include authority status and notes in bounded table output for `history preview --entry` and `--entries`.
5. Split metrics `selection_authority` into `selection_source_strength` and `admitted_decision_authority` before any downstream consumer treats it as policy truth.
6. Replace broad recursive `selected_branch_id` discovery with typed extraction by known record class where possible. Keep a diagnostic for field-name-only selection observations.
7. Add a dashboard-level source digest/root for metrics projections, even before live History admission, so projection artifacts can be cited repeatably.
8. Treat `JournalView` / `EvaluationView` as temporary report implementation names. The next pass should use the documented algebraic carriers: source, projection, kernel, provenance, verification, and pullback.
9. Keep legacy flattened journal labels at monitor/report boundaries only. Normalize them before History admission and avoid making them `EntryKind` or transition-state names.
10. When live History writing begins, make `Block<Open>::seal` callable only through the Crown lock transition surface, or keep the direct method private behind a module that owns `Crown<Locked>`.

## Bottom Line

The current report/history-preview/metrics surfaces do not appear to falsely write or claim sealed History. They do, however, expose provisional History-shaped coordinates in ways that can be mistaken for Crown-backed facts. The next correctness move is not another report format; it is tightening the boundary between projection vocabulary and authority vocabulary before other code starts depending on preview JSON as if it were the History substrate.
