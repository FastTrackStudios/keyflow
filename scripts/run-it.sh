#!/usr/bin/env bash
# `just dev`, `just demo`, `just phone` (Justfile): Studio's local stack
# with this checkout as its Keyflow (docs/local-stack.md in
# Studio). Keyflow draws on Studio's node; there is no stack here.
#
#   run-it.sh dev|demo|phone [ARGS…]
#
# Studio's checkout: LOCAL_STACK_STUDIO_PATH, else ../studio from this
# checkout or from its main checkout (a worktree's).
set -euo pipefail
root="$(cd "$(dirname "$0")/.." && pwd)"
what="${1:?usage: run-it.sh dev|demo|phone [ARGS…]}"
shift

studio="${LOCAL_STACK_STUDIO_PATH:-}"
if [ -z "$studio" ]; then
    main="$(dirname "$(git -C "$root" rev-parse --path-format=absolute --git-common-dir)")"
    for base in "$root" "$main"; do
        if [ -f "$base/../studio/scripts/local-stack.sh" ]; then
            studio="$(cd "$base/../studio" && pwd)"
            break
        fi
    done
fi
if [ -z "$studio" ] || [ ! -f "$studio/scripts/local-stack.sh" ]; then
    echo "run-it: Keyflow's charts run on Studio's local stack, and there is no Studio checkout with it beside this one." >&2
    echo "        Clone FastTrackStudios/studio to ../studio, or name one: LOCAL_STACK_STUDIO_PATH=/path/to/studio just $what" >&2
    exit 1
fi
echo "run-it: Studio's local stack ($studio), Keyflow from this checkout ($root)" >&2
cd "$studio"
case "$what" in
    phone) exec just phone "$@" ;;
    dev | demo) exec just "$what" keyflow="$root" "$@" ;;
    *) echo "run-it: dev, demo or phone" >&2; exit 64 ;;
esac
