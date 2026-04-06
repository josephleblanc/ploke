# Eval Benchmarking Report

- date: 2026-04-06
- task title: Benchmark structure and metric inventory for ploke application evaluation
- task description: Survey the current eval harness and adjacent crates to identify the full benchmark structure and data suite needed to improve the application, compare configs and features, analyze bottlenecks, evaluate tool and agent behavior, and support public-facing research and performance writing.
- related planning files:
  - `/home/brasides/code/ploke/docs/active/agents/2026-04-06_eval-benchmarking-next-steps-plan.md`

## Executive Summary

`ploke-eval` is currently an ingestion harness, not an application benchmark harness. Today it can normalize a Multi-SWE-bench item, reset a target repo to `base_sha`, run the headless app through `/index`, and persist a DB snapshot. That is a good base for reproducible corpus preparation, but it does not yet benchmark the behavior that matters for the actual application: retrieval quality, context construction, LLM session behavior, tool execution, edit staging/application, recovery paths, or task completion.

A full benchmark structure for `ploke` should be organized as a layered run pipeline:

1. Run specification and reproducibility metadata
2. Corpus and indexing baseline
3. Retrieval and context benchmark
4. Agentic task execution benchmark
5. Outcome scoring and regression analysis
6. Cross-run aggregation for ablations, scaling curves, and public reporting

The main design conclusion from this survey is that the codebase already contains many of the low-level observability hooks we want:

- `ploke-eval` already has prepared manifests, run directories, and per-run logs
- the indexer already emits progress and terminal status
- prompt assembly already emits context plans and token estimates
- the chat loop already produces structured session reports and typed tool lifecycle events
- observability already persists conversation turns and tool calls into the DB

The missing piece is not raw signal generation. The missing piece is a benchmark-side run model that captures those signals coherently, adds task-level success criteria, and makes the outputs stable enough for comparison, diagnosis, and publication.

## Research Scope

This report is based on direct review of:

- `crates/ploke-eval`
- `crates/ploke-tui`
- `crates/ploke-llm`
- `crates/ploke-rag`
- `crates/ploke-db`
- `crates/ploke-core`
- `crates/ingest/syn_parser`
- `crates/ingest/ploke-transform`
- `crates/ingest/ploke-embed`

It also incorporates parallel sub-agent surveys of:

- the current `ploke-eval` runner shape and artifact model
- the TUI/chat/tool/application surface
- the retrieval/index/storage/ingestion stack

## Current Baseline In The Codebase

### 1. What `ploke-eval` does today

The current prepared-run schema is centered on `PreparedSingleRun` and `EvalBudget` in [`spec.rs`](/home/brasides/code/ploke/crates/ploke-eval/src/spec.rs), with Multi-SWE-bench instance normalization in [`msb.rs`](/home/brasides/code/ploke/crates/ploke-eval/src/msb.rs).

The main runner in [`runner.rs`](/home/brasides/code/ploke/crates/ploke-eval/src/runner.rs):

- loads a run manifest
- resets the repo and checks out `base_sha`
- creates an in-memory runtime DB
- activates a hardcoded Codestral embedding runtime
- boots a headless `ploke-tui` `TestRuntime`
- runs `/index`
- waits on `AppEvent` and `IndexingStatus`
- writes checkpoint/failure/final DB snapshots and simple status files

This produces an ingestion-focused artifact set:

- `run.json`
- `repo-state.json`
- `execution-log.json`
- `indexing-status.json`
- `snapshot-status.json`
- `indexing-checkpoint.db`
- `indexing-failure.db`
- `final-snapshot.db`
- run-scoped `config/`

That is enough to validate that the indexing path runs and to preserve the produced graph DB. It is not enough to measure the application end to end.

### 2. What the application stack already exposes

The application stack is considerably richer than the current eval driver:

