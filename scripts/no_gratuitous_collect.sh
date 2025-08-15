#!/usr/bin/env bash
set -euo pipefail

# Gate to catch the specific anti-pattern:
#   let tmp: Vec<_> = iter.map(...).collect();
#   out.extend(tmp);
#
# or collecting just to call into_iter() right away.
#
# False positives can be suppressed with a comment on the collect line:
#   // ALLOW_GRATUITOUS_COLLECT
#
# Notes:
# - Scans only .rs files, excluding target/
# - Looks ahead up to 12 lines for the extend/into_iter use of the collected var

tmp_report="$(mktemp)"
trap 'rm -f "$tmp_report"' EXIT

while IFS= read -r -d '' file; do
  awk -v file="$file" -v maxlook=12 '
    {
      lines[NR] = $0
    }
    END {
      for (i = 1; i <= NR; i++) {
        line = lines[i]
        if (line ~ /ALLOW_GRATUITOUS_COLLECT/) continue

        # Capture: let <var> [: Vec<...>]? = ... collect::<Vec...>? ( ... )
        if (match(line, /let[ \t]+([A-Za-z_][A-Za-z0-9_]*)[ \t]*(:[ \t]*Vec<[^>]*>)?[ \t]*=[^;]*collect(::<[Vv]ec[^>]*)?\(/, m)) {
          var = m[1]
          # Look ahead a few lines for patterns using var in extend(...) or var.into_iter()
          for (j = i + 1; j <= i + maxlook && j <= NR; j++) {
            look = lines[j]
            if (look ~ ("extend\\([^)]*\\b" var "\\b") || look ~ ("\\b" var "\\b[ \t]*\\.into_iter\\(\\)")) {
              printf "%s:%d: gratuitous collect followed by extend/into_iter on `%s`\n", file, j, var
              print "__FOUND__SENTINEL__"
              break
            }
          }
        }
      }
    }
  ' "$file"
done < <(find . -type f -name '*.rs' -not -path './target/*' -print0) | tee "$tmp_report" >/dev/null

if grep -q "__FOUND__SENTINEL__" "$tmp_report"; then
  # Strip sentinel lines from output
  sed '/__FOUND__SENTINEL__/d' "$tmp_report"
  echo
  echo "Error: Detected gratuitous collect patterns. Refactor to extend directly from iterator."
  echo "Hint: To bypass for a legitimate case, add comment '// ALLOW_GRATUITOUS_COLLECT' on the collect line."
  exit 1
fi

echo "No gratuitous collect patterns found."
