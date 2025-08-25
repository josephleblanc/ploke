# Implementation log 015 — Preview config, optional unified diff, auto-approval gate (2025-08-20)

Summary
- Added EditingConfig to AppState::Config with:
  - preview_mode: codeblock | diff (default: codeblock)
  - auto_confirm_edits: bool (default: false)
  - max_preview_lines: usize (default: 300) for truncation
- Implemented preview generation in apply_code_edit staging:
  - Code-block previews include per-file Before/After sections, truncated per config.
  - Optional unified diff using the "similar" crate; concatenated across files.
- Auto-approval: when enabled, proposals are immediately applied after staging.

User-visible changes
- Staged edit summary now includes an inline preview (truncated) and indicates mode.
- If auto-approval is enabled, an extra notice appears and edits are applied automatically.

Internal changes
- New enum PreviewMode and struct EditingConfig added to Config with sane defaults.
- Handlers now build and store DiffPreview in proposals:
  - CodeBlocks { per_file } or UnifiedDiff { text }
- Brought in "similar" dependency for unified diff generation.

Next steps
- Wire EditingConfig to user-config file parsing (defaults currently in-code).
- Add tests for preview generation/truncation and auto-approval path.
- Optional: expose a command to toggle preview mode at runtime.

Risks/notes
- Extremely large previews are truncated but could still be verbose; consider paging in TUI if needed.
- Unified diff generation depends on "similar" crate; monitor for perf on very large files.