- Prompt construction and context planning happen in [`rag/context.rs`](/home/brasides/code/ploke/crates/ploke-tui/src/rag/context.rs), including `ContextPlanSnapshot` and token estimates.
- The LLM manager pairs `ChatEvt::Request` and `PromptConstructed`, then executes a multi-turn session loop in [`llm/manager/mod.rs`](/home/brasides/code/ploke/crates/ploke-tui/src/llm/manager/mod.rs) and [`llm/manager/session.rs`](/home/brasides/code/ploke/crates/ploke-tui/src/llm/manager/session.rs).
- Session-level error and outcome reporting already exists in [`llm/manager/loop_error.rs`](/home/brasides/code/ploke/crates/ploke-tui/src/llm/manager/loop_error.rs) via `ChatSessionReport`, `SessionOutcome`, and typed loop errors.
- Tool dispatch and result routing exist in [`tools/mod.rs`](/home/brasides/code/ploke/crates/ploke-tui/src/tools/mod.rs), [`rag/tools.rs`](/home/brasides/code/ploke/crates/ploke-tui/src/rag/tools.rs), and [`app_state/events.rs`](/home/brasides/code/ploke/crates/ploke-tui/src/app_state/events.rs).
- Tool and conversation observability is already persisted in [`ploke-tui/src/observability.rs`](/home/brasides/code/ploke/crates/ploke-tui/src/observability.rs) using DB types in [`ploke-db/src/observability.rs`](/home/brasides/code/ploke/crates/ploke-db/src/observability.rs).
- Retrieval strategies and context assembly are already configurable in [`ploke-rag/src/core/mod.rs`](/home/brasides/code/ploke/crates/ploke-rag/src/core/mod.rs) and surfaced to users in [`ploke-tui/src/user_config.rs`](/home/brasides/code/ploke/crates/ploke-tui/src/user_config.rs).
- Context-level stats already exist in [`ploke-core/src/rag_types.rs`](/home/brasides/code/ploke/crates/ploke-core/src/rag_types.rs) as `ContextStats`.

### 3. The major gap

The app has benchmarkable behavior. The harness just does not drive or record most of it yet.

## What Needs To Exist To Benchmark The Actual Application

The benchmark system should be structured around five benchmark modes that share a common run model.

### A. Ingestion Baseline Benchmark

Purpose:

- validate parser, transform, embed, and index health
- measure ingestion throughput and bottlenecks
- establish cold-start and warm-start baselines

Scope:

- repo checkout
- discovery
- parse
- resolve
- transform
- embed
- sparse/dense index build
- snapshot save/load

This is the current closest match to `ploke-eval`.

### B. Retrieval Benchmark

Purpose:

- evaluate how well the indexed application can recover relevant code for a task
- compare dense, sparse, hybrid, rerank, token-budget, and scope settings

Scope:

- load a prepared snapshot or live DB
- run retrieval queries derived from benchmark tasks
- assemble context
- score returned evidence against task-linked relevance signals

This mode isolates retrieval quality from LLM behavior.

### C. Agentic Task Benchmark

Purpose:

- evaluate the actual user-facing application loop
- compare prompts, instructions, tool sets, recovery strategies, and workflow changes

Scope:

- prompt assembly
- LLM turn loop
- tool selection and execution
- tool failures and retries
- edit staging and application
- task exit conditions
- optional validation commands

This is the benchmark mode needed for improving the application itself.

### D. Replay / Regression Benchmark

Purpose:

- rerun historical task traces, replay batches, and failure cases
- verify that changes do not regress known working or known failing paths

Scope:

- replay indexed embedding batches
- replay saved conversations/tool traces
- replay fixed prompt and context plans against updated code

This is important for deterministic regression checks and writeups about fixes.

### E. Config Sweep / Research Benchmark

Purpose:

- run controlled ablations across embeddings, retrieval settings, prompts, models, policies, and feature flags
- generate data suitable for internal decision-making and public-facing articles

Scope:

- identical task set
- matrixed config variants
- repeated runs for variance estimation
- aggregate analysis output

## Common Run Structure

Every benchmark mode should share a run structure with the following layers.

### 1. Immutable Run Spec

This should capture:

- benchmark mode
- dataset source and dataset version
- benchmark item id
- repo org/name/url
- repo base SHA
- repo head SHA at preparation time
- local checkout path
- `ploke` git SHA
- Rust toolchain version
- OS/host identity
- random seed if any
- config fingerprint
- prompt/instruction fingerprint
- tool schema fingerprint
- embedding model/provider/dims
- retrieval strategy and scope settings
- benchmark budget settings

This is the minimum reproducibility layer.

### 2. Timed Phase Ledger

Each run should record a phase ledger, not just a step list. For each phase:

- phase name
- parent phase
- start/end timestamps
- wall time
- CPU time if available
- success/failure/cancelled
- error classification
- selected summary counters
- referenced artifact ids

This is what will support bottleneck analysis and public performance charts.

### 3. Event Stream Capture

The benchmark driver should persist the typed application signals already produced:

