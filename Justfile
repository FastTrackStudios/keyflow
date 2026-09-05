# keyflow — root workspace recipes
# Run commands: just <recipe-name>

# List recipes by default
default:
    @just --list

# ── Build / test ─────────────────────────────────────────────────────────

check:
    cargo check --workspace

# Run tests. nextest: parallel per-test binaries, much faster than
# `cargo test` on this many crates. It does NOT run doctests — use
# `just test-doc` for those.
test:
    cargo nextest run --workspace

# Doctests only — nextest can't run them (libtest owns doctests).
test-doc:
    cargo test --workspace --doc

# The gate CI runs, in the order CI runs it.
ci: fmt-check tailwind-check check web-check test

fmt:
    cargo fmt --all

fmt-check:
    cargo fmt --all --check

# ── Tailwind ─────────────────────────────────────────────────────────────
# The site's sheet is compiled from apps/web/tailwind.css, which @sources
# this crate plus two crates that arrive as GIT DEPS — architect-ui and
# view-knowledge-graph. A git dep has no stable path on disk, so the globs
# cannot be written literally.
#
# `cargo metadata` knows where cargo actually resolved them. This recipe
# asks, and symlinks the answers into apps/web/.tailwind-src/ so the
# @source globs have something real to match. Without it the classes those
# crates use are simply absent from the sheet, and the failure is SILENT:
# a @source matching nothing is not an error, the component just renders
# unstyled.
_tw-link:
    #!/usr/bin/env bash
    set -euo pipefail
    mkdir -p apps/web/.tailwind-src
    for crate in architect-ui view-knowledge-graph; do
        dir=$(cargo metadata --format-version 1 2>/dev/null \
            | python3 -c "import json,sys,os;p=json.load(sys.stdin)['packages'];print(next(os.path.dirname(x['manifest_path']) for x in p if x['name']=='$crate'))")
        if [ -z "$dir" ] || [ ! -d "$dir" ]; then
            echo "cannot resolve $crate — is it in the dependency graph?" >&2
            exit 1
        fi
        ln -sfn "$dir" "apps/web/.tailwind-src/$crate"
    done

# Compile the site's Tailwind sheet. Gitignored output; `asset!()` needs
# it at compile time, so this runs before any build of apps/web.
tailwind: _tw-link
    cd apps/web && tailwindcss -i ./tailwind.css -o ./assets/tailwind.css --minify

