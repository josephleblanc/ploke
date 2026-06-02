# ploke-protocol

`ploke-protocol` is an internal, non-release-facing crate for protocol and
procedure experiments inside the `ploke` workspace.

It currently exists to support:
- typed procedure/protocol design experiments
- LLM-adjudicated bounded review procedures
- persisted protocol artifacts consumed by `ploke-eval`
- architectural exploration that is expected to change faster than the
  user-facing `ploke-tui` application

This crate is not part of the release-facing product surface and should be read
as experimental infrastructure rather than a stable external API.
