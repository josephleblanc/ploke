# Eval Benchmarking Report: Research Paper Extension

- date: 2026-04-06
- task title: Extend eval benchmark design using benchmark and Rust-focused research papers
- task description: Review local research papers on repository-level benchmarking, issue resolution, dependency inference, code generation, transpilation, and Rust tooling to refine the benchmark structure and metric suite proposed in the initial eval benchmarking report.
- related planning files:
  - `/home/brasides/code/ploke/docs/active/agents/2026-04-06_eval-benchmarking-research-papers-plan.md`

## Executive Summary

The paper review reinforces the central conclusion of the first report: `ploke` should be benchmarked as an execution-grounded repository system, not as a text-generation system and not only as an ingestion pipeline.

The strongest additions from the papers are:

1. Execution-grounded evaluation should dominate text similarity metrics.
2. Benchmark construction quality matters as much as benchmark size.
3. Repository-level evaluation should decompose into sub-capabilities, not only a single top-line score.
4. Secondary metrics like cost, localization, reproduction success, dependency hallucination, repair lift, and build stability are important enough to be first-class.
5. Rust and systems-language evaluation needs stronger attention to environment reproducibility, compiler behavior, and type/ownership failure modes than Python-centered benchmarks usually provide.

In practical terms, the papers push the `ploke` benchmark design toward:

- strict fail-to-pass and build/test execution validation
- richer benchmark curation metadata
- benchmark modes for retrieval, dependency/environment inference, issue reproduction, and repair loops
- more explicit measurement of repair and recovery paths
- optional stronger validation tiers beyond tests alone

## Papers Reviewed

The following local PDFs were reviewed:

- [`2406.03283v2.pdf`](/home/brasides/code/research/papers/benchmarks/2406.03283v2.pdf)
  - CatCoder: repository-level code generation with relevant code and type context
- [`2410.03981v3.pdf`](/home/brasides/code/research/papers/benchmarks/2410.03981v3.pdf)
  - survey on LLM-based code generation for low-resource and domain-specific languages
- [`2501.13699v1.pdf`](/home/brasides/code/research/papers/benchmarks/2501.13699v1.pdf)
  - DI-Bench: dependency inference with testable repositories
- [`2504.02605v1.pdf`](/home/brasides/code/research/papers/benchmarks/2504.02605v1.pdf)
  - Multi-SWE-bench: multilingual issue resolving
- [`2601.19207v1.pdf`](/home/brasides/code/research/papers/benchmarks/2601.19207v1.pdf)
  - REM2.0: Rust refactoring and equivalence verification
- [`2602.22764v1.pdf`](/home/brasides/code/research/papers/benchmarks/2602.22764v1.pdf)
  - automated repository-level Rust issue resolution with LLM agents
- [`2411.13990v6.pdf`](/home/brasides/code/research/papers/benchmarks/transpilation/2411.13990v6.pdf)
  - RustRepoTrans: repository-level translation benchmark targeting Rust
- [`2504.15254v3.pdf`](/home/brasides/code/research/papers/benchmarks/transpilation/2504.15254v3.pdf)
  - CRUST-Bench: C-to-safe-Rust transpilation benchmark

## What The Papers Change In The Original Report

The original report was already directionally right, but the papers sharpen several points.

### 1. Execution metrics should be primary, not just available

This is the clearest repeated theme.

CatCoder explicitly argues that text-similarity metrics are not adequate for function and repository code generation, and instead uses compile and test success as the main evaluation surface in the form of `compile@k` and `pass@k` ([2406.03283v2.pdf](/home/brasides/code/research/papers/benchmarks/2406.03283v2.pdf)).

Multi-SWE-bench and the Rust issue-resolution paper both center evaluation on resolved rate under held-out acceptance tests, with issue resolution counted only when the generated patch applies and passes the evaluation test suite ([2504.02605v1.pdf](/home/brasides/code/research/papers/benchmarks/2504.02605v1.pdf), [2602.22764v1.pdf](/home/brasides/code/research/papers/benchmarks/2602.22764v1.pdf)).

DI-Bench goes further by combining textual dependency matching with executability, and its results show that textual correctness and actual repository executability are not the same thing ([2501.13699v1.pdf](/home/brasides/code/research/papers/benchmarks/2501.13699v1.pdf)).

Implication for `ploke`:

- text similarity metrics should be secondary at most
- execution-grounded task outcomes should be primary
- retrieval and tool benchmarks should be connected to executable outcomes whenever possible