# Fail if the sheet is missing classes the imported components need.
# Cheap insurance against the silent-@source failure described above.
tailwind-check: tailwind
    #!/usr/bin/env bash
    set -euo pipefail
    missing=()
    for class in cursor-grab cursor-grabbing backdrop-blur-sm text-muted-foreground; do
        grep -q -- "$class" apps/web/assets/tailwind.css || missing+=("$class")
    done
    if [ ${#missing[@]} -ne 0 ]; then
        echo "tailwind sheet is missing: ${missing[*]}" >&2
        echo "the @source globs in apps/web/tailwind.css matched nothing — run 'just _tw-link'" >&2
        exit 1
    fi
    echo "tailwind sheet covers the imported components"

# ── Apps ─────────────────────────────────────────────────────────────────

# Serve keyflow.fasttrackstudio.app with hot reload.
web: tailwind
    cd apps/web && dx serve --platform web

# Same server as `just web`, with three differences that all matter:
#
# `--addr` is the Tailscale IP specifically, NOT `0.0.0.0`. Binding every
# interface would put an unauthenticated dev server on whatever café wifi
# the laptop is on; binding the tailnet address means only your own devices
# can reach it, enforced by the network rather than by remembering.
#
# It runs in the FOREGROUND, as a normal `dx serve` with its TUI. That is
# deliberate: `--interactive false` looks like the right flag for an
# unattended server and quietly costs you the file watcher, so a guide
# edit never rebuilds and the preview silently shows stale pages. Run it
# under `just guide-preview-bg` (tmux) if you need it to outlive a shell.
#
# `--fullstack` is NOT optional, even though nothing here calls a server
# function. The `web` feature turns on `dioxus-web/hydrate`, so the client
# always expects a hydration payload to be embedded in the page — without
# a server half to render one, hydration fails in `atob` and the app
# renders a blank white page with one console exception and no other clue.
#
# `--hot-patch false` is the one that makes guide edits show up at all.
# dx 0.8 turns Rust hot-patching (Subsecond) on by default, and the docs
# are explicit that it "only tracks the tip crate… if you edit code in any
# of your dependencies — which might be your crate in a workspace — DX
# does not register that change". The guide is compiled in by `build.rs`,
# so editing a chapter is a `build.rs` rerun, not a tip-crate Rust edit:
# with hot-patching on, dx sees the file change, has nothing it can patch,
# and serves the previous build forever. Nothing errors. You just read
# stale pages. Turning it off gets an ordinary full rebuild.
#
# `apps/web/Dioxus.toml` already lists `../../docs/guides/keyflow` in
# `[web.watcher] watch_path`, which is what makes the change visible to
# the watcher in the first place.

# Serve the site on this machine's Tailscale address, to workshop the guide from elsewhere.
guide-preview port="8095": tailwind
    #!/usr/bin/env bash
    set -euo pipefail
    addr="$(tailscale ip -4 2>/dev/null | head -1)"
    if [ -z "$addr" ]; then
        echo "no Tailscale address — is tailscaled running?" >&2
        exit 1
    fi
    name="$(tailscale status --json 2>/dev/null \
        | python3 -c 'import sys,json;print(json.load(sys.stdin)["Self"]["DNSName"].rstrip("."))' \
        2>/dev/null || true)"
    # `dx serve` writes into this directory and does not prune it, so a
    # route that has since been renamed leaves its old `index.html`
    # behind — and the server keeps serving it, or redirects a live route
    # to the dead one. That is how `/guide/rhythm` started 307ing to
    # `/rhythm/` while production served both correctly. `web-build` has
    # cleared it for release builds for the same reason.
    rm -rf target/dx/keyflow-web/debug/web/public
    echo "  guide preview → http://${addr}:{{port}}/guide"
    [ -n "$name" ] && echo "                  http://${name}:{{port}}/guide"
    echo
    cd apps/web
    exec dx serve --platform web --fullstack --hot-patch false \
        --addr "$addr" --port {{port}}

# Build the shipping web bundle into target/dx/keyflow-web/release/web/public,
# with the guide pre-rendered into it.
#
# Three flags, none of them optional:
#
# `--ssg` builds the app's server as well as its client, runs it, asks it
# for `static_routes`, and requests each — which writes it to disk as
# HTML. The guide's chapters ship engraved and readable; the bundle then
# hydrates them into the ordinary app.
#
# `--fullstack` because dx works out whether to build a server from the
# CLIENT's features, and this crate keeps `dioxus/fullstack` on its
# `server` feature alone (its reqwest is a second major in the wasm
# binary — see apps/web/Cargo.toml). Without this there is no server
# target, and `--ssg` silently does nothing.
#
# `--force-sequential` because the pre-render borrows `public/index.html`
# as its page shell and the CLIENT build writes that file. In parallel
# the server can render first, and every page comes out in Dioxus's bare
# fallback shell — no title, no charset, no hydration — with the build
# still reporting success. (dioxus#3518.)
#
# The `rm` is not tidiness either. The renderer's cache is configured
# `clear_cache(false)`, which it must be — the cache directory is the
# bundle — but that also means a page already in it is served rather
# than re-rendered, so a rebuild after a code change silently ships the
# OLD html. Deleting the directory is what makes a build a build.
web-build: tailwind
    rm -rf target/dx/keyflow-web/release/web/public
    cd apps/web && dx build --platform web --release \
        --ssg --fullstack --force-sequential

# Check the site compiles for the browser. `cargo check --workspace` builds
# it for the host, which does NOT catch wasm-only breakage — the WebGL
# surface and every `cfg(target_arch = "wasm32")` block are invisible there.
web-check: tailwind
    cargo check -p keyflow-web --target wasm32-unknown-unknown

# The iOS app. Must run on a Mac; see apps/mobile/ios/README.md.
ios *ARGS:
    cd apps/mobile && ./ios/build-ios.sh {{ARGS}}

# ── Keyflow CLI ──────────────────────────────────────────────────────────

# Parse / render a .kf chart — `just kf render examples/foo.kf`
kf *ARGS:
    cargo run -p keyflow-cli -- {{ARGS}}

# The keyflow language server, for editor integration.
lsp *ARGS:
    cargo run -p keyflow-lsp -- {{ARGS}}

# Regenerate the tree-sitter C parser from grammar.js. The generated
# src/parser.c is gitignored, so a fresh clone compiles a NULL stub and
# warns until this has been run once.
grammar:
    cd features/tree-sitter && tree-sitter generate

# ── Disk / build-time hygiene ────────────────────────────────────────────
# Cargo never garbage-collects target/: every rebuild with a changed
# fingerprint leaves the old artifact behind forever. Measured in this tree:
# 56 stale copies of a single crate, 77 G of `debug/incremental`, ~1 TB of
# target/ across the worktrees. These recipes are the GC cargo doesn't have.

# Reclaim stale artifacts in THIS worktree (keeps anything touched recently).
sweep days="7":
    cargo sweep --time {{days}}
    @du -sh target

# Sweep every worktree — the thing to run when the dev disk fills up.
# Uses `git worktree list` so new worktrees are picked up automatically.
sweep-all days="7":
    #!/usr/bin/env bash
    set -euo pipefail
    before=$(du -sc $(git worktree list --porcelain | awk '/^worktree /{print $2"/target"}') 2>/dev/null | tail -1 | cut -f1)
    for w in $(git worktree list --porcelain | awk '/^worktree /{print $2}'); do
      [ -d "$w/target" ] || continue
      echo "── sweeping $w"
      cargo sweep --time {{days}} "$w" || true
    done
    after=$(du -sc $(git worktree list --porcelain | awk '/^worktree /{print $2"/target"}') 2>/dev/null | tail -1 | cut -f1)
    echo "reclaimed $(( (before - after) / 1024 / 1024 )) GiB"

# Drop incremental-compilation caches everywhere. They are pure cache —
# safe to delete, costs one non-incremental rebuild. Was 77 G in main alone.
sweep-incremental:
    #!/usr/bin/env bash
    set -euo pipefail
    for w in $(git worktree list --porcelain | awk '/^worktree /{print $2}'); do
      rm -rf "$w"/target/*/incremental "$w"/target/incremental 2>/dev/null || true
    done
    echo "incremental caches cleared"

# Where is the disk actually going? Per-worktree target/ sizes, largest first.
disk:
    #!/usr/bin/env bash
    du -sh $(git worktree list --porcelain | awk '/^worktree /{print $2"/target"}') 2>/dev/null | sort -rh

# Why is the build slow? Writes target/cargo-timings/cargo-timing.html —
# a per-crate Gantt chart showing the critical path and link-time tail.
timings *ARGS:
    cargo build --timings {{ARGS}}
    @echo "→ target/cargo-timings/cargo-timing.html"

# ── Knowledge graph (graphify) ───────────────────────────────────────────
# Whole-repo knowledge graph for AI assistants — parses the tree with
# tree-sitter (100% local, no API calls) into graphify-out/ (graph.json +
# GRAPH_REPORT.md + interactive graph.html). graphify is bootstrapped in the
# nix dev shell (see flake.nix shellHook). Output is gitignored + regenerable;
# rebuild after large structural changes. `graph-serve` exposes it over MCP
# (wired into .mcp.json so Claude Code queries it instead of grepping cold).

# Build/refresh the repo knowledge graph (local AST + clustering, no LLM).
# --force so the graph shrinks when .graphifyignore excludes more (vendored
# trees); without it graphify refuses a rebuild that has fewer nodes.
graph:
    graphify update . --force

# Serve the knowledge graph over MCP (stdio) — used by .mcp.json
graph-serve:
    graphify-mcp --transport stdio --graph graphify-out/graph.json

# `just guide-preview-log` follows it; `tmux attach -t keyflow-guide` gives
# you the TUI; `just guide-preview-stop` ends it.

# Same preview, detached in tmux so it outlives the shell that started it.
guide-preview-bg port="8095":
    #!/usr/bin/env bash
    set -euo pipefail
    tmux kill-session -t keyflow-guide 2>/dev/null || true
    tmux new-session -d -s keyflow-guide -c "$(pwd)" \
        "nix develop --accept-flake-config -c just guide-preview {{port}} 2>&1 | tee /tmp/kfguide.log"
    echo "started in tmux session 'keyflow-guide' — just guide-preview-log to follow"

# Follow the detached preview's output.
guide-preview-log:
    @tail -f /tmp/kfguide.log

# Stop the detached preview.
guide-preview-stop:
    @tmux kill-session -t keyflow-guide 2>/dev/null && echo stopped || echo "not running"
