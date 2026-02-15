# test parsing `ploke`

Tests from the `ploke-tui` to run the full `/index start <abs/path/to/crate>` for each of the crates within the `ploke` workspace, separately.

Each item below is tested by first running the `/index` command, and then asking what the LLM can see, to get an idea for whether it is able to see the code from the crate.

These tests are done in ascending order of time required to parse the library, as determined in the `ploke-embed/src/indexer.rs` tests per crate.

- [x] `ploke-rag`
- [x] `ploke-ty-mcp` in 4 seconds
- [x] `ploke-` in 
