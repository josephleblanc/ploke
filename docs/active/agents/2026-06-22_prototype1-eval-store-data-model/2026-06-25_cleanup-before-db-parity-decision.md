# 2026-06-25 — Eval-store cleanup before DB parity

Status: active decision / restart guardrail.

Short description: decision to pause new Prototype 1 eval-store schema expansion and first clean up the persistence boundary after the `p1-dbdual-broad3g1x3-pplxembed-g25p-p25f-20260623-174553` audit showed partial DB/file parity and confused backend semantics.

Related files:

- [`README.md`](README.md)
- [`slice-by-slice-implementation-plan.md`](slice-by-slice-implementation-plan.md)
- [`implementation-log.md`](implementation-log.md)
- [`slice-11-db-only-readiness-audit.md`](slice-11-db-only-readiness-audit.md)
- [`transition-persistence-dependency-matrix.md`](transition-persistence-dependency-matrix.md)
- [`relational-data-model.md`](relational-data-model.md)

## Decision

Before adding more ordinary-eval database schema, stop and clean up the storage boundary.

The current implementation is useful as an owner-scoped eval DB mirror, but it is not yet a coherent file-backed / DB-backed / dual-strict backend abstraction across the loop. Later work must not interpret the completed Slices 1-11 as DB parity, DB-only readiness, or a reason to keep adding one-off DTO/write helpers.

## Clarifications that supersede ambiguous earlier wording

1. **`database` currently means DB mirror, not DB-only runtime.**
   Parent-start still writes JSONL first because DB ids depend on source receipts. Many later writes only mirror when `prototype1/eval-store.cozo.sqlite` already exists. Treat this as transitional `db-mirror` behavior unless/until production consumers are migrated and tested.

2. **`dual-strict` must mean strict parity for the specific migrated surface.**
   It should fail loudly when an expected mirror write fails after the filesystem authority write. It must not silently degrade into opportunistic mirroring.

3. **The `EvalStore` trait is currently too narrow to describe all implemented writes.**
   It only abstracts `put_parent_started`; most later DB writes use direct `write_*_to_owner_db` helpers. Next cleanup should either extend domain-specific ports intentionally or keep the direct helpers explicitly classified as mirror helpers. Do not claim backend-agnostic storage for a surface that bypasses the configured store.

4. **Compatibility rows are not authority-shaped facts.**
   `eval_record_ref` rows for scheduler nodes, runner requests/results, branch registry, and legacy projections are query/review evidence. They do not replace MessageBox, Channel, History, invocation/bootstrap, artifact/worktree, or transition-journal authority.

5. **Authority is not a reason to keep data out of the DB.**
   It means the DB representation must carry the domain contract. Channel facts should be represented as channel messages/receipts/imports; MessageBox facts as box lock/unlock/message rows; History as block/head/lineage refs or a HistoryStore; artifacts as refs/surfaces/patch/build/install provenance. Generic record refs are insufficient for those gates.

## Remote parent/child model

Do not rely on child VMs mutating a shared parent DB file.

Recommended runtime model:

1. Parent writes invocation/bootstrap and launches or dispatches the child.
2. Child owns local execution evidence, logs, LLM turn files, worktree/artifact state, and any child-local DB/cache.
3. Child reports parent-visible facts over its channel, including terminal result facts and verifiable refs/hashes for large payloads.
4. Parent validates channel payloads and writes parent-owned DB rows:
   - `eval_channel_message`
   - `eval_channel_receipt`
   - `eval_import_event`
   - derived attempt/evaluation/selection/continuation/artifact rows
5. Large or sensitive payloads stay out-of-line by default behind refs/hashes/sensitivity labels.

This model supports different machines without weakening parent-side replay: parent-visible facts enter the parent DB through validated import edges, not through shared paths.

## Immediate cleanup priorities

Do these before adding new schema families:

1. **Fix current parity gaps.**
   - Mirror terminal `ToParent::Result` channel messages.
   - Backfill or fix missing `eval_closure_ref` for dual-strict setup.
   - Populate `eval_log_ref` for runtime streams and turn-live/LLM sidecars when DB mirroring is enabled.

2. **Clarify backend naming/semantics.**
   - Keep existing TOML compatibility for `backend = "database"`, but document/code-comment that current behavior is mirror mode until read migration is complete.
   - Do not advertise true DB-only operation.

3. **Separate ports by domain.**
   - Ordinary eval evidence can use `EvalStore` or explicit eval mirror helpers.
   - Channel, MessageBox, History, invocation/bootstrap, artifacts, and logs need domain-aware ports or row models.

4. **Model agent turns as linked messages/events, not opaque LLM blobs.**
   - Persist ordered turn events, model exchanges, messages, tool calls/results, edit/proposal/apply events, and cost/usage rows.
   - Keep raw provider responses as optional log/blob refs with hashes and sensitivity labels.

5. **Use audit/checkpoint evidence as gates.**
   - `ploke-eval loop walk audit --format json --verbose --with-note` should move from broad partial counts toward semantic per-surface checks.
   - Checkpoint/authority-negative tests remain required before any DB-backed read claim.

## Non-goals for the cleanup slice

- Do not delete existing file authority surfaces.
- Do not make DB rows authoritative for protected domains by relaxing validation.
- Do not inline the whole codebase, worktrees, binaries, or full LLM provider traces in Cozo.
- Do not rename public profile values in a way that breaks existing run profiles without a migration/alias plan.

## Restart note

If later documentation appears to imply that Slices 1-11 already achieved DB parity, prefer this decision. The correct current state is: owner-scoped eval DB mirror rows exist for many surfaces; DB-only runtime is not enabled; and the next work is cleanup plus targeted parity fixes, not more unconstrained schema expansion.
