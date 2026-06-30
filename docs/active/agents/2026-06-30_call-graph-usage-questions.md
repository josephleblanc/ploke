# Call Graph Usage Questions

Date: 2026-06-30
Title: Call graph usage questions
Short description: Reference examples of the kinds of engineering questions a call graph can answer.

Related planning files:

- [call-graph/README.md](call-graph/README.md)
- [call-graph/2026-06-25_call-graph-coverage-inventory.md](call-graph/2026-06-25_call-graph-coverage-inventory.md)
- [call-graph/2026-06-28_real-corpus-call-site-oracle-matrices.md](call-graph/2026-06-28_real-corpus-call-site-oracle-matrices.md)

## Impact Analysis

- If I change this function's return type, which callers need to be updated?
- Which public APIs eventually call this helper?
- What feature paths might be affected if this method starts returning an error?
- Is this function only used by tests, or is it reachable from production entrypoints?

## Dead Code Detection

- Is this function reachable from any binary, test, macro entrypoint, or exported API?
- Which private helpers have no incoming callers?
- Is this implementation truly unused, or is it only called through trait dispatch or generated code?
- Can this deprecated wrapper be removed without breaking downstream call chains?

## Navigation

- Who calls this function?
- What functions does this request handler call directly?
- What are the callsites for this trait method binding?
- From this owner function, which local callees can I traverse to in the persisted graph?

## Security Analysis

- Can untrusted request data reach a SQL query, shell command, filesystem write, or network sink?
- Which call paths can reach `unsafe` blocks or FFI boundaries?
- Are authorization checks always called before protected state mutations?
- Which external dependency calls are made from this user-facing entrypoint?

## Performance Work

- Which hot-path handlers call allocation-heavy helpers?
- Does this request path call blocking I/O under async code?
- Which callers trigger repeated parsing, cloning, serialization, or database work?
- What call chains reach a function that is known to dominate runtime cost?

## Refactoring Support

- Which callers need migration before this helper can be split or removed?
- Can this function be moved to another module without creating dependency cycles?
- Are these two similarly named helpers called from the same owners or from separate layers?
- If this method is renamed, which direct callsites and generated proof rows should change?

## Test Planning

- Which tests should cover a change to this function?
- Are there production call paths into this branch that are not covered by fixture assertions?
- Which callsite shapes are covered only by synthetic fixtures and not by real corpus tests?
- Does the database fixture prove both the caller edge and the target-centered query surface?

## Architecture Review

- Which modules call across a boundary that should be one-way?
- Are lower-level crates depending on higher-level application code?
- Do any call chains bypass the intended abstraction layer?
- Are there unexpected cycles among service, storage, parsing, or UI modules?

## Debugging

- How can this error-producing function be reached?
- Which callers can pass this unsupported receiver shape into the resolver?
- Is this observed behavior coming from a direct call, a trait method path, or dynamic dispatch?
- What source callsite corresponds to this persisted call edge or proof blocker?

## API Understanding

- How is this library function used in real target code?
- What argument shapes do existing callers pass?
- Is this trait method usually called through method syntax, UFCS/path syntax, or a bound type parameter?
- Which constructors are used directly, and which are only reached through re-exports or aliases?

## Documentation And RAG

- What caller and callee context should be attached when explaining this symbol?
- Which proof facts show that this target-centered call context is trustworthy?
- What fail-closed blocker should be shown when a callsite is visible but targetless?
- Which source files should be retrieved to answer a question about this call chain?

## Build Or Deployment Optimization

- Which components are affected by a change to this API?
- Can CI run only the tests that exercise affected call chains?
- Which crates or binaries need rebuilding after this internal function changes?
- Are there feature-gated or platform-specific call paths that should be checked separately?
