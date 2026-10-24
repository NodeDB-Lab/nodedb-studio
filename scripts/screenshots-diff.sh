#!/usr/bin/env bash
# Compare two screenshot sets and print a Markdown report of the differences.
#
# Usage: scripts/screenshots-diff.sh <base_dir> <new_dir>
#
# Both directories hold light/*.png and dark/*.png, as written by
# scripts/screenshots.sh. A file counts as changed when its bytes differ: the
# capture is deterministic, so identical code writes identical files.
set -euo pipefail

if [[ $# -ne 2 ]]; then
  echo "usage: screenshots-diff.sh <base_dir> <new_dir>" >&2
  exit 2
fi
base="$1"
new="$2"

list() {
  (cd "$1" 2>/dev/null && find . -name '*.png' | sed 's|^\./||' | sort) || true
}

added=()
changed=()
removed=()
while IFS= read -r f; do
  if [[ ! -f "$base/$f" ]]; then
    added+=("$f")
  elif ! cmp -s "$base/$f" "$new/$f"; then
    changed+=("$f")
  fi
done < <(list "$new")
while IFS= read -r f; do
  [[ -f "$new/$f" ]] || removed+=("$f")
done < <(list "$base")

echo "## Screenshots"
echo
if [[ ${#added[@]} -eq 0 && ${#changed[@]} -eq 0 && ${#removed[@]} -eq 0 ]]; then
  echo "No screen changed."
  exit 0
fi
section() {
  local title="$1"
  shift
  [[ $# -eq 0 ]] && return
  echo "**$title ($#)**"
  echo
  printf -- '- `%s`\n' "$@"
  echo
}
section "Changed" "${changed[@]}"
section "Added" "${added[@]}"
section "Removed" "${removed[@]}"
echo "Download the \`screenshots\` artifact of this run to view them."
