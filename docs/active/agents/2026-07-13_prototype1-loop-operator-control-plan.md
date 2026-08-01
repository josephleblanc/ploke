# Prototype 1 loop operator-control plan

- Date: 2026-07-13
- Baseline: `de76eaee34e6f4c4c3a19543c4cf91b217aa3bb9`
- Status: active implementation; the typed setup/profile services, guarded
  sibling-client control, UI-local Step scheduler, successor endpoint following,
  and named owner-DB views are implemented and source-tested. Fresh CLI-only
  three-parent validation is the current live gate. Actual UI-only and
  mixed-client operation, plus durable Continuous ownership/pause/resume,
  remain unproven.

## Implementation progress

- Stage 0 configuration and readiness evidence is recorded through
  `df5d8ec79` (`docs: record Stage 0 operator findings`). Google eval and
  protocol routes passed live preflight. The explicit OpenRouter Perplexity
  embedding route remains externally blocked by the key-specific monthly
  limit, so no loop mutation was attempted.
- The canonical setup preview landed in `697c7468a` (`Add canonical Prototype 1
  setup preview`). It binds normalized campaign/profile payloads and their
  digests into one reviewed plan.
- The current Stage 1 slice adds receipt-first, resumable setup admission,
  exact completed-state verification, atomic authority-file replacement, and
  an exclusive Git-worktree setup lock. Its focused regressions pass, the full
  `ploke-eval` library suite passes 1007/1007 with 27 ignored tests, and the
  independent review reports no blocker. It is committed as `60c183df0` (`Add
  recoverable Prototype 1 setup admission`).
- Fresh live validation used campaign
  `p1-stage1-hardened-g35f-pplxembed-3g1x3-p3-20260713-1` in a clean worktree at
  that commit. Plan v2 SHA `233a3732be99bb7150560d4d678bf85c0ec1e40c4e0b86d89fd9cf789133f57d`
  admitted Parent(0) at `8c4fbae0e6b3b2b6ac07dd063f80f22a082f9903`; an immediate exact retry
  preserved all campaign file bytes, sizes, mtimes, Git HEAD, and clean status.
  Ordinary doctor, relative-root normalization, headless sparse/BM25 setup, and
  a live Direct Google protocol canary passed. The explicit OpenRouter
  Perplexity embedding preflight still returns the key-specific monthly-limit
  403 (`provider_account`) despite account credits, so the campaign remains
  unadvanced at `baseline_eval`.
- The Stage 2 implementation establishes the protocol-v5 durable controller
  session, cross-process file locking and monotonic fencing, exact attempt and
  cursor certificates, operation idempotency, fail-closed recovery/abandonment,
  generation-zero setup ownership, and recoverable predecessor/successor
  transfer. `prototype1-state`, `prototype1-step`, `prototype1-continue`, and
  walk mutations now pass explicit live-provider/Git capabilities through the
  same session boundary. Successor launch persists the exact predecessor
  attempt and cannot broaden those capabilities through argv or restart.
- Stage 2 includes an artifact-driven June 30 dual-driver regression using the
  real transition journal, child-plan bytes, and saved DB-query response. The
  final focused gate passes 13/13; `cargo test -p ploke-eval` passes 1,168 tests
  with 46 ignored/live tests; and independent authority review reports no
  actionable blocker. It is committed as `aa0af840a` (`Establish durable
  Prototype 1 controller authority`).
- Stage 3 publishes protocol v6 as the exact sibling-client contract. It adds
  typed session authority, blocker, action, job-kind, transition-receipt,
  success-payload, and error carriers; a revision-tagged immutable database
  query service; concurrent connection handling; endpoint following; and exact
  public request/response round trips. Outer and nested effectful operations
  share durable idempotent job supervision, and terminal operation receipts are
  published before terminal in-memory state is exposed. Restarted unresolved
  jobs become durable recovery blockers, operator abandonment is explicit, and
  per-operation terminal publication is serialized across processes. CLI and UI
  render the same carriers without parsing prose. Independent source review
  reports no actionable correctness blocker; focused socket, restart,
  supervision, recovery, and protocol tests pass; all five UI tests pass; and
  the full `ploke-eval` gate passes 1,195 tests with 46 ignored/live tests. The
  protocol-v6 epoch intentionally updates the golden controller-certificate
  digest because protocol version is part of its signed preimage.
- Stage 3 is committed as `ce465c06a` (`Publish durable Prototype 1 walk
  service`). A clean-worktree canary admitted campaign
  `p1-stage3-walk-g35f-pplxembed-3g1x3-p3-20260713-202206` at
  `baseline_eval` without advancing a typestate edge. Its profile commitment is
  `b156cde303ac74949805943cccc478a52d89f177589085cf6ebe3cd9169a12ac`.
  The campaign remains valid and intentionally unadvanced.
- Stage 4 is now active. The first implementation slice reconciles the passive
  run-profile carrier with admitted runtime defaults, adds strict typo
  rejection, projects configuration with explicit/derived provenance, exposes
  a revision-tagged typed transition delta, builds a canonical ordered session
  event view, and consolidates raw provider-response JSONL decoding. These are
  read-only sibling-client surfaces; the detailed parent/child/protocol trace
  envelope and inspection UI remain the next Stage 4/5 work. The new typed
  protocol surfaces advance the walk protocol to v7; because that version is
  part of the controller-certificate preimage, its golden evidence digest is
  intentionally updated rather than detached from protocol identity.
- The run-profile v1 reader now distinguishes archival vocabulary from current
  execution authority. Exact historical `generation.surface`, `edit-surface`,
  and `broad-harness` values remain readable in passive evidence without
  accepting unknown fields. Current runtime execution retains only supported
  generators, and both operator parsing and the admission-plan boundary reject
  retired surface declarations for new runs. Immutable profile bytes and their
  commitments are never rewritten to obtain compatibility.
- The same archival/current split applies to earlier token and protocol routing
  spellings. Passive evidence preserves historical `[model].max_tokens` and
  flat `[protocol]` routing keys, but runtime admission rejects them. Eval
  completion authority remains `campaign.json` at `eval.max_tokens`; the
  recommended setup writes `32768` explicitly, hashes it into eval-set identity,
  and does not revive a competing run-profile field.
- Owner-DB projection stores that campaign token cap in the additive
  `eval_campaign_eval_token` relation. This leaves the earlier campaign-policy
  relation shape intact; an absent relation or row means the campaign predates
  this field, while a newly admitted campaign writes a row even when the value
  is null. A present but malformed relation remains a hard error. Executable
  prepared campaign context carries the same admitted value so standalone
  agent reruns cannot silently fall back to provider defaults. After model-route
  resolution, an explicit or admitted cap below a registered model floor is
  rejected rather than silently raised.
- Prepared campaign execution now selects models in the order explicit CLI,
  admitted campaign model, then mutable active/default fallback. The admitted
  route and provider selection override mutable registry preferences, while an
  explicit mismatch fails closed. Historical prepared `run.json` artifacts
  that predate `route_source` remain readable for inspection, but they cannot
  be re-executed live by inferring mutable routing state; replay-to-live must
  create a newly admitted branch configuration.
- Session history decoding now validates event order, session identity,
  creation mirrors, cursor and abandonment summaries, and damage/revision
  relationships. Damaged journal inspection applies each candidate entry to a
  cloned replay state and commits it only on success, so rejected lines cannot
  contaminate the returned valid prefix. The live R7-to-R8 controller edge also
  requires a typed outer-attempt coordinate; unlinked tool-loop provenance is
  reserved for standalone debugging and historical records.
- Both authoritative and public session replay now enforce the same recovery
  schema gates, one-unresolved-attempt-per-fence invariant, finished-epoch
  contradiction check, and exact handoff target. Older v1/v2 evidence remains
  readable only within the fields those schemas actually authorized.
- Transition-delta inspection is published outside the controller mutex. It
  returns the last durably completed typed delta while a later provider-backed
  job owns the controller, and cache publication occurs only after the terminal
  operation receipt is visible as `Succeeded` with the same session version.
  Publication is monotonic by server job id, so a slower earlier job cannot
  overwrite a later completed result.
