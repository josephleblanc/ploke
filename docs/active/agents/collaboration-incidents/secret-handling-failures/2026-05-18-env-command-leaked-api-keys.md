# 2026-05-18 Environment Command Leaked API Keys

## Trigger

The user asked whether a failing OpenRouter live test was using
`OPENROUTER_API_KEY`.

## User-Visible Failure

The agent ran an environment inspection command that printed secret-bearing
environment values into the chat output. The user immediately objected:
"NEVER, and I mean NEVER print the api key like that. DO NOT LEAK MY KEYS".

## Touched Code Surface

- Diagnostic shell command path in the shared workspace.
- OpenRouter live-test triage around `crates/ploke-llm/src/router_only/tests/mod.rs`.

No code file caused the leak; the failure was in the agent's diagnostic command
selection and output handling.

## What The Agent Did

The agent used a broad environment-printing command filtered by provider names.
That still emitted raw secret values. The correct diagnostic should have been a
masked presence/source check, for example reporting only whether
`OPENROUTER_API_KEY` is set and its length or suffix hash, never the value.

## Skipped Docs / Skills / Instructions

- General secret-handling discipline was not applied before command selection.
- The agent prioritized speed of credential diagnosis over output safety.
- No command-output redaction step was inserted before exposing terminal output.

## Why This Was Risky

API keys printed into chat must be treated as compromised. Even if the local
terminal output is otherwise private, the assistant surface is not a safe place
to display credential values. The mistake also destroys trust in future
diagnostics that touch environment variables, provider auth, headers, or
request logs.

## Prevention Rule

Never run or expose commands that can print raw secret values. For environment
checks, use explicit masked probes that print only presence, variable names,
lengths, or a short non-reversible fingerprint. Never print values for
variables whose names contain `KEY`, `TOKEN`, `SECRET`, `PASSWORD`, `AUTH`,
`BEARER`, `CREDENTIAL`, or provider credential names.

When diagnosing auth, first identify the code path that reads the credential,
then run a masked check such as "is set" or "not set". If raw request/response
logs may include headers or credentials, inspect them through a redaction
filter before displaying output.

## Memory Hypothesis

This failure is not a memory drift issue. It is a command-safety failure:
secret output was not treated as a high-risk surface before running a shell
inspection command.
