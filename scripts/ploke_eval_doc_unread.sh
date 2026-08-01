#!/usr/bin/env bash
set -euo pipefail

# List mdbook pages under crates/ploke-eval/docs that have no explicit pi `read`
# tool calls in the scanned session files.
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

require comm
require find
require jq
require sort

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

all_pages="$(mktemp)"
read_pages="$(mktemp)"
trap 'rm -f "$all_pages" "$read_pages"' EXIT

find "$doc_dir" -type f -name '*.md' -printf '%P\n' | sort > "$all_pages"

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
  sort -u > "$read_pages"

comm -23 "$all_pages" "$read_pages"
