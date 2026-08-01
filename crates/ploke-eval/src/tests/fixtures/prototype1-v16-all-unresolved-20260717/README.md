# Prototype 1 v16 all-unresolved replay

Immutable artifacts from campaign
`p1-v16-oraclegate-mbe-g35f-direct-3g1x3-p3-20260717-020840`.

The live run produced one admitted child whose Multi-SWE-Bench report was valid
negative evidence: `regression::r2095` passed, `regression::r2208` failed, and
the oracle verdict remained `unresolved`. Under the admitted `all-resolved`
gate, the candidate must be excluded and selection must complete without a
successor.

The fixture intentionally contains no History segment or successor handoff.
The regression replays these stored artifacts through production channel,
evaluation, selection, and eval-DB projection code.

`baseline-record.json.gz.hex` and `treatment-record.json.gz.hex` are exact hex
encodings of the production compressed run records referenced by the branch
evaluation. The replay decodes them into its temporary campaign and checks the
compressed SHA-256 values before reconstructing selection evidence, so it does
not depend on mutable absolute paths under `~/.ploke-eval/instances`.
