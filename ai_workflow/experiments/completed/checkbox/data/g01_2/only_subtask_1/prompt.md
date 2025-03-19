```
You are an expert Rust software engineer assisting with a code refactoring project.

**Task:** {user_request}

**Instructions:**

Create a markdown table outlining the subtasks required to complete the above task.  The table should include the following columns:

*   **Subtask ID:** A unique identifier for the subtask.
*   **Description:** A clear description of the subtask.
*   **Estimated Time (hours):** An estimate of the time required to complete the subtask.
*   **Cfg Flag Required?:**  "Yes" if the subtask involves a change that could potentially break the codebase, "No" otherwise. If "Yes", suggest a feature name in the format `feature_ai_task_{task_id}`. Explain why the flag is needed.
*   **Dependencies:** A list of other subtask IDs that must be completed before this subtask can be started.
*   **Potential Issues:** Any anticipated challenges or error conditions.

**Output Format:**

| Subtask ID | Description | Estimated Time (hours) | Cfg Flag Required? | Dependencies | Potential Issues |
|---|---|---|---|---|---|
| 1 |  |  |  |  |  |
| 2 |  |  |  |  |  |
| ... |  |  |  |  |  |
```

user_request:
**Type Alignment:** Change the types used within `syn_parser` to align with the CozoDB schema (e.g., using `Bytes` for content hashes, appropriate numeric types for embeddings).
