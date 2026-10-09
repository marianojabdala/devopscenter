#!/usr/bin/env bash
# Tags origin/main's current HEAD and pushes *only* that tag — this is what
# triggers .github/workflows/release.yml (it only fires on a `v*` tag push).
# Run this after the release branch from scripts/release.sh has been merged
# into main via its PR.
#
# Usage:
#   scripts/tag-release.sh 1.2.3
set -euo pipefail

if [ "$#" -ne 1 ]; then
  echo "usage: $0 <X.Y.Z>" >&2
  exit 1
fi

version="$1"
tag="v$version"

cd "$(git rev-parse --show-toplevel)"

echo "Fetching origin/main..."
git fetch origin main

main_version=$(git show origin/main:Cargo.toml | sed -n 's/^version = "\(.*\)"/\1/p' | head -1)
if [ "$main_version" != "$version" ]; then
  echo "error: origin/main's Cargo.toml version is '$main_version', not '$version'." >&2
  echo "       Has the release/v$version PR been merged yet?" >&2
  exit 1
fi

if git rev-parse "$tag" >/dev/null 2>&1 || git rev-parse "origin/$tag" >/dev/null 2>&1; then
  echo "error: tag $tag already exists" >&2
  exit 1
fi

git tag -a "$tag" origin/main -m "$tag"
git push origin "$tag"

echo
echo "Done. $tag pushed, pointing at origin/main ($(git rev-parse --short origin/main))."
echo "This triggers .github/workflows/release.yml."
