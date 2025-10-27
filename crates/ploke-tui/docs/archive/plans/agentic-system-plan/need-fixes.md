It looks like you've broken something, because when I run the application there is a problem.

Steps to reproduce:
1. Run with `cargo run`
2. Enter `/load crate fixture_nodes`
  - load of backup database with pre-processed embeddings is successful
3. Enter a query, e.g. "Hello, can you see any code snippets?"

Expected:
- A response from the LLM

Actual:
- The message "Embedding User Message" appears, but is not changed or updated. Nothing further happens.
