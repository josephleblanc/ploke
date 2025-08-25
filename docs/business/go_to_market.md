# Ploke: Go-To-Market and Proof Plan

This document gives you a fast path from “insanely capable tech” to “paying customers,” with a concrete proof-of-efficacy plan, distribution tactics, monetization, and two executable options to get to revenue quickly.

Sections:
- What you’re really asking
- Five solid solution paths → five better ones
- Synthesis: choose two best plays
- Proof-of-efficacy (Rust-native benchmarks)
- Distribution and growth (without SF travel)
- Monetization options and pricing
- 14‑day execution plan
- Risk map and mitigations

---

## What you’re really asking

- How do I avoid being lost in the noise once I put up a website?
- How can I prove this is objectively valuable, especially to Rust devs?
- What’s the fastest path to first dollars so I can keep building?
- What’s a suitable strategy for this specific tech (Rust-native parser/graph/agents)?
- How do I position, price, and distribute without a network or budget?

---

## Five solid solution paths

1) Services-first (paying pilots)
- Position as “AI Code Archaeology for Rust.” Sell fixed-price pilots to Rust teams: code graph + insights + actionable refactor plan.
- Outcome: cash-in-hand, case studies, real users.

2) Open-core + Pro features
- Free CLI and SDK; paid: large-repo performance, enterprise auth, team collaboration, managed embeddings DB, priority support.

3) SDK-first for ecosystem embedding
- Become the Rust code understanding layer any AI tool can embed. Offer permissive license + commercial license.

4) Benchmarks-first credibility
- Publish rigorous Rust-native benchmarks vs baselines. Ship reproducible harness. Get cited and linked.

5) Developer-experience “wow” flow
- One-command “analyze my crate” with beautiful TUI + interactive artifacts (graph, queries, embeddings). Then capture emails.

---

## Five better solutions (ruthlessly focused)

1) “Paid Pilot in a Box”
- A templated 2-week engagement: ingest a target repo, produce code graph, identify hotspots, generate 3-5 PRs, deliver metrics. Flat $5k.

2) “Rust Code Intelligence as a Service”
- A hosted endpoint + CLI that returns graph slices, call graphs, symbol search, embeddings, and suggested refactors. Pay per seat or usage.

3) “Rust AI Agent Plugins”
- Prebuilt agents: “Resolve Traits,” “Find Dead Code,” “Extract Public API,” “Generate Call Graph.” Devs call agents as tools with stable IDs.

4) “Reproducible Benchmarks + Leaderboard”
- Host a public dashboard where crates like tokio, serde, bevy, rust-analyzer are re-ingested weekly with stable metrics (speed, coverage, recall). Let others add their crate.

5) “Viral Demo Repos”
- Curated 3 demo repos: “Refactor tokio module X,” “API map for serde,” “Visualize clap code graph.” With scripts that anyone can run and see value in minutes.

---

## Synthesis: two best plays to execute now

A) Services-first with public proofs (fastest to cash + credibility)
- Productize a $3k–$7.5k “Rust Code Intelligence Pilot.”
- Ship a reproducible benchmark suite + blog post to lend authority.
- Deliver rich PDFs and PRs to the client; extract case studies.

B) Open-core tool + Pro addon (repeatable distribution)
- Free: CLI/TUI for ingest + graph queries + embeddings export.
- Pro: large-repo acceleration, parallel ingest tuning, multi-user workspace, history timelines, BM25/HNSW bundles, SSO, support.
- Use pilots to seed the first Pro customers.

---

## Proof-of-efficacy: Rust-native benchmarks

Goal: show objective superiority where it matters to Rust teams: speed, completeness, and actionable outputs.

1) Corpora to test
- Real-world crates: tokio, serde, rust-analyzer, clap, bevy, hyper, reqwest, polars, ripgrep, actix-web.
- Sizes: small (10–30 KLOC), medium (50–150 KLOC), large (200–800 KLOC).

2) Metrics
- Parse throughput: files/s, LOC/s on fixed hardware.
- Graph coverage: number of nodes/edges extracted vs expected (sanity via known counts or determinism checks).
- Call graph resolution rate: percent of calls resolved to definitions (phase gate as you implement).
- Type resolution precision/recall: (start with subsets; document methodology).
- Embedding pipeline latency: time per N nodes; index build time; query latency; recall at k for known queries.
- Determinism: hash stability across runs with same commit and cfg.

