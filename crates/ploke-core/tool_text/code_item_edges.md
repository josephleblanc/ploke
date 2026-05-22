Shows all edges for the target item. Useful for discovering nearby code items.

Use this tool when you know the file path, item name, node kind, and crate-relative module path. `module_path` must start with `crate`, for example `crate` or `crate::module::submodule`; it is not the Cargo package name and it does not include the target item name.

If the module path is unknown, use `request_code_context`, `read_file`, or `list_dir` first. On success, the returned `node_info.canon_path` includes the item name and can be used as the `canon` value for `apply_code_edit`.
