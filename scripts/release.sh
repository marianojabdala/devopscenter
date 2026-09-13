#!/usr/bin/env bash
# Bumps the crate version, updates Cargo.lock, commits, and creates the
# matching git tag. Does NOT push — review the commit/tag, then push both
# yourself (pushing the tag is what triggers .github/workflows/release.yml).
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

current=$(sed -n 's/^version = "\(.*\)"/\1/p' Cargo.toml | head -1)
if [ -z "$current" ]; then
  echo "error: could not find a 'version = \"...\"' line in Cargo.toml" >&2
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

if git rev-parse "v$new" >/dev/null 2>&1; then
  echo "error: tag v$new already exists" >&2
  exit 1
fi

echo "Bumping devopscenter: $current -> $new"

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
git tag -a "v$new" -m "v$new"

cat <<EOF

Done. Review before pushing:
  git show HEAD
  git show v$new

Push both to publish (pushing the tag triggers the Release workflow):
  git push && git push origin v$new
EOF
