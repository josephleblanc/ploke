# Prototype 1 v25 selection-safety fixture

This fixture preserves the live campaign
`p1-v25-strictkeephandoff-mbe-g35f-direct-3g1x3-p3-obs2400-20260717-221126`.
`selection-decision-entry.json` is the exact 573,257-byte
`eval_selection_receipt.entry_json` value returned by a read-only walk
snapshot. Its typed domain hash is
`26e318146c7ca203b18f924b6b329e96975743dfbe416a92d743730912545de6`.
The remaining files are byte-for-byte campaign artifacts retained as upstream
provenance.

The historical profile had no `[selection.patch]` section, so its patch gate
resolved to `disabled`. All three operational evaluations recorded `keep`, but
the selected candidate contained an unsafe patch. The regression replays the
persisted `SelectionDecisionEntry` through typed shape validation,
`ParentSelectionOutcome::from_entry`, and the production traversal selector,
then proves both sides of the incident:

- the unchanged historical payloads reproduce the recorded unsafe selection and
  exact decision hash under the original disabled gate;
- explicit `reviewed-admissible` validation fails closed because the historical
  candidate set has no candidate-review evidence;
- typed post-incident reviews exclude both unsafe candidates while leaving the
  narrow error-boundary repair eligible; and
- an all-rejected overlay preserves a replayable typed no-selection receipt
  whose three formula rows record `patch_gate_not_satisfied`.

Reconstructing the receipt from the later node files is not a historical
replay. All three node records were updated after R11, and `updated_at` is
inside the hash-bearing candidate artifact. The receipt records
`06:11:04.538796911`, `06:11:02.938822642`, and `06:11:02.729077596` UTC;
the copied post-selection node files record `06:31:12.983015982`,
`06:30:59.690565144`, and `06:29:27.332281941`. Those three timestamp changes
alone alter every payload membership, the candidate-set root, the deterministic
ScoreChildProp sample, and the decision hash. The persisted entry is therefore
the authoritative replay carrier.

The copied carriers are intentionally immutable. Candidate reviews generated
after this incident would be post-incident overlays, not historical evidence,
and do not belong in this fixture. The source campaign remains preserved under
`~/.ploke-eval/campaigns`; its database was queried only through the walk
service's immutable snapshot and was not opened in place or modified.

The counterfactual overlays are constructed only in memory. Their shared
reviewer identity is hashed through the production admitted-config path after
admitting the exact copied V25 profile in a temporary directory. Candidate
`node-2f0b3cda4c2c8d89` changed two files, but V25 did not persist per-file
content hashes for its second path. The regression therefore pins the
post-incident source/proposed SHA-256 values for
`crates/ploke-tui/src/tools/get_code_edges.rs` as
`be0dd87df60e1aa5e1381ad9d9ee7d404863487dd58fa129a3c93a9686e91776` and
`055dd8d3c5a357ca1177b69125eff2dace008cb741b18523f88ad1f6659f6d66`;
it does not misrepresent those later-derived hashes as persisted V25 evidence.

## Immutable artifact provenance

Unless a row names the setup seed explicitly, its source path is relative to
`~/.ploke-eval/campaigns/p1-v25-strictkeephandoff-mbe-g35f-direct-3g1x3-p3-obs2400-20260717-221126`.

| Fixture artifact | Preserved source path | SHA-256 |
| --- | --- | --- |
| `selection-decision-entry.json` | read-only owner snapshot: `eval_selection_receipt.entry_json` for decision `63b8fe16c9b3570fb1a978992ec56266e5c2f2a68a47a3b760f3a936f32e2941` | `b453ddaf8d0ac90527835568f8ad16ad93a0d84158deb0fc035d082802ff2615` |
| `campaign.json` | `campaign.json` | `ab21d10c0efdaa003ac49312de5baef0b39fc282640b1808d769a9bce795fec5` |
| `run-profile.toml` | `prototype1/run-profile.toml` | `c3b2fd9147560917f64bb0d28a833016f0e46080fd9453e73f3883162631b85a` |
| `run-profile.commitment.json` | `prototype1/run-profile.commitment.json` | `cb021229bfde16585bb55eedaf62cab3244ac581270efb5e92ef574d6262807c` |
| `parent_identity.json` | `~/.ploke-eval/setup-seeds/p1-v25-strictkeephandoff-mbe-g35f-direct-3g1x3-p3-obs2400-20260717-221126/.ploke/prototype1/parent_identity.json` | `b000745fed82cc75fdd2b1fdd6c1340f473c1d682d2cde77af7be65ad70ad071` |
| `child-plan-node-461dba1909fb6cf7.json` | `prototype1/messages/child-plan/node-461dba1909fb6cf7.json` | `f68736a57839947494478b10a72dee26da2537e72a1bd34b4d7922e414a4bd64` |
| `node-f4f44e07e0be8caa.json` | `prototype1/nodes/node-f4f44e07e0be8caa/node.json` | `638c9094635474d7472179499d78549f7a49ac6f926601d1523ed18fa1afec0c` |
| `node-3247ff02a2053a57.json` | `prototype1/nodes/node-3247ff02a2053a57/node.json` | `f47153b214ee327afcfd5148763f23f5cb3500e670376ec8b8cbe6e65fa27beb` |
| `node-2f0b3cda4c2c8d89.json` | `prototype1/nodes/node-2f0b3cda4c2c8d89/node.json` | `8d50a7cb7b738602fb844a95287151c4132ef410e21ea45a152f825c8db3c562` |
| `branch-55892858db150ccd.evaluation.json` | `prototype1/evaluations/branch-55892858db150ccd.json` | `93e90a839c12ec8eacb74ebfca53b8a56d815fe98e59d0421d1dea75395e98c9` |
| `branch-269c5d245f354a1e.evaluation.json` | `prototype1/evaluations/branch-269c5d245f354a1e.json` | `df45a63dd9af27f2061a6fb82f6194c020082cfd32c5ee5bb45e1d38acd6928d` |
| `branch-42f0d2c400a40cee.evaluation.json` | `prototype1/evaluations/branch-42f0d2c400a40cee.json` | `0522142bd9fe8d887fcfda2b52c38bbc47e5b266ce882c7b79318e4f088069b0` |
| `node-f4f44e07e0be8caa.child-to-parent.jsonl` | `prototype1/nodes/node-f4f44e07e0be8caa/channels/03443fc0-0ba8-4bcc-82b3-950fd4834468/child-to-parent.jsonl` | `f1cf77fd595316055dfc9f2dd67ddb2240fd1178164749559e0e29f8954b3949` |
| `node-3247ff02a2053a57.child-to-parent.jsonl` | `prototype1/nodes/node-3247ff02a2053a57/channels/037545bc-78a5-4951-82c5-8b6bcdc09133/child-to-parent.jsonl` | `48c4c039fb5c822bd485a7c46830b18b38a342c9dc4351bb8341cf1c1ad752eb` |
| `node-2f0b3cda4c2c8d89.child-to-parent.jsonl` | `prototype1/nodes/node-2f0b3cda4c2c8d89/channels/82ee29fc-941e-416b-80be-fbdff4ae30fd/child-to-parent.jsonl` | `dddbc063fd326b505a0f08972fc535f73699c9b31f3617422aba8c38a76a71db` |
