Lists private code items that have no incoming persisted local call edges.

Use this for dead-code triage questions like "which private helpers are not called by the loaded source graph?" The result is based on the current call graph projection and stays fail-closed: unsupported, targetless, external, unresolved, or ambiguous calls do not become inferred callers.

This tool is workspace-scoped and takes only an optional `max_results` cap. The `nodes` list remains the raw zero-incoming source-call graph result. The optional `entrypoint_summaries` field attaches admitted generated-entrypoint proof rows, such as generated test harnesses, for returned nodes without treating those summaries as persisted local call edges.

Use `code_item_lookup`, `code_item_edges`, or `code_item_call_path` after this tool when you need exact snippets, outgoing calls, additional proof rows, or bounded paths for a returned item.
