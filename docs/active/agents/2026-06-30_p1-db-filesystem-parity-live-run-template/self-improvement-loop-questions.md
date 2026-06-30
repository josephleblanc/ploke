# Self-improvement loop questions

**Status:** pre-run / long-horizon observability worksheet.
**Scope:** questions we want the Prototype 1 DB and persisted evidence to answer about whether the `ploke-eval` loop is safely and effectively improving the harness over many generations.

## Purpose and framing

The broad purpose of the loop is to improve the underlying harness: the whole `ploke` codebase, including `ploke-eval` itself. The loop asks an LLM-driven agent to make code changes through tools, evaluates those changes with benchmark/protocol evidence, selects successors, and repeats.

The long-term goal is a self-improvement system that can safely expose the largest possible editable code surface while preserving the authority core needed for the loop to continue. Some files are currently treated as immutable because edits to them could break succession, corrupt evidence, weaken safety gates, or cause harmful host-machine side effects. Later, we want call-graph/dataflow/static-analysis proofs for properties such as “no orphaned background processes,” “no host file deletion outside declared scope,” and “no unsafe authority-surface mutation.” Until then, the DB should help us observe and audit the approximation.

The DB is intended to become the primary interface for understanding what happened: what the agent tried, which tools it called, what changed, which policies and protocols evaluated those changes, why a successor was selected or rejected, and whether the harness is improving over time across benchmark score, correctness, reliability, cost, code quality, performance, and safety axes.

## How to use these questions

For each question, record the strongest available answer during the run:

- **Yes:** answered by authority-bearing evidence, not just a convenience projection.
- **Partial:** evidence exists but is incomplete, mirrored after the fact, or tied to projection-only state.
- **No:** absent, contradictory, or not persisted.
- **Not reached:** phase or feature did not execute in this run.

Keep DB facts, filesystem facts, operator-output facts, benchmark facts, protocol facts, and LLM-adjudication facts separate. A DB row may be excellent evidence, but only if the row is normalized enough to answer the question it is being used for.

For every question, start a Cozo-query trail:

1. Create the best available read-only Cozo query under `queries/`, named with a stable question id and short slug, for example `queries/surface-01__editable-files.cozo`.
2. Run it against the live loop owner DB through the CLI, not by opening SQLite directly:

   ```bash
   P1_BIN=/home/brasides/.ploke-eval/worktrees/p1-gated-parent-3g1x3-p3-20260630-174316/target/debug/ploke-eval
   P1_ROOT=/home/brasides/.ploke-eval/worktrees/p1-gated-parent-3g1x3-p3-20260630-174316
   "$P1_BIN" loop walk db_query \
     --repo-root "$P1_ROOT" \
     --script "$(cat queries/<question-id>__<slug>.cozo)"
   ```

3. Save the command, stdout/stderr, and interpretation under `responses/`, using the same question id and campaign/step context, for example `responses/surface-01__editable-files__step-05.md`.
4. When answering the question, reference both the query file and response file, then note whether the query was helpful.
5. If no useful DB query can answer the question, still record the best attempted query or probe, mark the answer as not DB-answerable, and add a schema-improvement note. Prefer normalized relations over `eval_record_ref`; use `eval_record_ref` only as provenance evidence, not as the answer itself.
6. For each response, include a short database-design note: what the current schema made easy, what required awkward joins or path/string interpretation, and what relation/field/index would make the question easier to answer next time.

## Surface partition and edit-scope questions

