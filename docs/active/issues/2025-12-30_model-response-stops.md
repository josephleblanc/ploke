# Model Response Issue

2025-12-30

When running the `ploke-tui` application, the model was able to help find code
as requested, but after the next user message the model first responded and
seemed to be calling a tool, but stopped with no following tool call or error
message.

Expected behavior: If the model calls a tool there should be a response of some kind, and if the model is returning conversation control to the user, the model shouldn't seem to be calling a tool.

This looks like an error of some kind. The logs for this chat are found in the following log file and other files with the same timestamp:

`crates/ploke-tui/logs/ploke_20251230_175922_61914.log`

If other chats result in the same issue, those logs will also be found below:

`crates/ploke-tui/logs/ploke_20251230_181816_65267.log`

`crates/ploke-tui/logs/ploke_20251231_081429_15271.log`
