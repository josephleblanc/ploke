```
## Subtasks:

1.  **Subtask:** Align primitive types with CozoDB schema (Strings → Bytes/Uuid, numeric types)
    **Effort:** 2-3 hours
    **Breaking Change?** Maybe
    **Feature Flag (if applicable):** feature_cozo_type_alignment
    **Commit Message:** align: convert string hashes to Bytes, numbers to Cozo-compatible types

2.  **Subtask:** Add #[derive(Send, Sync)] to all public-facing types
    **Effort:** 1 hour
    **Breaking Change?** Yes (if any types can't implement these traits)
    **Feature Flag (if applicable):** feature_threadsafe_types
    **Commit Message:** chore: enforce Send+Sync on public API types

3.  **Subtask:** Replace CodeGraph with streaming AST visitor emitting Cozo-compatible structures
    **Effort:** 4-5 hours
    **Breaking Change?** Yes
    **Feature Flag (if applicable):** feature_direct_cozo_streaming
    **Commit Message:** refactor: replace CodeGraph with direct Cozo schema emitter

4.  **Subtask:** Update serialization module for Cozo-native formats
    **Effort:** 2 hours
    **Breaking Change?** Yes
    **Feature Flag (if applicable):** feature_cozo_serialization
    **Commit Message:** serde: implement Cozo-native serialization format

5.  **Subtask:** Update integration tests for type changes and streaming
    **Effort:** 1-2 hours
    **Breaking Change?** No
    **Feature Flag (if applicable):** N/A
    **Commit Message:** test: update integration tests for Cozo compatibility
```

This breakdown maintains the existing architecture while shifting to Cozo compatibility. The feature flags allow gradual rollout - we can enable them incrementally in CI/CD. The most impactful change is the CodeGraph removal (subtask 3), which the flag protects against until fully validated.
