**Historical chat with gemma-3-27b-it:free on 17-03-2025**

## *Question:*

So I would like your help thinking through hoe I can effectively collaborate with an LLM on code projects. I know there are a lot of different factors to take into account when doing pair programming with an LLM, and want to start using a more systematic approwch than the simple approach I have taken so far.  
  
I would like to start integrating git CI/CD to my AI-assisted workflow. So far I have just been interacting with the AI in the following ways:  
1. Question and answering about desogn documents  
2. Requesting changes to specific files or lines within files   
3. Brainstorming about larger questions on project acrchitecture  
4. Requesting inplementation of a feature or larger change,  
- I request the LLM to break the task into smaller chunks, and it shows me its response  
- I tell the LLM to write the tasks to a file I keep for trscking progress on immediate tasks.  
- I tell the LLM to work through the first task on the list and show me the proposed change for that one smaller task.  
  - If I like the proposal, I ask the LLM to make the change and check the box on the task list for that task.  
  - If i do not like the change, I clarify the task or request additional refinements until I like the proposed solution.  
  
Tasks I have worked on with AI assistance in this manner include:  
- project design: going from brainstorming to nailing down specifics of types used in different places in the code.  
- Generating code: Adding new files or modifying files to add new code.  
- Debugging: Finding and correcting errors in the code tgat prevent it from compiling  
- Writing tests: Ensuring code worjs as intended. Validation and testing edge cases.  
Refafactoring: Improving logic or function approach in one file or in various files for larfer efforts using task-basedbapproach eith checkboxes.  
  
Some difficulties I have encountered are:  
1. LLM begins making changes counter to my desired overall strategy  
- Solution (in progress): Construct a comprehebsive design document for the scope of this large project (80% done)  
2. LLM makes changes that seem to be improvements, but result in errors when I test later.  
- Solution: (in progress 10%) Use CI/CD automated testing to verify correctness before moving on to next task.  
- possible challenge: some refactors might require multiple tasks to be completed before program can compile.  
  - how to verify improvements in incremental steps?  
  - cfg flags may help, but could become tedious  
3. Knowledge preservation: currently there are no mechanisms to automatically track LLM reasoning for making changes.  
- Solution: not started.  
- possible tools: CD/CI, cargo checks for docs, formatted prompts with instructions to add rustdoc comments, others?  

This is a great start, and you've already identified some key challenges and
are proactively thinking about solutions. Integrating CI/CD is a *very* smart
move. Let's break down your points and brainstorm further, focusing on a more
systematic approach.


## *Answer:*

**Overall Assessment:** Your current workflow is a good iterative approach,
especially the task breakdown and checkbox system. It's a solid foundation. The
difficulties you're encountering are common when working with LLMs on larger
codebases.

**1. LLM Drifting from Overall Strategy (Design Document is Key)**

You're right to focus on a comprehensive design document. This is the single
most important thing.  Here are some additions to consider *within* the design
document:

*   **Explicit Constraints:**  Don't just describe *what* you want, but also
*what you don't want*.  For example, "Avoid introducing new dependencies unless
absolutely necessary," or "Prioritize zero-copy parsing where possible (see
CONVENTIONS.md)."  Referencing the `CONVENTIONS.md` and `IDIOMATIC_RUST.md`
files directly in the design doc is excellent.
*   **API Surface Definition:**  Clearly define the public API of any new
components.  This gives the LLM a concrete boundary to work within.
*   **Data Flow Diagrams:** Visual representations of how data moves through
the system can help the LLM understand the bigger picture.
*   **Regular Review & Reinforcement:**  Periodically remind the LLM of the key
principles from the design document.  You can even include a snippet of the
relevant section in your prompts.  "Remember, as outlined in the design
document, we're aiming for zero-copy parsing..."

**2. LLM Changes Introducing Errors (CI/CD & Incremental Verification)**

You're on the right track with CI/CD.  Here's how to tackle the incremental
verification challenge:

*   **Feature Flags/`cfg` Attributes (but strategically):** You're right,
overuse is tedious. But *targeted* use can be powerful.  If a refactor touches
a core area, wrap the changes in a feature flag.  Enable the flag only when
you're ready to test the entire refactored section.
*   **Unit Tests Focused on Boundaries:**  Write unit tests that specifically
target the interfaces between the modified code and the rest of the system.
This helps catch integration issues early.  Think about testing the inputs and
outputs of functions/modules.
*   **Small, Testable Chunks:**  The LLM's task breakdown is good, but ensure
each chunk is *independently testable*.  Avoid tasks that require multiple
files to be modified simultaneously if possible.
*   **"Canary" Tests:**  If a refactor is large, introduce a small number of
"canary" tests that specifically check for regressions in existing
functionality.  These are quick to run and provide early warning signs.
*   **Staged Commits:**  Encourage the LLM to generate changes in small,
logical commits. This makes it easier to revert if something goes wrong.  You
can even ask it to write the commit message.

**3. Knowledge Preservation (LLM Reasoning)**

This is the hardest problem. Here are some ideas:

*   **Prompt Engineering for Explanation:**  *Always* ask the LLM to explain
its reasoning *in the prompt response*.  For example: "Please implement this
function and *explain your choices, including why you selected this approach
over alternatives*."  This is crucial.
*   **Rustdoc Comments:** You're right to consider this.  Instruct the LLM to
add detailed rustdoc comments explaining the purpose of the code, the rationale
behind design decisions, and any potential caveats.  This is a great way to
capture its reasoning.
*   **Commit Messages:**  As mentioned before, have the LLM generate commit
messages that summarize the changes and the reasoning behind them.
*   **CI/CD Integration - Documentation Checks:**  Use a CI/CD step to verify
that all public APIs have rustdoc comments.  Tools like `cargo doc` can help
with this.
*   **Dedicated "Reasoning" File:**  For complex changes, you could ask the LLM
to write a separate markdown file explaining its approach.  This file would be
committed alongside the code.  (This is more overhead, but useful for critical
sections.)
*   **Version Control History:** While not perfect, the commit history *is* a
record of changes.  Good commit messages are essential.
*   **Consider a "Knowledge Base" (Advanced):**  If you're doing a lot of
AI-assisted development, you might consider building a simple knowledge base
(e.g., using a wiki or a dedicated tool) to store key design decisions, LLM
reasoning, and lessons learned.



**Workflow Refinements:**

*   **"Review & Refine" Loop:**  After the LLM proposes a change, *always*
review it carefully, even if it looks good at first glance.  Ask clarifying
questions.  Don't be afraid to reject changes and ask for alternatives.
*   **Explicitly State Assumptions:**  When asking the LLM to implement
something, explicitly state any assumptions you're making.  This reduces
ambiguity.
*   **Use Examples:**  Provide the LLM with examples of the desired code style
and functionality.
*   **Iterative Refinement:**  Don't try to get the perfect solution in one go.
Start with a basic implementation and then iteratively refine it.

**Tools to Consider:**

*   **GitHub Copilot/Codeium:** These are AI-powered code completion tools that
can assist with coding in real-time.
*   **CI/CD Platforms:** GitHub Actions, GitLab CI, CircleCI, etc.
*   **Code Review Tools:**  GitHub pull requests, GitLab merge requests, etc.
*   **Static Analysis Tools:** `clippy`, `rustfmt` (integrated into CI/CD)
