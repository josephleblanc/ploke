# Prototype 1 Single-Ruler Readiness Audit

Date: 2026-05-01

## 1. Verdict

Ready with caveats.

The current code is good enough to resume single-lineage, one-local-ruler loop experiments if the operator treats the claim narrowly: local single-ruler continuity for one configured History store and lineage, not global finality, process uniqueness, or proof of improvement. I did not find a specific code/docs blocker that makes a 1000-generation single-lineage run impossible as-is. I did find concrete long-run risks that should be monitored or patched soon, especially store crash consistency and the temporary successor-selection shortcut.

This verdict follows the 2026-05-01 correction: sharing an Artifact/tree key is not itself an authority conflict. The implemented authority coordinate is the History lineage head, with Artifact/surface checks used as admission evidence, not as global ownership of a branch, worktree, runtime id, or tree key. The module docs say the same: same branch/worktree/Artifact key is not an authority conflict; the conflict is advancing the same History lineage head without the required Crown/lock/consensus/fork-choice rule (`crates/ploke-eval/src/cli/prototype1_state/mod.rs:145`, `crates/ploke-eval/src/cli/prototype1_state/history.rs:193`).

## 2. Implemented Single-Ruler Invariants

- Local claim is explicitly narrow. History docs define the current claim as local, lineage-scoped, transition-checked, and not distributed consensus or OS-process uniqueness (`crates/ploke-eval/src/cli/prototype1_state/history.rs:27`, `crates/ploke-eval/src/cli/prototype1_state/history.rs:255`, `docs/workflow/evalnomicon/chat-history/history-blocks-v2.md:360`).

- Crown construction and sealing are structurally gated. `Crown<S>` has private state, `Crown<Ruling>::for_lineage` is private, locking consumes the ruling crown, and `Parent<Selectable>::seal_block_with_artifact` consumes the parent into `Parent<Retired>` before sealing (`crates/ploke-eval/src/cli/prototype1_state/inner.rs:47`, `crates/ploke-eval/src/cli/prototype1_state/inner.rs:84`, `crates/ploke-eval/src/cli/prototype1_state/inner.rs:134`, `crates/ploke-eval/src/cli/prototype1_state/inner.rs:188`).

- Open/admit/seal operations check Crown lineage. Open block, entry admission, successor opening, and sealing reject mismatched Crown lineage (`crates/ploke-eval/src/cli/prototype1_state/history.rs:3023`, `crates/ploke-eval/src/cli/prototype1_state/history.rs:3049`, `crates/ploke-eval/src/cli/prototype1_state/history.rs:3070`, `crates/ploke-eval/src/cli/prototype1_state/history.rs:3127`).

- Startup has a real local gate. Gen0 startup requires an absent local History head and generation 0; predecessor startup requires a present sealed head, recomputes the current clean tree key, verifies it against the sealed artifact claim, recomputes the current surface, and verifies it against the sealed head before `Parent<Checked>::ready` can return `Parent<Ready>` (`crates/ploke-eval/src/cli/prototype1_state/parent.rs:442`, `crates/ploke-eval/src/cli/prototype1_state/parent.rs:481`, `crates/ploke-eval/src/cli/prototype1_state/parent.rs:553`).

- History append advances from an observed lineage state, not an arbitrary status write. `FsBlockStore::append` verifies the sealed hash, re-reads current lineage state, rejects stale expected state, verifies the opening state root, and then enforces genesis/non-genesis/head-parent rules (`crates/ploke-eval/src/cli/prototype1_state/history.rs:831`, `crates/ploke-eval/src/cli/prototype1_state/history.rs:1187`, `crates/ploke-eval/src/cli/prototype1_state/history.rs:1234`).

- The live handoff seals and stores History before the successor is spawned. `spawn_and_handoff_prototype1_successor` prepares the active successor Artifact, seals via `Parent<Selectable>::seal_block_with_artifact`, appends the sealed block, then writes the successor invocation and spawns from the active checkout (`crates/ploke-eval/src/cli/prototype1_process.rs:930`, `crates/ploke-eval/src/cli/prototype1_process.rs:966`, `crates/ploke-eval/src/cli/prototype1_process.rs:989`, `crates/ploke-eval/src/cli/prototype1_process.rs:1036`).

- Surface commitment is implemented as an admission boundary. The backend computes `Immutable = crates/ploke-eval`, `Mutated = ToolName::ALL description files`, and empty `Ambient`; it rejects immutable-surface changes (`crates/ploke-eval/src/cli/prototype1_state/backend.rs:446`, `crates/ploke-eval/src/cli/prototype1_state/backend.rs:1217`). The live path validates child surface before build and after persistence, and successor startup validates the current surface against the sealed head (`crates/ploke-eval/src/cli/prototype1_process.rs:391`, `crates/ploke-eval/src/cli/prototype1_process.rs:716`, `crates/ploke-eval/src/cli/prototype1_state/parent.rs:515`).

