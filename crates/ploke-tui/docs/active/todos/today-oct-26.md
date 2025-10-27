1. Add token cost estimator
- use the token amounts contained in the responses from the OpenRouter API
- first just keep a count of the tokens in the current conversation
- consider how to estimate cost. Where is this information contained?

2. Migrate to Ratzilla
- evaluate which changes need to be made to implement a webassembly build of Ploke
- Q: is it as simple as replacing the ratatui rendering with ratzilla-based rendering?
- Q: are there other crates that will need to be changed or that won't work in a webassembly context?
- Q: is there anything extra that needs to be considered regarding file access in the webassembly build?
- Q: should the webassembly build be its own crate or should it be a build flag within ploke-tui?
