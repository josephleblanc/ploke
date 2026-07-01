# Live-loop functionality questions

**Status:** pre-run review worksheet.
**Scope:** questions for deciding whether a live Prototype 1 loop run is actually exercising the functionality described by `crates/ploke-eval/src/cli/prototype1_state/mod.rs` and the Prototype 1 mdbook docs.

## Very brief purpose and loop shape

Prototype 1 is a typed, local self-improvement/succession protocol over **Runtime** and **Artifact** coordinates. It is not just a rerun script, not a global consensus protocol, and not a mutable “current best branch” singleton.

The intended single-lineage shape is:

1. a Parent runtime starts from a stable active checkout and validates its parent identity/startup state;
2. the Parent establishes baseline evidence and publishes/receives a typed child-plan message;
3. it creates bounded candidate Artifacts, usually through broad-harness patch generation in temporary child worktrees;
4. it builds and spawns child runtimes from those Artifacts;
5. children self-evaluate and emit result evidence;
6. the Parent compares results to baseline, records selection under admitted policy, and decides whether to stop or continue;
7. if continuing, it installs the selected Artifact into the stable active checkout, seals/appends History, launches the successor from that checkout, waits for acknowledgement, and retires.

The live run should therefore prove not only that files were created, but that authority, provenance, policy, evaluation, selection, History, and persistence boundaries were respected.

## How to use these questions

For each question, record the strongest available answer during the run:

- **Yes:** answered by authority-bearing evidence, not just a convenience projection.
- **Partial:** evidence exists but is incomplete, mirrored after the fact, or tied to projection-only state.
- **No:** absent, contradictory, or not persisted.
- **Not reached:** phase did not execute in this run.

Keep DB facts, filesystem facts, and operator-output facts separate. `scheduler.json`, CLI summaries, and DB mirror rows can be useful, but they are not automatically authority sources.

For every question, also start a Cozo-query trail:

1. Create the best available read-only Cozo query under `queries/`, named with a stable question id and short slug, for example `queries/authority-01__parent-runtime.cozo`.
2. Run it against the live loop owner DB through the CLI, not by opening SQLite directly:

   ```bash
   P1_BIN=/home/brasides/.ploke-eval/worktrees/p1-gated-parent-3g1x3-p3-20260630-174316/target/debug/ploke-eval
   P1_ROOT=/home/brasides/.ploke-eval/worktrees/p1-gated-parent-3g1x3-p3-20260630-174316
   "$P1_BIN" loop walk db_query \
     --repo-root "$P1_ROOT" \
     --script "$(cat queries/<question-id>__<slug>.cozo)"
   ```

3. Save the command, stdout/stderr, and interpretation under `responses/`, using the same question id and campaign/step context, for example `responses/authority-01__parent-runtime__step-03.md`.
4. When answering the question, reference both the query file and response file, then note whether the query was helpful.
5. If no useful DB query can answer the question, still record the best attempted query or probe, mark the answer as not DB-answerable, and add a schema-improvement note. Prefer normalized relations over `eval_record_ref`; use `eval_record_ref` only as provenance evidence, not as the answer itself.
6. For each response, include a short database-design note: what the current schema made easy, what required awkward joins or path/string interpretation, and what relation/field/index would make the question easier to answer next time.

## Core run identity and authority questions

1. Which Runtime is acting as Parent, from which active checkout, and under which campaign/lineage identity?
2. Was the live command run with the `ploke-eval` binary built from the active parent checkout, not a different dev checkout?
3. Can we prove the parent binary digest and source commit used for authority-bearing live execution?
4. Does the active checkout carry `.ploke/prototype1/parent_identity.json`, and does it agree with campaign, node, generation, branch, and History expectations?
5. Did startup validation reach `Parent<Ready>` only after checking parent identity and either genesis absence or predecessor sealed History?
6. Is there any evidence of overlapping mutable Parent authority for the same lineage?
7. Are branch names and worktree paths treated as handles, with separate artifact/runtime/provenance identity where available?
8. Did any projection file become a de facto authority source contrary to the model?

## Admitted policy and budget questions

1. Which admitted run profile controlled the run, and can its digest be tied to the profile file used by setup?
2. Were generation limits, child-count limits, parallel caps, timeouts, backend, model route, and edit-surface restrictions honored?
3. Was the broad-harness generation path used when the profile said it should be used?
4. Was the tool loop gated where policy requires gated mode?
5. Did the run preserve the “append observations and decisions, do not overwrite evaluation state” constraint?
6. Were scores/evaluations recorded with evaluator, eval-set, target, baseline, and policy identity rather than as bare numbers?
7. If the oracle was configured `record-only`, did `require_evidence` remain non-blocking as expected?
8. If a policy could not be satisfied, did the run block or reject explicitly instead of silently relaxing it?

