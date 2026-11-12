#!/usr/bin/env bash
# Tag a keyflow release: the one step a person takes, and only when Studio
# needs a new keyflow (an Architect MINOR, or keyflow's own changes; an
# Architect patch release needs none, see Cargo.toml's architect comment).
#
#   just release-tag              tag origin/main as v<its workspace version>
#   just release-tag --dry-run    say what it would tag, push nothing
#
# It refuses unless: the commit is origin/main's tip; its [workspace.package]
# version is above every v* tag (the bump PR set it: `just bump-architect
# vX.Y.Z --version auto`); `CI passed` is green on it. Then it pushes an
# annotated tag `v<version>` as you (a person, or fts-forge from an agent:
# the v* tag ruleset, scripts/tag-ruleset.sh, lets only those create one).
set -euo pipefail

dry=0
[ "${1:-}" = --dry-run ] && dry=1
repo=FastTrackStudios/keyflow
git fetch -q origin main --tags
sha="$(git rev-parse origin/main)"
version="$(git show "$sha:Cargo.toml" | sed -n '/^\[workspace\.package\]/,/^\[/{s/^version = "\([^"]*\)".*/\1/p}' | head -1)"
[[ "$version" =~ ^[0-9]+\.[0-9]+\.[0-9]+$ ]] || { echo "release-tag: no workspace version at $sha" >&2; exit 1; }
tag="v$version"
if git rev-parse -q --verify "refs/tags/$tag" >/dev/null; then
    echo "release-tag: $tag exists already ($(git rev-parse --short "$tag^{commit}")): bump the version first" >&2
    exit 1
fi
highest="$(git tag --list 'v[0-9]*' | grep -E '^v[0-9]+\.[0-9]+\.[0-9]+$' | sort -V | tail -1)"
if [ -n "$highest" ] && [ "$(printf '%s\n%s\n' "${highest#v}" "$version" | sort -V | tail -1)" != "$version" ]; then
    echo "release-tag: $tag is not above the highest tag $highest" >&2
    exit 1
fi
ci="$(gh api "repos/$repo/commits/$sha/check-runs?check_name=CI%20passed" --jq '[.check_runs[] | .conclusion] | first // "none"')"
[ "$ci" = success ] || { echo "release-tag: CI passed is '$ci' on $(git rev-parse --short "$sha"), not success" >&2; exit 1; }
architect="$(git show "$sha:Cargo.toml" | sed -n 's/^architect = .*tag = "\(v[^"]*\)".*/\1/p')"
echo "release-tag: $tag on $(git rev-parse --short "$sha") (architect $architect), CI passed green"
[ "$dry" = 1 ] && exit 0
git tag -a "$tag" "$sha" -m "keyflow $tag (architect $architect)"
git -c credential.helper= -c 'credential.helper=!gh auth git-credential' push -q origin "refs/tags/$tag"
echo "pushed $tag"
