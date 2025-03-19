```
You are an expert Rust software engineer. I need your help with the following task:

[USER TASK DESCRIPTION]

Please decompose this task into a series of subtasks suitable for incremental implementation and testing.  For each subtask, provide a brief rationale explaining *why* this step is necessary.  Also, assess the risk of introducing breaking changes. If a breaking change is possible, propose a feature flag name (e.g., `feature_send_sync_fixes`) to allow for controlled rollout.

Present your response in a table format:

| Subtask | Rationale | Effort (hours) | Breaking Change? | Feature Flag (if applicable) | Commit Message |
|---|---|---|---|---|---|
|  |  |  |  |  |  |
|  |  |  |  |  |  |
|  ... | ... | ... | ... | ... | ... |

After you provide the table, wait for my approval before proceeding.
```


USER TASK DESCRIPTION:
1.  **Minimal `syn_parser` Rework:** Focus on adapting the existing `syn_parser` to output data directly compatible with CozoDB, *without* fundamentally altering its data flow or concurrency model. This means:
    *   **Type Alignment:** Change the types used within `syn_parser` to align with the CozoDB schema (e.g., using `Bytes` for content hashes, appropriate numeric types for embeddings).
    *   **Send + Sync:** Ensure all publicly exposed types are `Send + Sync`. This is a good practice regardless and will prepare the codebase for future concurrency improvements.
    *   **Remove `CodeGraph` (or significantly reduce its scope):** You're right to question the value of the `CodeGraph` as an intermediary. It adds complexity without necessarily providing significant benefits. We can likely stream data directly from the `syn` AST to CozoDB.