- indexing status events
- prompt/context events
- session reports
- tool lifecycle events
- conversation turn records
- provider responses and token usage
- recovery events

This should be stored as structured JSONL or equivalent append-only records per run.

### 4. Outcome Ledger

Each run should end with a scored outcome record:

- task status: success, partial, failed, infra-failed, cancelled
- produced patch status
- validation status
- retrieval quality summary
- agent behavior summary
- cost/latency summary
- dominant failure category if not successful

This is the comparison surface for regressions and article tables.

### 5. Artifact Bundle

The harness should retain:

- normalized prompt input
- prompt actually sent
- context plan snapshots
- tool request and response payloads
- assistant outputs
- DB snapshots or deltas
- edit proposals and final applied edits
- validation logs
- traces/log files

This is required for deep diagnosis and compelling public examples.

## Metric Inventory

The goal in this first pass is breadth, not expedience. The list below is intentionally broader than what will be easy to implement immediately.

### 1. Reproducibility And Run Metadata

Track:

- dataset name, version, split, language, row hash
- benchmark item id and title
- repo identity and size descriptors
- repo checkout SHA and detached base SHA
- `ploke` git SHA
- runner binary version
- host OS, CPU, memory class, disk class
- Rust toolchain version
- API providers and endpoints used
- model ids, embedding ids, dimensions
- config hash
- prompt hash
- tool schema hash
- enabled feature flags and env-driven behavior

Why it matters:

- compare runs honestly
- explain variance
- support reproducibility in research articles

### 2. Corpus And Repo Shape Descriptors

Track:

- files discovered
- Rust files considered
- workspace/member counts
- crates discovered
- total bytes indexed
- LOC
- dependency graph size if derivable
- macro-heavy file counts
- generated-file counts if detectable
- test-file and benchmark-file counts
- file size distribution

Why it matters:

- normalize performance by corpus size/shape
- explain outliers
- build scaling curves for public reporting

### 3. Parse / Resolve / Transform Quality

Track:

- discovery time
- parse time per file and total
- resolve time
- transform time
- files parsed successfully
- partial parse count
- failed parse count
- unresolved item count
- merge counts
- prune counts
- module-tree build failures
- node counts by type
- edge counts by relation
- workspace metadata rows written
- schema/setup time
- diagnostics emitted

Why it matters:

- identify correctness and performance bottlenecks in ingestion
- understand graph shape changes introduced by parser or transform work

### 4. Embedding And Index Build Metrics

Track:

- snippets queued
- snippets embedded
- skipped snippets
- empty snippet count
- truncation count
- batch sizes
- batches processed
- provider round-trip latency
- queue wait time
- retry count
- timeout count
- request concurrency
- requests/sec throttling behavior
- cost estimate
- embedding dims mismatch errors
- active embedding-set transitions
- HNSW build/update time
- BM25 build/rebuild time
- checkpoint counts

Why it matters:

- improve throughput and stability
- compare embedding backends
- quantify ingestion cost and latency tradeoffs

### 5. DB / Storage Metrics

Track:

- DB initialization time
- relation write counts
- write latency by relation family
- snapshot size
- snapshot write/load time
- DB file growth across phases
- restore time
- backup failure rate
- row count deltas
- index footprint

Why it matters:

- storage cost and persistence behavior are part of product performance
- public articles often need concrete storage and persistence numbers

### 6. Retrieval Quality Metrics

Track:

- strategy used: dense, sparse, hybrid, reranked hybrid
- retrieval latency total and by sub-path
- BM25 timeout/fallback rate
- dense query embedding latency
- overlap between dense and sparse hits
- top-k hit count
- empty-result rate
- relevance metrics where labels exist:
  - recall@k
  - precision@k
  - MRR
  - nDCG
- scope-filter correctness
- candidate diversity
- reranker lift
- evidence coverage of gold files or gold edit locations

Why it matters:

- retrieval quality is a first-order determinant of app quality
- this supports ablations and research writeups

### 7. Context Assembly And Prompt Metrics

Track:

- prompt construction latency
- conversation-only fallback rate
- context-plan count per run
- included message count
- excluded message count
- context parts count
- unique files represented
- per-part token estimates
- total estimated context tokens
- truncation count
- dedup removal count
- per-file quota hits
- ratio of user-history tokens to code-context tokens
- exact prompt hash and stored prompt text

Why it matters:

