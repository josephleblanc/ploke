#!/usr/bin/env bash
set -euo pipefail

# Count explicit pi `read` tool calls for mdbook pages under crates/ploke-eval/docs.
#
# Defaults scan the pi sessions for the current git worktree. Override with:
#   PROJECT_ROOT=/path/to/ploke
#   DOC_DIR=/path/to/ploke/crates/ploke-eval/docs
#   SESSION_ROOT=$HOME/.pi/agent/sessions
#   SESSION_DIR=$HOME/.pi/agent/sessions/--home-brasides-code-ploke--

require() {
  if ! command -v "$1" >/dev/null 2>&1; then
    echo "error: required command not found: $1" >&2
    exit 2
  fi
}

project_root() {
  if [ -n "${PROJECT_ROOT:-}" ]; then
    printf '%s\n' "$PROJECT_ROOT"
  else
    git rev-parse --show-toplevel 2>/dev/null || pwd
  fi
}

encode_pi_session_dir() {
  local root="$1"
  root="${root%/}"
  root="${root#/}"
  root="${root//\//-}"
  printf -- '--%s--\n' "$root"
}

require jq
require find
require sort
require uniq

root="$(project_root)"
doc_dir="${DOC_DIR:-$root/crates/ploke-eval/docs}"
if [ ! -d "$doc_dir" ]; then
  echo "error: docs directory not found: $doc_dir" >&2
  exit 1
fi

doc_abs="$(cd "$doc_dir" && pwd -P)/"
session_root="${SESSION_ROOT:-${PI_CODING_AGENT_SESSION_DIR:-$HOME/.pi/agent/sessions}}"
session_dir="${SESSION_DIR:-$session_root/$(encode_pi_session_dir "$root")}"
if [ ! -d "$session_dir" ]; then
  echo "error: session directory not found: $session_dir" >&2
  exit 1
fi

find "$session_dir" -type f -name '*.jsonl' -print0 |
  xargs -0 -r jq -r --arg abs "$doc_abs" '
    select(.type == "message" and .message.role == "assistant")
    | .message.content[]?
    | select(.type == "toolCall" and .name == "read")
    | .arguments.path // empty
    | sub("^\\./"; "")
    | if startswith($abs) then ltrimstr($abs)
      elif startswith("crates/ploke-eval/docs/") then ltrimstr("crates/ploke-eval/docs/")
      else empty end
    | select(test("\\.md$"))
  ' |
  sort |
  uniq -c |
  sort -nr