## Baseline and evaluation-closure questions

1. Was a generation-0 baseline established before child comparison?
2. Which baseline instance(s), run ids, metrics, and artifacts were used?
3. Can the baseline be reconstructed from persisted files and normalized DB rows?
4. Are protocol/eval closure refs stable and tied to the campaign/target profile?
5. Did later treatment comparisons name the baseline they compare against?
6. If baseline or closure setup failed, did the run stop before child planning?

## Child-plan and broad-harness generation questions

1. Did the Parent publish a typed child-plan box for the expected parent node and child generation?
2. Can we prove the child-plan producer and receiver used the same concrete box path and parent identity?
3. Were planned children bound to node files, runner requests, branch/artifact records, and the expected generation?
4. Did the broad-harness request record the exact prompt, protected-write policy, workspace, target, model, and child budget?
5. Did each model attempt produce either a submitted harness result or explicit rejected/timeout diagnostics?
6. Are rejected attempts preserved as evidence rather than lost when no child is admitted?
7. Are LLM/provider exchanges, tool calls, and message events persisted well enough to review why a candidate was produced or rejected?
8. Are mid-turn/in-flight facts distinguishable from post-attempt bundle persistence?
9. Did the harness avoid admitting edits outside the allowed edit surface?
10. Did child planning depend on the typed child-plan message rather than inferring candidates from mutable scheduler projection state?

## Artifact, patch, and operation-provenance questions

1. For each candidate, can we identify the generator Runtime, target Artifact, patch attempt, and derived Artifact?
2. Is the candidate Artifact durable/recoverable, or only a dirty/provisional workspace?
3. Are git branch names, paths, text-file fallback ids, commits, hashes, and artifact ids clearly distinguished?
4. Did applying a patch create append-only operation/apply evidence rather than only mutating files?
5. Can we recover which files changed, why, from which prompt/model/tool attempt, and under which policy?
6. Were failed patch/apply/build attempts recorded with enough reason to explain rejection?
7. Did the system avoid minting semantic artifact identity from branch names or worktree paths alone?

## Child runtime build/spawn/observe questions

1. For every planned child, did the run either materialize, build, spawn, observe, and evaluate it, or record a typed failure/rejection?
2. Was each child binary built from the child workspace/artifact it claims to represent?
3. Is the child binary digest recorded and tied to source Artifact/provenance?
4. Was each child spawned through an attempt-scoped invocation file with runtime id, role, argv, workspace, binary, channels, and streams?
5. Did channel evidence show ready/terminal messages instead of relying only on process exit or scheduler projection?
6. Did child self-evaluation produce result evidence and treatment run artifacts?
7. Did the Parent compare child treatment evidence to the correct baseline before selection?
8. Were stdout/stderr/logs captured and linked to the runtime attempt?
9. If a child failed or timed out, did that outcome become durable evidence and affect selection policy correctly?

## Selection and continuation questions

1. What candidate set was selection allowed to consider?
2. Did selection operate over terminal/evaluated children and rejected-attempt evidence according to policy?
3. Was child self-report treated as evidence, not automatic promotion?
4. Which candidate, if any, was selected, and why?
5. Are selection scores/findings tied to evaluator, policy, baseline, candidate artifact/runtime, and attempt identity?
6. Did the run distinguish “rejected-only / stop” from “selected successor / handoff” as typed outcomes?
7. Can the selection decision be reconstructed without trusting mutable scheduler latest-status fields?
8. If no successor was chosen, did the run record an explicit continuation decision and final status?

## Handoff, History, and successor questions

1. If a successor was selected, was the selected Artifact installed into the stable active checkout before successor spawn?
2. Did the temporary child worktree avoid becoming the next long-lived parent checkout?
3. Was a History block sealed with previous head, selected Artifact, selected runtime, surface commitment, policy/evidence refs, and lineage identity?
4. Did `FsBlockStore::append` validate expected head/state before advancing local History projections?
5. Was the predecessor Parent retired at the handoff boundary?
6. Did the successor launch from the updated active checkout, not from the temporary child workspace?
7. Did successor startup validate sealed History, selected artifact tree, surface commitment, and artifact-carried parent identity?
8. Is ready acknowledgement treated as transport evidence rather than a replacement for sealed-History validation?
9. Did the successor either become the next Parent for a bounded turn or record a clear handoff failure?
10. Is there evidence that no late/backchannel facts rewrote the sealed block after lock?

## Persistence and reconstruction questions

