Find the definition of a known code item. Better than grep.

Returns the code snippet of the item if it exists, and provides positive proof if the item does not exist.

Use this tool when you know the file path, item name, node kind, and crate-relative module path. `module_path` must start with `crate`, for example `crate` or `crate::module::submodule`; it is not the Cargo package name and it does not include the target item name.

If the module path is unknown, use `request_code_context`, `read_file`, or `list_dir` first. On success, the returned `canon_path` includes the item name and can be used as the `canon` value for `apply_code_edit`.

When call graph data is available, the result also includes node-scoped `call_context` and `proof_context` rows for callers, callees, callsites, and projected proof facts related to the resolved item. Exact lookups also include bounded `call_paths_from_owner`, `call_paths_to_target`, `call_impact`, and `call_reach` summaries.

Use `call_impact` to answer target-centered questions such as who calls this item, which callers eventually reach it, whether those callers are in test or non-test source, and what direct callsites resolve to it. The impact summary includes direct and eventual callers, `direct_call_sites`, `test_callers`, `non_test_callers`, public direct callers under the stored visibility predicate, and `source_files`.

Use `call_reach` to answer owner-centered questions such as what this item calls directly, which callees are reachable through resolved local call paths, what exact resolved `direct_call_sites` this owner makes, which nonresolved `frontier_calls` are visible but fail closed, which `external_frontier_calls` represent dependency calls, and which `unsupported_frontier_calls` represent visible resolver blockers. Its `source_files` list identifies files to retrieve for explaining the call chain.

Pro tip: use it with parallel tool calls to look up as many code items as you want.
