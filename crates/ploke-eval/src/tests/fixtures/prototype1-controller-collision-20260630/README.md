# June 30 Prototype 1 controller collision fixture

These six records are copied from the generation-1 coordinate where an
autonomous successor and a restarted walk server both drove R7 to R8 for
campaign `p1-gated-parent-3g1x3-p3-20260630-174316`.

Original paths and SHA-256 digests:

- worktree `.ploke/prototype1/parent_identity.json`:
  `f44e95b0ab26ab1e33f2874c486c4586804dd7f08df56d8319e0f41701391376`
- campaign `prototype1/run-profile.commitment.json`:
  `441c051ea9e0eb3f3fe2914838f4a7571ba6fc3a1dcbd63a32ade50ac2afa7e9`
- successor invocation
  `prototype1/nodes/node-2fe75acd9e9cf6c3/invocations/a96b5766-b5e0-43a8-99d8-b81936cb5449.json`:
  `b6e3d9b9dc1b76d94637a37873b0232194f56ca098561a17968d72e5aeeafb46`
- campaign `prototype1/transition-journal.jsonl`:
  `b1fc907a7b56eaa240a6252a5a82458c217dad9234da4b77aa09b4f833d1b373`
- overwritten campaign child plan
  `prototype1/messages/child-plan/node-2fe75acd9e9cf6c3.json`:
  `361a41e203097d709cafb1cc7bb0088e2f6448a1278b3811f7ebd8e3dec0941f`
- saved `walk_db_query` response
  `responses/child-plan-03__generation2-plan-drift__terminal.json`:
  `68ac3ef851ec2a0ea1e193f1f081cb6644786ef242fd11f804793e4ce7824629`

The identity, profile, invocation, and overwritten child-plan originals had no
trailing newline. Repository text files add one LF; the replay staging helper
removes exactly that LF and asserts the original digest. The journal and saved
DB-query response retain their original LF and exact repository digest.

The regression loads the identity, profile, invocation, and transition journal
through production readers, and deserializes the child plan through the
production `ChildPlanFiles` carrier. The saved query response is structured
evidence that the normalized DB row names child `node-e9be81e08bda07a1` with
message digest `be1e284b2b8b8fe878a3fc1836a74a5f1baa33c97d78014d420b4086d20d1499`,
while the surviving overwritten file contains a different three-child plan.

This is a bounded artifact-driven replay, not an exact provider replay. The
first child-plan bytes were overwritten before capture, no durable model
session journal survived, and the failed walk transition was not persisted as
an event. Exact provider and harness replay therefore remains deferred rather
than being replaced with synthetic success evidence. The longer incident record
remains in
`docs/active/agents/2026-06-30_p1-db-filesystem-parity-live-run-template/`.