1. Which facts are recoverable from normalized DB rows alone?
2. Which facts are recoverable only from filesystem artifacts, History, MessageBox, Channel, or logs?
3. Which facts exist in both DB and files, and do the values/hashes/paths agree?
4. Are DB rows normalized enough to answer provenance questions, or do they only contain `eval_record_ref`/path references?
5. Are all DB relations used for proof defined through the normalized schema macro?
6. Can `walk db_query` answer progress, harness, child-plan, runner, evaluation, selection, and handoff questions without inspecting SQLite directly?
7. Can `walk replay` or reconstruction explain the run from durable transition evidence, not from live memory?
8. Are mutable projections (`scheduler.json`, latest runner-result, CLI summaries) clearly labeled as projections?
9. If the run is interrupted mid-turn, what evidence is already durable and what is lost?
10. Does the run preserve enough attempt-level identity to debug nondeterministic provider/tool behavior?

## Safety, failure, and cleanup questions

1. Did provider/API/tool failures create explicit blocked/rejected/error evidence?
2. Did the loop avoid weakening correctness checks to continue past missing evidence?
3. Were timeouts bounded and recorded with the phase/attempt that timed out?
4. Did workspace mutation require the expected operator/live-edge gates?
5. Were protected-write denials authoritative?
6. Were temporary worktrees/build products cleaned up or explicitly retained under policy?
7. Can cleanup be distinguished from evidence deletion?
8. Did the run leave the active checkout clean or intentionally dirty with recorded reason?
9. Are stale worktree registrations, orphan processes, sockets, or logs detectable after stop/failure?
10. Did blocked/complete/final states make it clear what an operator may safely do next?

## Minimal “this loop is doing the described thing” evidence set

A live run is most convincing if it can answer **yes or intentionally not reached** for these checkpoints:

1. parent identity/startup authority is validated in the active checkout;
2. admitted profile policy controls baseline, child generation, selection, and continuation;
3. baseline evidence exists before treatment comparison;
4. child-plan message-box evidence binds candidates to parent/generation;
5. broad-harness attempts are recorded with request, prompt, tool/model evidence, workspace diffs, and admission/rejection outcome;
6. child artifacts/runtimes carry recoverable provenance beyond branch/path labels where currently implemented;
7. child invocation/channel/result/evaluation evidence is attempt-scoped;
8. selection names candidate scope, policy, evidence, and reason;
9. handoff, if reached, installs selected Artifact into the stable active checkout and seals/appends History before successor admission;
10. predecessor retirement and successor startup are observable as separate authority steps;
11. DB/file parity is clear: normalized rows prove what they claim, filesystem authority surfaces remain explicit, and `eval_record_ref` is not used as a substitute for missing normalized facts;
12. failures block or reject loudly rather than silently skipping expected evidence.

## Run answers from `p1-gated-parent-3g1x3-p3-20260630-174316`

- **Parent/policy/baseline:** Yes for generation 0 through R7. See `answers-ledger.md` rows 06–08 and queries `authority-01`, `eval-02`, `policy-01`.
- **Broad harness / child plan:** Yes for generation 0. R7→R8 produced normalized harness request/diagnostic/agent-turn/tool-event rows and a two-child plan, with one rejected BM25 timeout preserved. See `child-plan-01`, `child-plan-02`, `trace-01`.
- **Child runtime/evaluation:** Yes after recovery. First R10→R11 timed out as the second result arrived; a second R10→R11 recovered the missing evaluation from terminal channel evidence without respawning. See `child-outcome-01__runner-evaluation-selection` and `child-outcomes__step-10__after-timeout.md`.
- **Selection/continuation/handoff:** Yes for generation 0. Selection recorded a rejected candidate but continuation policy allowed `continue_explore_from_rejected`; R12→R13b installed/spawned successor `node-2fe75acd9e9cf6c3`. See `continuation-01__selection-handoff`.
- **Final report:** No. R13b→R14 was blocked by stale walk-server binary after handoff; restart reconstructed the successor at generation-1 R7 rather than the predecessor R13b final-report state.
- **Generation-1 continuation:** Terminal failure. A full successor `prototype1-state --stop-after complete` process spawned by handoff was still running; manually driving gen1 R7 with `walk step` introduced a second controller. This caused child-plan DB/file drift: DB plan `9903062b...` stored child `node-e9be...` and hash `be1e...`, while the file at the same `message_path` was overwritten with child `node-5a...` and hash `361a...`. Strict DB validation blocked reconstruction/continuation. See `child-plan-03__generation2-plan-drift__terminal.md` and `terminal-cleanup__gen1-controller-collision.md`.
- **DB/file parity:** The failure is a positive correctness signal: normalized DB rows detected overwritten child-plan authority and did not silently accept drift. Do not weaken this invariant; fix single-writer/controller ownership and add explicit recovery/quarantine tooling.
