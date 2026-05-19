# Collaboration Incidents

Durable ledger for agent-caused collaboration failures, especially trust breaks,
instruction drift, and overreach into sensitive authority surfaces. Use this to
track whether skills, repo guidance, and memory actually reduce repeats.

## Categories

- [`authority-boundary-violations/`](authority-boundary-violations/README.md)
  Incidents where the agent treated a sensitive authority surface like routine
  edit territory without first respecting the documented boundary.
- [`diagnosis-without-evidence/`](diagnosis-without-evidence/README.md)
  Incidents where the agent presented a causal diagnosis before reading the
  code that actually controlled the runtime behavior.
- [`model-communication-failures/`](model-communication-failures/README.md)
  Incidents where the agent buried a practical recommendation under mixed
  architecture models, formal vocabulary, or decision scaffolding.
- [`secret-handling-failures/`](secret-handling-failures/README.md)
  Incidents where the agent exposed, copied, logged, or risked exposing API
  keys, bearer tokens, private credentials, or secret-bearing environment
  output.
- [`semantic-naming-failures/`](semantic-naming-failures/README.md)
  Incidents where the agent reused or invented names that collapsed distinct
  domain concepts and made later graph, record, or UI work ambiguous.
- [`verification-surface-drift/`](verification-surface-drift/README.md)
  Incidents where the agent blurred the distinction between CLI snapshots,
  focused renderer tests, and live interactive UI verification.
