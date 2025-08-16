# Possible features

## Simple
Add a file-explorer side-panel. 
- Selecting a file from teh side-panel and pressing a command key will:
  - a. Filter nodes in the file from being returned by the RAG
  - b. Filter returned RAG results to only come from the target file(s)
  - c. Halve the relevance score for the RAG
  - d. Boost the relevance score for the RAG
  - e. Add the entire file to the context sent to the LLM

Add a default file name for instructions for the LLM, similar to the AGENT.md for some editors.

## Code Context Map
Crate a map that shows how the code snippets are connected in addition to
providing code snippets relevant to the user query. The code-graph structure
could prove to be an inherent strength to the project outside the explicit
functionality of enabling database queries.

If we had this feature, it might have helped to avoid the error linked below:
[See Potential Insights from E-DESIGN-DRIFT (Violating Core Design)] "Section 4 System/Tooling Factors"

## Cracked Smol Model

- Change the agentic framework to optimize for uptime, meaning the AI does as much as possible as often as possible.
- Rather than minimize the number of steps, instead it will
  - review + cargo check more often,
  - step through code every few lines,
  - always use a relatively large number of parallel attempts, either literally or prompting the same model multiple times with cycling histories
  - other cool shit!
- Goal is to use the small AI as usefully as possible within the given time frame, not to reduce token cost.
  - This does not mean inefficient processes
