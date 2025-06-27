# ploke-rag research

Notes on project structure, speculative planning documents.

### Retrieval

- Iterative Query Refinement
  - initial retrieval
  - analyzes content of retrieved documents
  - uses that new information to expand/refine subsequent query loops

- Generation-in-the-loop ([RepoCoder])
  1. Initial retrieval on existing codebase
  2. Generate intermediate code
  3. Use newly generated code as foundation for new query

- Execution-in-the-Loop ([ARCS])
  1. Retrieve: Gather relevant code snippets from corpus.
  2. Synthesize: Generate a candidate code solution using an LLM
  3. Run the generated code in a sandbox w/ tests
  4. Use execution feedback to iterate on code until it passes

### Reranking and Judging

- Provide a scoring framework to an LLM to score retrieved snippets (REBEL)






### Sources

- RepoCoder: arXiv:2303.12570
- ARCS: arXiv:2504.20434
- REBEL: arXiv:2504.07104

[RepoCoder]:https://arxiv.org/abs/2303.12570
[ARCS]:https://arxiv.org/abs/2504.20434
[REBEL]:https://arxiv.org/abs/2504.07104v1
