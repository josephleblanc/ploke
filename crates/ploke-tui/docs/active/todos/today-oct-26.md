1. Migrate to Ratzilla
- create another test project with a Ratzilla toy program.
- evaluate which changes need to be made to implement a webassembly build of Ploke
- Q: is it as simple as replacing the ratatui rendering with ratzilla-based rendering?
- Q: are there other crates that will need to be changed or that won't work in a webassembly context?
- Q: is there anything extra that needs to be considered regarding file access in the webassembly build?
- Q: should the webassembly build be its own crate or should it be a build flag within ploke-tui?

2. Add token cost estimator
- use the token amounts contained in the responses from the OpenRouter API
- first just keep a count of the tokens in the current conversation
- consider how to estimate cost. Where is this information contained?

3. Add more precise control of indexing and database state, such as:
- check sync status
- pause indexing (currently stub), cancel indexing
- initiate database sync
- add automatic saving of database to local config XDG

4. Fill out chat control functionality
- cancel response/return control to user
- handle reasoning (maybe too big + needs own todo item)
- handle streaming responses (same)

5. Clean up semantic code edits
- add highlighting to list item selection in edit approvals overlay
- add a way to clear the history of approved/denied items in overlay
- make the selection of an item circle back around after moving past the end of the list (current poor functionality allows user to select non-existent items)
- stretch:
  - better previews: Add a way to get the changed code snippet area, instead of the file/item as a whole
  - better selection: add fuzzy search (probably should be its own todo item, see (7) below)

6. Start work on non-semantic editor
- can begin as a fairly simple tool
- must have basic editing functionality, e.g.
  - read file (start line to end line)
  - read file (max chars)
  - search + replace: try a basic search + replace w/ fallbacks, if this fails see how other code editors handle this (e.g. aider seems to have different edits modes for different models)

7. Start work on internal represenation of cargo commands:
- cargo seems to have a json flag for returning values, research this to see if it can work for us
- consider what information is most useful
- add a way to include the cargo feedback in the database + link to node edges in code graph

8. Add a fuzzy search tool that can be dropped into the different overlays

9. Add a more clearly defined structure for overlay windows to unify behavior.
- Consider implementing widget or similar

10. Consider other things to do:
- improve `ploke-embed`
  - add gpu acceleration to vector embedding processing
  - add selection to vector embedding model
  - add remote handling of vector embeddings
- write a short doc on how to add a new model to the list of usable models (if needed, I just forget how this works)
- expand tools
  - list files in directory
  - access non-rust files for read-only
- expand database functionality, esp. for tools
  - add more logging by default: 
    - conversation history, 
    - tool use + success/failure
    - edits made by user vs. LLM, 
    - statistics on usage: % LLM-submitted code that lives for X period of time, accepted code edits
  - add beginnings of user profile:
    - start experimenting with LLM-authored notes on user behavior, check current research here.
    - try to categorize the way the LLM interaction is occurring
    - revisit earlier conversations I had on Gemini for more far-out ideas
    - revisit research on user behavior embeddings to see how it might be adjusted for this use-case
- work on connecting MCP
- Improve the parser for the code graph, in particular:
  - initial type resolution (not handling traits)
  - call graph: start getting a basic approach working w/out handling edge cases + do research
    - get more comfortable with the `syn` representation of `syn::Expr`
    - understand the tree structure of `syn::Expr`
- expand LLM-facing tools to include more code analysis:
  - nodes by centrality
  - node edit history + rank nodes by churn
- Consider advanced functionality like detecting code patters + antipatterns with minimal edit distance
  - review existing research
  - assess feasibility and estimate amount of time it would take to implement