## Paper-Derived Design Principles

### 1. Benchmark the repository as a runnable system

DI-Bench, Multi-SWE-bench, Rust-SWE-bench, RustRepoTrans, and CRUST-Bench all treat repository tasks as executable tasks with real environments, not isolated completions.

For `ploke`, this supports:

- base-commit checkout
- environment capture
- execution validation
- full artifact retention for debugging

This aligns with the current `ploke-eval` direction, but it strengthens the case that environment and execution are not optional secondary concerns. They are part of the benchmark definition.

### 2. Separate task success from sub-capability success

The papers consistently avoid relying on a single top-line score.

Examples:

- Multi-SWE-bench uses resolved rate, success location, and average cost.
- Rust-SWE-bench adds reproduction success rate and average token/cost.
- DI-Bench tracks executability plus textual precision/recall/F1 and fake rate.
- RustRepoTrans reports pass@1 and self-debugging success `DSR@1`.
- CRUST-Bench distinguishes build success, test success, and repair-loop improvements.
- REM2.0 separates correctness, coverage, compilation behavior, latency, and verification success.

Implication for `ploke`:

The benchmark suite should be explicitly multi-layered:

- top-line task outcome
- sub-capability outcomes
- infra health
- cost and latency
- recovery behavior

### 3. Repair loops and recovery paths should be measured as their own capability

Several papers show that “initial attempt” and “after compiler/test/debug feedback” are materially different benchmark modes:

- RustRepoTrans measures `Pass@1` and `DSR@1`
- CRUST-Bench compares single-shot, compiler repair, test repair, and agent-based debugging
- the Rust issue-resolution paper adds reproduction success and studies full agent workflows

Implication for `ploke`:

Our benchmark design should not only log retries and recovery events. It should score them.

That means tracking:

- first-attempt success
- success after directed recovery
- success after tool-guided repair
- incremental cost of recovery
- whether recovery improves test pass but harms build stability

That last point is explicitly visible in CRUST-Bench, where test-informed repair improved correctness but reduced build stability ([2504.15254v3.pdf](/home/brasides/code/research/papers/benchmarks/transpilation/2504.15254v3.pdf)).

### 4. Systems-language evaluation needs stronger compiler- and environment-aware metrics

The Rust- and transpilation-focused papers repeatedly emphasize that systems languages are harder because of:

- stricter type systems
- borrow checking and ownership
- compilation pipelines
- environment and dependency complexity
- lower training-resource availability

Multi-SWE-bench explicitly notes lower performance in systems languages than in Python or Java and attributes this to build complexity and low-level semantics ([2504.02605v1.pdf](/home/brasides/code/research/papers/benchmarks/2504.02605v1.pdf)).

The LRPL/DSL survey also highlights the lack of standard benchmark datasets and standard evaluation approaches in lower-resource and specialized languages, including Rust ([2410.03981v3.pdf](/home/brasides/code/research/papers/benchmarks/2410.03981v3.pdf)).

Implication for `ploke`:

Rust-specific benchmarking should treat these as core signal, not noise:

- compile failures
- borrow/type mismatch classes
- dependency and environment setup failures
- snapshot reproducibility
- tooling and Cargo behavior

## Specific Benchmark-Curation Lessons

The papers provide strong guidance on how benchmark instances should be selected and validated.

### 1. Reproducible environment construction should be explicit

DI-Bench reuses repository CI workflows to build scalable execution-based evaluation ([2501.13699v1.pdf](/home/brasides/code/research/papers/benchmarks/2501.13699v1.pdf)).

Multi-SWE-bench constructs Docker-based runtime environments and iteratively fixes or discards broken environments during curation ([2504.02605v1.pdf](/home/brasides/code/research/papers/benchmarks/2504.02605v1.pdf)).

Rust-SWE-bench snapshots the crates.io index at the time of the original PR to preserve build reproducibility ([2602.22764v1.pdf](/home/brasides/code/research/papers/benchmarks/2602.22764v1.pdf)).

Implication for `ploke`:

The run manifest should grow to include:

- environment recipe identity
- dependency snapshot identity
- validation command identity
- build/test execution provenance

### 2. Fail-to-pass validation should be formalized

Multi-SWE-bench and Rust-SWE-bench both use controlled log states to verify that tasks really contain a fail-to-pass transition and do not introduce unrelated regressions.