3) Baselines
- tree-sitter based tools (where applicable), syn-only flows, rust-analyzer indexes (as a proxy), ripgrep+ctags style grep baseline for search.

4) Harness design
- Single binary “ploke-bench” that:
  - Clones repo at fixed commit.
  - Runs ingest with pinned flags and warm/cold caches.
  - Emits JSON Lines per stage with timestamps.
  - Computes summary tables and writes Markdown + CSV.
- Version pinning: rustc/stable version; hardware profile; OS; CPU governors.
- Deterministic seeds; warm-up passes; N=5 runs; 95% CI.

5) Output
- Publish: Benchmarks repo, generated report, plots (png/svg), raw CSV/JSON, scripts to reproduce.
- Blog: “Rust-native Code Graphs at Scale: Speed and Fidelity Benchmarks.”

6) Pass/fail gates
- Greenlight for website launch when medium corpus ingestion < 2 min, and call-graph partial resolution > 60% (early target); raise thresholds over time.

---

## Distribution and growth (without travel)

- Launch stack
  - 5-min polished demo video (voiceover: problem → demo → results).
  - README with copy that sells outcomes, not features.
  - Quickstart: curl | bash or cargo install; “analyze my crate in 60 seconds.”
  - Public benchmarks with reproducibility.
  - One killer example repo with saved artifacts.

- Channels (sequence matters)
  - Show HN: “Rust-native code graph + autonomous agents (benchmarks inside)”
  - r/rust “Showcase + benchmarks + reproducible harness”
  - Twitter/X: threaded breakdown with GIF demos and numbers.
  - Dev.to/Medium: “How we built a parallel Rust parser and why not tree-sitter.”
  - Rust meetups: remote talk; offer to benchmark attendee crates live.

- Capture
  - Waitlist form: use cases, company size, repo size, email.
  - “Request pilot” CTA button.
  - Convert 10% of signups to 1–2 pilots/month initially.

---

## Monetization options and pricing

- Services (cash now)
  - Pilot: $3k–$7.5k for 2 weeks; deliver code graph, hotspots, 3–5 PRs.
  - Ongoing: $2k–$5k/mo retainer for code intelligence and automated reports.

- Open-core + Pro
  - Free: CLI/TUI, basic graph, limited repo size, local embeddings.
  - Pro Individual: $29–$79/mo (larger repo cap, advanced queries).
  - Team: $299–$999/mo (5–20 seats, collaboration, SSO, history, BM25 server).
  - Enterprise: custom, on-prem, SLAs, dedicated support.

- Licensing
  - Consider Apache-2 for core; BSL for selected modules; dual license for SDK embedding in commercial tools.

---

## 14-day execution plan

Day 1–2: Define ICP and collateral
- ICP v1: Rust teams at mid-size startups with 100–800 KLOC Rust codebases (devtools, infra, embedded).
- Draft website copy, record a raw demo.

Day 3–5: Benchmarks harness + first results
- Implement ploke-bench; benchmark 3 repos; publish raw outputs and a summary.

Day 6–7: Demo repos + docs
- Create “Analyze tokio module X” and “API map for serde” repos with scripts and screenshots.

Day 8–9: Pilot offer and pricing page
- Ship a “Request a Pilot” page with scope and price; add Stripe payment link (even if invoicing later).

Day 10: Launch to r/rust and HN
- Post with numbers, reproducibility, and demo video. Be present for comments.

Day 11–12: Outreach (20 laser-targeted emails)
- Template: “We built a Rust-native code graph + agents. Benchmarks attached. Can I run a free 30‑min discovery on your repo? Pilot fixed price if useful.”

Day 13–14: Case study draft + follow-ups
- Turn your first internal runs into a PDF case study; use it in follow-up outreach.

---

## Risk map and mitigations

- Risk: Getting ignored
  - Mitigation: Lead with numbers and GIFs; reproducibility; name known crates in benchmarks.
- Risk: Too broad a product
  - Mitigation: Narrow to 3 repeatable agent tasks (call graph, API map, refactor hotspots).
