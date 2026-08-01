# P1 v22 Selection Receipt Float Round-Trip Review

Campaign:
`p1-v22-strictkeephandoff-mbe-g35f-direct-3g1x3-p3-obs2400-20260717-174349`

Status: preserved R11 live run; no R12 projection or successor handoff.

## Verdict

V22 validated the 2,400-second observer configuration: all three parallel
children completed and R10 committed to R11 after approximately 21 minutes
53 seconds. It then found a typed persistence defect before the next state
transition: the strict receipt loader rejected a one-ULP float change caused
by default `serde_json` decoding.

That rejection was healthy. V22 was stopped and preserved rather than changing
its stored hash. It also was not a handoff candidate on merit: all three
children resolved the benchmark but were strict operational Rejects.

## Admission And Cursor

- Setup plan hash:
  `e153538aa9661e0ac0cb700355a2fb7f33d34c07e5248291506ff014a4657655`
- Profile commitment:
  `b9f5f30774f6a185e23c6bca374e663ae46b7541a3d135dd587b2a527e124a04`
- Admitted source HEAD:
  `684674e6ddcf86c106c3728e6f785fffe0e346b3`
- Identity commit:
  `5ad23182483fc94e8d6f8d3f83a4570d5b081200`
- Parent:
  `node-35c3cdbe137b4142`
- Walk session:
  `a2e44aa7-9c57-4430-bd1e-80318068a433`
- R11 evidence:
  `95b9a799a9abaaa5764e54bb2e1f66d79eaf9b42c2023f159dc978d396aceedc`

The admitted profile matched v21 except:

```toml
observe_child_stale_after_secs = 2400
```

## Child Outcomes

Every child had resolved MBE evidence and changed the benchmark result from
274 passing / 2 failing tests to 276 / 0:

- `node-c27c7bab7c0a2331` / `branch-37069c70c8405724`: Reject because
  same-file retries regressed 0→1 and maximum streak 0→2.
- `node-c6afbcea978bfa3f` / `branch-27158c2492b589e2`: Reject because tool
  failures regressed 0→1 and maximum retry streak 0→1.
- `node-4ef5d6dd4c9daf31` / `branch-7f1260d00b5de1e1`: Reject because tool
  failures regressed 0→2 and maximum retry streak 0→1.

The score-child-prop formula selected `node-4ef5d6dd4c9daf31` as the sampled
coordinate, but its base outcome remained `Stop`. With
`require_keep_for_continuation = true`, v22 could not legitimately hand off
even if receipt loading had succeeded.

All three persisted traces had `final_assistant_message: null` despite applied
patches and completed closure. That is a separate trace-accounting gap and
should not be conflated with the receipt hash defect.

## Receipt Failure

The stored selection entry:

```text
decision_id = 30114489b9a0d8dfa5e40bdb83c3590cb408646025282d8bbe2062060528a628
stored_hash = 80d507d6a28820fad6779151cfbeb19d3138386af7ae0465b048d269e48cdd22
entry_bytes = 539596
```

Typed decoding changed only the formula sample from
`0.9020023504759567` to `0.9020023504759568`, producing hash
`827f0d705e2b4a027c62bc6eff37a418a0d21e83c2234b93f64fa86800109fbc`.
Generic JSON replay and `serde_json/float_roundtrip` both retained the exact
stored bytes.

Commit `9001a134d` enables exact float replay and adds a write-side typed
rehash guard without weakening the loader.

## Read-Only Post-Repair Check

A repaired binary was used only to start a read-only reconstruction server.
`walk status` and `walk show` recovered the original session, R11 cursor,
evidence hash, and all three channel-derived child outcomes with no blocker.
No job or transition was admitted, and the sidecar was then stopped.

That check validates reconstruction, not executable provenance for v22.
V22 must never be advanced as the fresh handoff proof.

## Next Proof

Admit a new campaign from committed repair HEAD. Retain the strict profile and
2,400-second observer budget. At R11, restart or reconstruct from the typed
receipt before taking R12. Continue only if a strict Keep exists, then verify
R13b retirement/successor identity and advance the successor through at least
one more generation on the same walk endpoint.
