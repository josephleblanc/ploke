# Prototype 1 V29 terminal walk inspection

These are exact persisted artifacts from campaign
`p1-v29-strictpatchhandoff-mbe-g35f-direct-3g1x3-p3-obs2400-ptok8k-20260719-001018`.
On 2026-07-20, the completed generation-2 runtime could no longer be inspected
through auto-spawned `loop walk` commands because generic server admission
treated the strict terminal controller-transfer rejection as a reason not to
serve read-only requests.

Original artifacts:

- `parent-identity.json.hex`: exact bytes from the active checkout's
  `.ploke/prototype1/parent_identity.json`, hex encoded for a text-only
  fixture.
- `successor-lifecycle.jsonl`: the exact unmodified transition-journal records
  at lines 94, 95, 96, and 140: Spawned, Ready, acknowledged handoff, and
  Completed for runtime `8bb89724-76d8-4b54-b273-2930a967ec38`.

SHA-256:

- parent identity:
  `bfac7d321d8ccc8eb19db7fe0bdc98d1dcea1f7284beb920dc449cd83c19b18e`
- exact lifecycle-record sequence:
  `2f3d3d1a11217fa1cbfc2a7331ceaee5745647b16e90282e631353c0bc627fe8`
- source lines 94, 95, 96, and 140 respectively:
  `e5ab95f6c932665a8cddcfe721ac489c2e2647f36d4dc4b1198a2a361180c88f`,
  `fedb2190273bf392a8ac2e22da848780b320f35186c8e3b95f958b6e11132eef`,
  `fd9e09a01b47e57dbcf48f35166b830bbf742874cb7534d8c212e28dd886b63c`,
  and
  `fe393c3c7463d08563575791742d785924c93a1ae02384c74c7d571b4ad7fcb5`.

The failing production command was:

```text
ploke-eval loop walk show \
  --repo-root /home/brasides/.ploke-eval/setup-seeds/p1-v29-strictpatchhandoff-mbe-g35f-direct-3g1x3-p3-obs2400-ptok8k-20260719-001018 \
  --with-version
```

Foreground `walk serve` exposed the inner error:
`successor transfer cannot open after runtime lifecycle Completed`.

The historical regression loads the exact records through
`PrototypeJournal::load_entries` and classifies the exact runtime through the
production successor-lifecycle reader. A sibling server regression verifies
that the resulting terminal read-only gate admits inspection and rejects every
transfer-required request. No provider call, checkout change, path rewrite,
field rewrite, or reconstructed fixture payload is involved.
