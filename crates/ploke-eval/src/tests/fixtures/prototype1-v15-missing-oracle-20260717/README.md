# Prototype 1 v15 missing-oracle selection fixture

This fixture is copied byte-for-byte from the preserved live campaign
`p1-v15-scanbarrier-livehandoff-keeponly-g35f-oropenai-3g1x3-p3-20260716-234429`.
It captures the R12 candidate that received an operational `keep` disposition
while MBE was disabled and no oracle evaluation existed.

The fixture is intentionally limited to the persisted production carriers used
to reconstruct the candidate and call `select_successor_for_profile`. It must
not be edited to retrofit oracle evidence or to rewrite the original run's
commitments.

The source campaign remains preserved under `~/.ploke-eval/campaigns`; this copy
is the durable regression input.

## Immutable artifact hashes

| Artifact | SHA-256 |
| --- | --- |
| `branch-68afac57e92d5ebd.evaluation.json` | `d41b3dd2effebc4dde22cc9b9dddc59d4890b3b37f2f3b44f692f4e1d460bbb4` |
| `campaign.json` | `2f21c8af424c15603bc0219446b87c161a01f7f2c2ce044cb82aa54d32c00dfb` |
| `child-node.json` | `5cb0ae5153a198522077ef0946d6c37b4ce74f115450ec275bb86bfa4d70039c` |
| `child-plan-node-9c9dcbeeb3a4d400.json` | `c3251db8d66075579d114891432fb128715e6d2379ffbe5cf7dafda26e45e3c2` |
| `child-runner-result.json` | `0ba334192a79524eda1aa227701cc5c41c5eb2a0058ce4ee9e7f379021439368` |
| `child-to-parent.jsonl` | `64d919e0d0bbc814bab76fb4d8de671056781a1957841ae0a606e252dd4ad372` |
| `parent_identity.json` | `96f0c9cbb3e00b5d78d7d55e18b6bee1b2a2aa3ae1b7eb8f2828de3532bc5fbc` |
| `run-profile.commitment.json` | `d693b80d9cf92a1790c8d1019aee9be5e2ba2d1468ddd400be7c8f3a93403bd7` |
| `run-profile.toml` | `cfda200a2ad71803cb405c6e109bca8c80dd6b2101d4ec4a12abe9eedf1c4f95` |
