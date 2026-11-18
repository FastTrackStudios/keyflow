#!/usr/bin/env bash
# The bump script's test: replay real Architect bumps and check that the
# script writes exactly what was merged then (`just bump-architect-test`).
#
#   1. keyflow#67 (v0.12.3 -> v0.12.4, version 0.5.3): a mechanical bump.
#      The result, Cargo.lock included, must equal the merged commit.
#   2. keyflow#61 (v0.11.2 -> v0.12.0): Architect moved the editor revision
#      this repo follows, which the script does not move. It must exit 3
#      (needs a person) and say so.
#   3. v0.10.0 -> v0.12.4: a minor jump of two must exit 3.
# Not hermetic: it reads GitHub (the tags, Architect's manifests and lock,
# whether a tag is on Architect's main), so it is a manual check, not CI.
#
# Each case checks the base out in a scratch worktree and runs THIS
# checkout's scripts/bump-architect.py there. Needs the network (Architect's
# manifests, cargo's git fetch of the tag); compiles nothing.
set -euo pipefail

root="$(git rev-parse --show-toplevel)"
fail=0

# replay BASE TAG EXPECT_STATUS WANT [ARGS...]
#   WANT: the merged commit (status 0), or a string the report names (status 3)
replay() {
    local base="$1" tag="$2" expect="$3" want="$4"
    shift 4
    git -C "$root" cat-file -e "$base^{commit}" 2>/dev/null || git -C "$root" fetch -q origin main
    local scratch status=0
    scratch="$(mktemp -d "${TMPDIR:-/tmp}/bump-replay.XXXXXX")"
    git -C "$root" worktree add -q --detach "$scratch" "$base"
    mkdir -p "$scratch/scripts"
    cp "$root/scripts/bump-architect.py" "$scratch/scripts/bump-architect.py"
    (cd "$scratch" && python3 scripts/bump-architect.py "$tag" "$@" --report "$scratch.report") >"$scratch.log" 2>&1 || status=$?
    rm -f "$scratch/scripts/bump-architect.py"
    rmdir "$scratch/scripts" 2>/dev/null || true

    if [ "$status" -ne "$expect" ]; then
        echo "FAIL $base -> $tag: exit $status, expected $expect" >&2
        cat "$scratch.log" >&2
        fail=1
    elif [ "$expect" -eq 3 ]; then
        if grep -q "$want" "$scratch.report" 2>/dev/null; then
            echo "ok   $base -> $tag: needs a person ($want moved)"
        else
            echo "FAIL $base -> $tag: the report does not name $want" >&2
            cat "$scratch.report" "$scratch.log" >&2 2>/dev/null || true
            fail=1
        fi
    elif git -C "$scratch" diff --quiet "$want" --; then
        echo "ok   $base -> $tag $*: identical to $want"
    else
        echo "FAIL $base -> $tag: the result differs from $want:" >&2
        git -C "$scratch" diff --stat "$want" -- >&2
        git -C "$scratch" diff "$want" -- | head -80 >&2
        fail=1
    fi
    git -C "$root" worktree remove --force "$scratch" >/dev/null 2>&1 || true
    rm -rf "$scratch" "$scratch.log" "$scratch.report"
}

# keyflow#66 merged (v0.12.3, 0.5.2) -> keyflow#67 merged (v0.12.4, 0.5.3)
replay d8797252670917f13ef69ab0863df20c460512c4 v0.12.4 0 b71833eddacf8763a3d5d1f95ad999235f6ebbec --version 0.5.3
# the same bump with --version auto: 0.5.2 -> 0.5.3, as #67 set by hand
replay d8797252670917f13ef69ab0863df20c460512c4 v0.12.4 0 b71833eddacf8763a3d5d1f95ad999235f6ebbec --version auto
# keyflow#60 merged (v0.11.2) -> v0.12.0, which moved the editor to 6817dc2
replay 44865fcd3bfdc56ebfdfe871a001e4e478d2d9ce v0.12.0 3 fasttrackstudios/editor --no-lock
# a keyflow on architect v0.10.0 asked for v0.12.4: a minor jump of 2 is
# not mechanical (a mistaken high tag must not merge itself)
replay de637060846a089a9fdae10ad95e164a7d86c185 v0.12.4 3 "minor jump of 2" --no-lock

exit "$fail"
