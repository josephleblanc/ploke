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
