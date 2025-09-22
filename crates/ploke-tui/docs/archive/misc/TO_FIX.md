# Bugs and high priority features

1. When the user enters a query and the embeddings are not loaded and/or indexed, and/or when the bm25 is not loaded, the query should still complete the request to the LLM with no included context.
