# Prototype 1 Selection Receipt Float Round Trip

Status: source repaired at `9001a134d`; exact fixture and production receipt
round-trip tests pass; fresh strict live validation pending.

Discovered: 2026-07-17

## Broken Contract

A typed `SelectionDecisionEntry` must have the same content hash before it is
persisted and after it is loaded from `eval_selection_receipt.entry_json`.
The strict loader must continue to reject any receipt whose typed content no
longer matches its stored hash.

The repair must preserve that rejection. Rewriting a stored hash, accepting a
nearby floating-point value, or treating the receipt as unhashed projection
data would weaken the History and successor-handoff chain.

## Preserved Incident

Campaign:

```text
p1-v22-strictkeephandoff-mbe-g35f-direct-3g1x3-p3-obs2400-20260717-174349
```

At R11, the persisted selection decision was:

```text
decision_id = 30114489b9a0d8dfa5e40bdb83c3590cb408646025282d8bbe2062060528a628
parent_id   = node-35c3cdbe137b4142
stored_hash = 80d507d6a28820fad6779151cfbeb19d3138386af7ae0465b048d269e48cdd22
```

The exact 539,596-byte entry JSON reproduced its stored hash when parsed and
serialized as generic JSON. Typed deserialization through the default
`serde_json` float parser changed only:

```text
$.formula.formula.sample
0.9020023504759567 -> 0.9020023504759568
```

That one-ULP change produced:

```text
827f0d705e2b4a027c62bc6eff37a418a0d21e83c2234b93f64fa86800109fbc
```

The strict loader correctly rejected it:

```text
selection entry hash 827f0d... does not match stored receipt hash 80d507...
```

V22 was stopped at R11. No row, hash, profile, journal entry, or workspace was
changed to make the admitted run pass.

## Root Cause

`serde_json` 1.0.149 gates exact lexical float decoding behind its
`float_roundtrip` feature. Without that feature, this token was rounded to the
adjacent binary value during typed deserialization. Enabling the feature made
the complete preserved entry byte-identical after its typed round trip.

The source path was:

```text
selection_rows
-> SelectionDecisionEntry::decision_hash
-> serde_json::to_string
-> persist receipt hash and entry JSON

selection_entry
-> serde_json::from_str::<SelectionDecisionEntry>
-> SelectionDecisionEntry::decision_hash
-> strict stored-hash comparison
```

The loader was not defective. The writer lacked proof that the typed wire
representation it published was replay-stable under the binary's parser.

## Repair

Commit `9001a134d`:

- enables `serde_json/float_roundtrip` for `ploke-eval`;
- serializes, typed-deserializes, and rehashes a selection entry before any
  receipt rows are published;
- fails closed with both initial and replayed hashes if the entry is not
  stable; and
- leaves all existing loader shape, candidate-set, procedure, content-address,
  and hash checks unchanged.

The owner database is persisted only after the mutation closure returns
success, so a guard failure cannot publish a partial receipt.

## Regression Evidence

The checked-in v22 fixture is the exact persisted
`eval_selection_receipt.entry_json.formula` value and records its campaign,
parent, decision, relation, and stored-hash provenance.

Tests prove:

- the exact hostile float token is byte-stable through typed formula replay;
- an incident-class score-child-prop entry traverses the production
  writer, persisted receipt, hash loader, and strict typed receipt loader;
- the new writer guard rejects an initial/replayed hash mismatch; and
- all 55 eval-store tests pass.

The earlier full `ploke-eval` library run had 1,393 passing tests and two
unrelated failures: one setup-preview lock interference that passed alone,
and one stale canonical backup fixture missing `array_type`. The fixture
registry was last reviewed on 2026-06-22 and is overdue under the seven-day
review policy; this receipt repair does not loosen that importer.

## Footprint

No new crate or `Cargo.lock` change was introduced. The final debug binary was
565,600 bytes larger than the pre-feature binary, approximately 0.057%.
Selection runs the extra typed replay while holding the owner-database mutex;
that is a small, infrequent correctness cost and remains a performance
follow-up rather than a reason to weaken the guard.

## Next Proof

Use a newly admitted campaign from `9001a134d` or later. At R11, stop and prove
that a fresh server reconstructs the exact typed receipt and hash before
advancing to R12. Do not advance v22 with the repaired executable as live
handoff evidence.
