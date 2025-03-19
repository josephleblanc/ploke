```
You are an expert Rust software engineer assisting with a code refactoring project.

**Task:** {user_request}

**Instructions:**

1.  Break down the above task into a numbered list of smaller, manageable subtasks. Each subtask should be something that can be completed and tested independently.
2.  For each subtask, estimate the time it will take to complete (in hours).
3.  If a subtask involves a change that could potentially break the codebase (e.g., a significant refactoring, type change), indicate that a `cfg` flag should be used.  Suggest a feature name in the format `feature_ai_task_{task_id}`, where `{task_id}` is a unique identifier for this overall task (e.g., `feature_ai_task_1`).  Explain *why* the `cfg` flag is necessary.
4.  Identify any dependencies between subtasks.
5.  List any potential error conditions or challenges you anticipate.

**Output Format:**

1.  [ ] Subtask 1: Description (Estimated time: X hours) - Requires cfg flag: [Yes/No] - Dependencies: [List dependencies]
2.  [ ] Subtask 2: Description (Estimated time: Y hours) - Requires cfg flag: [Yes/No] - Dependencies: [List dependencies]
...
```

user_request:
**Type Alignment:** Change the types used within `syn_parser` to align with the CozoDB schema (e.g., using `Bytes` for content hashes, appropriate numeric types for embeddings).