- helps diagnose whether poor outcomes are retrieval failures, assembly failures, or prompt-budget failures
- makes prompt and context changes experimentally comparable

### 8. LLM Session Metrics

Track:

- session count per task
- turns per session
- attempt count
- chain depth
- final `SessionOutcome`
- finish-reason distribution
- abort vs exhaust vs success
- token usage:
  - prompt
  - completion
  - total
- provider latency
- time to first token if available
- queue time if available
- tokens per second if available
- cost estimate
- unknown tool rate
- malformed response rate
- provider protocol error rate
- transport timeout rate
- user cancel rate

Why it matters:

- this is the core application behavior
- it directly supports work on prompts, instructions, workflow design, and provider choices

### 9. Tool-Call And Agentic Workflow Metrics

Track:

- tool-call count per turn and per task
- tool mix by tool name
- tool-call latency
- tool-call success rate
- tool-call failure rate by tool and failure kind
- deserialize/validation/execution failure breakdown
- timeout rate
- retry rate
- repeated identical tool-call rate
- tool-call chain limit hits
- recovery path usage
- ratio of successful tool calls to successful tasks
- tool output size
- tool argument size
- staged edit count
- auto-confirm rate
- apply success rate
- denial/rejection rate
- create-file success rate
- namespace/path resolution failure rate

Why it matters:

- this is where agentic infrastructure quality shows up
- it supports work on tool schemas, prompts, recovery instructions, and workflow design

### 10. Validation / Outcome Metrics

Track:

- patch produced or not
- files changed
- diff size
- syntactic validity
- compile success
- test success
- benchmark-item success
- partial success
- human-review-needed status
- time to first viable patch
- time to final successful patch
- validation command latency
- post-edit regression rate

Why it matters:

- this is the actual task-completion surface
- it bridges infrastructure metrics to user-visible utility

### 11. Reliability / Operations Metrics

Track:

- cold-start vs warm-start performance
- cache hit/miss rates
- channel lag and backpressure
- event drop rates
- memory peak and average
- CPU usage
- disk IO
- network error rates
- API quota/rate-limit events
- determinism across repeated identical runs
- variance across repeated identical runs
- flaky benchmark item rate

Why it matters:

- needed for diagnosing instability
- important for serious performance articles

### 12. Research / Blog-Facing Aggregate Metrics

Track:

- scaling curves versus repo size
- success versus latency frontiers
- success versus cost frontiers
- retrieval-quality lift from config changes
- failure taxonomy frequency
- dominant bottleneck by phase
- cold versus warm improvement
- ablation tables for prompts, tools, retrieval, and embedding configs
- variance bars across repeated runs
- representative artifact examples for case studies

Why it matters:

- this is the material needed for public-facing research and performance writeups

## Additional Evaluation Axes Worth Considering

Beyond the items you explicitly listed, the benchmark suite should also support:

### 1. Determinism And Replayability

- same config, same task, repeated runs
- variance in outcome, tool sequence, retrieval set, latency, and cost

### 2. Sensitivity Analysis

- how fragile outcomes are to:
  - prompt wording
  - token budgets
  - retrieval top-k
  - provider/model changes
  - embedding model changes
  - tool timeout changes

### 3. Error Recovery Quality

- whether recovery hints actually recover the run
- whether retries help or just add cost
- whether specific failure classes need new instructions or new tooling

### 4. User-Experience Proxies

- time to first useful answer
- time to first actionable code reference
- time to first valid patch proposal
- number of wasted turns before useful work begins

### 5. Incremental And Update Workflows

The current runner is full-ingest oriented. The actual app also needs evaluation around:

- reindex/update flows
- save/load/restore correctness and speed
- edit-then-retrieve coherence
- workspace freshness transitions

### 6. Benchmark Artifact Usability

The outputs themselves should be reviewed as a product surface:

- can a failed run be debugged from artifacts alone
- can an article graph be generated from the aggregates without manual log archaeology
- can an internal regression be localized quickly

## Recommended Benchmark Modes And Their Primary Metrics

### 1. Ingestion Baseline

Primary metrics:

- ingestion success rate
- phase timings
- parser/transform failure taxonomy
- embedding throughput
- index build time
- snapshot time and size

### 2. Retrieval Benchmark

Primary metrics:

- recall@k
- MRR
- nDCG
- retrieval latency
- context-token efficiency
- strategy comparison lift

### 3. Agentic Task Benchmark

Primary metrics:

- task success rate
- turns per task
- tool success/failure
- recovery success rate
- time to viable patch
- validation pass rate
- end-to-end latency
- total cost

### 4. Regression / Replay Benchmark

Primary metrics:

- deterministic replay rate
- regression count against saved traces
- diff in outcome, latency, or tool behavior versus baseline

### 5. Config Sweep Benchmark

Primary metrics:

- success/latency/cost frontier
- retrieval quality deltas
- prompt/tool/policy ablation impact
- variance by config

## Existing Hooks The Harness Should Reuse

The first extension of `ploke-eval` should prefer reuse over building duplicate observability systems.

### Strong existing hooks

- `PreparedSingleRun` and run directories in [`ploke-eval/src/spec.rs`](/home/brasides/code/ploke/crates/ploke-eval/src/spec.rs)
- run-scoped logs in [`ploke-eval/src/tracing_setup.rs`](/home/brasides/code/ploke/crates/ploke-eval/src/tracing_setup.rs)
- indexing events in [`ploke-embed/src/indexer/mod.rs`](/home/brasides/code/ploke/crates/ingest/ploke-embed/src/indexer/mod.rs)
- `ContextPlanSnapshot` and prompt construction events in [`ploke-tui/src/rag/context.rs`](/home/brasides/code/ploke/crates/ploke-tui/src/rag/context.rs)
- `ChatSessionReport` and loop errors in [`ploke-tui/src/llm/manager/loop_error.rs`](/home/brasides/code/ploke/crates/ploke-tui/src/llm/manager/loop_error.rs)
- tool lifecycle events in [`ploke-tui/src/app_state/events.rs`](/home/brasides/code/ploke/crates/ploke-tui/src/app_state/events.rs)
- persisted conversation/tool observability in [`ploke-tui/src/observability.rs`](/home/brasides/code/ploke/crates/ploke-tui/src/observability.rs) and [`ploke-db/src/observability.rs`](/home/brasides/code/ploke/crates/ploke-db/src/observability.rs)
- retrieval/config surfaces in [`ploke-rag/src/core/mod.rs`](/home/brasides/code/ploke/crates/ploke-rag/src/core/mod.rs) and [`ploke-tui/src/user_config.rs`](/home/brasides/code/ploke/crates/ploke-tui/src/user_config.rs)

### Important current limitations

- eval currently hardcodes the embedding preset in [`ploke-eval/src/runner.rs`](/home/brasides/code/ploke/crates/ploke-eval/src/runner.rs)
- `execution-log.json` is currently a simple step list, not a timed phase report
- eval budgets are present in manifests but not yet acting as a benchmark-side enforcement and reporting layer
- the current runner does not drive retrieval, tool, or session paths
- `ploke-embed/src/events.rs` is effectively empty, so embedding telemetry does not yet have a dedicated persisted event schema

## Suggested Output Tables

To make the data useful for internal analysis and external writing, the benchmark suite should eventually export stable tables such as:

- `runs`
- `run_phases`
- `run_errors`
- `repo_descriptors`
- `ingestion_metrics`
- `db_metrics`
- `retrieval_queries`
- `retrieval_hits`
- `context_plans`
- `llm_sessions`
- `llm_turns`
- `tool_calls`
- `edit_proposals`
- `validation_results`
- `aggregate_experiments`

These can be materialized as JSONL, SQLite, or DB rows, but the key requirement is that they be queryable and stable across benchmark versions.

## Practical Takeaway

The benchmark system you want should not be framed as one harness. It should be framed as one run model with multiple benchmark modes.

The priority ordering implied by the current codebase is:

1. preserve and enrich the ingestion baseline already in `ploke-eval`
2. add retrieval and context benchmarks next
3. add the full agentic task loop after that
4. make all of them comparable through a shared run schema and aggregate tables

That structure gives you:

- the data needed to improve internals
- the data needed to compare configs and new features
- the data needed to identify bottlenecks
- the data needed to evaluate tool calls and agentic workflows
- the data needed to write credible public-facing performance and research articles

## Immediate Follow-Up Questions For The Next Design Pass

The next pass should turn this inventory into a concrete spec by answering:

1. What is the canonical run schema for all benchmark modes?
2. Which outputs belong in the per-run artifact bundle versus aggregate tables?
3. What are the first success criteria for retrieval and agentic task benchmarks?
4. Which gold signals are available from Multi-SWE-bench items directly, and which must be derived or annotated?
5. Which existing app events should become first-class persisted benchmark events?