1. **surface-01:** What files, modules, crates, symbols, or graph nodes were declared editable for this run?
2. **surface-02:** What files, modules, crates, symbols, or graph nodes were declared immutable/protected?
3. **surface-03:** Which authority-bearing surfaces are protected because mutating them could break parent creation, child/successor execution, Crown transitions, History admission, handoff, DB schema, provider safety, tool permissions, or host-machine safety?
4. **surface-04:** Did any proposed or applied edit touch an immutable/protected surface?
5. **surface-05:** Were protected-write denials recorded with enough detail to identify the attempted path, tool call, rationale, and policy rule?
6. **surface-06:** Did the allowed surface include the largest safe set currently available, or are there broad exclusions whose safety rationale is unclear?
7. **surface-07:** Can we distinguish “temporarily immutable because static proofs are missing” from “intrinsically outside scope”?
8. **surface-08:** Did any generated code introduce new behavior that effectively bypasses the surface partition, such as invoking shell commands, modifying config, or changing tool schemas indirectly?
9. **surface-09:** Did the sealed or recorded artifact surface commitment match the actual files changed by the candidate?
10. **surface-10:** Can the DB answer surface questions without interpreting ad hoc path strings from logs?

## Host-machine and process-safety questions

1. **host-01:** Did any tool call attempt to read, write, delete, chmod, move, or symlink files outside the declared workspace/eval home?
2. **host-02:** Did any command spawn background processes, daemons, watchers, servers, subprocess trees, or detached children?
3. **host-03:** For every spawned process, do we know parent process, command, working directory, environment class, stdout/stderr paths, timeout, exit status, and cleanup status?
4. **host-04:** Did any process outlive the parent or walk command that launched it?
5. **host-05:** Were timeouts enforced and recorded for model calls, tool calls, builds, tests, benchmark runs, child runtimes, and successor waits?
6. **host-06:** Did any command access network, credentials, home-directory paths, temp directories, or global caches outside the admitted policy?
7. **host-07:** Were host-side effects bounded by policy and recoverable from logs/DB rows?
8. **host-08:** Could a future static-analysis proof consume the DB/code graph evidence to verify no forbidden process/filesystem side effects occurred?
9. **host-09:** When a safety policy blocked an action, did the loop continue safely, reject the candidate, or stop explicitly?
10. **host-10:** What schema gaps prevent answering host-machine safety questions today?

## Static-analysis and proof-readiness questions

1. **proof-01:** Which safety claims are currently enforced dynamically, which are enforced by static configuration, and which are only operator assumptions?
2. **proof-02:** Which code graph/call graph/dataflow nodes are part of the current immutable authority core?
3. **proof-03:** Are tool implementations, command execution wrappers, file-write APIs, process-spawn APIs, network clients, and cleanup paths identifiable as graph nodes in persisted evidence?
4. **proof-04:** Can a candidate edit be mapped from changed files to affected symbols, call edges, dataflow paths, and safety-sensitive APIs?
5. **proof-05:** Did the run record enough information to know whether an edit newly introduced calls to process-spawn, filesystem delete, network, environment, or credential APIs?
6. **proof-06:** Can we compare the pre-edit and post-edit call graph for safety-sensitive changes?
7. **proof-07:** Did the selection policy consider static-analysis or compile/test evidence when available?
8. **proof-08:** Are static-analysis failures preserved as first-class evaluation evidence?
9. **proof-09:** What DB schema would make formal proof artifacts queryable by generation, candidate, artifact, symbol, and safety property?
10. **proof-10:** Which current fields are too path/string-based to support future graph proofs?

## LLM/tool-loop trajectory questions

1. **trace-01:** Which model/provider/reasoning tuple generated each candidate attempt?
2. **trace-02:** What prompt, context snippets, retrieved files, benchmark description, and tool schemas were visible to the LLM at each step?
3. **trace-03:** Which tool calls were made, in what order, with what arguments and outputs?
4. **trace-04:** Which tool calls failed, retried, timed out, or produced denied edits?
5. **trace-05:** Did the agent inspect relevant evidence before editing, or edit without sufficient context?
6. **trace-06:** Did the agent validate after editing? If so, which checks were run and what did they prove?
7. **trace-07:** Were tool-call failures due to model behavior, tool design, stale context, permissions, retrieval errors, timeouts, or harness bugs?
8. **trace-08:** Did the LLM trajectory show improvement over generations: fewer failed tools, better localization, less churn, better validation, more direct fixes?
9. **trace-09:** Are intermediate/in-flight provider and tool events persisted, or only post-attempt bundles?
10. **trace-10:** Can we reconstruct why an accepted candidate looked better than rejected candidates from trace evidence alone?
11. **trace-11:** Did the model attempt to modify the harness/tooling in ways that improve its future ability to solve benchmarks?
12. **trace-12:** Which parts of the tool-loop trace are normalized in DB rows, and which require reading JSON logs or files?

