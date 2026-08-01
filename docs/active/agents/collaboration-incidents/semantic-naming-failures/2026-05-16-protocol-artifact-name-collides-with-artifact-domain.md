# 2026-05-16 Protocol Artifact Name Collides With Artifact Domain

- Trigger:
  User reacted to the `ploke_records::protocol::Artifact` name while asking
  whether full protocol procedure data was already available through
  `ploke-tree::Graph`.
- User-visible failure:
  The name `Artifact` made it sound like protocol procedure records were the
  same kind of domain object as Prototype 1 checkout Artifacts, even though they
  are passive protocol evidence records persisted under `protocol-artifacts/`.
- Touched code surface:
  - `crates/ploke-records/src/protocol/mod.rs`
  - `crates/ploke-records/src/protocol/artifacts.rs`
  - `crates/ploke-tree/src/store/evidence.rs`
  - `crates/ploke-tree/src/graph/types/evidence.rs`
  - `crates/ploke-tree/src/graph/build/passive.rs`
- What the agent did:
  The agent accepted or introduced a generic `Artifact` carrier in the protocol
  module instead of preserving the distinction between checkout Artifact,
  protocol procedure record, and graph evidence projection.
- Skipped docs / skills / instructions:
  - structural naming discipline around overloaded domain words
  - typed UI projection guidance that distinguishes source facts from evidence
    locators and renderer rows
- Why the behavior was risky:
  Prototype 1 already uses Artifact as a core semantic object: every checkout is
  an Artifact and every Artifact is a dehydrated Runtime. Reusing `Artifact` for
  protocol evidence records makes graph and Inspector discussions ambiguous and
  invites accidental flattening of source records into UI/evidence rows.
- Concrete prevention rule:
  Do not reuse `Artifact` unqualified for non-checkout records in Prototype 1
  graph, protocol, or UI-facing code. Name passive protocol procedure files as
  protocol records/evidence, and keep any compatibility alias behind the
  protocol module boundary.
- Memory hypothesis:
  If memory helps, it should flag `Artifact` as reserved vocabulary in
  Prototype 1 unless the carrier is a checkout/dehydrated-runtime object.
