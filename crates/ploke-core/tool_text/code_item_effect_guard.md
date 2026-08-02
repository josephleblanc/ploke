Classify whether a reachable proof-annotated effect is guarded by a known code item.

Use this when you know the exact owner item, the intended guard item, and the
effect class, and need to answer whether every resolved local path from the
owner to a callsite with that effect passes through the guard before reaching
the effect owner.

Provide exact coordinates for both `owner` and `guard`: `item_name`, Rust
`file_path`, `node_kind`, and crate-relative `module_path` beginning with
`crate`. For methods, use `owner_type` for inherent impl methods or
`owner_trait` for trait methods when the file/module/name tuple is ambiguous.

The tool returns `guarded`, the matching reachable effect rows, violating
effect rows, source files to retrieve, and proof-context rows for the involved
items and paths. It only uses resolved local paths to the owner that contains
the effect callsite. Targetless external, unsupported, unresolved, or ambiguous
effect callsites remain targetless; the tool does not fabricate local call
edges to those callees.