- Strict replay admission now rejects duplicate or missing response indexes.
  The historical R10 incident bundle exposed why: its response sidecar
  multiplexes several distinct session-local chains under one assistant ID, so
  a raw index is not a unique replay coordinate. Forensic inspection remains
  available, and the historical regression still selects uniquely identified
  real tool-call responses, rebases them into one contiguous derived tape, and
  exercises that tape through the production loader and broad harness.
- Forensic provider-response inspection is now distinct from executable replay
  ordering: inspection preserves physical JSONL chronology and one-based source
  lines, while replay admission alone sorts the validated session-local tape by
  response index.
- The public phase inventory now includes every walk phase exactly once,
  including recovery-critical `R13c` incomplete successor handoff. This is a
  prerequisite for an evidence-driven UI rail that does not hide a valid
  durable state.
- Current live readiness is asymmetric: Google application-default credentials
  now pass, while the exact OpenRouter Perplexity embedding request still
  returns the key-specific monthly-limit 403. The supported configuration-only
  workaround, direct OpenAI `text-embedding-3-small`, passed a live preflight
  with a 1536-dimensional vector. A fresh post-checkpoint campaign will use
  that route; the Stage 3 campaign will not be rewritten.
- The next Stage 4 slice publishes walk protocol v8 with a typed completed-run
  inventory and exact-run trace. Each registration, sealed turn summary,
  compressed run record, provider-response tape, and protocol artifact stays
  paired with the digest and path of its exact stored bytes. The reader follows
  the global run registry, preserves nested treatment campaign identity and
  physical JSONL order, validates typed protocol input/output identity and the
  lifecycle anchor, and fails closed on source drift or malformed evidence.
  Non-completed runs expose lifecycle authority only. Even completed reads do
  not open the rewritten `agent-turn-trace.json`; live observation remains
  deferred until the writer provides an atomic or append-only identity-bearing
  source. This slice remains filesystem-authoritative and makes no database or
  executable-replay claim.
- The Stage 4 foundation and typed configuration/delta/session read model are
  checkpointed in `ffe3e59c3` (`Build Prototype 1 operator observability
  foundation`). Canonical evaluation-trace inventory and exact completed-run
  evidence are checkpointed in `1e03f5e7e` (`Expose canonical evaluation
  traces through walk`). The latter is the protocol-v8 compatibility boundary.
- A preserved completed-setup canary then exposed a protocol-v8 authority
  contradiction: Show strictly reconstructed R4c while Status published Empty
  with an empty durable version and an invalid bootstrap action. Protocol v9 is
  the repair boundary. Its typed `WalkPosition` distinguishes no session,
  pre-session reconstruction, durable session without a cursor, durable
  session cursor, and receive-only legacy status. Fresh Start and recovery
  admission are enforced before operation persistence and again at their typed
  authority boundaries. Session positions reject empty or malformed cursors,
  and an indeterminate supervised job cannot advertise controller recovery
  before its own outcome is resolved. This boundary is checkpointed in
  `5c9745b2a` (`Expose authoritative Prototype 1 walk state`) and is live
  verified on the preserved canary: v9 reconstructed pre-session R4c, guarded
  Start created and released a v5 session at R3, and reuse of the old empty
  guard failed stale without operation persistence. Doctor/closure state and
  the clean canary checkout remained unchanged outside the controller journal.
- `ploke-walk-ui` now consumes that same position carrier, renders explicit
  source badges in the phase rail and details panel, and discards stale walk or
  DB-query replies after run selection. Complete authority snapshots are shown
  only for Status responses, so same-phase Show, Job, or Error responses cannot
  combine a newer epoch with stale actions, blockers, or attachment state.
- At that checkpoint, the first Stage 5 slice kept the UI read-only and consumed the existing
  completed-run index and exact trace carriers rather than creating a second
  trace schema. Evaluation Traces is the default central surface; Database
  Query remains a sibling expert tab. Index/detail replies are bound to the
  selected client generation and exact run coordinate, so a delayed response
  from a prior campaign cannot replace current evidence.
- The trace inspector labels its scope as sealed completed evidence and does
  not claim live/in-flight telemetry. It exposes prompts, physical provider
  response order, turns, tool arguments/results, typed protocol reviews, and
  the full path and SHA-256 of every included source. Tool-call review linkage
  uses the producer's run-global focal index. The canonical trace reader now
  validates the artifact's internal focal target, index, and tool-name identity
  before the UI may present it as linked; it intentionally does not impose a
  direct-run-record cross-check that would reject historical records whose
  tool calls are reconstructed from sealed turn events.
- Focused review-link validation and the native UI suite pass (22/22 UI tests).
  A substantial historical completed trace contains 59 calls, 60 provider
  exchanges, and 73 protocol artifacts; its compact source components total
  about 7.84 MB, below the 16 MiB frame cap before envelopes. That campaign
  predates strict setup admission, so it is a sizing fixture only. The loader
  will not be loosened to make it a current live endpoint; end-to-end UI-only
  validation remains gated on a completed run from an admitted campaign.
- The Stage 4 answerability audit still leaves live/in-flight last-event and
  typed absence-causality views, exact authority-bearing executable digest,
  child-plan DB/authority-byte comparison, and a combined no-selection /
  no-handoff / sealed-History view unresolved. The completed-trace UI does not
  paper over those gaps.
- Fresh campaign
  `p1-handoff-drainfix-g35f-orembed-3g1x3-p3-20260715-112133` passed its explicit
  setup, doctor, embedding, protocol, and headless preflights, then advanced by
  CLI through live broad generation, three child treatments, evaluation,
  selection, sealed History, and R12-to-R13b handoff. Branch
  `branch-be25fae7ba5db2ce` was selected as generation-1 parent
  `node-1c6b86abba9f76c7`; the predecessor operation reached `Succeeded` at
  revision 53 and unpinned walk status followed the successor to R4c. This
  validates stepped authority transfer and outer-receipt draining, but does not
  claim R14b, continuous mode, UI-only control, or mixed-client control.
- The same canary exposed two separate follow-ups. Child process ownership and
  reaping is source-repaired in `7b862e4e9` with production-path and lifecycle
  regressions, but still needs fresh live validation. Parallel broad trace
  ownership is fixed in `dc5868b66` and live verified with the `24f18e1cd`
  binary: explicit session capture follows
  `Attempt -> AttemptDriver -> runtime -> LlmRequestArgs -> ChatSession`, the
  live lanes retain exact debug/response identity through terminal persistence,
  and no response crosses a lane. The companion `24f18e1cd` cancellation repair
  keeps the caller's partial `HeadlessRun` and drains capture to sender
  disconnection; that path is source/test verified, with a post-repair
  outer-timeout live canary still pending.
- The accepted Direct Google r16-r18 sweep joined debug steps to response tapes
  exactly 46/46, 9/9, and 39/39, with contiguous indexes, matching assistant
  identities, exact lane/workspace manifests, and no pairwise response-ID
  intersection. The provider-deserialization and validation failures were
  correctly rejected without publishing result JSON. An earlier post-fix
  r13-r15 attempt was stopped and preserved after disk exhaustion corrupted one
  manifest; it is explicitly abandoned as full-run evidence. The main and
  stopped-lane build caches were then cleaned while all run/debug artifacts and
  candidate edits were retained.
- The r17 failure pins a Stage 4/5 projection requirement: the headless bundle
  records the aborted provider turn and diagnostic body excerpt, while the
  lower-level debug session remains `paused` at its last decoded response. The
  operator view must present both provenance layers without silently converting
  one into the other.
- Protocol v10 and the second Stage 5 slice are checkpointed in `bbb468116`
  (`Expose typed live LLM traces through walk`). `llm sessions` and exact
  `llm observe --session-id ... [--step ...]` now use the same public typed
  index/detail carriers as the new `Live LLM` UI tab. The read path stays
  outside the mutation-controller lock, retains every session in a lane, binds
  replies to the requested session/step, and pairs every present artifact with
  its exact path and SHA-256.
