#!/usr/bin/env bash
# Every package source in a Cargo.lock (on stdin) that is NOT crates.io,
# one per line, normalised: git sources of any scheme (https, http, ssh) and
# host spelling (www.), without their ?tag= / #commit, and any other
# registry. The bump bot refuses to merge a bump that adds one (unreviewed
# code): `comm -13 <(main's) <(the PR's)` must be empty.
# Tested by scripts/lock-sources-test.sh.
set -euo pipefail
sed -n 's|^source = "\([^"]*\)"|\1|p' \
    | grep -v -e '^registry+https://github.com/rust-lang/crates.io-index$' -e '^sparse+https://index.crates.io/$' \
    | sed -e 's|[?#].*||' -e 's|://www\.|://|' -e 's|\.git$||' \
    | tr '[:upper:]' '[:lower:]' | sort -u
