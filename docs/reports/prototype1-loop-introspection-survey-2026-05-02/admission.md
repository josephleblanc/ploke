# Admission

## Domain

Startup validation, predecessor sealed head, current clean tree key, surface commitment, bootstrap absence, and History admission boundaries.

## Questions To Answer

Short 5-10 generation run questions:

- Did this runtime enter as `Startup<Genesis>` or `Startup<Predecessor>`?
- Which lineage/store state was observed at startup: absent head, or present sealed head with block height/hash?
- For predecessor startup, what clean tree key did the active checkout produce, and did it match the artifact claim in the sealed head?
- For predecessor startup, what surface commitment did the active checkout produce, and did immutable/mutated/ambient roots match the sealed head?
- For genesis startup, what exact store-scoped absence observation justified bootstrap, and was the parent generation actually 0?
- Which parent identity, node id, generation, invocation path, and active checkout root were validated before `Parent<Ready>`?
- If startup rejected, which gate failed: parent checkout identity, invocation/campaign/node/root check, continuation decision, missing sealed head, tree mismatch, surface mismatch, dirty tree, or stale store head?
- Was the sealed block append completed before successor spawn, and which block hash/height became the new head?

Longer-run questions:

- Are all admitted predecessor startups advancing from the same lineage coordinate rather than reconstructing lineage from generation, branch, path, pid, or runtime id?
- Are bootstrap starts limited to a configured-store absence proof, or are later runtimes accidentally falling back to genesis?
- Are surface roots stable across ordinary succession, and are any policy-surface changes being rejected as protocol-upgrade candidates rather than admitted as normal children?
- Are repeated startup failures clustered around stale store state, dirty checkout state, missing heads, or surface/tree mismatches?
- Can an operator reconstruct the sequence `sealed head -> validated successor startup -> next block opened` without reading multiple mutable projections by hand?

## Already Answered By Persisted Data

- Sealed History blocks persist the block header material needed for admission checks: lineage id, block height, parent block hashes, `opened_from_state`, opening authority, ruling/opening actors, procedure label, surface commitment, selected successor, active artifact, artifact claims, sealed time, entry root, and block hash.
- The filesystem block store persists sealed blocks and rebuildable indexes under `prototype1/history`, including the segment, by-hash index, by-lineage-height index, and `heads.json` projection. `append` verifies the expected `LineageState` before advancing the head.
- `LineageState` records the current local state root plus an absent/present `StoreHead` proof. `StoreHead::Absent` is explicitly store-scoped local absence, not global absence.
- Predecessor startup now loads the sealed head block, derives the active checkout clean tree key, verifies it against the sealed artifact claim, recomputes the current surface, and verifies it against the sealed surface commitment before returning `Startup<Validated>`.
- Genesis startup now reads the configured History store and rejects if generation is not 0 or if any head is present for the lineage.
- Parent readiness is gated by a private `Startup<Validated>` carrier whose fields cannot be fabricated from invocation JSON alone; `Parent<Checked>::ready` revalidates the startup carrier against the current parent identity.
- Existing non-History records already persist partial evidence around this area: parent identity files, successor invocation/ready/completion records, `transition-journal.jsonl`, `ActiveCheckoutAdvanced`, `Successor`, and `SuccessorHandoff` entries. These are evidence/projections, not History authority.

## Partially Derivable

- A successful predecessor admission can be inferred from a successor invocation followed by `record_prototype1_successor_ready` and entry into the typed parent path, but no single persisted operational record says "startup admission validated" with the observed head, tree key, surface roots, and result.
- A successful genesis admission can be inferred when a no-handoff startup reaches `Parent<Ready>` with generation 0 and no configured store head, but the absence observation is not logged as its own operational event.
- The expected tree key is committed inside the sealed block artifact claim, and the observed tree key is computed at startup, but the observed key is not currently persisted unless admission proceeds into a later sealed block/handoff path.
- The expected surface is committed in the sealed block and the observed surface is recomputed at startup. On success this proves enough for the transition, but operators do not get a compact diff-friendly event containing the compared roots.
- Store-head continuity is enforced by `FsBlockStore::append(expected, block)`, but startup observation and append observation are separated by code path and store projection. A long-run operator can inspect blocks and heads, but cannot easily see "this runtime admitted from head H under root R".
- Current monitor/report/preview surfaces expose useful evidence classes and source refs, but they are read-only projections over legacy JSON and can still look History-shaped without being authority.

