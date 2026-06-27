Find the definition of a known code item. Better than grep.

Returns the code snippet of the item if it exists, and provides positive proof if the item does not exist.

Use this tool when you know the file path, item name, node kind, and crate-relative module path. `module_path` must start with `crate`, for example `crate` or `crate::module::submodule`; it is not the Cargo package name and it does not include the target item name.

If the module path is unknown, use `request_code_context`, `read_file`, or `list_dir` first. On success, the returned `canon_path` includes the item name and can be used as the `canon` value for `apply_code_edit`.

When call graph data is available, the result also includes node-scoped `call_context` and `proof_context` rows for callers, callees, callsites, and projected proof facts related to the resolved item.

Pro tip: use it with parallel tool calls to look up as many code items as you want.
