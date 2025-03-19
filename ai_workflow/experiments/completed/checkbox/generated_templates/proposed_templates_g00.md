## gemma-3-27b free default settings

---

**Reasoning for Template Choices:**

I'm focusing on templates that encourage the LLM to:

1.  **Explicitly list subtasks:** This is crucial for tracking progress.
2.  **Estimate effort:**  Helps with prioritization and planning.
3.  **Identify potential breaking changes:**  Triggers the `cfg` flag discussion.
4.  **Provide rationale:**  Captures the LLM's reasoning (important for knowledge preservation, as discussed in `musings.md`).
5.  **Suggest commit messages:** Streamlines the workflow.

I'm also assuming the LLM has access to the codebase (via the git repository) and the design documents (`CONVENTIONS.md`, `IDIOMATIC_RUST.md`).

**Uncertainties:**

*   The optimal level of detail in the templates is hard to predict without more experience with the specific LLM and codebase. I'm aiming for a balance between being prescriptive and allowing the LLM flexibility.
*   The effectiveness of the `cfg` flag approach depends on the LLM's ability to accurately assess the risk of breaking changes.



---

**Template 1: Simple Checklist Format**

This template is the most straightforward. It's good for smaller tasks where the subtasks are relatively obvious.

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

---

**Template 2:  Table Format with Rationale**

This template is more structured and explicitly asks for the LLM's reasoning.

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

**Example Usage (for "Send + Sync" request):**

```
You are an expert Rust software engineer. I need your help with the following task:

Ensure all publicly exposed types are `Send + Sync`. This is a good practice regardless and will prepare the codebase for future concurrency improvements.

Please decompose this task into a series of subtasks suitable for incremental implementation and testing.  For each subtask, provide a brief rationale explaining *why* this step is necessary.  Also, assess the risk of introducing breaking changes. If a breaking change is possible, propose a feature flag name (e.g., `feature_send_sync_fixes`) to allow for controlled rollout.

Present your response in a table format:

| Subtask | Rationale | Effort (hours) | Breaking Change? | Feature Flag (if applicable) | Commit Message |
|---|---|---|---|---|---|
|  |  |  |  |  |  |
|  |  |  |  |  |  |
|  ... | ... | ... | ... | ... | ... |

After you provide the table, wait for my approval before proceeding.
```

---

**Template 3:  Detailed Narrative with Dependencies**

This template is the most complex. It encourages the LLM to think about dependencies between subtasks.

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
## Implementation Plan:

1.  **Subtask:** [Description]
    **Effort:** [Hours]
    **Breaking Change?** [Yes/No/Maybe]
    **Feature Flag:** [Flag Name]
    **Commit Message:** [Message]
    **Dependencies:** [List of subtask numbers]
    **Rationale:** [Explanation]

2.  ... (repeat for each subtask)
```

After you provide the plan, wait for my approval before starting any code modifications.
```

**Example Usage (for "Remove CodeGraph" request):**

```
You are an expert Rust software engineer.  I am requesting assistance with:

Remove `CodeGraph` (or significantly reduce its scope). You're right to question the value of the `CodeGraph` as an intermediary. It adds complexity without necessarily providing significant benefits. We can likely stream data directly from the `syn` AST to CozoDB.

Your goal is to create a detailed plan for implementing this task.  Please provide a numbered list of subtasks, including:

*   A clear description of each subtask.
*   An estimate of the effort required (in hours).
*   An assessment of whether the subtask introduces a potential breaking change. If so, suggest a feature flag name (e.g., `feature_code_graph_removal`).
*   A concise commit message.
*   A list of any *dependencies* on other subtasks (i.e., which subtasks must be completed before this one can start).
*   A brief explanation of your reasoning for this approach.

Format your response as follows:

```
## Implementation Plan:

1.  **Subtask:** [Description]
    **Effort:** [Hours]
    **Breaking Change?** [Yes/No/Maybe]
    **Feature Flag:** [Flag Name]
    **Commit Message:** [Message]
    **Dependencies:** [List of subtask numbers]
    **Rationale:** [Explanation]

2.  ... (repeat for each subtask)
```

After you provide the plan, wait for my approval before starting any code modifications.
```

---

These templates should provide a good starting point.  I recommend experimenting with them and adjusting them based on your experience with the LLM and the specific tasks you're tackling.  Let me know if you'd like me to refine these further!

> Tokens: 9.4k sent, 2.1k received.  
