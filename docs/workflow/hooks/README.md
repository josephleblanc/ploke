# Workflow Hooks

Repo-local Codex hook scripts that consume durable workflow artifacts.

The pipeline-registry hook is implemented in `xtask`, not as a separate script:

```bash
target/debug/xtask pipeline hook-context
```

It reads Codex hook event JSON on stdin, checks
[`../pipeline-registry.jsonl`](../pipeline-registry.jsonl), and emits
`hookSpecificOutput.additionalContext` when a prompt or tool input mentions
registered pipeline code.

Example `.codex/config.toml` wiring:

```toml
[[hooks.UserPromptSubmit]]
[[hooks.UserPromptSubmit.hooks]]
type = "command"
command = "sh -lc 'root=$(git rev-parse --show-toplevel 2>/dev/null) || exit 0; bin=\"$root/target/debug/xtask\"; [ -x \"$bin\" ] || exit 0; exec \"$bin\" pipeline hook-context'"
timeout = 10
statusMessage = "Checking pipeline registry"

[[hooks.PreToolUse]]
matcher = "Bash|apply_patch|Edit|Write"
[[hooks.PreToolUse.hooks]]
type = "command"
command = "sh -lc 'root=$(git rev-parse --show-toplevel 2>/dev/null) || exit 0; bin=\"$root/target/debug/xtask\"; [ -x \"$bin\" ] || exit 0; exec \"$bin\" pipeline hook-context'"
timeout = 10
statusMessage = "Checking pipeline registry"
```

Run `cargo build -p xtask` before relying on this hook. Use `/hooks` in Codex
to inspect and trust changed hook definitions.
