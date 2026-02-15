# DB Persistence: Overview

## todo:

- [x] Save db state to file
  - [x] Add command
- [ ] Read Db from file
  - [ ] Add command
- [ ] On startup, check user directory
  - [ ] If directory exists in config folder's backups, prompt the user to try loading.
  - [ ] If the user does want to try loading, then parse and check to see if
  the embeddings need to be updated.

## detailed todo
- [ ] Read Db from file
  - [ ] Add command to app command input handling "/load db"
    - [ ] Write immediate message in history to tell user we are loading the db
    - [ ] send event `SystemCommand::LoadDb`
  - [ ] Load db from default config dir, if it exists
    - [ ] Modify the loading command to specify the name of the crate to be loaded

## Database and memory workflow

At query time:
  - Short term: check file hash, reparse full repo on change detection
    - Q: How does time travel work here?
    - Full insertion would be a problem, since we would lose embeddings.
  - Long term, we want incremental parsing.
    - Let's first work out type resolution, though.

  To avoid going fully into incremental parsing implementation strategy right now, just filter parsed/merged graph by file/file-based module before transform.

After update + embeds, on different threads

1. save db: When this happens, update a hashmap or branching structure with the
  - overall database version 
    - Q: what is this exactly in cozo?
  - git tracking hash
    - TODO: git integration
    - Easiest with just mcp integration, probably

2. prepare to send prompt to LLM
  - memory: before sending prompt, the built context should be logged to the database and embedded.
  - Then we trigger an update of the memory graph 
    - which is in the same database as the code graph, but uses its own set of edges to connect its nodes.
    - The general idea is for there to be very specific and intentional linkages between the memories and the code that the memory is about. For example, if the user wants to make some changes to the code, then the code snippets which are included in the LLM's context window and that were put there by querying the graph in the first place, those nocdes are linked to the memory node containing the context sent to the LLM.
    - More needs to be done to build out the design of the memory system (1).
  - possibly this includes other RAG-oriented steps such as multi-turn search (likely using a small, fast model), or reranking w/ LLM as judge, or various trad reranking strategies.

3. Send the context to the LLM
- When we send the augmented prompt to the LLM, we save the database to file again.
- Either this results in generated text or a tool call.

4. a. Tool call: TODO (likely something needs to be added to memory)

4. b. LLM-generated response is added to the conversation
- additionally, we process the response into embeddings, and save both in the conversation history part of the memory.
  - One cool analysis or evaluation we might do here is see what code snippets the RAG returns for the LLM's response, and cross-reference those with the nodes returned by the user's original context. Not exactly sure what this could mean, but I'd be happy to find out this was a useful comparison.


## Notes on cozo time-travel
Example of defining a type for time-travel:

I pasted a lot of stuff here before, but basically just look at the cozo docs for time travel and the tutorial on time travel.

At the end of the day it is basically just adding a field in a certain place, and then you still need at add updates or delete items as usual - the main difference is that there needs to be a field for `Validity`, which contains an integer that indicates a time (defaulting to Unix epoch), and then if you add a new item, you just add a the timestamp, and you can query for the node's state at a certain time. The main benefit is that when you do time-sensitive queries, it will be an optimized search on the `cozo` side that is much faster than a similar datalog implementation, since cozo is doing some semi-fancy things under the hood to speed it up.
