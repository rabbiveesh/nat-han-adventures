#!/usr/bin/env bash
# Usage: scripts/bandstand/release-notes.sh v0.1.0 [CHANGELOG]
#
# Prints the crates/bandstand/CHANGELOG.md section for a version: the exact
# body the GitHub Release uses (see .github/workflows/release-bandstand.yml).
# Run it to preview what merging a version bump will publish.
set -euo pipefail
cd "$(dirname "${BASH_SOURCE[0]}")/../.."

VERSION="${1:?Usage: $0 <v-version> [changelog]}"
CHANGELOG="${2:-crates/bandstand/CHANGELOG.md}"

# Section = the lines under `## <version>` up to the next `## ` header. Headers
# are dated (`## v0.1.0 — 2026-11-07`), so match the version as a PREFIX: an
# exact-line match misses the date suffix and silently yields nothing.
section=$(awk -v hdr="## ${VERSION}" '
  $0 == hdr || index($0, hdr " ") == 1 { found = 1; next }
  /^## / && found { exit }
  found
' "$CHANGELOG" | awk 'NF { seen = 1 } seen')

if [ -z "$section" ]; then
  echo "bandstand ${VERSION}."
  echo "::warning::No ${CHANGELOG} section for ${VERSION}; using a stub body." >&2
  exit 0
fi

printf '%s\n' "$section"
