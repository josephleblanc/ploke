# Possible features

## Simple + needed and/or parity

- Add a policy for handling things like executing commands in the command line.
  - Add a general purpose "cowboy mode"
  - Add a simplistic permissions model, where you are allowing the AI to have access to some given initial command (identified as the first word in the string that is returned to be piped to the console)

## Finished (needs demo/postings/blogs)
- semantic edits
- bm25 edits
- (in progress) picking a model for the backend
  - need to add a way to configure local embedding models beyond just using sentence-transformers.

## Highly visible
- UX improvements/customization
- maybe keyword search in UI
- make semantic search more visible (e.g. show the x first results or something, expose a kind of "user view" that gives the user an idea of what they AI is doing, kind of like what serena does)

## Simple
Add a file-explorer side-panel. 
- Selecting a file from the side-panel and pressing a command key will:
  - a. Filter nodes in the file from being returned by the RAG
  - b. Filter returned RAG results to only come from the target file(s)
  - c. Halve the relevance score for the RAG
  - d. Boost the relevance score for the RAG
  - e. Add the entire file to the context sent to the LLM

Add a default file name for instructions for the LLM, similar to the AGENT.md for some editors.

- Create a way to drop the current database, or switch to a different database/crate for analysis

## Experimental/needs testing

- Add a vector search check on the AI's suggested code edits to see if there is code that is very similar to the AI's suggestion, then prompt the AI to ask if the search results should be used instead of its suggested code.

## Agent Tools

- Create a tool that allows the agent to search the DB without exposing the cozo script.
  - For example, a search based on the values for some of the common fields of pimary node types that could allow for a searches like:
    - All nodes that have a cfg flag
    - All nodes with async
    - All nodes with/without doc comments
    - and so on

## Possibly complex
- Allow LLM to modify the AST for restructuring and reorganization.

For example, it would be nice if the LLM could refactor by submitting a new organizational structure for functions, methods, etc, without needing to submit a full code edit. That way the LLm lan reason about the abstractions around project architecture and implement the user's requests. Stops the user from needing to copy/paste items around when doing refactoring, speeds things up.

- Do a parse of the suggested edits by the LLM to use semantic search or other
methods to determine if there is already code elsewhere in the project that
does the same thing. That might be a good way to prevent a common issue when
using LLMs, like creating essentially the same function in multiple places.

## Code Context Map
Crate a map that shows how the code snippets are connected in addition to
providing code snippets relevant to the user query. The code-graph structure
could prove to be an inherent strength to the project outside the explicit
functionality of enabling database queries.

If we had this feature, it might have helped to avoid the error linked below:
[See Potential Insights from E-DESIGN-DRIFT (Violating Core Design)] "Section 4 System/Tooling Factors"

## Visual Representation of Vector Embeddings
Open questions:
- dimensionality reduction

Small touches
- subtle sound effects
- fade in/out of filtered items
- undo/redo
- right click

Functional:
- Variable similarity cloud/highlight
- Clustering button
- Search filters/highlights nodes
- Inject node into TUI

Context:
- Context profiles with context items to add to the context window (maybe separate from rust code base)

## Cracked Smol Model

- Change the agentic framework to optimize for uptime, meaning the AI does as much as possible as often as possible.
- Rather than minimize the number of steps, instead it will
  - review + cargo check more often,
  - step through code every few lines,
  - always use a relatively large number of parallel attempts, either literally or prompting the same model multiple times with cycling histories
  - other cool shit!
- Goal is to use the small AI as usefully as possible within the given time frame, not to reduce token cost.
  - This does not mean inefficient processes
