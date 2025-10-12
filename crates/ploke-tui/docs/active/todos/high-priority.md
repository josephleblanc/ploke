# Important next todos

Date: Oct 2, 2025

Note: If today is after Oct 4, disregard these priorities and reassess their relevance

In no particular order, these are the most important things to get done in the project.

- [x] fix the way the user's submitted message is handled so they will receive a helpful response from the LLM along with a system help message with instructions on how to index a target crate + how to use the "help" command. This will likely be the initial entrypoint for most users who will run the application and then just enter something, so it is important to make sure this case is handled well.

- ensure there is a proper test harness for testing events, so we can add better tests for end-to-end event flows + mock LLM responses realistically for offline tests and check the values are as expected for online events at different checkpoints along the event flow of any given process.

- Fix the way `Tool` messages are not being displayed. Currently we are just seeing the LLM's message that is part of their response while calling the tool (the `content` part of the message), but it would be good to additionally provide feedback so the user can see that our system is calling the tool.
  - I believe we already have some setup to do this already, but may have made the tool feedback not be displayed to the user. What we want is to be able to provide some feedback like "calling the X tool with <args>" -> "called tool X successfully, <summary of results>"

- larger, more broad task that requires design attention: How to handle cases when there is an error in the parsing phase due to a malformed file?
  - If there is a malformed file, then the `syn` parsing will fail, causing our `syn_parser` to fail, which will cause the overall user message embedding process to fail. This is not good.
  - Need to decide how to handle errors. Maybe set up a way to handle the response from `cargo test` or `cargo build` or `cargo check` as a structured response? We want to make sure we have a way to show the information in a helpful way to both the user and the LLM, for both the conversation-style workflow of back-and-forth with the LLM and a good way to handle such errors in the agentic tool-calling loop.
  - Q: What else breaks when we can't parse the target file? Basically everything, I think. We would lose semantic code edits (currently our only method of editing), along with all code search and currently built ways of displaying the code in the target crate to the LLM. Hmmm... This one will need some thought.
  - maybe we can add a way to grep, but only when there is a problem parsing the target crate? The main issue I have with grep is that I'm worried the LLM will prefer grep as the $ore familiar option over our semantic search tools. Maybe if the parsing fails then we expose the grep tool to the LLM - we could do this through a state tracking field we add to `AppState` or something, where if the parser fails we change the value, and while the parser fails the LLM can access the grep tool. Hmmm... Then we would need to add a way to edit the files without using our semantic code replacement tool, which would be kind of a pain. Maybe just use a file-editing MCP instead? Check on the progress of the `ploke-ty-mcp` crate to see how much work it would take to set up an external MCP for when we have failing parses to assess.
    - Eventually we probably want to add a way to hook up to `rust-analyzer`, but I'm not sure we are ready for that commitment yet.
