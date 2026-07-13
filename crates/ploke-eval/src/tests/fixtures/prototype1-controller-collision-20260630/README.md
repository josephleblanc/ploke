# June 30 Prototype 1 controller collision fixture

These three records are copied from the generation-1 coordinate where an
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

The persisted originals had no trailing newline. Repository text files add one
LF; the replay staging helper removes exactly that LF and asserts the original
digest before passing the files to production identity, profile, and invocation
loaders.

The overwritten child-plan file and surviving normalized DB evidence are not
projected into a synthetic first-plan fixture. The incident evidence remains in
`docs/active/agents/2026-06-30_p1-db-filesystem-parity-live-run-template/`,
especially `responses/child-plan-03__generation2-plan-drift__terminal.json` and
`responses/terminal-cleanup__gen1-controller-collision.md`.
