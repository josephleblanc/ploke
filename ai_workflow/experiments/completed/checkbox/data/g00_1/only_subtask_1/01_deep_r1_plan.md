```
## Subtasks:

1.  **Subtask:** Audit existing types for CozoDB alignment
    **Effort:** 2 hours
    **Breaking Change?** No
    **Feature Flag (if applicable):** N/A
    **Commit Message:** Audit type usage against CozoDB schema requirements

2.  **Subtask:** Convert content hash fields from String to Bytes type
    **Effort:** 3 hours
    **Breaking Change?** Maybe
    **Feature Flag (if applicable):** feature_cozo_bytes_hashes
    **Commit Message:** Use Bytes type for content hashes (optional feature flag)

3.  **Subtask:** Implement numeric vector types for embeddings
    **Effort:** 4 hours
    **Breaking Change?** Yes
    **Feature Flag (if applicable):** feature_cozo_vector_types  
    **Commit Message:** Add support for F32/F64 vector types per Cozo schema

4.  **Subtask:** Update serialization formats (RON/JSON) to match Cozo types
    **Effort:** 3 hours
    **Breaking Change?** Yes
    **Feature Flag (if applicable):** feature_cozo_serialization
    **Commit Message:** Align serialization formats with Cozo type expectations

5.  **Subtask:** Add validation for Cozo type invariants
    **Effort:** 2 hours
    **Breaking Change?** No
    **Feature Flag (if applicable):** N/A
    **Commit Message:** Enforce Cozo type constraints in parser outputs

6.  **Subtask:** Update documentation and examples
    **Effort:** 1.5 hours
    **Breaking Change?** No
    **Feature Flag (if applicable):** N/A
    **Commit Message:** Update docs for Cozo type alignment changes
```

The biggest potential breakage points are in subtasks 2-4 where type changes propagate through serialization. Feature flags allow gradual rollout while maintaining compatibility.

Would you like me to elaborate on any of these subtasks or proceed with implementing a specific one?