## Requires New Logging

- Startup admission attempt/result: one record per runtime startup, emitted after the genesis/predecessor admission path succeeds or rejects.
- Admission inputs: `campaign_id`, `lineage_id`, `parent_id`, `node_id`, `generation`, optional `runtime_id`, `startup_kind`, `active_parent_root`, optional `handoff_invocation_path`, and parent identity evidence ref.
- Store observation: local `history_store_root`, `store_head_kind`, optional predecessor `block_hash` and `block_height`, and an absence/proof digest or source ref when the head is absent.
- Artifact check: observed clean tree key hash, expected tree key hash when present, dirty-check result, and the source of the expected claim.
- Surface check: observed immutable root, observed mutated-after root, observed ambient-after root, expected roots when present, and mismatch classification rather than full file lists.
- Result: `validated`, `rejected`, or `error`, with `error_phase`, structured reason, and source path refs. This should be telemetry only; it must not claim to admit History.
- Bootstrap gap marker: for genesis, record that the authority basis was configured-store absence plus bootstrap policy, not a predecessor sealed head.
- Handoff sealing result: the existing `debug!` log says a block was sealed before spawn, but a uniform JSONL event should carry stored block height/hash, lineage id, store root opened from, successor artifact ref, and whether append advanced the expected head.

## Natural Recording Surface

- Best surface: a small shared startup-admission helper used by `Startup::<Genesis>::from_history` and `Startup::<Predecessor>::from_history`, with `.log_result()` at the transition boundary that creates `Startup<Validated>` or rejects. This keeps operational telemetry attached to the typed startup transition rather than to CLI parsing or backend internals.
- The predecessor branch should log after `sealed.verify_current_artifact_tree` and `sealed.verify_current_surface`, because that is where the sealed head, current clean tree key, and current surface commitment meet.
- The genesis branch should log after `store.lineage_state` and generation/head checks, because that is where bootstrap absence is validated.
- Handoff sealing should log beside `FsBlockStore::append(&expected_state, &sealed_block)` in `spawn_and_handoff_prototype1_successor`, since that is the natural point where an open block becomes the new local sealed head before successor spawn.
- Shape: one uniform tracing-backed JSONL operational event, e.g. `prototype1.operational`, with an `event_kind` such as `startup_admission` or `history_block_append`, optional admission-specific fields, and explicit `authority_status: "telemetry_not_history"`.
- Avoid adding per-domain files. These events should be another view over the shared operational stream, not a new `admission.json` or a second History store.

## Essential

- Startup result and rejection reason, including the exact failed gate.
- Startup kind: genesis or predecessor.
- Lineage id, parent identity ids, generation, active checkout root, and optional runtime/invocation refs.
- Store observation: head absent/present, block hash/height, and state root or proof digest/source ref.
- For predecessor startup: observed and expected clean tree key hash, observed and expected surface roots, and boolean/mismatch outcome.
- For genesis startup: explicit local configured-store absence and generation-0 check.
- Handoff append result: sealed block hash/height and expected-state/root used to advance the head.
- A clear telemetry boundary field so no consumer treats the operational JSONL event as sealed History authority.

## Nice To Have

- Duration of each admission sub-step: store read, sealed block load, tree-key derivation, surface computation, and append.
- Source path refs for the sealed block segment/index line, `heads.json`, parent identity, invocation, ready record, and transition journal line.
- Dirty checkout path count and a bounded sample of dirty paths on rejection.
- Surface partition version and pathspec names, without logging every file hash by default.
- Backend kind and git object algorithm, since `GitTreeKey` can represent SHA-1 or SHA-256 object ids.
- A run/session correlation id tying startup admission, successor spawn, ready, and completion events together.

## Too Granular Or Noisy

