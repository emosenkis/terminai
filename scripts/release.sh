#!/usr/bin/env bash

set -euo pipefail

usage() {
	echo "usage: $0 patch 'release note' ['release note' ...]" >&2
}

[[ ${1:-} == patch && $# -ge 2 ]] || {
	usage
	exit 2
}

repo_dir=$(cd "$(dirname "$0")/.." && pwd)
cd "$repo_dir"

[[ $(git branch --show-current) == main ]] || {
	echo "release must run from main" >&2
	exit 1
}
[[ -z $(git status --short) ]] || {
	echo "working tree must be clean" >&2
	exit 1
}

current=$(bash scripts/version.sh)
[[ $current =~ ^([0-9]+)\.([0-9]+)\.([0-9]+)$ ]] || {
	echo "unsupported version: $current" >&2
	exit 1
}
version="${BASH_REMATCH[1]}.${BASH_REMATCH[2]}.$((BASH_REMATCH[3] + 1))"
tag="v$version"
git rev-parse --verify --quiet "refs/tags/$tag" >/dev/null && {
	echo "$tag already exists" >&2
	exit 1
}
shift

cargo fmt --manifest-path src/Cargo.toml -- --check
(cd src && cargo test)

tmp=$(mktemp)
trap 'rm -f "$tmp"' EXIT
awk -v version="$version" '
  !done && /^version = "/ { print "version = \"" version "\""; done=1; next }
  { print }
' src/Cargo.toml >"$tmp"
mv "$tmp" src/Cargo.toml

{
	printf '## %s - %s\n\n' "$version" "$(date +%F)"
	for note in "$@"; do printf -- '- %s\n' "$note"; done
	printf '\n'
	cat CHANGELOG.md
} >"$tmp"
mv "$tmp" CHANGELOG.md

cargo run --manifest-path src/Cargo.toml --features schema --bin dump-config-schema
cargo fmt --manifest-path src/Cargo.toml -- --check
(cd src && cargo test)

git add src/Cargo.toml Cargo.lock CHANGELOG.md docs/schema.json "docs/schema-v$version.json"
git commit -m "Release $version"
git push origin main
git tag "$tag"
git push origin "$tag"

run_id=
for _ in {1..30}; do
	run_id=$(gh run list --workflow release.yml --event push --json databaseId,headBranch \
		--jq ".[] | select(.headBranch == \"$tag\") | .databaseId" | head -1)
	[[ -n $run_id ]] && break
	sleep 2
done
[[ -n $run_id ]] || {
	echo "release workflow did not start for $tag" >&2
	exit 1
}
gh run watch "$run_id" --exit-status
gh release view "$tag" --json assets,url --jq \
	'if (.assets | length) > 0 then .url else error("release has no artifacts") end'
