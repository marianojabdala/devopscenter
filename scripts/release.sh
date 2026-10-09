#!/usr/bin/env bash
# Prepares a release: creates a `release/vX.Y.Z` branch off the latest
# `origin/main`, bumps the crate version there, and pushes that branch —
# never `main` directly, so the bump goes through the same PR review as any
# other change (branch protection stays intact).
#
# Does NOT tag or push a tag. After the release branch's PR is merged, run
# scripts/tag-release.sh to tag main's new HEAD and trigger the release
# workflow (.github/workflows/release.yml, which only fires on a tag push).
#
# Usage:
#   scripts/release.sh 1.2.3   # explicit version
#   scripts/release.sh patch   # X.Y.Z -> X.Y.(Z+1)
#   scripts/release.sh minor   # X.Y.Z -> X.(Y+1).0
#   scripts/release.sh major   # X.Y.Z -> (X+1).0.0
set -euo pipefail

if [ "$#" -ne 1 ]; then
  echo "usage: $0 <patch|minor|major|X.Y.Z>" >&2
  exit 1
fi

cd "$(git rev-parse --show-toplevel)"

if [ -n "$(git status --porcelain)" ]; then
  echo "error: working tree is not clean; commit or stash first" >&2
  exit 1
fi

original_branch=$(git rev-parse --abbrev-ref HEAD)

echo "Fetching origin/main..."
git fetch origin main

current=$(git show origin/main:Cargo.toml | sed -n 's/^version = "\(.*\)"/\1/p' | head -1)
if [ -z "$current" ]; then
  echo "error: could not find a 'version = \"...\"' line in origin/main's Cargo.toml" >&2
  exit 1
fi

IFS='.' read -r major minor patch <<<"$current"

case "$1" in
patch) new="$major.$minor.$((patch + 1))" ;;
minor) new="$major.$((minor + 1)).0" ;;
major) new="$((major + 1)).0.0" ;;
[0-9]*.[0-9]*.[0-9]*) new="$1" ;;
*)
  echo "error: argument must be patch, minor, major, or an explicit X.Y.Z" >&2
  exit 1
  ;;
esac

if [ "$new" = "$current" ]; then
  echo "error: new version ($new) is the same as the current version ($current)" >&2
  exit 1
fi

if git rev-parse "v$new" >/dev/null 2>&1 || git rev-parse "origin/v$new" >/dev/null 2>&1; then
  echo "error: tag v$new already exists" >&2
  exit 1
fi

release_branch="release/v$new"
if git rev-parse "$release_branch" >/dev/null 2>&1 || git rev-parse "origin/$release_branch" >/dev/null 2>&1; then
  echo "error: branch $release_branch already exists" >&2
  exit 1
fi

echo "Bumping devopscenter: $current -> $new (on $release_branch, off origin/main)"

git checkout -b "$release_branch" origin/main

cleanup() {
  git checkout --quiet "$original_branch"
}
trap cleanup EXIT

# Only the first `version = "..."` line — the [package] version, which comes
# before any dependency table in this file. Don't reuse this against a
# Cargo.toml where that assumption doesn't hold.
sed -i.bak "0,/^version = \"$current\"/s//version = \"$new\"/" Cargo.toml
rm -f Cargo.toml.bak

# Refreshes Cargo.lock's own recorded version for this package (does not
# touch dependency versions).
cargo check --quiet

git add Cargo.toml Cargo.lock
git commit -m "Bump version to $new"
git push -u origin "$release_branch"

cat <<EOF

Done. $release_branch pushed (main untouched). Next steps:
  1. Open a PR: $release_branch -> main, and get it merged.
  2. Once merged, run: scripts/tag-release.sh $new
     (this tags main's new HEAD and pushes only the tag, which triggers
     .github/workflows/release.yml)
EOF
