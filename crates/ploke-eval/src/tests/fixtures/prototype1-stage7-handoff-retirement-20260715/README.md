# Prototype 1 Stage 7 successor-retirement cycle

These are exact persisted artifacts from campaign
`p1-stage7-handoff-canary-g35f-orembed-3g1x3-p3-20260715-023614`.
On 2026-07-15, the generation-1 successor published its atomically released
R4c Ready receipt while the predecessor walk operation
`50d299f7-ee5c-4c28-91fd-8bd31af9aaf7` was still internally `Running`.
The predecessor session committed R12 to R13b and released fence 12, but the
first successor process exited while trying to retire the predecessor. The
walk operation was not terminalized until 12.7 seconds after successor Ready.
After outer-receipt reconciliation, the operator used the supported
predecessor `Stop` path to retire the old endpoint.

Original artifacts:

- `predecessor-control-journal.jsonl.gz.hex`: deterministic gzip (`gzip -n`),
  hex encoded for a text-only fixture, from
  `prototype1/control/sessions/845e6e61427fdd2b7e81b850f1281f8fb6d97069404c429cbef61ad0f900cd4b/control-journal.jsonl`.
- `successor-control-journal.jsonl.gz.hex`: deterministic gzip (`gzip -n`),
  hex encoded for a text-only fixture, from
  `prototype1/control/sessions/951389e16e61f1cf15ffd813922a10737c5275a3c8f539a90fd82cffa20d2be3/control-journal.jsonl`.
- `successor-invocation.json.hex`: the exact generation-1 successor invocation,
  hex encoded for a text-only fixture.
- `successor-ready-channel.jsonl`: the typed Ready channel envelope written by
  PID 1620416.
- `predecessor-operation.json.hex`: the exact eventual terminal walk-operation
  record, hex encoded for a text-only fixture. Its admitted identity, expected
  R12 session version, start time, and final receipt preserve the operation
  that remained Running at Ready time.

SHA-256:

- predecessor journal, uncompressed:
  `c7e72973b41c4897e31cb185989b7e5fbd7fbbb88a480be2775d3e83b53a8f95`
- predecessor journal, deterministic gzip:
  `cf1f44d705bb7f0853303538d0d962f103523c737f092a9ec2531d19cac66db3`
- successor journal, uncompressed:
  `1035c209861cc89cc23b5e817aea554d41b52746e139c0e4ea19de75d8b8c9d0`
- successor journal, deterministic gzip:
  `768c259a7f8bf75ee340073c94e454848510b1590908793dbfdd2ec0dd9491c7`
- successor invocation, decoded exact bytes:
  `25b31000d9c76bed5e807e0205fb18c05960df3af7519c24359e0ce34de7b14a`
- Ready channel:
  `388b266b09dc39575a3b7bf7551453b3e1af021c868fdc599b91efe8a6344abe`
- terminal operation, decoded exact bytes:
  `bfc5b64ed7a0326a6cd1e2697acec232fa8bf15f47773f0ccaaa663d01aeaa46`

The historical replay inflates both exact journals and feeds them through the
production session reader, loads the exact invocation through the production
authority loader, decodes the typed Ready channel, and decodes the durable
operation carrier. A sibling socket regression exercises the production walk
server and predecessor-retirement path with an active outer handoff job beyond
the former deadline, then publishes its typed terminal receipt through
`finish_job`. No provider calls or checkout changes are involved.