- Debugger session status, resume publication frontier, selected provider
  checkpoint, outer headless terminal, and agent-turn terminal remain separate
  evidence layers. Missing, unreadable, invalid, and not-selected states remain
  distinct; absence is not relabeled as in flight. Full growing prompt/response
  detail is returned only for the selected checkpoint, while the timeline stays
  compact. Resume frontiers above 1,024 steps fail before filesystem iteration.
- Primary session/resume/checkpoint files and derived request/headless/turn
  sources are bounded to their admitted evidence roots before reads. Parent or
  symlink escape, request-path/workspace mismatch, cross-session identifiers,
  and source drift fail closed. Tool-loop JSON publication now uses atomic
  replacement so a failed rewrite cannot truncate an already published file;
  per-session read errors still remain visible without hiding healthy siblings.
- Final live read-only validation used the rebuilt v10 binary against the
  preserved Direct Google campaign
  `p1-gemini35-flash-direct-15g2x3-fixed-20260525-123824`. The index returned five
  healthy lanes/sessions plus the zero-byte r15 manifest as one independent
  invalid issue with SHA-256
  `e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855`.
  Exact r17 observation returned debugger `paused`, resume `next_step=9` and
  `terminal=false`, nine checkpoints, selected response index 8, headless
  `exhausted`, and agent-turn `aborted` after 10 attempts. Exact r16 remained
  debugger/resume terminal with 46 checkpoints, headless
  `applied_validation_failed`, and agent-turn `completed`.
- The final source gate passes 1,546 `ploke-eval` tests with zero failures and
  46 ignored across all targets (1,526 library tests, one binary test, and 19
  integration tests). The `ploke-walk-ui` suite passes 68/68. The separate
  full-loop OpenRouter embedding canary, durable imported r15-r18 historical
  replay fixture, and native UI-to-live-server interaction remain pending.
- The current operator-control slice promotes the public `WalkClient` from a
  read-only facade to the shared guarded Start/Step/operation-status/idle-Stop
  client used by both CLI and UI. The service, not either frontend, captures the
  authoritative endpoint, server epoch, and exact session version. Explicit
  live-provider and Git-change grants remain required, and typed jobs/receipts
  remain the only success signal.
- Fresh-run UI setup now uses typed profile load/default/validate/create-only
  save, then typed preview and hash-bound admission through the canonical
  CLI-owned parsers and setup planner. The UI has no copied policy defaults or
  raw setup JSON. Successful save and admission are read back and hash checked;
  `WalkConfigSnapshot` keeps CLI/schema defaults, prepared cohort/eval budget,
  selected profile policy, and derived effective control/provenance visibly
  distinct. Name selectors accept one non-`.toml` registry component; Path
  selectors remain explicit paths and are absolutized before legacy
  resolution. The UI does not prepare batches/worktrees or run doctor/live
  preflights.
- UI auto-advance is intentionally labeled
  **UI-local automation · Step-mode server**. It waits for one guarded Step's
  terminal receipt before submitting the next. Pause prevents the next edge
  only; resume is UI-local; the intent is lost on UI exit; no exclusive lease
  exists between steps; successor following waits at R13b; and predecessor
  R13b-to-R14b is not driven. This is not durable `RunMode::Continuous`.
- Closed `db_query` views now cover `relations`, `counts`, `config-evidence`,
  `lineage`, `progress`, and `handoff-evidence` over one immutable,
  revision-tagged owner-DB snapshot. `walk status --with-version` remains the
  live controller authority view; DB evidence and JSON artifacts do not replace
  it. The `progress` projection preserves generation roles: evaluation subjects
  resolve to evaluated scheduler nodes while their parents remain actors;
  unresolved joins stay visible with null identity/generation fields.
- The recent `ploke-llm` provider-attempt lifecycle cleanup did not block the
  V36 live Parent(0) R5-to-R6 baseline edge. A producer-to-importer regression
  also sends actual provider-attempt tracing JSON through the Prototype 1
  eval-store importer, so validation stays tied to emitted typed trace fields
  rather than a hand-written duplicate log shape.

The UI is now an active but deliberately bounded client. Every live edge still
passes through the server-owned stepped authority. Manual configuration changes
or unadmitted source drift are never adopted silently and require a fresh
reviewed setup; loop-owned successor handoff remains the admitted path for the
parent/source identity changes produced by self-editing.

### Pending three-parent live validation packet

Do not fill this packet from source inspection or older canaries. Record only a
fresh run from a newly created worktree:

- campaign id and worktree path: **pending**;
- setup preview SHA-256 and read-back `WalkConfigSnapshot`: **pending**;
- binary/build fingerprint and initial server epoch: **pending**;
- Parent(0) -> Parent(1) R12-to-R13b operation id and receipt: **pending**;
- successor endpoint/epoch/session resolution for Parent(1): **pending**;
- Parent(1) -> Parent(2) R12-to-R13b operation id and receipt: **pending**;
- final `status --with-version`, `session-history`, `summary --verbose`, and
  `handoff-evidence` coordinates: **pending**;
- UI-only and mixed-client observations, including grant prompts and
  auto-advance pause/resume behavior: **pending**.

#### Attempt ledger: does not satisfy the packet

Seven fresh campaigns exercised the new operator boundary but stopped before the
first successor handoff. They are preserved as negative evidence and must not
fill any pending packet field:

- `p1-v30-walkop-llmtrace-g35f-3g-20260726-122809`, worktree
  `/home/brasides/.ploke-eval/setup-seeds/p1-v30-walkop-llmtrace-g35f-3g-20260726-122809`,
  admitted setup plan
  `c26db4804c48c4eedd44f42171477ddf288877fd7ecac3d164f451d91d9e8c22`
  into initial session `833824d7-730f-4955-9587-5dc1b7ecfa25` with build
  fingerprint
  `c04d759e9b58384a264ecd796402add0e17e1193bdb5005d1713b0a524d585b6`.
  R5-to-R6 operation `b1390e2f-31bf-4725-8574-5c07751266dd`
  successfully reconstructed typed eval/protocol evidence. R7-to-R8 operation
  `f59bd097-f58c-493c-9189-2609b46a6b66`, transition
  `b42ac3e5-04ed-5c65-a71e-e909c7f4fca8`, then stopped on direct-Google HTTP
  429 `RESOURCE_EXHAUSTED`, error
  `1d54b138-6afa-44fb-8e2b-0559730f0a2f`. The controller retained R7 as
  `AttemptIndeterminate`; the operation and session were explicitly abandoned.
  The typed trace compatibility evidence is durable in
  [`2026-06-09-prototype1-broad-child-google-429-zero-admission.md`](../bugs/2026-06-09-prototype1-broad-child-google-429-zero-admission.md).
- `p1-v31-walkop-handoff-g25fl-3g1x1-20260730-035352`, worktree
  `/home/brasides/.ploke-eval/setup-seeds/p1-v31-walkop-handoff-g25fl-3g1x1-20260730-035352`,
  admitted one-child, parallel-cap-one setup plan
  `c34ea38168f4b59a0594ff90324569bf8c7f35590973807ee7b789e54b28c487`
  with profile hash
  `a8320469eb121da18551412afe35ee5f45030960a4557009ba2f87a188ef8c7e`,
  Parent(0) `node-ffd149c9304c0834`, session
  `350350a7-ab97-4ef8-b58b-77470ac1901d`, and the same build fingerprint.
  R5-to-R6 operation `eedf5dbe-2fc4-4933-983d-549e397ea71c`,
  transition `23bbb5e7-a401-5b56-89e1-129ff386c5db`, stopped on a single-attempt
  direct-Google `HTTP_SEND_TIMEOUT` after 39,088 ms, error
  `9ebe7d77-ddfb-4fbb-86b0-7c82baf90f6a`. The baseline instance was `Failed`;
  closure remained partial with one failed instance; the controller retained
  R5 as `AttemptIndeterminate`; the operation and session were explicitly
  abandoned.