## Patch, artifact, and harness-change questions

1. **change-01:** What exact files, hunks, symbols, tests, docs, configs, and generated artifacts changed in each candidate?
2. **change-02:** Did the edit target tool descriptions, TUI harness behavior, eval runner behavior, DB schema, protocol prompts, selection policy, benchmark harness, or unrelated code?
3. **change-03:** What was the stated rationale for each change, and can it be tied to prompt/context/tool evidence?
4. **change-04:** Did the change improve the harness itself, the benchmark target, the agent tools, the evaluation protocol, or only the current instance outcome?
5. **change-05:** Did any change alter the rules by which future generations are evaluated or selected?
6. **change-06:** Did any change make the system more permissive by weakening validation, skipping expected data, or tolerating schema drift?
7. **change-07:** Can we distinguish safe harness extensibility from unsafe self-modification of authority core?
8. **change-08:** Is each patch attempt tied to generator runtime, target artifact, base artifact, derived artifact, and child/successor runtime?
9. **change-09:** Are rejected patches retained for later learning/trajectory analysis?
10. **change-10:** Can the DB show whether successful changes cluster in particular crates/modules/tools over generations?

## Benchmark, protocol, and metric questions

1. **eval-01:** Which benchmark(s), instances, protocol(s), and evaluator versions were used for this run?
2. **eval-02:** What heldout benchmark result did each candidate achieve, and how does it compare to baseline?
3. **eval-03:** What non-benchmark metrics were computed: tool-call failure rate, validation pass/fail, compile/test status, runtime speed, cost, token usage, latency, code churn, static-analysis findings, or code-quality rubric?
4. **eval-04:** Which metrics were mechanized, which were LLM-adjudicated, and which were operator notes?
5. **eval-05:** Were LLM-adjudicated metrics insulated from circular self-authority and tied to rubric/protocol identity?
6. **eval-06:** Are protocol artifacts persisted enough to replay or audit the adjudication?
7. **eval-07:** Did selection prefer benchmark improvement, protocol quality, safety, robustness, cost, or exploration, and can we see the weighting?
8. **eval-08:** Did a candidate overfit the current single multi-swe-bench instance at the expense of general harness quality?
9. **eval-09:** Can we compare candidates across different benchmarks/domains once more benchmarks are added?
10. **eval-10:** What evidence would show that a tool created for one domain improved future performance on that domain rather than just this instance?
11. **eval-11:** Did benchmark/evaluator failure get distinguished from candidate failure?
12. **eval-12:** Are scores recorded with enough identity to prevent comparing incompatible evaluator versions or policies?

## Longitudinal self-improvement questions

1. **trend-01:** Across generations, is the harness improving on benchmark score?
2. **trend-02:** Across generations, is the harness improving on reliability: fewer crashes, fewer blocked runs, fewer stale-index failures, fewer tool failures?
3. **trend-03:** Across generations, is the harness improving on efficiency: fewer tokens, fewer tool calls, shorter wall time, fewer retries, faster benchmark/runtime execution?
4. **trend-04:** Across generations, is the harness improving on code quality or maintainability according to stated protocols?
5. **trend-05:** Across generations, is the editable safe surface expanding, shrinking, or staying the same, and why?
6. **trend-06:** Are changes accumulating technical debt in the authority core, DB schema, tool schemas, or protocols?
7. **trend-07:** Are regressions detected and attributed to a generation/candidate/artifact?
8. **trend-08:** Can we compare selected successors against rejected siblings to learn what selection pressure is doing?
9. **trend-09:** Are exploratory rejected attempts useful later, or are they just noise/cost?
10. **trend-10:** Does the run preserve enough lineage and metric history to answer improvement questions without replaying every file artifact manually?

