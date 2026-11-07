#!/usr/bin/env bash
# Usage: scripts/bandstand/check-version.sh [--release]
#
# Fails unless everything that carries bandstand's version agrees with
# crates/bandstand/Cargo.toml. CI runs it on every PR, so a bump that misses a
# file fails before it merges. `--release` also requires the `## v<version>`
# section in crates/bandstand/CHANGELOG.md (the release workflow runs it before
# creating the tag).
set -euo pipefail
cd "$(dirname "${BASH_SOURCE[0]}")/../.."

want=$(sed -n 's/^version = "\(.*\)"/\1/p' crates/bandstand/Cargo.toml | head -1)
[ -n "$want" ] || { echo "::error::no version in crates/bandstand/Cargo.toml"; exit 1; }

lock=$(awk '/^name = "bandstand"$/ { getline; gsub(/version = |"/, ""); print; exit }' Cargo.lock)
bad=0
if [ "$lock" != "$want" ]; then
  echo "::error file=Cargo.lock::bandstand is '${lock:-<not found>}', crates/bandstand/Cargo.toml is '$want'"
  bad=1
fi

if [ "${1:-}" = --release ] && ! grep -q "^## v$want\( \|$\)" crates/bandstand/CHANGELOG.md; then
  echo "::error file=crates/bandstand/CHANGELOG.md::no '## v$want' section"
  bad=1
fi

[ "$bad" = 0 ] && echo "bandstand's version strings are all $want."
exit "$bad"
