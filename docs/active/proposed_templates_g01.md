Okay, this is a good challenge. I'll create three templates for prompting an LLM to break down tasks, with consideration for potential `cfg` flag usage for safety during refactoring. I'll also explain my reasoning. I'll aim for templates suitable for Deepseek-R1, gemma-3-27b-it, and qwen2.5-32b-instruct, but acknowledge that performance will vary.

**Reasoning & General Principles:**

*   **Task Decomposition:** The core idea is to get the LLM to explicitly list subtasks. This makes progress trackable and allows for easier review/rollback.
*   **Cfg Flag Strategy:** The `cfg` flag is reserved for potentially breaking changes. The LLM should *identify* when a change might break compilation and suggest using a flag.  The flag name will be `feature_ai_task_{task_id}` where `task_id` is a unique identifier for the overall request.
*   **Formatting:** I'm using different formats (numbered lists, bullet points, markdown tables) to see which the LLM responds to best.  Markdown tables are often good for structured output.
*   **Explicit Instructions:**  I'm including clear instructions about the desired output format and the use of the `cfg` flag.
*   **Context:** The templates assume the LLM has access to the codebase (as indicated by your initial message).
*   **Error Handling:** I'm including a request for the LLM to identify potential error conditions or dependencies.
*   **Uncertainty:** I'm uncertain about the optimal level of detail to include in the prompt. More detail might lead to better results, but also make the prompt more verbose and potentially harder for the LLM to process.



**Template 1:  Numbered List (Simple)**

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

**Template 2: Markdown Table (Structured)**

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

**Template 3:  Bullet Point List with Detailed Explanation (Verbose)**

```
You are an expert Rust software engineer assisting with a code refactoring project.  You are skilled at breaking down complex tasks into smaller, manageable steps.

**Task:** {user_request}

**Instructions:**

1.  Analyze the task and identify the core components that need to be modified or created.
2.  For each component, create a bullet point list of subtasks.
3.  For each subtask:
    *   Provide a concise description of the work to be done.
    *   Estimate the time required to complete the subtask (in hours).
    *   **Crucially:**  Assess whether the subtask introduces a risk of breaking the existing codebase.
        *   If **yes**, recommend using a `cfg` flag with the name `feature_ai_task_{task_id}` (where `{task_id}` is a unique identifier for this overall task).  Explain *specifically* why the `cfg` flag is necessary – what functionality might be broken if the change is applied directly?
        *   If **no**, state that no `cfg` flag is required.
    *   Identify any dependencies on other subtasks.  Specify the subtask ID.
    *   List any potential error conditions, edge cases, or challenges you foresee.
4.  Present the subtasks in a logical order, considering dependencies.

**Output Format:**

*   **Subtask ID: 1**
    *   Description: [Description of subtask 1]
    *   Estimated Time: [X hours]
    *   Cfg Flag Required?: [Yes/No] - [Explanation if Yes]
    *   Dependencies: [List of subtask IDs]
    *   Potential Issues: [List of potential issues]
*   **Subtask ID: 2**
    *   Description: [Description of subtask 2]
    *   Estimated Time: [Y hours]
    *   Cfg Flag Required?: [Yes/No] - [Explanation if Yes]
    *   Dependencies: [List of subtask IDs]
    *   Potential Issues: [List of potential issues]
...
```

**Applying the Templates to Your Example Requests:**

Let's briefly consider how these templates would be used with your example requests:

*   **Type Alignment:**  This would likely require `cfg` flags for many subtasks, as changing types can easily break existing code.
*   **Send + Sync:**  This is generally a safer change, so fewer `cfg` flags would be needed.  However, if a type is *not* `Send + Sync` due to internal mutability, a `cfg` flag might be necessary to isolate the changes.
*   **Remove `CodeGraph`:** This is a potentially large refactoring and would almost certainly require extensive use of `cfg` flags.

**Clarifying Questions (If I were to refine these further):**

1.  **What is the preferred method for assigning `task_id` values?** Should I let the LLM generate them, or should I provide them?
2.  **What level of detail do you want in the "Potential Issues" section?**  Should it be a high-level overview, or a more detailed list of specific problems?
3.  **Do you have a preferred LLM for this task?** Knowing this might help me tailor the prompts more effectively.



I hope these templates are a good starting point.  I'm ready to iterate on them based on your feedback and testing.
