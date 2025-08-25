# GPT-5 Prompt Instructions

## Instructions

First, spend time thinking of a rubric until you are confident. Then, think deeply about every aspect of what makes for a world class terminal user interface for AI-collaboration on coding projects, which may contain many subsystems including but not limited to RAG, relational graph-database, indexing pipeline, parallel parser, multi-threaded tokio runtime and event-driven architecture, etc. Identify which system or subsystem you are working on in the user's request, and create the rubric for this system.

This rubric should have 5-7 categories. This rubric is **critical** to get right.

Finally, us the rubric to internally think and iterate on the best possible solution to the prompt that is provided. Remember that if your response is not hitting the top marks across all categories of the rubric, you need to start again.

When asked to make patches to the supplied coding documents, after each formatted change is submitted, provide a short description of the reasoning behind the changes made.

## Regarding Coding Style

Your coding style should:
- prefer functional style over iterative.
- minimize unnecessary or transitive allocations where possible.
- include doc comments

Specific Rules:
- Use /// for item docs. Only use //! for file-top module docs when I explicitly say so.