- `p1-v32-walkop-handoff-g35f-global-3g1x1-20260730-040809`, worktree
  `/home/brasides/.ploke-eval/setup-seeds/p1-v32-walkop-handoff-g35f-global-3g1x1-20260730-040809`,
  admitted setup plan
  `cb97d666e67e809eb711b845df22fecfa8f9cb3c5397dc2427c05a385f677287`
  into session `0aa363ef-a8fe-4a6c-88f2-3b86e52fe64e` with the same build
  fingerprint. Durable session receipts committed R5-to-R6
  (`15527ad2-d553-56f9-9660-243528d70648`), R7-to-R8
  (`544ac1e2-e4fe-5764-915c-51ffd8825d00`), and R10-to-R11
  (`5649ec33-476a-5480-9e93-29e8fff26c56`). The emitted typed
  `provider_attempt` lifecycle also retained a real request-241 HTTP 429 as
  failed/not-parsed with a scheduled retry, followed by attempt 2 HTTP 200 as
  completed/parsed, so the cleaned-up trace fields remained usable through a
  live retry. Child `node-ac2fbbb9ab535d2e` was admitted and ran successfully,
  but strict evaluation selected the rejected branch with `Stop`; continuation
  recorded `stop_selected_branch_rejected`. R12-to-R13a transition
  `c2803052-2fc3-5a8d-9d3a-9837c506acdc` and R13a-to-R14a transition
  `943e8cf4-c69d-5ae1-8358-f377cd5e8af9` therefore completed a stopped parent
  turn. The campaign was not terminal and no successor runtime was started.
- `p1-v33-walkop-handoff-g35f-global-3g1x3-20260730-050445`, worktree
  `/home/brasides/.ploke-eval/setup-seeds/p1-v33-walkop-handoff-g35f-global-3g1x3-20260730-050445`,
  admitted setup plan
  `fcff8938e7f1eb69b760dda30058daf4fa53c5bec610678aef66f225dd7fb283`
  with profile hash
  `6619b87761ee4f484f73439fb034d8395fa9a82d7a12f1d98d22738431b90510`
  for Parent(0) `node-2f71b746a0b70661`, session
  `a473da7f-8133-433c-a64e-72fee1887bed`, and the same build fingerprint.
  Durable receipts committed R5-to-R6 operation
  `ad6bf802-57d4-4811-84f8-6293d958f9f1`, R6-to-R7 operation
  `8b7fa898-7125-40b9-996e-9980c507f117`, and R7-to-R8 operation
  `7130b6f3-96cb-407d-bde1-922e8ba08f26`; the R7-to-R8 terminal receipt
  committed at session revision 31. Four fresh-slot terminal artifacts were
  retained under `prototype1/messages/edit-harness-result`:
  `node-2f71b746a0b70661.headless-tui.json` returned `ok=false`, while
  `node-2f71b746a0b70661-r2.headless-tui.json`,
  `node-2f71b746a0b70661-r3.headless-tui.json`, and
  `node-2f71b746a0b70661-r4.headless-tui.json` returned `ok=true`. Those three
  viable slots produced planned children `node-1e28a20104237375`,
  `node-89042096c5c44750`, and `node-c063e4e69729539f`. Typed provider-attempt
  rows retained 21 HTTP 429 attempts across 16 requests, with every scheduled
  retry recovering to HTTP 200 completed/parsed and no retry exhaustion; for
  example, request 186 recorded three failed/not-parsed 429 attempts before
  attempt 4 completed and parsed with HTTP 200. The named `progress` DB view
  read owner-DB revision
  `7a7b68a0bb780c8e5eb3ad0f9ee1f53845619652aed69cef21f6a682fe103e8d`
  at session revision 31 and retained child-plan
  `515cd691063e12904143aaa57b2957db0ff3ff1992c53862503a72c04ca6d035`.
  The idle server then stopped cleanly, and typed status reported `offline`.
  V33 was deliberately stopped at R8 before handoff because endpoint/UI safety
  defects found during validation were fixed afterward. Its evidence is
  therefore superseded and non-satisfying; it cannot fill the three-parent
  packet.
- `p1-v34-walkop-handoff-g35f-global-3g1x3-20260730-065737`, worktree
  `/home/brasides/.ploke-eval/setup-seeds/p1-v34-walkop-handoff-g35f-global-3g1x3-20260730-065737`,
  admitted setup plan
  `b593fec4f17c9e0cf96a8e67bb26e6367777d0b234effcd58dc05bdefac2d67c`
  with profile hash
  `6619b87761ee4f484f73439fb034d8395fa9a82d7a12f1d98d22738431b90510`
  for Parent(0) `node-bda1d65ea90e7839`, session
  `66e8446e-f161-4fc8-b182-a40ad0b87672`. Baseline operation
  `5cb0ea6a-e0ba-46fc-b46a-b263f629f7d3` succeeded R5-to-R6 at session
  revision 23, and policy operation
  `1677229a-1037-4c88-bc42-b5552fa31114` succeeded R6-to-R7 at revision 27.
  Child-plan operation `3ebb67f3-e4e9-4ee6-9590-cda6a270e9eb` began
  transition `dbb32ee2-6e2a-5d4a-8853-0c08b6354b43`, but a server
  interruption left no `AttemptFinished`. The base and r2 lanes were terminal
  at head 61 while r3 was paused at head 30, so no child-plan authority was
  admitted. The job was durably abandoned, then recovery operation
  `f8fe0e55-211d-4dd2-9279-bf6a863cd254` permanently abandoned the session at
  revision 30. V34 is superseded, non-satisfying canary evidence and is
  explicitly outside the three-parent handoff packet.
- `p1-v35-walkop-handoff-g35f-global-3g1x3-20260731-145700`, worktree
  `/home/brasides/.ploke-eval/setup-seeds/p1-v35-walkop-handoff-g35f-global-3g1x3-20260731-145700`,
  admitted setup plan
  `39f887adfe290a581c855781de11a35f665813271176b2a288fea4676418dd35`
  with profile hash
  `6619b87761ee4f484f73439fb034d8395fa9a82d7a12f1d98d22738431b90510`
  for Parent(0) `node-31921bc9908713d5`, session
  `34c5098b-954a-4962-bf18-2ec41d171640`, setup head
  `1700115f5094503c27c98d2f080ebbd975762f45`, and build fingerprint
  `4fbe865ae8e27cd1e28e76f24920b9edb809fed01f321c327a6db3fa467683c7`.
  The durable session journal committed bootstrap transitions
  `17cba2a0-e7c7-5531-8eb6-2b249f5f8f0c` (R3-to-R4a),
  `45dcbf40-c39c-5019-986e-1ba7af3ebbf1` (R4a-to-R4b),
  `c131991b-0405-50b1-999e-c2eab32ccd29` (R4b-to-R4c), and
  `3ccaf407-f0c2-51ce-91a5-ed311fb5c45b` (R4c-to-R5). The earlier
  Start/bootstrap client operation identifiers survived only as truncated
  interactive-transcript prefixes, so they are deliberately omitted rather
  than promoted as durable evidence. R5-to-R6 operation
  `2f3dfc89-fa2c-4c40-827d-ddcd75c5dc0e`, fence 6, began transition
  `8fa51613-c756-55cf-ba95-eca41b486200` and finished `Indeterminate` at
  session revision 22 because the baseline instance was `Failed`. Typed
  `walk trace show` resolved run
  `run-1785535497895-structured-current-policy-2507d18a` through the run
  registry as execution `completed`, one turn, a sealed turn summary, zero
  tool calls, zero model exchanges, and zero protocol artifacts. Targeted
  inspection of that sealed JSON summary classified the one agent turn as
  `aborted` with `HTTP_SEND_FAILED`: the systemd service used to host
  `walk serve` had not inherited `GOOGLE_PROJECT_ID`. This was a canary-launch
  environment error, not a `ploke-llm` trace or retry regression. The failed
  job was explicitly abandoned; recovery operation
  `42744bde-00f1-4afb-b1a4-05584e4de7d6` then permanently abandoned the
  session at revision 23. Typed `walk status --with-version` subsequently
  reported `offline`; named `progress` retained only the generation-zero
  parent and `handoff-evidence` returned zero rows. After the idle server
  stopped, only this worktree's build target was cleaned. V35 is superseded,
  non-satisfying canary evidence and is explicitly outside the three-parent
  handoff packet.
