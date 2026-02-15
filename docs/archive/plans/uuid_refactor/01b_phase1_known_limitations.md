# Phase 1 (`uuid_ids`) Known Limitations

This document tracks known limitations, missing features, or areas where the Phase 1 "Discovery and Context Creation" (`uuid_ids` feature) deviates from complete Rust syntax coverage or desired graph structure. These limitations were primarily discovered during testing and are documented here to inform future development and prevent regressions.

---

## 1. Crates with both a `lib.rs` and `main.rs`

*   **Limitation:** The parser currently supports creating one code graph at a time, and a crate that has both a `lib.rs` and `main.rs` can be thought of as two code graphs, with the `lib.rs` being the primary code graph exposing the project's API, and the `main.rs` as a wrapper that allows for execution of the methods and definitions within the `lib.rs` package.

    For now we will support only one or the other, defaulting to processing the code graph that begins from the root of the `lib.rs` file.

*   **Patch Solution**: In a crate that contains both a `lib.rs` and `main.rs`, default to the `lib.rs` code graph for processing, module tree creation, and pruning.
    * See `run_discovery_phase` in `crates/ingest/syn_parser/src/discovery.rs`, specifically the section noted with a `WARN:` comment and linking this document.

*   **Impact:**  
    • Small? Most likely the crates we will be analyzing in the immediate future will be one or the other, or will mostly be concerned with the `lib.rs` part of their project.

*   **Future Work:**  
    • Longer term, we should support this, as it is valid Rust. Perhaps once we turn our attention to handling dependencies it will be more clear how to handle this case.
    • Not a priority, but a nice-to-have.
    • TODO: Add specific tests to track this case.

---

*(Add subsequent limitations below this line)*