- Risk: Slow ingest on very large repos
  - Mitigation: Document scaling flags; implement chunked ingest; offer Pro acceleration.
- Risk: No immediate buyers
  - Mitigation: Sell pilots to consulting-friendly teams; partner with OSS maintainers to co‑publish case studies.

---

## Two best plays (detailed)

1) Services-first + Proof
- Offer: “Rust Code Intelligence Pilot” ($5k, 2 weeks)
- Deliverables:
  - Repo ingest + graph; benchmark report; 3–5 PRs (e.g., dead code removal, API organization, doc generation).
  - TUI/CLI artifacts and a PDF summary with heatmaps.
- Why it wins: fastest to cash, builds references, forces product hardening.

2) Open-core tool + Pro addon
- Launch the free CLI/TUI and artifacts export to hook developers.
- Ship Pro with: parallel ingest accelerator, history timeline, multi-user workspaces, managed BM25/HNSW, SSO, and support.
- Why it wins: compounding distribution, upsell path, and defensible positioning in Rust.

Execute A and seed B. Reinvest pilot revenue to harden Pro.

---

## Tactical notes you can use word-for-word

- Positioning sentence:
  “Ploke is a Rust-native code intelligence engine that turns your repository into a live, queryable graph with call graphs, embeddings, and autonomous refactoring agents—built for large Rust codebases.”

- CTA:
  “Analyze my repo in 60 seconds” and “Request a 2‑week pilot.”

- Benchmark headline:
  “Ingested tokio (xx KLOC) in 94s; resolved yy% of calls; live queries in <50ms; fully reproducible.”

Ship this, and you’re no longer “a website lost in the noise.” You’re a benchmarked, reproducible, outcomes-first Rust code intelligence product with a clear path to revenue.

---

# Appendix A: Real-world example (played end-to-end)

Client profile
- Company: StreamForge (hypothetical), Rust-heavy backend (tokio + hyper), 350 KLOC, CI flakiness, perf regressions.
- Pain: p95 latency spikes, dev onboarding slow, unclear module ownership, dead code accreting.

Your outreach email (ask for $5k–$15k without cringing)
- Subject: Rust code intelligence pilot for StreamForge (benchmarks attached)
- Body:
  Hi {Name}, 
  I build Ploke, a Rust-native code intelligence engine. It ingests a repository and produces a live call graph, trait/impl map, and searchable embeddings. 
  In a 2-week fixed-scope pilot I typically deliver:
  - A benchmarked ingest report (LOC/s, coverage, determinism)
  - 3–5 production-ready PRs (e.g., dead code removal, API map, CI flakes triage, async hotspot fixes)
  - A reproducible knowledge base (graphs + queries you can run locally)
  Price: $7,500 fixed. If you don’t find it valuable, I’ll refund the fee.
  Would you be open to a 20-minute discovery to confirm scope and pick the first PRs?
  – {Your Name}
- Why it works: outcome-first, fixed price, risk reversal.

Discovery call script (15–20 min)
- Open: “I’ll ask 6 questions, then reflect back a proposed scope and confirm success criteria.”
- Questions:
  1) What’s your biggest Rust pain today? (latency, flakiness, onboarding, ownership, unsafe, etc.)
  2) Repo topology? (workspace size, crates, features, build profile)
  3) Success criteria you’d love in 2 weeks? (e.g., reduce CI flakes by 30%, generate call graph for top 5 services)
  4) Constraints? (no network, on‑prem only, CI only, GH App?)
  5) Decision process and timeline?
  6) If I deliver X, Y, Z, is $7.5k sane for you?
- Close: “I’ll send a 1-page scope with 3 deliverables, schedule, and acceptance tests. If that looks good, we start Monday.”

Pilot scope (what you do day-by-day)
- Day 1: Baseline ingest + deterministic run report. Artifacts: JSONL timings, coverage stats, IDs stability, machine profile.
- Day 2: Graph exports (call graph for top crates/modules). Produce top-20 hotspots by fan-in/out, alloc pressure hints, async spawn map.
- Day 3–4: PR 1 — Dead code cull + unused feature flags. Evidence: compile-time diff, binary size diff, clippy lint drop.
- Day 5–6: PR 2 — Async hotspot fix (blocking in async, unnecessary Arc<Mutex<>> contention). Evidence: microbench and production perf guardrail plan.
- Day 7: PR 3 — API surface map + doc gen (public items, trait impls, module ownership). Evidence: generated docs, owner map.
- Day 8: Optional PR 4 — Flaky test triage via call graph + span traces.
- Day 9: Knowledge base handoff — TUI quickstart, saved queries, embeddings index, “how to rerun analyses.”
- Day 10: Final report + live readout. Ask for expansion.