- Per-file surface hash rows for every admitted startup. Keep aggregate roots in the event; emit file-level data only for a mismatch investigation.
- Full serialized sealed blocks in the operational event. Store block hash/location refs instead.
- Full parent identity or invocation JSON copied into every event. Store source refs and selected ids.
- Poll-loop status for successor ready checks unless the final timeout/ready/exit outcome changes.
- Repeating scheduler, branch registry, or node JSON snapshots inside admission events. Those are projections and already have their own evidence surfaces.
- Raw stdout/stderr excerpts for successful admission. Keep stream refs.

## Source Notes

- `crates/ploke-eval/src/cli/prototype1_state/mod.rs:62` defines History as sealed lineage-local authority, not scheduler/report/transport records; `:85` states the intended startup admission check; `:93` defines bootstrap as configured-store absence; `:96` and `:106` record current tree/surface startup checks.
- `crates/ploke-eval/src/cli/prototype1_state/history.rs:75` states the intended startup sequence; `:103` narrows genesis absence to local configured-store absence; `:140` says the live successor handoff gate checks current clean Artifact tree, but does not prove OS-process uniqueness; `:221` introduces `SurfaceCommitment`.
- `crates/ploke-eval/src/cli/prototype1_state/history.rs:331` documents remaining blocked wiring: successor startup verifies tree/surface, bootstrap still lacks a uniform admission carrier, gen0 setup does not append a genesis block until first handoff.
- `crates/ploke-eval/src/cli/prototype1_state/history.rs:587` defines `BlockStore`; `:611` makes append consume expected `LineageState`; `:831` verifies hash, current state, and append validity before writing block/index/head projections.
- `crates/ploke-eval/src/cli/prototype1_state/history.rs:1152` defines `LineageState`; `:1199` defines `StoreHead` and emphasizes local absence; `:1455` defines partitioned `SurfaceCommitment`; `:1496` verifies current surface roots.
- `crates/ploke-eval/src/cli/prototype1_state/history.rs:2053` defines `GenesisAuthority`; `:2086` defines `PredecessorAuthority`; `:2408` documents open-block invariants and the lack of a uniform typed startup/admission carrier; `:2652` lists sealed block header material.
- `crates/ploke-eval/src/cli/prototype1_state/history.rs:2950` verifies the current Artifact tree against the sealed head; `:2983` verifies current surface against sealed roots.
- `crates/ploke-eval/src/cli/prototype1_state/parent.rs:72` documents private `Startup<Validated>` as the local single-ruler startup gate; `:442` implements genesis-from-history; `:481` implements predecessor-from-history; `:553` revalidates the startup carrier before `Parent<Ready>`.
- `crates/ploke-eval/src/cli/prototype1_state/cli_facing.rs:3073` chooses genesis startup when no handoff invocation is present and predecessor startup when a successor invocation is present; `:3134` validates continuation before predecessor History startup.
- `crates/ploke-eval/src/cli/prototype1_process.rs:930` seals/appends the handoff block before spawning the successor; `:1125` builds block fields from the current store state, current clean tree key, opening authority, and surface commitment.
- `crates/ploke-eval/src/cli/prototype1_state/backend.rs:438` identifies `clean_tree_key` as the operation History admission should rely on; `:1185` rejects dirty checkouts and derives `HEAD^{tree}`; `:446` defines the surface commitment boundary; `:1217` computes immutable/mutated/ambient roots.
- `docs/reports/prototype1-record-audit/history-admission-map.md:27` classifies existing records as evidence/projections; `:79` lists weak fields, some now partly addressed by current code; `:173` preserves the warning that preview/report records do not become authoritative History.
- `docs/reports/prototype1-record-audit/2026-04-29-history-crown-introspection-audit.md:47` warns about provisional History-shaped preview fields; `:114` lists earlier authority/provenance gaps. The audit predates current successor startup tree/surface verification, so use it as cautionary context, not as the latest implementation state.
- `docs/reports/prototype1-record-audit/2026-04-29-monitor-report-coverage-audit.md:7` labels monitor report output as projection only; `:20` and `:31` list evidence families omitted or weakly surfaced by reports.
