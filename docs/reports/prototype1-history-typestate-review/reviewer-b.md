# Prototype 1 History / Crown / Authority Review - Reviewer B

Date: 2026-04-27

## Executive Verdict

The current implementation is not yet a tamper-evident, block-sealed History system. It has useful typed scaffolding for child-plan messages, child runtime transitions, successor records, append-mode JSONL, and git-backed parent checkout validation, but it does not implement `Block<Sealed>`, a concrete `Crown<Locked>` transition, `Successor<Admitted>`, ingress import policy, or successor verification of a sealed authority block.

The safest current claim is narrower: Prototype 1 has partial typed transition records and some active-checkout validation. It does not yet provide block sealing, end-to-end replay of authority epochs, durable append-only guarantees beyond ordinary local JSONL append, or type-state prevention of forged persisted successor authority.

## Claim-By-Claim Assessment

| Claim | Assessment |
| --- | --- |
| History is a chain of sealed Blocks | Not implemented. The docs define `History = chain of sealed Blocks` and `Block = authority epoch` in `docs/workflow/evalnomicon/drafts/history-blocks-and-crown-authority.md:25`, but code search finds no `Block<Open>`, `Block<Sealed>`, block header, entries root, or block hash implementation. |
| `Crown<Locked>` seals the prior epoch and gives the successor a validation target | Not implemented. `inner::Crown` and `inner::LockBox` exist only as unused carriers at `crates/ploke-eval/src/cli/prototype1_state/inner.rs:42` and `crates/ploke-eval/src/cli/prototype1_state/inner.rs:54`. The module docs explicitly admit the live handoff still uses invocation and ready files at `crates/ploke-eval/src/cli/prototype1_state/mod.rs:164`. |
| Successor verifies sealed History before becoming `Parent<Ruling>` | Not implemented. Successor acknowledgement validates campaign/node/root and scheduler continuation, then records ready at `crates/ploke-eval/src/cli/prototype1_state/cli_facing.rs:2733` and `crates/ploke-eval/src/cli/prototype1_process.rs:369`; no block hash or sealed evidence is verified. |
| Journal is append-only | Partially implemented. `PrototypeJournal::append` opens JSONL with `.append(true)` and `sync_data()` at `crates/ploke-eval/src/cli/prototype1_state/journal.rs:563`, but there is no hash chain, sequence check, seal, lock, exclusive writer, or protection against truncation/rewrite by a local process. |
| Records preserve role accountability | Partial. Some records include runtime id, node id, campaign id, pid, paths, and identities, for example `SuccessorRecord` at `crates/ploke-eval/src/cli/prototype1_state/successor.rs:58`. They do not preserve the full required custody split: proposer, observer, submitter, recorder, admitting authority, committer, and ruler as distinct fields. |
| Ingress handles late/backchannel observations under import policy | Not implemented. The draft requires ingress at `docs/workflow/evalnomicon/drafts/history-blocks-and-crown-authority.md:284`, but code has no ingress type, log, import transition, import policy, or `ingress_prior_block_hash`. Late successor ready/completion records write directly to attempt files and the transition journal. |
| Replay can reconstruct authority correctness | Partial and not authority-complete. Replay covers materialize/build/spawn/observe at `crates/ploke-eval/src/cli/prototype1_state/journal.rs:484`, but ignores `ParentStarted`, `ChildArtifactCommitted`, `ActiveCheckoutAdvanced`, `SuccessorHandoff`, and `Successor` entries in the spawn replay path at `crates/ploke-eval/src/cli/prototype1_state/journal.rs:418`. It cannot validate a Crown epoch. |
| Mutable projections are not authority | Stated, but not enforced. The docs warn that mutable scheduler state is not authority at `crates/ploke-eval/src/cli/prototype1_state/mod.rs:333`, yet live successor admission relies on `scheduler.last_continuation_decision` at `crates/ploke-eval/src/cli/prototype1_process.rs:382`. |
| Type-state prevents forged `Parent<Ruling>`, `Crown<Locked>`, `Block<Sealed>` today | No. `Crown` is private-field but unused, `Block<Sealed>` does not exist, `Parent<Ruling>` does not exist, and successor admission is a JSON invocation plus scheduler validation rather than a typed `Successor<Admitted>` transition. |

## Findings

### Critical: There is no sealed History block or tamper-evident authority epoch

The draft's minimum seal requires deterministic header and entry serialization, payload hashes, entries root, previous block hash, writer/runtime identity, Crown lock transition identity, and a block hash at `docs/workflow/evalnomicon/drafts/history-blocks-and-crown-authority.md:93`. The implementation has none of those fields or transitions. The nearest durable stream is `transition-journal.jsonl`, whose append path writes raw JSON lines without any entry hash, previous hash, block membership, or seal at `crates/ploke-eval/src/cli/prototype1_state/journal.rs:551`.