Artifacts clients care about (proof, not hype)
- Before/after metrics: cargo check time, binary size, clippy lints, flaky tests count, p95/p99 for targeted endpoints, compile cache hit rate.
- Graph coverage: nodes/edges counts by item kind; call resolution %; trait impl coverage; determinism hash.
- “Show me”: short screen capture of the TUI running the exact queries → matching PR diffs.

The close
- “We hit 3/3 deliverables, reduced CI flakes by 28%, removed 14KLOC of dead code, and resolved 72% of calls in target modules. I propose we continue at $3,000/mo for ongoing code intelligence and monthly drift reports, or a one-time $12,000 for a deeper refactor on the scheduler. Which option makes most sense?”

Objection handling (tight answers)
- “We can’t share code.” → “I run on your machine, offline. Deterministic scripts, no network. I sign an MNDA.”
- “We don’t have budget.” → “Let’s timebox to 1 week at $3k to prove value. If it hits your criteria, we extend.”
- “We already have rust-analyzer.” → “Great for IDE. Ploke generates a reproducible repository-wide graph + embeddings + agents that output PRs. Different layer, complementary.”

---

# Appendix B: What Ploke must do to prove value (capabilities checklist)

Minimum viable proof (Phase 1)
- Deterministic ingest of a Rust workspace (same commit + flags => identical IDs).
- Stable typed IDs for modules/items; canonical item paths.
- Call graph extraction (partial OK): function → function edges; record unresolved edges ratio.
- Trait/impl index: map types/traits/impl blocks; list orphan/blanket impl hot spots.
- Embeddings + BM25/HNSW search over items with file/line backlinks.
- TUI with:
  - Query palette (top queries wired)
  - Live agent logs
  - Export to Markdown/CSV/JSON for report
- Benchmark harness: JSONL per stage, CSV summary, deterministic seeds, N runs.
- PR generator: surface dead code, unused imports, feature flags; prep PR diffs and rationale.

Phase 2 (raise the bar)
- Type resolution coverage for common patterns (monomorphization-aware for standard libraries and popular crates).
- Trait resolution heuristics with confidence scores.
- Change impact analysis: “What breaks if I change X?”
- Ownership map: crate/module owners via VCS signals + code graph.
- CI-mode runner (no network), artifact bundle (.tar.zst) with full reproduction.

Phase 3 (SOTA frontier)
- HIR/MIR taps for unsafe hotspots, borrow-check stress map, inlining/monomorphization pressure hints.
- Evolutionary search agents (AlphaEvolve style) to propose transformations + backtest.
- SE-Agent style trajectory mixing for multi-path refactor proposals.
- SWE-Search style MCTS for test generation and search over patches.
- Formal methods hooks (creusot) for critical modules.

Stage gates (what “done” means)
- Gate A: Medium repo < 2 min ingest, >60% call resolution in target modules, deterministic ID hash stable across 5 runs.
- Gate B: Deliver 3 PRs with measurable diffs (compile time, size, clippy).
- Gate C: One-click artifact reproduction on client machine.

---

# Appendix C: Rust-first benchmark protocol (engineering-grade)

Scope
- Corpora: tokio, serde, rust-analyzer, clap, bevy, hyper, reqwest, polars, ripgrep, actix-web.
- Hardware profile: publish CPU, RAM, OS, governor; pin rustc and cargo.

Metrics (definitions)
- Ingest throughput: LOC/s, files/s; include 95% CI over N=5 cold runs.
- Coverage: counts by node/edge kind; unique item paths; module tree completeness.
- Call graph resolution:
  - Precision: resolved edges that point to the correct definition / all resolved edges (spot-check via oracle subset).
  - Recall: resolved edges / all edges.