- `p1-v36-walkop-handoff-g35f-global-3g1x3-20260731-151050`, worktree
  `/home/brasides/.ploke-eval/setup-seeds/p1-v36-walkop-handoff-g35f-global-3g1x3-20260731-151050`,
  live-validated canonical setup/config read-back and a successful Parent(0)
  R5-to-R6 baseline edge, showing that the recent `ploke-llm` trace cleanup did
  not block that downstream live transition. Its typed verbose summary remains
  nonterminal at generation 0 with active node `node-5798cc0855390ba4`, three
  admitted and observed children, and journal entry 40 `observe_child` as the
  latest durable entry. It has no selected successor, continuation decision,
  or handoff and therefore does not satisfy any three-parent handoff field.

None of these campaigns reached Parent(0)-to-Parent(1) R12-to-R13b, so the
requested three-parent handoff, successor endpoint following, and final typed
evidence queries remain pending.

## Purpose

Turn the current Prototype 1 loop, `loop walk` tooling, and `ploke-walk-ui`
into one understandable operator system that can be inspected and controlled from
either the CLI or the native UI without creating competing loop authorities.

The desired experience includes:

- validate and explain a complete run setup before admitting it;
- start and drive a live run from either UI or CLI;
- see the current R0-R14 phase, the evidence that admits it, the blocker if it
  cannot advance, and the actions currently allowed;
- follow parent agent turns, tool calls, model exchanges, and `ploke-protocol`
  adjudications while they happen;
- hand control from a predecessor runtime to its successor without losing the
  operator session or starting two drivers;
- replay historical transitions without mutating the historical run;
- create an explicitly derived live branch from historical evidence;
- query persisted run data through a backend-neutral application boundary;
- preserve the existing correctness boundaries around History, identity,
  MessageBox/channel authority, worktrees, immutable evidence, and typestate.

This is not primarily a UI project. The UI is the clearest consumer of a larger
missing object: an authority-preserving loop execution session.

## Assumptions and decisions to confirm

This plan proceeds with the following recommendations:

1. The crate present at this commit, `ploke-walk-ui`, is the implementation
   vehicle. `ploke-loop-ui` is treated as the product concept, not a requested
   crate rename.
2. UI and CLI are sibling clients. Neither frontend executes R-state edges
   directly.
3. There is at most one admitted mutating controller for a loop session. Any
   number of clients may inspect it.
4. “Back” and “rewind” are read-only replay-cursor operations. Alternative
   execution from the past creates a new branch; it does not undo a live run.
5. “One source of truth” means one application-level authority and one coherent
   snapshot for clients. It does not yet mean that every authority-bearing fact
   must be forced into one physical database file.
6. Filesystem authority is migrated to database-backed adapters one domain at a
   time, with parity and negative tests. DB rows do not silently replace History,
   MessageBox/channel, invocation, artifact, or worktree authority.
7. Near-term default validation should prefer configuration/profile changes.
   If a current-code blocker is found, stop, preserve a reproducer, and plan the
   smallest correctness-preserving fix instead of weakening validation.

If these decisions are accepted, they remove several architectural forks from
the implementation sequence. In particular, destructive rollback and a generic
“put everything in the DB” rewrite are out of scope.

## What exists at the reset baseline

### Three different meanings of “advance”

The current code has three control models:

- `prototype1-state` is the canonical typed batch driver. It carries R0-R14
  values and calls the direct edges in `driver/advance.rs` and `live_edges.rs`.
- `loop walk` keeps one concrete R-state in a server-local `WalkController` and
  invokes those same direct edges incrementally.
- `prototype1-step` and `prototype1-continue` reconstruct a separate
  `DiagnosedPhase` and dispatch phase-specific operations. They do not carry the
  same R0-R14 value.

The first architectural goal is therefore not “add UI step buttons.” It is to
make every mutation pass through one admitted driver boundary with one meaning
of step.

### Useful capabilities already present

- Direct typed transitions cover the complete R0-R14 batch path.
- `WalkController::step_once` covers the principal phase branches and uses the
  direct edges.
- The walk protocol has health, start, step, stop, status, audit, files, LLM
  debugging, replay cursor, and branch-provenance operations.
- Mutations carry a server epoch, and source/binary drift fails closed.
- The walk server tracks outer jobs and suppresses duplicate outer start/step
  submissions.
- Reconstruction is side-effect free and intentionally strict.
- Replay back/forward already moves an immutable journal cursor without undoing
  effects.
- The eval store has normalized run-policy, parent-start, agent-turn,
  model-exchange, message-event, and tool-event relations.
- The UI already has run/socket selection, an online indicator, a current-phase
  rail, Health/Show, and read-only Cozo queries.

These are foundations to preserve, not replace.

### Historical gaps that constrained the initial order of work

This list records the architecture at the start of the plan. It is retained as
design history, not as a description of the current UI/client contract; the
current implementation status and remaining live-proof boundary are recorded
above.

1. `WalkController::refresh_from_disk` does not refresh while it holds any
   in-memory state. External `prototype1-state`, `prototype1-step`, or
   `prototype1-continue` activity can therefore diverge from a running walk
   server.
2. The public `WalkClient` is intentionally read-only and flattens structured
   `Job` and `Status` responses into prose. The UI cannot render reliable job,
   blocker, transition, or cancellation state from that facade.
3. A fresh continuous walk has no admitted continuation after the R2a identity
   initialization branch.
4. Reconstruction omits several intermediate carriers, including R8, R9, and
   R11, and may rebuild early command data from defaults. It is strict recovery,
   not exact replay of every in-memory edge.
5. `branch-live` writes provenance only. It does not create a worktree, run
   identity, controller session, or live provider call.
6. R13b handoff launches a successor `prototype1-state` process rather than a
   successor walk controller. The predecessor server epoch is also invalidated
   by the checkout change.
7. A successor-ready timeout can still produce the live R13b type whose name
   claims a committed handoff, while reconstruction requires both durable
   handoff and ready evidence. Live and reconstructed semantics disagree.
8. A June 30 run demonstrated that an autonomous successor and a manually
   stepped walk server can both drive the same generation. The resulting
   child-plan file/DB collision was correctly rejected by `dual-strict`.
9. Stop aborts a running task, but effectful edge cancellation has no explicit
   transaction/recovery boundary.
10. The server accept loop is serial. Some long operations still hold the
    controller lock, and a response-write failure can terminate the serve loop.
11. The owner Cozo file is restored into an in-memory database and replaced
    under a process-local mutex. That is a useful query mirror, not yet a safe
    independent multi-process write authority.
12. Agent-turn database writes are primarily completed bundles. The detailed
    `walk llm` surface reads filesystem checkpoints and protocol artifacts, so
    live trace observability is not yet one coherent read model.

## The semantic object to build

The target is a durable loop execution session, not a larger walk DTO.

```text
                         read snapshots / submit commands
                 +---------------------------------------------+
                 |                                             |
          +------v------+                               +------v------+
          | native UI   |                               | CLI client  |
          +------+------+                               +------+------+
                 |                                             |
                 +--------------------+------------------------+
                                      |
                             typed control service
                                      |
                         +------------v-------------+
                         | loop execution session   |
                         | controller lease         |
                         | expected phase + epoch   |
                         +------+------------+------+
                                |            |
                       one typed edge     immutable receipts
                                |            |
                         +------v-----+  +---v----------------+
                         | R0-R14     |  | operator read model |
                         | driver     |  | DB/query projections|
                         +------+-----+  +---------------------+
                                |
                    typed authority/domain ports
                 History, identity, messages, channel,
                 artifacts, invocation, protocol, eval store
```

The exact Rust carriers should be chosen only after a type-reuse audit. The
semantic responsibilities are:

### Loop session

A loop session identifies one controllable lineage position and its lifecycle.
It relates:

- one campaign and current node/generation;
- zero or one active controller lease;
- one admitted configuration commitment;
- an ordered sequence of transition attempts and committed receipts;
- zero or more observer clients;
- zero or one predecessor session and zero or one successor session;
- zero or more replay cursors and derived branch origins.

The session must survive a UI restart, CLI disconnect, and walk-server restart.

### Controller lease

The controller lease grants exclusive mutation authority. It needs enough
durable identity to reject a second driver and diagnose a stale owner:

- session identity and campaign/node/generation coordinates;
- run mode: stepped or continuous;
- controller process/binary epoch;
- acquired/heartbeat/expiry or explicit terminal state;
- predecessor/successor link during handoff;
- takeover/recovery reason when ownership changes.

A lease is not permission to weaken the underlying edge checks. It prevents two
otherwise-valid processes from applying them concurrently.

The exclusion mechanism must itself be atomic across processes. The current
owner Cozo query mirror, protected only by a process-local mutex around backup
replacement, is not eligible to serve as the lease store. The first local
implementation should choose an actual OS-level ownership primitive held by one
authoritative control process, or use a store with proven transactional lease
semantics. It must issue a monotonic fencing token carried by every mutation so a
stale former owner cannot continue after takeover. Socket bind/takeover must
prove ownership and liveness before replacing an endpoint. Persisted lease
metadata is evidence about the owner; it is not by itself the exclusion
primitive.

### Transition command and receipt

Every mutation should carry:

- intended action;
- expected session, phase, controller epoch, and relevant evidence version;
- an idempotency key;
- explicit admission flags for high-risk edges;
- operator/client provenance.

Every attempt should return a typed receipt with:

- accepted, running, committed, blocked, failed, cancelled, or recovered state;
- phase before and after;
- transitions crossed;
- evidence written or consulted;
- blocker/error category and recovery advice;
- job identity and timing;
- controller and binary provenance.

The UI and CLI must render this same receipt. Neither should infer capabilities
by searching display text such as `--watch`.

### Loop snapshot

The primary read model should answer, without parsing prose:

- what session/run is selected;
- whether a controller is online and owns mutation authority;
- current reconstructed phase and confidence/admission evidence;
- current/last job and its progress;
- last committed transition and evidence delta;
- next allowed actions and their gates;
- active blocker and where to inspect it;
- effective configuration values and provenance;
- links to parent trace, tool calls, protocol reviews, child runs, selection,
  History, and handoff records.

Raw files, raw JSON, and raw CozoScript remain expert escape hatches, not the
primary operator story.

### Replay cursor and branch origin

Keep three operations separate:

1. **Historical replay** reconstructs and displays immutable evidence. It never
   calls providers or writes live state.
2. **Response fork** re-runs a recorded model/tool interaction with a new binary
   or implementation and stores new provenance without pretending the outer
   loop moved.
3. **Loop branch** creates a new campaign/worktree/session from an admitted
   historical checkpoint and records its source snapshot.

This separation prevents “branch-live” from becoming an ambiguous command that
mixes LLM experimentation, worktree derivation, and outer-loop mutation.

## Configuration model

“Defaults” must be split into three explicit layers:

| Layer | Meaning | Required behavior |
| --- | --- | --- |
| Schema defaults | Values Rust applies when fields are absent | Versioned, tested, and identical wherever the profile is parsed |
| Recommended setup | An explicit operator-facing template known to work for a stated environment | No hidden provider, embedding, concurrency, or storage choices |
| Effective setup | Fully resolved values admitted for one campaign | Includes provenance: default, template, campaign, CLI, environment, or registry resolution |

The effective setup is composed from more than `run-profile.toml`. Today,
embedding model/provider, eval budgets, and the per-turn eval completion cap are
setup inputs persisted in `campaign.json`; they are not run-profile fields. The
recommended explicit eval cap is `32768`. The UI must not imply that the profile
alone is the complete configuration, and it must label retired profile-local
token fields as archival evidence rather than executable authority.

The profile-editor type-conformance boundary is now resolved without making the
passive record authoritative. `RunProfileRecord` is the shared editable
projection, while the canonical runtime parser/defaults/validator must accept
its exact serialized bytes before review or save. The typed profile service
returns hash-bound preview/save receipts, and setup separately returns the
normalized profile and effective `WalkConfigSnapshot`; no UI-local parser may
silently drop or rewrite runtime fields.

The shared typed profile/setup services now:

- parses through the canonical runtime owner;
- resolves setup-only fields and provider/embedding choices;
- shows the source of every effective value;
- shows validation constraints and explanatory copy for UI tooltips;
- returns an exact normalized profile and setup commitment preview;
- save reviewed profile bytes atomically and create-only, then commit the
  previewed setup identity through the existing receipt-first resumable path;
- reports each later setup side effect with staged receipts and an explicit
  recovery/abandon path rather than claiming one cross-filesystem/DB/Git
  transaction;
- compares the preview, admitted profile, campaign manifest, and commitment;
- refuses unapproved drift.

Do not rebuild a `Prototype1LoopCommand` from UI-local strings, JSON values, and
copied defaults. The discarded post-reset setup-form implementation remains
negative evidence for this rule.

## Controller modes and successor handoff

Stepped service authority is implemented as an explicit ownership contract.
Durable Continuous ownership remains a design target rather than a delivered
pause/resume surface.

### Stepped mode

- The session controller is the only driver.
- UI or CLI submits one transition command at a time.
- Long edges run as supervised jobs while reads remain available.
- At R13b, the successor starts in service-ready mode and acquires the successor
  session lease; it does not independently run to completion.
- The predecessor becomes read-only or exits after the successor is ready and
  ownership has been durably transferred.
- Clients resolve/reconnect to the successor endpoint from the session record.

### Continuous mode

Status: not delivered. The items below remain requirements for a future durable
controller rather than claims about UI-local Step automation.

- One controller owns autonomous advancement.
- UI and CLI observe, pause/cancel at admitted boundaries, or request an explicit
  takeover. They cannot manually step alongside the controller.
- Successor startup transfers the continuous lease; it does not create a second
  independent driver.

### Handoff lifecycle

R13b must expose durable sub-states rather than claiming “committed” too early.
The exact names should follow existing carriers, but the lifecycle must
distinguish at least:

1. selected successor and handoff admitted;
2. artifact installed and identity prepared;
3. predecessor/History retirement committed;
4. successor process started;
5. successor ready and lease acquired;
6. predecessor relinquished;
7. recovered or terminally blocked.

Each irreversible action needs an idempotent receipt and a documented recovery
path. The implementation does not need magical rollback, but it must never hide
which irreversible boundary was crossed.

## Dependency-ordered implementation roadmap

### Stage 0 — Establish a configuration-first live baseline

Goal: determine whether a current loop can pass setup/readiness and a bounded
walk using an explicit, evidence-backed setup before changing loop code.

Actions:

1. Use the June 30 admitted profile as the primary candidate template:
   `p1-gated-parent-3g1x3-p3-20260630-174316`.
2. Use the June 23 Perplexity-embedding run as the setup-only-field reference:
   `p1-dbdual-broad3g1x3-pplxembed-g25p-p25f-20260623-174553`.
3. Keep the June 15 filesystem-backed two-target run as a control.
4. Preserve the July 1 `p1-walkctx-...` embedding failure as a negative
   regression case.
5. Copy into a fresh disposable campaign; do not select a profile by mtime.
6. Make every model, route, embedding, storage, child count, parallelism,
   generation limit, timeout, and protocol setting explicit.
7. Diff source and candidate setup; reject every unexplained change.
8. Run setup, verify normalized profile/commitment/campaign values, then run
   ordinary doctor plus its current headless-TUI and live-protocol preflights.
   There is no standalone operator embedding execution preflight at this
   baseline. Verify the explicit embedding route and credential resolution, but
   record actual embedding execution as unproven unless a narrow preflight is
   approved or the first baseline attempt is deliberately used as the probe.