Exploit/workaround: a local process can rewrite or truncate `transition-journal.jsonl`, `scheduler.json`, an invocation JSON, or a successor-ready JSON and there is no sealed block hash for the successor to recompute. The successor cannot distinguish "this prior epoch selected me" from "mutable local files currently say I was selected."

Minimum fix: introduce real `Block<Open>` and `Block<Sealed>` transition carriers tied to `Parent<Ruling> -> Crown<Locked>`, hash every entry into an ordered root, include previous block hash and selected successor evidence in the header, and make successor admission verify that block before it can become `Parent<Ruling>`.

### Critical: Successor authority is admitted through mutable scheduler state and forgeable JSON, not sealed evidence

Successor validation loads `scheduler.last_continuation_decision` and accepts if it is `ContinueReady` for the node branch at `crates/ploke-eval/src/cli/prototype1_process.rs:376`. That decision is a singleton mutable field at `crates/ploke-eval/src/intervention/scheduler.rs:170`, set by `record_continuation_decision` at `crates/ploke-eval/src/intervention/scheduler.rs:676`, and cleared by node registration paths at `crates/ploke-eval/src/intervention/scheduler.rs:321` and `crates/ploke-eval/src/intervention/scheduler.rs:829`.

The invocation itself is plain JSON loaded and classified by its `role` field at `crates/ploke-eval/src/cli/prototype1_state/invocation.rs:310`; writes use `fs::write` at `crates/ploke-eval/src/cli/prototype1_state/invocation.rs:296`.

Exploit/workaround: create or edit a successor invocation JSON with `role: "successor"`, matching campaign/node/runtime, and a repo root that matches the command root. If mutable scheduler state currently points to the same branch, the path can acknowledge handoff without proving membership in a sealed Crown epoch.

Minimum fix: make the successor invocation cite the sealed block hash, selected successor runtime/artifact, active checkout commit, and policy decision. Treat scheduler continuation as a projection only; admission must consume `Crown<Locked> + Block<Sealed> + SuccessorEvidence`.

### High: Ready-file acknowledgement can be spoofed or misbound

The parent waits for a successor ready path and only parses the file at `crates/ploke-eval/src/cli/prototype1_process.rs:818`. It does not compare the loaded record's campaign, node, runtime id, or pid against the expected invocation before returning `Ready` at `crates/ploke-eval/src/cli/prototype1_process.rs:824`. The ready record is written with `fs::write` at `crates/ploke-eval/src/cli/prototype1_state/invocation.rs:411`.

Exploit/workaround: after the parent spawns the successor and before the successor writes ready, another local process can create syntactically valid JSON at the expected ready path. The parent observes the path and records `SuccessorHandoff` at `crates/ploke-eval/src/cli/prototype1_process.rs:921` even if the actual successor has not unlocked anything.

Minimum fix: validate ready-file contents against the expected `SuccessorInvocation`, require exclusive create for ready/completion files, include the ready event in an append-only ingress or block transition, and do not let a ready path alone unlock succession.

### High: Type-state authority exists mostly as scaffolding, not as the live gate

The docs say the live code should be audited for private fields, constructors, sealed markers, move-only transitions, and durable records at `crates/ploke-eval/src/cli/prototype1_state/mod.rs:396`. The actual live parent states are `Parent<Unchecked>`, `Parent<Checked>`, `Parent<Ready>`, `Parent<Planned>`, and `Parent<Selectable>` at `crates/ploke-eval/src/cli/prototype1_state/parent.rs:24`; there is no `Parent<Ruling>`. `authority.rs` has promising carriers such as `Bootstrap<Branch>::become_parent` at `crates/ploke-eval/src/cli/prototype1_state/authority.rs:559`, but the file is marked dead-code design sketch at `crates/ploke-eval/src/cli/prototype1_state/authority.rs:1`.

The strongest live structural box is the child-plan message: it binds candidate files to parent identity and generation at `crates/ploke-eval/src/cli/prototype1_state/parent.rs:56`, validates receiver identity at `crates/ploke-eval/src/cli/prototype1_state/parent.rs:128`, and unlocks via `Message::ready_receiver` at `crates/ploke-eval/src/cli/prototype1_state/parent.rs:282`. That does not cover Crown transfer or block sealing.

Exploit/workaround: within the crate, code can construct typed child and successor records through public(crate) constructors without a sealed authority chain. Outside the crate, a local process can forge the persisted JSON files those typed paths trust.

Minimum fix: wire the authority path into live handoff, make authoritative state constructors private to the transition module, remove direct state casts that bypass durable IO, and ensure every authoritative transition emits the durable record as a projection of the transition.

### High: Ingress and backchannel handling are absent

The draft says late child status, process exits, diagnostics, and monitor events during `Crown<Locked>` must go to ingress and later be imported under policy at `docs/workflow/evalnomicon/drafts/history-blocks-and-crown-authority.md:286`. Current successor ready and completion artifacts write directly to attempt files and append successor records at `crates/ploke-eval/src/cli/prototype1_process.rs:306` and `crates/ploke-eval/src/cli/prototype1_process.rs:333`. There is no import disposition, prior block hash, or admitting authority.

