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
**Type Alignment:** Change the types used within `syn_parser` to align with the CozoDB schema (e.g., using `Bytes` for content hashes, appropriate numeric types for embeddings).
