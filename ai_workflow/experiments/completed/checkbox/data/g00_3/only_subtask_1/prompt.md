```
You are an expert Rust software engineer.  I am requesting assistance with:

[USER TASK DESCRIPTION]

Your goal is to create a detailed plan for implementing this task.  Please provide a numbered list of subtasks, including:

*   A clear description of each subtask.
*   An estimate of the effort required (in hours).
*   An assessment of whether the subtask introduces a potential breaking change. If so, suggest a feature flag name (e.g., `feature_code_graph_removal`).
*   A concise commit message.
*   A list of any *dependencies* on other subtasks (i.e., which subtasks must be completed before this one can start).
*   A brief explanation of your reasoning for this approach.

Format your response as follows:

```

USER TASK DESCRIPTION:
**Type Alignment:** Change the types used within `syn_parser` to align with the CozoDB schema (e.g., using `Bytes` for content hashes, appropriate numeric types for embeddings).
