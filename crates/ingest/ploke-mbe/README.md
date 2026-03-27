# `ploke-mbe`

`ploke-mbe` is a ploke-local declarative macro crate intended to support
restricted `macro_rules!` parsing and later structural expansion for
`syn_parser`.

The initial implementation is intentionally narrow:

- parse `macro_rules!` definitions into a ploke-local IR
- preserve enough structure for future matching/transcription
- expose helpers for identifying top-level structural items

It does not yet perform macro expansion.

