I would like you to create templates that can be used to prompt an LLM to break down a requested task into a set of subtasks that can be tracked as they are completed, and possibly includes using a cfg flag while making the changes. The use of the cfg flag would only be for changes that would cause the code base to break. For example, a refactor of the `syn_parser` crate to use

**In your response:**  
- Use a different feature name for the cfg to reflect set of tasks the LLM is working on.  
- Propose templates the user can reference when requesting that the LLM perform a task that may or may not require using a cfg flag (as described in musings.md).  
  - Create three example templates that can be used to generate the list of trackable subtasks for a given user request.  
  - Include a short description of the reasoning for your choices in formatting the example templates.  
  
Proposed Example Templates **should**:  
  - be generic: applicable to any given request  
  - vary in complexity  
  - use different formatting  
  
Your response **should**  
- Explicitly note parts of your response you are uncertain of.  
  
**Example user requests**  
*   **Type Alignment:** Change the types used within `syn_parser` to align with the CozoDB schema (e.g., using `Bytes` for content hashes, appropriate numeric types for embeddings).  
*   **Send + Sync:** Ensure all publicly exposed types are `Send + Sync`. This is a good practice regardless and will prepare the codebase for future concurrency improvements.  
*   **Remove `CodeGraph` (or significantly reduce its scope):** You're right to question the value of the `CodeGraph` as an intermediary. It adds complexity without necessarily providing significant benefits. We can likely stream data directly from the `syn` AST to CozoDB.  
  
You are encouraged to ask clarifying questions **if you believe the answer will significantly improve** your proposed example templates.  
