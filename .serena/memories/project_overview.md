# Project Overview

**Ploke** is building a production-ready agentic system that can autonomously implement user goals via code edits, while preserving user control, observability, and correctness.

## Key Components
- **Terminal UI (TUI)** for LLM interaction with vim-like bindings
- **RAG System** with comprehensive code graph built by parsing Rust crates
- **Agentic Workflow** with human-in-the-loop by default, progressive autonomy
- **Safety-First Editing** with staged edits, verified file hashes, atomic application

## Tech Stack
- **Language**: Rust (2021 edition)
- **Async Runtime**: Tokio for I/O, networking, UI
- **CPU-Intensive**: Rayon for parsing and analysis
- **Database**: CozoDB (hybrid vector-graph storage, Datalog queries)
- **UI**: Terminal-based with vim-like bindings
- **API Integration**: OpenRouter for LLM calls
- **Communication**: Flume channels between async/sync domains

## Current Development Focus
Working on agentic system development in `feature/agentic-system-00` branch:
1. **OpenRouter API & Tool Calling System** - Strong typing, telemetry, persistence
2. **Safe Editing Pipeline** - Human-in-the-loop approval with diff previews
3. **Testing & Validation** - E2E testing with test harness support

## Architecture Principles
- **Strong typing everywhere** - No stringly typed plumbing
- **Performance by design** - Static dispatch, zero-cost abstractions
- **Safety-first editing** - Atomic operations with hash verification
- **Evidence-based changes** - Run tests, update docs for trade-offs