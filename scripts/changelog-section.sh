#!/usr/bin/env bash
# Print the body of the CHANGELOG.md section for version $1 (without its
# "## [x.y.z]" heading), up to the next "## " heading. Fails if missing.
set -euo pipefail
version="$1"
file="${2:-CHANGELOG.md}"
awk -v v="$version" '
  /^## / { if (found) exit; if (index($0, "## [" v "]") == 1) { found = 1; next } }
  found { print }
  END { if (!found) exit 1 }
' "$file" | sed -e '/./,$!d'