- Trait coverage: % of impls correctly associated to trait/type pairs (subset ground truth).
- Search quality: recall@k for a set of 50 curated intent queries (e.g., “where do we block in async?”).
- Determinism: collision rate of ID hashes across 5 runs (should be 0).
- Outcome metrics on PRs: cargo check time delta, binary size delta, lint count delta, flaky tests delta.

Harness
- Single command: ploke-bench --repo <url> --rev <sha> --runs 5 --mode {cold,warm} --out out/
- Outputs: JSONL per stage, CSV summary, Markdown report with sparkline plots, seed + env manifest.
- Publish everything in a public repo; accept PRs to add corpora.

Pass/fail thresholds (for public claims)
- Medium repo: ingest < 120s; determinism OK; call recall > 60% initially (iterate to 80%+).
- Search recall@10 > 0.6 on curated set.
- Reproducibility: report regenerates byte-identical on same machine.

---

# Appendix D: Branching plan (if/else based on current capability)

- If call graph not ready:
  - Lean on dead code cull, API surface extraction, owner maps, embeddings search. Promise call graph for 2nd engagement.
- If type/trait resolution partial:
  - Ship confidence-scored edges and focus on modules with high confidence; document gaps; propose a week to raise coverage.
- If HIR/MIR not integrated:
  - Offer “unsafe hotspot map” via static heuristics; plan MIR-based pass as a paid upgrade.
- If repo is massive (>1M LOC):
  - Chunked ingest + sampling benchmarks; propose Pro accelerator or on-prem run with tuned flags.

---

# Appendix E: ROI calculator (use live)

Inputs
- Hourly loaded dev cost: $120
- Team size: 12 devs
- CI minutes/day saved: 40
- PRs merged from pilot: 4
- Estimated compile time reduction: 8%

Computation
- CI savings/month = (40/60) hours/day × 22 days × $120 ≈ $1,760
- Dev iteration savings/month = (8% × 6 hours/day coding) × 12 devs × 22 days × $120 ≈ $12,160
- One-time cleanup (dead code, size, flakes) impact ≈ $6,000
- 30‑day ROI on $7,500 pilot: roughly 3–4x (be conservative on calls).

---

# Appendix F: Exact asks, lines, and closes

The ask (email or call)
- “I run a 2‑week, fixed-scope Rust code intelligence pilot. It includes a benchmarked ingest report, three PRs with measurable impact, and a reproducible knowledge base you keep. The fee is $7,500, fully refundable if you don’t find it valuable. Shall we book Monday?”

Anchoring and options
- “We can start with 1 week at $3,000 if you prefer to reduce risk; or 2 weeks at $7,500 to include call graph and PRs.”

Silence handling
- Ask, stop talking, wait. If they speak, address; if not, “Happy to send the scope doc. If it looks good, we lock a start date.”

Payment/logistics
- “I can invoice Net 15 or set up Stripe. MNDA is fine; I run offline on your infra and leave reproducible artifacts.”

---

# Appendix G: Top-10 “killer queries” to demo live

- Show top fan-in functions by crate/module.
- List all async functions calling blocking ops.
- Map trait impls for Type X across the workspace.
- Find modules without owners (via VCS signal).
- Extract public API surface for crate Y.
- Show call chain from API endpoint A to DB layer.
- List unsafe blocks ranked by proximity to public APIs.
- Identify feature flags defined but never referenced.
- Compare call graphs between commits (drift).
- Search “where is this error produced?” via BM25+embedding hybrid.

---

# Appendix H: Your TUI demo narrative (5 minutes)

- 30 seconds: “Problem, promise, proof.” 
- 2 minutes: Ingest a known crate with flags; show determinism and coverage counters tick.
- 1 minute: Run 3 killer queries; show results side-by-side with file links.
- 1 minute: Open an auto-generated PR diff (dead code + doc update).
- 30 seconds: “Pilot details, price, refund guarantee.”

---

# Appendix I: Minimal guarantees to print on the site (truthful)

- Deterministic runs on the same commit/flags.
- Reproducible reports with raw data published (on your machine or ours).
- At least 2 production-ready PRs in the standard 2‑week pilot or your fee back.
- Benchmarks and methodology fully public.

Make the benefit obvious, measurable, and hard to refute. Lead with artifacts and numbers, ask for a fixed price confidently, and provide risk reversal. Your product is Rust-native; your proof should be too.
