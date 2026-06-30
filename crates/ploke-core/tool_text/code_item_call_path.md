Find bounded resolved call paths between two exact Rust code items.

Use this when you know both the source item and target item and need to answer
whether the source can reach the target through the persisted call graph. Provide
exact coordinates for both items: `item_name`, Rust `file_path`, `node_kind`,
and crate-relative `module_path` beginning with `crate`.

For methods, use `owner_type` for inherent impl methods or `owner_trait` for
trait methods when the file/module/name tuple is ambiguous. The tool returns
`reachable`, the ordered call paths, and source metadata for path nodes. It only
traverses resolved local call edges; targetless unsupported, external,
unresolved, or ambiguous calls remain fail-closed and do not create paths.
