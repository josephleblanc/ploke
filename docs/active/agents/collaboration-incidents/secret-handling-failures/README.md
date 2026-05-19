# Secret Handling Failures

Incidents where the agent exposed, copied, logged, or risked exposing secret
material such as API keys, bearer tokens, private credentials, or secret-bearing
environment output.

## Entries

- [`2026-05-18-env-command-leaked-api-keys.md`](2026-05-18-env-command-leaked-api-keys.md)
  The agent ran an environment-printing command during OpenRouter credential
  diagnosis and allowed API key values to appear in chat output instead of using
  a masked presence check.