This is especially important for `ploke` because evaluation on issue-resolution tasks depends on knowing:

- what constitutes the fail state
- what constitutes the pass state
- whether the patch simply avoided the test path or actually fixed the issue

Implication for `ploke`:

The benchmark schema should retain:

- base-run test results
- test-patch-only results when available
- fix-patch results when available
- regression counts
- pass-to-pass preservation

### 3. Manual verification still matters

Multi-SWE-bench uses large-scale manual verification.
Rust-SWE-bench also performs manual verification across issue quality, fail-to-pass integrity, and solution leakage.
The survey paper reinforces that manual evaluation remains important where automated metrics miss domain nuance.

Implication for `ploke`:

Even if the core suite is execution-based, it is worth planning for a small manually reviewed gold subset for:

- retrieval relevance
- tool-sequence quality
- prompt/context adequacy
- whether a run is “technically passing but practically poor”

### 4. Difficulty and corpus descriptors should be retained

Several papers discuss task difficulty, repository size, or corpus complexity.
CatCoder reports repository/task complexity descriptors.
Multi-SWE-bench discusses language and difficulty distribution.
DI-Bench shows that large repositories materially change outcomes.
CRUST-Bench reports repository size, coverage, and project-type descriptors.

Implication for `ploke`:

Benchmark items should retain descriptors such as:

- repo size
- file count
- LOC
- test count
- workspace/member count
- task type
- difficulty bucket
- language/system domain

These should be part of aggregate analysis, not just metadata.

## Metrics The Papers Suggest We Add Or Elevate

The first report listed a broad metric inventory. The papers indicate which metrics should be promoted from “nice to have” to “important”.

### 1. Outcome Metrics To Elevate

- resolved rate / pass@1 style task success
- compile success
- test success
- build stability after repair
- reproduction success rate
- fault localization accuracy or success location

Why:

- these repeatedly appear in repository-level and issue-resolution benchmarks

### 2. Cost Metrics To Elevate

- average cost per task
- average token usage per task
- cost per successful resolution
- extra cost from repair/retry/recovery loops

Why:

- both Multi-SWE-bench and Rust-SWE-bench treat cost as important
- public-facing performance writing is much stronger with cost curves, not just success curves

### 3. Dependency And Environment Metrics To Add

DI-Bench is especially useful here. It suggests adding:

- dependency precision
- dependency recall
- dependency F1
- fake rate for hallucinated or nonexistent dependencies
- executability rate after inferred environment/config

For `ploke`, this generalizes beyond external packages:

- hallucinated paths
- hallucinated tools or commands
- bad Cargo/package assumptions
- incorrect repo-internal dependency/context identification

### 4. Recovery Metrics To Add

Inspired by RustRepoTrans and CRUST-Bench:

- first-attempt success
- success after compiler/error feedback
- success after test feedback
- self-repair lift
- agent-based repair lift
- failure-mode transition after repair

This is directly relevant to your interest in directed prompting and recovery paths.

### 5. Secondary Quality Metrics To Add

Inspired by RustRepoTrans and REM2.0:

- code complexity/simplicity change versus reference
- coverage/feature coverage
- equivalence or semantic-preservation evidence when feasible
- overhead of extra instrumentation or tracing

These are not the first metrics to implement, but they are useful for deeper research work and article-quality analysis.

## New Benchmark Modes Suggested By The Papers

The first report proposed several benchmark modes. The papers suggest making the following modes explicit.

### 1. Dependency / Environment Inference Benchmark

Motivated by DI-Bench.

Purpose:

- measure whether the system can infer and assemble the environment needed to operate on a repo correctly

For `ploke`, this could include:

- build/test command inference
- Cargo/workspace interpretation
- dependency and toolchain assumptions
- workspace root/member identification

Primary metrics:

- executability
- dependency precision/recall/F1
- fake rate
- environment setup latency

### 2. Reproduction Benchmark

Motivated by Rust-SWE-bench.

Purpose:

- measure whether the system can reproduce an issue before trying to fix it

For `ploke`, this is especially relevant if agent workflows start with diagnosis or reproduction steps.

Primary metrics:

- reproduction success rate
- time/cost to reproduce
- correctness of reproduced failure mode

### 3. Retrieval With Type/Structure Context Benchmark

Motivated by CatCoder.

Purpose:

- measure whether adding structural or type-aware context improves repository-level outcomes

For `ploke`, that implies evaluating:

- plain retrieval
- retrieval plus graph/semantic context
- retrieval plus type/context-derived evidence

Primary metrics:

- task success lift
- compile/pass lift
- retrieval relevance
- prompt token overhead

### 4. Repair Loop Benchmark

Motivated by RustRepoTrans and CRUST-Bench.

Purpose:

- measure the value of iterative repair rather than only first-attempt generation

Primary metrics:

- repair lift
- repair cost
- build stability change
- error-class resolution rate

### 5. Strong Validation Benchmark

Motivated by REM2.0.

Purpose:

- evaluate stronger guarantees than tests alone where feasible

For `ploke`, this does not mean formal proof everywhere. It means keeping a tiered validation model:

- tier 1: build
- tier 2: tests
- tier 3: stronger semantic checks, invariants, or equivalence-style validation where available

## How The Papers Affect The Data We Should Gather

### Data We were underweighting before

The paper review suggests that the following data should be gathered more explicitly than the first report emphasized:

- issue reproduction outcomes
- localization accuracy
- dependency hallucination / fake-rate style errors
- build stability before and after repair
- repair-loop deltas
- repository size and difficulty bucket effects
- environment-construction provenance
- language/domain-specific failure classes

### Data that should likely remain secondary

The papers do not support making text similarity a primary benchmark target for `ploke`.

Metrics like:

- BLEU
- ROUGE
- edit similarity
- exact match

may still be useful in isolated sub-studies, but the paper set strongly suggests they should not drive the main benchmark narrative.

## Implications For Public-Facing Writing

The paper set also affects what claims will be credible in external research or blogs.

### 1. Stronger public claims

If `ploke` collects the recommended metrics, you can make stronger public claims about:

- end-to-end repository task success
- cost versus success tradeoffs
- retrieval-context improvements
- repair/recovery effectiveness
- scaling behavior by repository size and task difficulty
- systems-language-specific bottlenecks

### 2. Claims to avoid unless supported

The papers also imply caution around claims like:

- “better code quality” without executable outcomes
- “better repository reasoning” without localization or retrieval evidence
- “stronger Rust support” without explicit systems-language benchmarks

## Recommended Adjustments To The Original Benchmark Plan

Relative to the first report, I would make the following adjustments.

### 1. Promote these metrics into the first implementation wave

- task resolved rate
- compile success
- test success
- average cost
- average tokens
- retrieval/context latency
- tool-call success/failure
- reproduction success rate
- localization/file-level success
- repair lift
- environment/dependency hallucination rate

### 2. Add explicit benchmark modes

- dependency/environment inference
- issue reproduction
- repair-loop evaluation

### 3. Expand run metadata

Include:

- dependency snapshot identity
- environment recipe identity
- difficulty bucket
- task type
- repository size class
- language/domain class

### 4. Preserve richer failure taxonomies

At minimum:

- parse/transform failures
- retrieval misses
- dependency/environment failures
- compile failures
- borrow/type/system-language failures
- tool validation failures
- reproduction failures
- repair failures
- regression-introducing fixes

### 5. Keep room for stronger validation tiers

Inspired by REM2.0 and CRUST-Bench, the design should not assume “tests passed” is the only meaningful terminal validation signal.

## Final Takeaway

The papers validate the overall direction of the original report, but they tighten the standard.

The benchmark suite for `ploke` should be:

- repository-level
- execution-grounded
- multi-stage
- recovery-aware
- cost-aware
- Rust/systems-language conscious

And the benchmark outputs should not stop at:

- “did indexing finish”

or even:

- “did the task resolve”

They should also explain:

- whether the system could reproduce the problem
- whether it localized the right area
- whether the environment assumptions were correct
- whether recovery improved or destabilized the run
- how much the result cost
- which failure class dominated

That is the level of data collection that will best support:

- application improvement
- feature/config experimentation
- bottleneck diagnosis
- tool and agent workflow evaluation
- credible public-facing performance and research writing

## Most Important Open Question For The Next Pass

The next concrete design question is:

Which of the paper-inspired secondary metrics should be first-class in the initial `ploke-eval` schema, and which should be deferred to later benchmark versions?

My current answer would be:

First-class now:

- resolved/build/test outcomes
- cost/tokens
- localization success
- reproduction success
- repair lift
- environment/dependency correctness
- difficulty/repo-size descriptors

Later:

- deeper semantic-preservation checks
- code simplicity/complexity comparisons
- richer human-review overlays
