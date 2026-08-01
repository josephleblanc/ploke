# Direct OpenAI Embeddings Reject Overlong Parsed Nodes

Status: open backend-policy gap; affected live run preserved for abandonment;
config recovery selected.

Discovered: 2026-07-14

## Broken Contract

Prototype 1's direct-OpenAI embedding route admits and passes a live execution
preflight, but production indexing can still submit one parsed node that exceeds
the model's per-input token limit. The resulting provider error is persisted as
terminal baseline failure only after the run has created indexing evidence.

The backend also replaces every non-success response body with
`OpenAI response body omitted`. That protects credentials and source text, but
it prevents the persisted failure from distinguishing an invalid request from
other HTTP 400 causes.

## Affected Run

```text
campaign:  p1-stage4-observe-g35f-oaiembed-3g1x3-p3-20260714-004446
parent:    node-4a8027fd76935902
session:   7835dfe6-8ef5-437d-87f1-7a8e98c36663
operation: 069cfc82-b865-47ef-8965-ebee678991f1
instance:  BurntSushi__ripgrep-2209
run:       run-1784089150332-structured-current-policy-ac09b99a
phase:     R5 -> R6
```

The admitted campaign selected:

```text
embedding_route:    direct_openai
embedding_model_id: openai/text-embedding-3-small
```

The run profile commitment remained
`bf132cccaddabf04a4681ba908a743e14a67185e335107409531e8fbc2fd832a`.

## Evidence Chain

The production log records 749 unembedded nodes. Five 100-node requests
completed and persisted checkpoints. The sixth request included this node:

```text
node_type:   Module
node_name:   tests
file:        crates/printer/src/standard.rs
byte_range:  59353..126160
snippet_len: 66807
```

The sixth request failed at the direct OpenAI endpoint. The persisted indexing
status records 500 processed items and HTTP 400. The batch summary records one
attempted instance, zero successes, and no agent execution log, turn trace,
record, patch, or protocol artifacts.

A bounded live diagnostic sent only that exact 66,807-byte source range to the
same model outside the run. The sanitized OpenAI error was:

```text
type:    invalid_request_error
message: Invalid 'input[0]': maximum input length is 8192 tokens.
```

This joins the opaque persisted HTTP 400 to the concrete provider rejection.
Credentials, credit, route reachability, model identity, and vector dimensions
are not causal: the same production run completed five preceding requests.

Campaign closure, the global run registration, batch summary, indexing status,
walk job, and controller journal agree on the same failure. The persisted state
is valid failed evidence, not corruption.

## Source Boundary

`EmbeddingProcessor` limits direct-OpenAI requests to 100 snippets, and
`OpenAIBackend` repeats that count-based split. Neither layer enforces a
per-snippet token or character bound before `fetch_batch` sends the request.

OpenRouter already has the missing policy boundary. ADR 006 requires its
backend to apply the shared `TruncatePolicy`, with `truncate` as the default and
`reject` and `pass_through` as explicit alternatives. It derives a conservative
character cap from model context metadata. Direct OpenAI was added later and
does not carry that policy.

The one-item doctor preflight correctly proves credentials, endpoint access,
model execution, and dimensions. It does not prove that every future repository
node fits the model context, so its success is not contradictory evidence.

## Run Disposition

The R5-to-R6 walk operation is correctly `indeterminate` because baseline work
crossed a durable effect boundary before closure classified the instance as
failed. The R5 cursor did not advance. The job and session must be explicitly
abandoned; the failed closure and indexing snapshots must not be rewritten or
retried in place.

The config-first recovery is a fresh campaign using the existing OpenRouter
route with an explicit `openai/text-embedding-3-small` model. That route applies
the approved backend truncation policy and uses the now-funded OpenRouter key.
The new run must receive a fresh campaign, parent, session, registry entry, and
baseline evidence.

## Missing Repro Coverage

Direct OpenAI has mock-server coverage for batching, cancellation, response
ordering, dimensions, and redaction, but no regression for an overlong input.
A future direct-OpenAI repair needs a backend-level test that exercises the
selected policy before any HTTP request. Count-only batching is not sufficient.

Do not repair this by silently skipping the node, treating failed closure as
partial success, or rerunning over terminal evidence. Adding truncation to
direct OpenAI changes embedding semantics and needs an explicit policy decision;
`reject` and length-aware chunking/aggregation remain alternatives.

## Related Decisions and Bugs

- [`ADR 006: OpenRouter Snippet Truncation Policy`](../ADRs/006-openrouter-snippet-truncation-policy.md)
  is the approved policy used for the fresh config recovery.
- [`2026-07-13-prototype1-embedding-preflight-after-admission.md`](./2026-07-13-prototype1-embedding-preflight-after-admission.md)
  records the earlier OpenRouter key-limit failure and the live preflight that
  now distinguishes route readiness from production input coverage.