- Focused tests cover the main local invariants: History/Crown/store tests passed, including stale head, duplicate genesis, wrong parent, wrong opening root, artifact mismatch, surface mismatch, and Crown-lineage mismatch; parent/backend/invocation tests also passed. Commands run:
  - `cargo test -p ploke-eval prototype1_state::history::tests`
  - `cargo test -p ploke-eval prototype1_state::parent::tests`
  - `cargo test -p ploke-eval prototype1_state::backend::tests`
  - `cargo test -p ploke-eval prototype1_state::invocation::tests`

## 3. Remaining Gaps That Matter Before Long Runs

- No OS-process uniqueness. The docs intentionally do not claim it, and code has no lease/lock/consensus mechanism for duplicate local invocations (`crates/ploke-eval/src/cli/prototype1_state/history.rs:140`, `docs/workflow/evalnomicon/chat-history/history-blocks-v2.md:129`). In a disciplined one-ruler experiment this is acceptable; for unattended long runs, duplicate launch prevention would reduce operator-error risk.

- Bootstrap/predecessor admission is still not a uniform typestate carrier. Docs and code both say bootstrap startup is weaker than the intended full admission model (`crates/ploke-eval/src/cli/prototype1_state/history.rs:75`, `crates/ploke-eval/src/cli/prototype1_state/history.rs:354`). Current gen0 absence plus first handoff genesis block is usable, but it is not the final startup proof shape.

- Ruler actor identity is still data in some History calls. The Crown proves lineage possession, but `OpenBlock` and `admit_claim` still receive actor identity as caller-supplied data until a fuller `Parent<Ruling>` carrier supplies it structurally (`crates/ploke-eval/src/cli/prototype1_state/history.rs:3024`, `crates/ploke-eval/src/cli/prototype1_state/history.rs:3094`).

- Ingress capture/import is typed but not live-wired. Docs list ingress while locked as not yet enforced (`crates/ploke-eval/src/cli/prototype1_state/history.rs:264`). Late observations are therefore not a blocker to the one-ruler control loop, but they are not yet part of durable run completeness.

- Full artifact provenance is still partial. History commits a backend clean tree key and surface, but artifact-local provenance manifest and canonical whole-artifact/runtime/build refs remain future work (`crates/ploke-eval/src/cli/prototype1_state/history.rs:248`, `crates/ploke-eval/src/cli/prototype1_state/history.rs:369`).

## 4. Specific Discernible 1000-Generation Failure Risks

- Store update is multi-file and not atomic. Append writes the block segment, by-hash index, by-lineage-height index, then rewrites `heads.json` (`crates/ploke-eval/src/cli/prototype1_state/history.rs:861`). A crash between those writes can leave projections inconsistent. The code correctly rejects missing heads when blocks exist or lineage index exists without a head (`crates/ploke-eval/src/cli/prototype1_state/history.rs:707`, `crates/ploke-eval/src/cli/prototype1_state/history.rs:738`), but that means a long run can halt after an interrupted append until repaired.

- Non-empty sealed blocks cannot currently be loaded for startup. `sealed_head_block` deliberately rejects stored blocks with entries until verified entry loading exists (`crates/ploke-eval/src/cli/prototype1_state/history.rs:797`, `crates/ploke-eval/src/cli/prototype1_state/history.rs:956`). The current live handoff appears to write zero-entry blocks and stores artifact admission in header claims, so this is not an as-is blocker. It becomes an immediate failure mode if live code starts admitting block entries before adding the loader transition.

- Duplicate local process launch remains possible. A second local process from the same admitted Artifact could pass startup because process uniqueness is out of scope. The stale-head append check should prevent both from cleanly advancing the same lineage head (`crates/ploke-eval/src/cli/prototype1_state/history.rs:839`), but side effects before the append boundary could still occur.

- The successor-selection policy is intentionally weakened. The live path currently treats any succeeded child execution as successor-eligible even when evaluator disposition is Reject, with comments saying this was temporary for handoff testing (`crates/ploke-eval/src/cli/prototype1_state/cli_facing.rs:3419`). This is not a single-ruler invariant blocker, but it is a real 1000-generation quality/collapse risk.

- Long runs still depend on repeated git/cargo/process operations. Each handoff builds the active successor binary from the active checkout (`crates/ploke-eval/src/cli/prototype1_process.rs:439`), switches the active checkout to the selected branch (`crates/ploke-eval/src/cli/prototype1_process.rs:610`), and waits for a ready file with timeout/kill behavior (`crates/ploke-eval/src/cli/prototype1_process.rs:897`). These are ordinary operational risks, not authority-model blockers.

## 5. Recommendation

Run now, with caveats.

For the narrow question "can the current codebase as-is plausibly run single-lineage experiments again under one local ruler?", I see no specific invariant blocker. Do not defer the whole program on single-ruler proof work. Run a bounded campaign first, monitor History append/head consistency, active checkout cleanliness, successor ready/completion, and disk growth, then scale.

Patch first only if the target is an unattended 1000-generation run with low operator intervention. The most valuable pre-run patches would be: atomic/repairable History store commits, duplicate-process lease/lock, removal of the temporary accept-any-succeeded-child successor shortcut, and verified loading for non-empty sealed blocks before adding live block entries.