9. Advance only through a predeclared bounded transition and inspect after each
   step.

Success criteria:

- one fresh admitted campaign with exact config provenance;
- all currently preflightable provider dependencies proven before baseline
  work, with embedding execution explicitly marked proven or unproven;
- no unapproved profile drift;
- a bounded CLI-only walk reaches its declared stopping phase;
- any failure is classified as config/environment versus code contract, with a
  reproducer and no weakened invariant.

Stage 0 makes no source-code changes unless a code blocker is explicitly
approved for the next stage. Fresh disposable setup necessarily creates campaign,
closure, database, branch, identity, and commit artifacts; those effects must be
kept inside the new run and recorded.

### Stage 1 — Make configuration and admission one canonical contract

Goal: CLI and future UI preview and admit the same complete setup.

Deliverables:

- a type-reuse inventory of runtime profile, passive record, setup command,
  campaign manifest, setup report, and commitment carriers;
- conformance tests for field coverage, default equivalence, validation, and
  round-trip preservation;
- one effective-setup resolver with per-field provenance;
- one preview/admit service used first by the CLI;
- a narrow embedding execution/readiness preflight so a baseline run is not the
  first proof of its embedding route;
- structured explanations/constraints suitable for CLI help and UI tooltips;
- an explicit resolution for the documented full-batch parallelism drift:
  source currently derives `min(3, children.max)` while docs say
  `children.max`.

Acceptance:

- the same input produces the same normalized profile, campaign values, and
  commitment through every public setup path;
- missing/implicit embedding and provider readiness is visible before admit;
- no parser can successfully reserialize a profile while dropping runtime
  fields;
- preview has no side effects, configuration admission persists exactly the
  previewed values, and later setup stages expose receipts/recovery for partial
  filesystem, database, identity, branch, or commit effects.

### Stage 2 — Establish one durable controller/session authority

Goal: remove competing drivers before exposing mutations in the UI.

Deliverables:

- a historical regression reproducing the June 30 dual-driver collision through
  production replay/session code;
- a durable session identity and exclusive controller lease;
- an atomic cross-process ownership/fencing primitive plus a crash-recoverable
  attempt/receipt journal chosen explicitly; the current owner query mirror must
  not be used as the lease store;
- explicit stepped/continuous mode enforcement;
- expected-phase, epoch, evidence-version, and idempotency admission on mutation;
- one driver operation meaning “reconstruct, admit one typed edge, persist,
  produce receipt”;
- adapters so `prototype1-state`, `prototype1-step`, `prototype1-continue`, and
  walk mutation cannot bypass or disagree with the authority boundary;
- generation-zero identity bootstrap remains owned by `prototype1-setup`, whose
  completed checkout/identity authority creates the controller session at R3;
  walk and batch adapters reject R0/R1/R2a as mutable session targets while
  exposing setup/bootstrap evidence read-only for operator observability;
- cancellation/recovery semantics for effectful jobs;
- an R13b regression and corrected live/reconstruction meaning;
- atomic or recoverable predecessor-to-successor lease transfer.

Acceptance:

- two clients can inspect one session;
- two attempted drivers cannot mutate it concurrently;
- a repeated command is idempotent or rejected with a typed conflict;
- server restart reports the last committed boundary plus any explicit
  indeterminate/reconciliation state; it claims the same admitted phase only
  where sufficient durable carriers exist;
- a successor-ready timeout never reports a committed/ready state;
- old and new runtimes cannot both own generation mutation;
- checkout/binary epoch changes result in a clear reconnect/handoff receipt, not
  silent divergence.

This is the hard gate before UI Start/Step/Stop controls.

### Stage 3 — Publish a lossless sibling-client contract

Goal: make CLI and UI consumers of the same typed application service.

Deliverables:

- public typed request/response carriers for session snapshot, job state,
  transition receipt, blockers, allowed actions, and structured errors;
- operation-specific typed payloads for existing success responses that still
  use `Ok { message: String }`;
- a nonblocking snapshot path that does not wait behind a long effectful job;
- a backend-neutral, revision-tagged query/read endpoint so clients do not each
  restore an owner-DB snapshot at potentially different revisions;
- every effectful action, including nested live LLM work, supervised as a job;
- reliable server handling of client disconnects and response-write failures;
- client reconnection/session resolution across server and successor handoff;
- CLI rendering moved onto the public contract before UI mutation is added.

Acceptance:

- CLI and UI receive equivalent structured snapshots and receipts;
- no client parses human-readable messages to determine state or capability;
- session snapshots, evidence queries, and raw-query results identify the
  revision they observed;
- status/health remains responsive during live jobs;
- a timed-out/disconnected client cannot kill the server;
- public carriers preserve all `Job`, `Status`, `Audit`, and error structure.

Transport can remain framed JSON over a Unix socket for this stage. Streaming or
remote transport should be added only when the application contract is stable.

### Stage 4 — Build the operator observability read model

Goal: answer loop questions through typed summaries and evidence links while
preserving authority distinctions.

Deliverables:

- a canonical session snapshot and ordered transition-attempt/receipt view;
- explicit evidence references for phase admission and blockers;
- config provenance and derived-policy views;
- normalized parent/child/model/message/tool timelines using existing record
  types where possible;
- protocol adjudication/review records linked to the model/tool events they
  evaluate;
- incremental, ordered telemetry for live agent turns where post-attempt bundles
  are insufficient;
- a typed “what changed in this transition?” view. Do not assume existing
  `eval_transition_event` or `eval_walk_event_transition` is a generic relation
  delta ledger;
- raw immutable query retained as an expert surface.

Acceptance:

- for every phase, an operator can identify current state, admitting evidence,
  prior transition, next allowed action, and blocker;
- parent evaluation traces expose prompts, exchanges, tool calls/results, and
  linked protocol reviews without filesystem archaeology;
- incomplete/in-flight work is distinguishable from missing persistence;
- projections identify their source authority and integrity/provenance fields;
- the existing 57-question DB/filesystem parity worksheet is used as the
  answerability test rather than replaced by a new ad hoc checklist.

### Stage 5 — Make the UI excellent at inspection

Goal: provide an intuitive native operator view before adding full control.

Status: partially delivered. Controller/config/evidence panels, typed operation
feedback, and `egui_kittest` interaction coverage are implemented. A real
operator UI session against the live three-parent canary and complete UI-only
answerability/screenshot review remain pending.

Deliverables:

- persistent online/controller/job indicator;
- phase rail showing completed, active, blocked, and future phases;
- current-state card with evidence, last transition, blocker, and next actions;
- setup/effective-config inspector with shared explanatory tooltips;
- expandable parent/child agent trace, tool-call details, and protocol reviews;
- transition evidence/delta and provenance navigation;
- clear links or copyable coordinates for files, records, nodes, and reviews;
- raw response and raw query panels as secondary tools.

Acceptance uses three access tiers:

- **Full access:** source/files/CLI/DB can validate the UI claim.
- **DB/service only:** normalized/queryable evidence can answer the claim without
  browsing arbitrary files.
- **UI only:** a person can answer the question through actual UI interaction.

UI-only validation should use `egui_kittest`, real navigation, and screenshots.
The target is not merely that data exists; it is that the UI explains it.

### Stage 6 — Add UI/CLI control parity

Goal: complete a parent turn through UI only, CLI only, or mixed sequential
clients over one controller.

Status: the shared guarded Start/Step/status/idle-Stop client, UI buttons,
UI-local stop/resume scheduling, and successor endpoint following are
implemented and source-tested. CLI-only live validation is the current gate;
actual UI-only/mixed-client control and durable Continuous halt/resume remain
pending.

Deliverables:

- setup preview/admit, start, step, pause/stop, resume/recover, and explicit
  high-risk confirmations through the shared service;
- job progress and cancellation/recovery presentation;
- allowed actions derived from typed capabilities, not duplicated frontend
  policy;
- UI controls disabled with an explanatory blocker when another continuous
  controller owns the session;
- equivalent CLI JSON and human-readable receipts.

