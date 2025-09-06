Agents + Git Best Practices — 2025-08-26 20:47:58Z

Threat Model & Permissions
- The agent should never push to remote without explicit user consent. Default: local branch only.
- Use the user’s local repository (no GitHub tokens by default). If remote interaction is desired, prompt for explicit user credentials/token, and scope to minimal rights.
- Prefer signed commits only if configured by user; otherwise leave Git identity untouched.

Recommended Integration (Local)
- Prefer Rust-native git crates:
  - git2 (libgit2 bindings): mature, widely used; requires libgit2.
  - gix (gitoxide): pure Rust; no native deps; rapidly improving.
- Minimal wrapper: init/ensure repo, create/check out branch, stage selected files, commit with templated message (include request_id/call_id), list diff, checkout previous, revert commit.
- UX:
  - Show pending changes and a diff preview before commit.
  - Offer “apply on branch” as default; “revert” returns to previous HEAD or creates a revert commit.
  - Detect uncommitted workspace changes; require user confirmation.

Security Considerations
- Never execute arbitrary Git hooks; avoid invoking shell git commands where possible; use library APIs.
- Validate all file paths via ploke-io roots/symlink policy; refuse writes outside workspace.
- Avoid embedding secrets in commit messages; store sensitive metadata in local DB instead.

Alternatives & Industry Survey
- Local-only VCS flow: branch/commit/revert without remote. Easiest and safest.
- Remote integration: push to user fork/branch; requires tokens; higher risk; out-of-scope for baseline.
- What others do:
  - Claude Code / Copilot / Cursor: typically integrate with editor/VCS UI and rely on local VCS state; do not auto-push without user action.
  - Aider: writes patches and uses git locally; user supplies tokens if pushing/PRs; local diffs dominate the flow.
  - Gemini Julie/Void: mixed approaches but commonly keep human-in-the-loop for commits.

Recommendations
- Start with local-only branch workflow using gix (pure Rust) for portability; fall back to git2 if needed.
- Ask before commit; default commit messages structured (feature/agent: request_id short hash + summary).
- Implement revert safely: prevent data loss by using backup and branch switching, not destructive resets.