## Tool and harness extensibility questions

1. **tools-01:** Did the agent create, modify, or remove tools used by future LLM calls?
2. **tools-02:** Were new or modified tools evaluated for safety, correctness, argument schema quality, error reporting, and usefulness?
3. **tools-03:** Can a tool change be tied to later improvements or regressions in trajectory metrics?
4. **tools-04:** Did tool schemas remain honest about side effects and protected-write behavior?
5. **tools-05:** Were tool outputs structured enough for the LLM and DB to reason about them?
6. **tools-06:** Did the agent improve context retrieval, editing, validation, or benchmark-running capabilities?
7. **tools-07:** Did any tool change expand host-machine authority or bypass intended permissions?
8. **tools-08:** Can tool changes be benchmarked across different domains, not just the current instance?
9. **tools-09:** Are tool-call traces linked to the exact tool implementation version used at the time?
10. **tools-10:** What DB schema would make tool evolution queryable as a first-class object?

## DB-as-primary-interface questions

1. **db-01:** Can an operator understand current run state primarily from normalized DB queries rather than reading many files?
2. **db-02:** Which important facts still require direct filesystem inspection?
3. **db-03:** Which facts are stored only as path refs, display strings, JSON blobs, or `eval_record_ref` provenance?
4. **db-04:** Which questions require joins that are awkward because candidate, artifact, runtime, attempt, tool call, evaluation, and selection identities are not aligned?
5. **db-05:** Are relation keys stable enough for long-horizon, multi-generation, multi-benchmark analysis?
6. **db-06:** Can the DB distinguish setup facts, live in-flight facts, post-attempt bundles, terminal facts, and operator/debug facts?
7. **db-07:** Can the DB represent failed/rejected/blocked paths as well as successful selected paths?
8. **db-08:** Can the DB answer both “what happened in this run?” and “is the harness improving across runs?”
9. **db-09:** Which useful query patterns should become documented views or higher-level CLI commands?
10. **db-10:** What schema changes would most improve answerability without weakening correctness or making imports permissive?

## Reproducibility and audit questions

1. **audit-01:** Can we replay enough of the run to understand each decision without live provider calls?
2. **audit-02:** Can we recover exact prompts, model responses, tool schemas, tool args/results, patches, validation output, benchmark output, and selection rationale?
3. **audit-03:** Are hashes/digests recorded for important files and artifacts so later mutation is detectable?
4. **audit-04:** Can two independent reviewers reach the same answer about why a successor was selected?
5. **audit-05:** Can we distinguish incomplete evidence from evidence that proves failure?
6. **audit-06:** Are operator interventions, restarts, timeouts, and manual cleanups recorded separately from autonomous loop actions?
7. **audit-07:** Can a run be resumed or diagnosed after interruption without relying on live memory?
8. **audit-08:** Can old runs be compared to new runs despite schema evolution, without silently accepting drift?
9. **audit-09:** Are benchmark and protocol inputs versioned enough for reproducibility?
10. **audit-10:** What evidence should be sealed into History vs stored as append-only observation vs left as debug logs?

## Minimal long-horizon evidence set

A run is most useful for evaluating self-improvement if it can answer these at minimum:

1. what surface was editable/protected and whether edits stayed within it;
2. which LLM/model/tool trajectory produced each candidate;
3. what exact patch/artifact/runtime resulted from each candidate;
4. what host-machine side effects occurred and how they were bounded/cleaned up;
5. what benchmark/protocol/metric evidence evaluated each candidate;
6. why the selected successor beat rejected alternatives, or why no successor was selected;
7. whether the selected change improved the harness, the target instance, tooling, protocols, safety, or only a projection;
8. whether authority-bearing surfaces and History/handoff invariants were preserved;
9. whether the DB schema made those answers easy and normalized, or forced file/log/path-string reconstruction;
10. how the run contributes to longitudinal evidence of improvement or regression over generations.
