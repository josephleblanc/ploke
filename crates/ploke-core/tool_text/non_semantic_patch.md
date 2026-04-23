Apply a non-semantic code edit. This tool is most useful in two cases:

1. You need to read/edit non-Rust files.
   - While this application as a whole is focused on Rust code, the user may ask you to read or edit non-Rust files.
   - This `non_semantic_patch` tool can be used to patch non-Rust files.

2. The parser that allows for semantic edits fails on the target directory.
   - Usually because there is an error in the target crate, for example a missing closing bracket.
   - In this case, this `non_semantic_patch` tool can be used to apply a code edit.
   - Do not use this tool on Rust files (`*.rs`) before trying the semantic code edit tool first.
