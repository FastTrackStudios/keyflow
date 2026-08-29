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

# Build the shipping web bundle into target/dx/keyflow-web/release/web/public.
web-build: tailwind
    cd apps/web && dx build --platform web --release

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
    cd crates/keyflow/tree-sitter-keyflow && tree-sitter generate

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
