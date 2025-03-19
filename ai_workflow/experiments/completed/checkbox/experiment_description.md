# Experiment: Creating Prompt Templates For Generating Task List
These prompt templates are intended to be used to have an LLM break down a user request into a set of sub-tasks with trackable progress (e.g. checkboxes).

## Description of Experiment

This experiment used two phases. In phase 1 we requested the LLMs to generate three templates that could be used to prompt an LLM to break down a list of tasks into sub-tasks that included trackable progress (e.g. checkboxes). Then in phase 2 we submitted the prompts to the LLMs along with a user request.

During each phase, the LLM was additionally provided with files that described best practices for the programming language the user was using to create their project( [IDIOMATIC_RUST.md] ), as well as conventions specified by the user ( [ CONVENTIONS.md ] ). Furthermore, a "repo-map" with some of the most relevant code snippets was included as well ( [ experiment_repo_map.md ] )

During Phase 1 only, files were added to the context that contained some considerations on what the possible challenges would be when generating a task list for an LLM. During Phase 2 only a file was included with a description of the database types, to help make the task list more applicable to the user's request ([ cozo_db_docs_types.txt ]).

The LLMs used in this experiment were:
- Claude 3.7 Sonnet
- Deepseek-R1
- Gemma-3-27B-it

## Phase 1: Generate Templates

The three LLMs were asked to create the templates. They were all given the same prompt to create the templates [template_generation_prompt.md].

The generated prompts are included in the directory [generated_prompts.md/].
* Note: Although the Deepseek-R1 model received the same prompts as the other two LLMs, it did not generate the desired output. Reasons for this are unclear. As a side note, it seems the Deepseek-R1 model generated snippets of code that were intended to be used as parts of the template, perhaps intending to use the code as a form of communication. This behavior was unique to Deepseek-R1 and requires further study.

## Phase 2: Use Generated Templates to Generate Sub-Task List

### User Requests Tested
The following user requests were tested:
1. full_task
```markdown
1.  **Minimal `syn_parser` Rework:** Focus on adapting the existing `syn_parser` to output data directly compatible with CozoDB, *without* fundamentally altering its data flow or concurrency model. This means:
    *   **Type Alignment:** Change the types used within `syn_parser` to align with the CozoDB schema (e.g., using `Bytes` for content hashes, appropriate numeric types for embeddings).
    *   **Send + Sync:** Ensure all publicly exposed types are `Send + Sync`. This is a good practice regardless and will prepare the codebase for future concurrency improvements.
    *   **Remove `CodeGraph` (or significantly reduce its scope):** You're right to question the value of the `CodeGraph` as an intermediary. It adds complexity without necessarily providing significant benefits. We can likely stream data directly from the `syn` AST to CozoDB.
```
2. only_subtask_1
```markdown
**Type Alignment:** Change the types used within `syn_parser` to align with the CozoDB schema (e.g., using `Bytes` for content hashes, appropriate numeric types for embeddings).
```

Due to the way prompt [c00_1] was created, an additional request was tested:
```markdown
1.  **Minimal `syn_parser` Rework:** Focus on adapting the existing `syn_parser` to output data directly compatible with CozoDB, *without* fundamentally altering its data flow or concurrency model. This means:
    *   **Type Alignment:** Change the types used within `syn_parser` to align with the CozoDB schema (e.g., using `Bytes` for content hashes, appropriate numeric types for embeddings).
    *   **Send + Sync:** Ensure all publicly exposed types are `Send + Sync`. This is a good practice regardless and will prepare the codebase for future concurrency improvements.
    *   **Remove `CodeGraph` (or significantly reduce its scope):** You're right to question the value of the `CodeGraph` as an intermediary. It adds complexity without necessarily providing significant benefits. We can likely stream data directly from the `syn` AST to CozoDB.


Task Name: Type Alignment
```



### Phase 2 Results
The results of the experiment are included in the `data` directory, with the following naming schema and structure:
- Model, generated templates version, prompt number
- c = Claude, d = Deepseek, g = Gemma

- For example, `c00_1/`
  - Claude model, generated prompts version 00, prompt 1