Exploit/workaround: a late completion, diagnostic, or process event can be appended as just another successor or child record with no distinction between "inside prior Crown epoch" and "late observation admitted by the next Parent."

Minimum fix: add an ingress stream for observations while `Crown<Locked>`, include payload hash and prior block hash, and require `Parent<Ruling>` to import ingress under a named policy into a later `Block<Open>`.

### Medium: Replay does not validate the authority timeline

`PrototypeJournal::replay_all` only combines materialize, build, spawn, and completion replay at `crates/ploke-eval/src/cli/prototype1_state/journal.rs:484`. The spawn replay explicitly ignores successor and active-checkout authority entries at `crates/ploke-eval/src/cli/prototype1_state/journal.rs:418`. Duplicate detection exists for phases within those limited projections at `crates/ploke-eval/src/cli/prototype1_state/journal.rs:712`, but no replay validates a complete sequence from selected child through active checkout advancement, Crown lock, successor ready, and next `Parent<Ruling>`.

Exploit/workaround: an inconsistent journal can contain a successor ready or active checkout advancement record that replay never considers when deciding whether the authority epoch is coherent.

Minimum fix: replay sealed blocks, not only transition families. Replay should recompute block hashes, validate previous block linkage, enforce one selected successor per Crown epoch, and reject late observations unless imported from ingress.

### Medium: Mutable projections and overwritten files remain authority-adjacent

The docs identify mutable JSON buffers and projections as non-authoritative at `crates/ploke-eval/src/cli/prototype1_state/mod.rs:568`, but live control still depends on `scheduler.json`, node records, runner result files, invocation files, and successor-ready files. The report/trace path is a singleton `prototype1-loop-trace.json` at `crates/ploke-eval/src/cli/prototype1_state/cli_facing.rs:4598`, and successor completion currently records `trace_path: None` on success at `crates/ploke-eval/src/cli/prototype1_state/cli_facing.rs:3257`.

Exploit/workaround: the historical narrative can be split across mutable files where the latest value wins, while the append-only journal lacks enough hashes and custody fields to prove which projection was valid at the authority boundary.

Minimum fix: make mutable files disposable projections from sealed History. Persist per-attempt reports by content hash or stable identity, and make scheduler/node/ready files cite sealed entries rather than acting as authority.

### Medium: Role accountability is incomplete

The draft requires custody fields including subject, procedure/policy, executor, observer, recorder, proposer, ruling authority, admitting authority, input/output refs, timestamps, and payload hash at `docs/workflow/evalnomicon/drafts/history-blocks-and-crown-authority.md:187`. Current entries capture useful operational facts, but `JournalEntry` variants at `crates/ploke-eval/src/cli/prototype1_state/journal.rs:235` do not enforce those role fields uniformly.

Exploit/workaround: a later audit cannot reliably tell whether a fact was proposed by a child, observed by the parent, admitted by the successor, imported from a late channel, or merely recorded by the current process.

Minimum fix: define a common entry envelope for History entries with separate custody roles and payload hash, and require transition-specific payloads to live inside that envelope.

## Positive Controls Worth Preserving

- `Parent<Unchecked>::check` validates active checkout, identity generation, branch, and selected instance before returning `Parent<Checked>` at `crates/ploke-eval/src/cli/prototype1_state/parent.rs:343`.
- Git parent checkout validation checks dirtiness, branch identity, commit message, and parent identity path at `crates/ploke-eval/src/cli/prototype1_state/backend.rs:965`.
- Child runtime state is structurally modeled as `Child<Starting> -> Child<Ready> -> Child<Evaluating> -> Child<ResultWritten>` and transition methods append records at `crates/ploke-eval/src/cli/prototype1_state/child.rs:119`.
- The child-plan box is a real typed message shape and should be the pattern for Crown handoff, not a side channel to work around it.

## Recommended Minimum Fixes

1. Implement the real authority spine: `Parent<Ruling> -> Crown<Locked> -> Block<Sealed>`, then `Crown<Locked> + Block<Sealed> + SuccessorEvidence -> Successor<Admitted> -> Parent<Ruling>`.
2. Seal blocks with deterministic serialization, per-entry payload hashes, ordered entries root, previous block hash, selected successor runtime/artifact, active checkout commit, policy reference, and block hash.
3. Move successor admission off `scheduler.last_continuation_decision`; keep the scheduler as a projection of sealed History.
4. Validate successor ready/completion records against the exact invocation and sealed successor evidence; use exclusive creation for attempt files.
5. Add ingress for late observations during `Crown<Locked>` and require explicit import policy before they affect control flow.
6. Expand replay to verify complete authority epochs and reject unsealed, duplicate, out-of-order, or late-without-import records.
7. Narrow public(crate) constructors and casts so authoritative states are obtained only through move-only transition methods that emit durable records.
