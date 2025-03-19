```
You are an expert Rust software engineer. I want you to help me with the following task:

[USER TASK DESCRIPTION]

Please break down this task into a numbered list of smaller, manageable subtasks. For each subtask, estimate the effort (in hours) and indicate if it's likely to introduce a breaking change. If a subtask *could* introduce a breaking change, suggest a feature flag name (e.g., `feature_cozo_type_alignment`) that we can use to control its rollout.  Also, suggest a concise commit message for each subtask.

Format your response as follows:

```
## Subtasks:

1.  **Subtask:** [Brief description of the subtask]
    **Effort:** [Estimated hours]
    **Breaking Change?** [Yes/No/Maybe]
    **Feature Flag (if applicable):** [Suggested feature flag name]
    **Commit Message:** [Concise commit message]

2.  ... (repeat for each subtask)
```

After you provide the list, wait for my approval before making any code changes.
```

**Example Usage (for "Type Alignment" request):**

```
You are an expert Rust software engineer. I want you to help me with the following task:

Change the types used within `syn_parser` to align with the CozoDB schema (e.g., using `Bytes` for content hashes, appropriate numeric types for embeddings).

Please break down this task into a numbered list of smaller, manageable subtasks. For each subtask, estimate the effort (in hours) and indicate if it's likely to introduce a breaking change. If a subtask *could* introduce a breaking change, suggest a feature flag name (e.g., `feature_cozo_type_alignment`) that we can use to control its rollout.  Also, suggest a concise commit message for each subtask.

Format your response as follows:

```
## Subtasks:

1.  **Subtask:** [Brief description of the subtask]
    **Effort:** [Estimated hours]
    **Breaking Change?** [Yes/No/Maybe]
    **Feature Flag (if applicable):** [Suggested feature flag name]
    **Commit Message:** [Concise commit message]

2.  ... (repeat for each subtask)
```

After you provide the list, wait for my approval before making any code changes.
```

USER TASK DESCRIPTION:
1.  **Minimal `syn_parser` Rework:** Focus on adapting the existing `syn_parser` to output data directly compatible with CozoDB, *without* fundamentally altering its data flow or concurrency model. This means:
    *   **Type Alignment:** Change the types used within `syn_parser` to align with the CozoDB schema (e.g., using `Bytes` for content hashes, appropriate numeric types for embeddings).
    *   **Send + Sync:** Ensure all publicly exposed types are `Send + Sync`. This is a good practice regardless and will prepare the codebase for future concurrency improvements.
    *   **Remove `CodeGraph` (or significantly reduce its scope):** You're right to question the value of the `CodeGraph` as an intermediary. It adds complexity without necessarily providing significant benefits. We can likely stream data directly from the `syn` AST to CozoDB.
