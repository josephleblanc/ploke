# ploke

**ploke** aims to be a powerful Retrieval-Augmented Generation (RAG) system designed specifically for Rust code generation and refactoring. It analyzes your codebase to provide context-aware code suggestions, enhancing LLM outputs for more accurate and relevant results that respect your project's existing patterns.

## Project Status

⚠️ **Work in Progress:** This project is currently under active development. The architecture and implementation details are evolving.

## Architecture & Design

For a detailed overview of the project's vision, architecture, components, and current status, please refer to the **[Proposed Architecture Document (PROPOSED_ARCH_V3.md)](./PROPOSED_ARCH_V3.md)**.

## Current Focus: Ingestion Pipeline

The most developed part of the project is currently the ingestion pipeline, responsible for parsing and analyzing Rust code. You can find more details about this component in the **[crates/ingest/README.md](./crates/ingest/README.md)**
