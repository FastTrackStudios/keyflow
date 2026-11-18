#!/usr/bin/env bash
# lock-sources.sh: every non-crates.io source, whatever its spelling.
set -euo pipefail
here="$(cd "$(dirname "$0")" && pwd)"
got="$(bash "$here/lock-sources.sh" <<'LOCK'
[[package]]
name = "a"
source = "registry+https://github.com/rust-lang/crates.io-index"
[[package]]
name = "b"
source = "sparse+https://index.crates.io/"
[[package]]
name = "c"
source = "git+https://www.github.com/FastTrackStudios/Architect?tag=v0.12.6#d418e65d"
[[package]]
name = "d"
source = "git+ssh://git@github.com/evil/payload.git?rev=1#1"
[[package]]
name = "e"
source = "git+http://example.com/x?branch=main#2"
[[package]]
name = "f"
source = "registry+https://example.com/other-index"
[[package]]
name = "g"
source = "sparse+https://other.example/index/"
LOCK
)"
want="git+http://example.com/x
git+https://github.com/fasttrackstudios/architect
git+ssh://git@github.com/evil/payload
registry+https://example.com/other-index
sparse+https://other.example/index/"
if [ "$got" = "$want" ]; then echo "ok   lock-sources: git (https, http, ssh), other registries; crates.io left out"
else echo "FAIL lock-sources:"; diff <(echo "$want") <(echo "$got"); exit 1; fi
