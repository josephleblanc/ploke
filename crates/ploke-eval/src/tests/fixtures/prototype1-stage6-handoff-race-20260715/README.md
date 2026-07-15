# Prototype 1 Stage 6 successor-handoff race

These are exact persisted artifacts from campaign
`p1-stage6-observe-g35f-orembed-3g1x3-p3-20260714-224040`. On 2026-07-15,
the generation-0 controller committed and released its fenced R12 to R13b
attempt after the generation-1 successor had already reached its atomically
released R4c Ready state. The active checkout identity therefore named gen1
while the predecessor walk job was still publishing its terminal receipt.

Original artifacts:

- `predecessor-control-journal.jsonl.gz`: `prototype1/control/sessions/bd06524af3bae42e429eb171a5ef7759df2dfc1a973a116af57be8c4d8e3fb64/control-journal.jsonl`
- `successor-control-journal.jsonl.gz`: `prototype1/control/sessions/efd4963472a1c06ee028a6e2533b494e6373c4b6a0a63f3218e42754d03b354f/control-journal.jsonl`
- `successor-invocation.json`: `prototype1/nodes/node-86c45cb829a036cb/invocations/4f8baa81-ba52-49f6-826a-52d6c3492e82.json`
- `successor-ready-channel.jsonl`: `prototype1/nodes/node-86c45cb829a036cb/channels/4f8baa81-ba52-49f6-826a-52d6c3492e82/child-to-parent.jsonl`
- `transition-journal.jsonl.gz`: `prototype1/transition-journal.jsonl`
- `predecessor-parent-identity.json`: the exact gen0 identity retained in the
  historical edit-harness workspace; it is byte-equivalent as JSON to the
  predecessor session's Created parent.
- `successor-parent-identity.json`: the exact active identity from the shared
  setup source after checkout transfer.

SHA-256 of the original uncompressed journals:

- predecessor control journal: `79bc9b12118c6fb439f8f26f36b429042e89060c967462ecd827d154afa01324`
- successor control journal: `cb4ebcf8df4c3d1bf538f9cab46fe85b1cb6a2b770e367a809a2d1a6d37cfaf1`
- transition journal: `32ff7e8ad1246529142c234faa779eaf0eb2d69e03f0daaf77979fbd4d2912b5`

SHA-256 of the uncompressed fixture files:

- predecessor identity: `66c3eaf8ea46aa769433a98956d2e9e535d3c0cdbba0fca8867b48e9f38f0045`
- successor identity: `ee57ad588f729b5341054c3f3bf993da6edb87d7c2521aa32a7b7e80351e622d`
- successor invocation: `bbb53b97491bca8f0cecb15ba7e6ff6503f31607fc9e7c38f437891693d73090`
- Ready channel: `21b43ec87a6375102911c84e7ae1870492bb59441a0859ea2009f0ea019f6a36`

The walk-operation JSON is deliberately excluded. The original running record
was later resolved as `abandoned` during operator recovery, so its current bytes
no longer represent the moment that exposed the race. The immutable controller,
invocation, channel, transition, and identity evidence above carries the
historical authority ordering without rewriting that later operator decision.

The fixture regression proves that historical ordering through the production
session, invocation, journal, and channel readers. Focused server regressions
exercise the repaired runtime behavior: service readiness, exact predecessor
restore, terminal-receipt refresh, atomic predecessor admission fencing, and
bounded endpoint retirement.
