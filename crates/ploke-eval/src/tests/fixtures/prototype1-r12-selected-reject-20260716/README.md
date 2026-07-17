# Prototype 1 R12 selected-reject replay

Real persisted artifacts from campaign
`p1-v11-keeponly-g35f-oropenai-3g1x3-p3-20260716-185301`, stopped at R12 on
2026-07-16 after `walk step --until r13a` incorrectly rejected the target.

The selected child `node-cb1b41e21e01ddc7` completed, but
`branch-aef83be6f4105a58.evaluation.json` records `overall_disposition = reject`.
The admitted profile requires a kept child and disables rejected exploration,
so the continuation authority is `StopSelectedBranchRejected` and the valid
edge is R12 to R13a without checkout permission.

The fixture preserves the exact profile commitment, selection inputs, child
terminal channel, transition journal, and controller journal. The regression
re-homes only filesystem locations needed to execute the production R12 driver
edge; it does not change the recorded policy or evaluation. It also replays the
same selected artifact under the explicitly admitted rejected-exploration
policy to prove a mismatched stopped-branch permit fails before any durable
handoff mutation.
