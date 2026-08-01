# child-plan-03 generation2 plan drift terminal evidence

timestamp: 2026-06-30T14:53:12.954099-07:00

query: `queries/child-plan-03__generation2-plan-drift.cozo`
raw_json: `responses/child-plan-03__generation2-plan-drift__terminal.json`

## File evidence

- child-plan file: `/home/brasides/.ploke-eval/campaigns/p1-gated-parent-3g1x3-p3-20260630-174316/prototype1/messages/child-plan/node-2fe75acd9e9cf6c3.json`
- current file sha256: `361a41e203097d709cafb1cc7bb0088e2f6448a1278b3811f7ebd8e3dec0941f`

## DB rows

- plan `2a630c6065afe51ff110cc615f93188473c43c00697b58d427aa89a1b4d81ec3` parent `node-ec383aa38762a3d4` generation `1` child `node-2fe75acd9e9cf6c3` index `1` status `planned`
  - db message_path: `/home/brasides/.ploke-eval/campaigns/p1-gated-parent-3g1x3-p3-20260630-174316/prototype1/messages/child-plan/node-ec383aa38762a3d4.json`
  - db message_sha256: `03a343ecea11e8bb5c5cad65973c0e69dfc323bdbb08f358bd98f7b79b1ea425`
  - branch/candidate: `branch-88aaa7a4b0328baf` / `broad-harness-g1-02`
- plan `2a630c6065afe51ff110cc615f93188473c43c00697b58d427aa89a1b4d81ec3` parent `node-ec383aa38762a3d4` generation `1` child `node-fe4a6decb46ca481` index `0` status `planned`
  - db message_path: `/home/brasides/.ploke-eval/campaigns/p1-gated-parent-3g1x3-p3-20260630-174316/prototype1/messages/child-plan/node-ec383aa38762a3d4.json`
  - db message_sha256: `03a343ecea11e8bb5c5cad65973c0e69dfc323bdbb08f358bd98f7b79b1ea425`
  - branch/candidate: `branch-f41b072e4787d706` / `broad-harness-g1-01`
- plan `9903062bde18d24e7a958d4c426efba5c6f927336ce54a99a44e2bcd3ee3ba04` parent `node-2fe75acd9e9cf6c3` generation `2` child `node-e9be81e08bda07a1` index `0` status `planned`
  - db message_path: `/home/brasides/.ploke-eval/campaigns/p1-gated-parent-3g1x3-p3-20260630-174316/prototype1/messages/child-plan/node-2fe75acd9e9cf6c3.json`
  - db message_sha256: `be1e284b2b8b8fe878a3fc1836a74a5f1baa33c97d78014d420b4086d20d1499`
  - branch/candidate: `branch-864d1a76684b74c5` / `broad-harness-g2-01`

## Interpretation

- DB has a generation-2 child-plan row for successor parent `node-2fe75acd9e9cf6c3` with `message_sha256=be1e284b2b8b8fe878a3fc1836a74a5f1baa33c97d78014d420b4086d20d1499`.
- The current file at the same `message_path` hashes to `361a41e203097d709cafb1cc7bb0088e2f6448a1278b3811f7ebd8e3dec0941f`.
- Because these hashes differ, the eval-store correctly rejects a second put for the same `plan_id` with different content.
- This is a terminal run/reconstruction failure under strict normalized persistence unless an explicit recovery/migration is chosen.

## Helpfulness / schema notes

- Helpful: normalized `eval_child_plan` exposed the exact key (`plan_id`) and stored content hash that drifted from the file.
- Missing ergonomic aid: a read-only `walk db_query` helper or relation metadata view that computes current file hash for file-backed rows would make DB/file parity checks faster.
- Correctness note: do not weaken `message_sha256` validation; this failure is valuable because it caught file overwrite/drift after DB persistence.