Acceptance:

- a fresh run can be set up and driven through one full parent turn CLI-only;
- the same can be done UI-only;
- UI and CLI may alternate commands without state divergence;
- a second simultaneous mutation is attached, rejected, or idempotently
  coalesced—it never launches another driver;
- all correctness failures remain visible and fail closed.

### Stage 7 — Implement exact replay and derived live branches

Goal: inspect history and test fixes without corrupting or pretending to rewind
the source run.

Deliverables:

- exact historical snapshots/receipts through production reconstruction and
  replay code;
- typed back/forward/jump cursor operations with no live side effects;
- response-fork operation for a recorded LLM/tool boundary;
- loop-branch operation creating a new campaign, worktree, session, and explicit
  source provenance;
- historical replay tests built from real persisted incident artifacts;
- comparison UI/CLI views between original and derived results.

Acceptance:

- replay performs no provider calls, worktree writes, History append, or live
  mutation;
- a live branch has a new identity and cannot write into the source run;
- source cursor, artifact hashes, code/binary version, config, and reason are
  durable;
- a historical bug can be replayed, a fixed binary can create a derived result,
  and both outcomes remain comparable.

### Stage 8 — Migrate authority to database-backed adapters by domain

Goal: support selectable database backends without erasing domain semantics.

Approach:

1. Keep the operator read model/query store backend-neutral.
2. Define backend capabilities and concurrency/transaction behavior explicitly.
3. Migrate one existing authority port at a time, reusing its domain types:
   transition receipts/journal, protocol/trace artifacts, MessageBox/channel,
   History, invocation/bootstrap, and other stores only after their contracts are
   inventoried.
4. For each surface, provide import/migration, dual-write parity, restart,
   corruption, and negative tests before claiming DB authority.
5. Keep filesystem implementations as adapters where useful; do not make schema
   drift permissive.

Acceptance:

- clients do not depend on a specific database backend;
- each migrated domain states which store is authority and which is projection;
- concurrent UI/CLI reads and controller writes have tested isolation semantics;
- a two-generation `dual-strict` canary proves parity before DB-only claims;
- “DB-only” is claimed surface by surface, never because one owner file exists.

## Verification ladder

Implementation should advance through the cheapest proof that can falsify each
slice. All test commands should be run by a test sub-agent and summarized back to
the primary implementation thread.

1. Static/type tests: profile field/default conformance, serde round-trip,
   capability vocabulary, session invariants.
2. Contract tests: preview/admit identity, command/receipt idempotency, two-client
   conflicts, typed snapshot equivalence.
3. Historical replay regressions: June 30 dual-driver collision, July 1 embedding
   fallback failure, R13b timeout semantics, known server/LLM incidents.
4. Temp-socket integration: disconnected clients, concurrent status, job
   cancellation, server restart, stale epoch, successor endpoint resolution.
5. Fake/local provider tests: setup readiness and handoff without live cost.
6. Bounded live gates: R0-R5, R5-R7, R7-R8, R10-R12, then R13b separately.
7. One full parent-turn canary in stepped mode.
8. Two-generation handoff canary in continuous mode.
9. UI access-tier tests with `egui_kittest`, screenshots, and interaction.
10. Backend parity and DB-only tests only after the relevant domain adapter exists.

Every stage should have a committed checkpoint before live advancement. A live
failure should result in a durable bug/reproducer and a fresh run after repair;
do not mutate a failed campaign into appearing valid.

## Initial work packet (completed)

The next implementation effort should be deliberately narrower than this full
roadmap:

1. Produce a read-only effective-config comparison for the four historical
   candidates listed in Stage 0.
2. Choose and copy one explicit candidate setup into a fresh disposable campaign.
3. Validate profile normalization, commitment, campaign/setup-only fields,
   derived parallelism, and provider/embedding readiness.
4. Run doctor and bounded preflights without advancing the loop.
5. If green, start one walk server and advance only to an agreed early boundary,
   using inspect-after-each-step discipline.
6. Classify any blocker. Prefer a config/profile correction; if it is a code
   contract, stop and create a reproducing regression/repair slice.
7. Do not add UI mutation controls during this packet.

Expected output:

- selected recommended explicit setup and why;
- exact effective values and provenance;
- readiness/preflight evidence;
- bounded phase ledger;
- blocker report or clean handoff into Stage 1.

## Current work packet

1. The shared public mutation seam is implemented and reviewed: CLI and UI use
   the same guarded Start, branch-aware exact Step, operation-status, and
   idle-server Stop requests without duplicating protocol admission.
2. Canonical UI profile load/default/validate/create-only save, setup
   preview/admit, and Run Config read-back are implemented. Defaults remain in
   the CLI/schema owner, loop policy in the selected profile, and effective
   values/provenance in returned typed carriers.
3. The bounded UI control surface and typed feedback are implemented and
   source-tested. Auto-advance remains explicitly Step-mode and UI-local; Pause
   suppresses the next Step and does not cancel admitted work.
4. Use `walk status --with-version`, `config`, `session-history`,
   `summary --verbose`, and the named immutable DB views for operator checks.
   Keep raw CozoScript expert-only and `jq` limited to targeted
   JSON/log/artifact forensics, never active-status inference.
5. Focused/full source and UI tests, change-scope review, and default-value
   conformance are complete. Finish the fresh V37 CLI-only three-parent packet
   from its new worktree, then separately perform actual UI-only/mixed-client
   interaction; do not promote source tests or older canaries into live proof.
6. Preserve the child-process lifecycle canary and historical r15-r18 replay
   fixture as separate gates; neither is proof of the three-parent operator
   path.

## Existing documents to preserve and reuse

- [`2026-06-17_typestate-loop-driver-plan.md`](2026-06-17_typestate-loop-driver-plan.md):
  phase/evidence map and the rule that walk is an operator interface, not
  authority.
- [`2026-06-22_prototype1-eval-store-data-model/`](2026-06-22_prototype1-eval-store-data-model/README.md):
  domain-aware persistence model and authority/projection distinctions.
- [`2026-06-25_cleanup-before-db-parity-decision.md`](2026-06-22_prototype1-eval-store-data-model/2026-06-25_cleanup-before-db-parity-decision.md):
  current DB-read and domain-port gaps.
- [`2026-06-25_remote-safe-db-persistence-implementation-plan.md`](2026-06-22_prototype1-eval-store-data-model/2026-06-25_remote-safe-db-persistence-implementation-plan.md):
  owner-scoped DB and remote-safe import direction.
- [`2026-06-30_p1-db-filesystem-parity-live-run-template/`](2026-06-30_p1-db-filesystem-parity-live-run-template/README.md):
  57-question answerability matrix and live step ledger.
- [`2026-06-30_p1-gated-live-run-handoff.md`](2026-06-30_p1-gated-live-run-handoff.md):
  dual-driver/handoff incident evidence.

This document integrates those lanes. It should not replace their detailed
inventories or rewrite their historical findings.

## Explicit non-goals

- No broad reorganization of `ploke-eval` before the controller and data
  contracts are proven.
- No UI-local copy of run defaults or transition policy.
- No second executor hidden inside the UI.
- No destructive rewind of live evidence.
- No generic storage trait that erases History, MessageBox, channel, artifact, or
  protocol semantics.
- No permissive loading to disguise schema/config drift.
- No claim that the current DB mirror is already the shared runtime authority.
- No user-facing polish that masks missing typed blocker/evidence data.

## Definition of the desired end state

The work is complete only when:

- one durable session and controller can drive the direct typed loop edges;
- UI and CLI show the same current state and submit the same typed commands;
- configuration is explicit, validated, explained, and provenance-bearing;
- predecessor/successor handoff transfers one controller authority safely;
- phase evidence, traces, tools, protocol reviews, and blockers are navigable;
- replay is exact and read-only, while branching creates a new explicit lineage;
- database backends can be selected behind domain-preserving adapters;
- CLI-only, UI-only, mixed-client, restart, handoff, and historical-fix workflows
  are all proven without weakening correctness guarantees.
