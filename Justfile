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
ci: fmt-check check wasm-check test

fmt:
    cargo fmt --all

fmt-check:
    cargo fmt --all --check

# ── The Architect pin ────────────────────────────────────────────────
# Move every Architect pin to TAG: the [workspace.dependencies] pins, the
# [patch] table, features/ui's architect-workspace, the comments and
# AGENTS.md lines that name the tag, and Cargo.lock (cargo resolves the
# edit; nothing compiles). `--version X.Y.Z` also sets the workspace
# version, for a keyflow release. Exit 3: done, but Architect moved a pin
# this repo follows (editor, dioxus, blitz) and a person must move it.
# The bot (.github/workflows/bump-architect.yml) runs exactly this.
# Move every Architect pin and Cargo.lock to TAG (e.g. v0.12.5)
bump-architect TAG *ARGS:
    python3 scripts/bump-architect.py {{TAG}} {{ARGS}}

# keyflow#67's replay must equal the merged commit, Cargo.lock included;
# keyflow#61's must say "needs a person".
# Test the bump script against real bumps (keyflow#67, #61)
bump-architect-test:
    bash scripts/bump-architect-replay.sh

# ── wasm ─────────────────────────────────────────────────────────────
# Task's and Studio's web apps build these crates for the browser, and
# `cargo check --workspace` builds them for the host, where every
# `cfg(target_arch = "wasm32")` block and the WebGL surface are
# invisible. This is the browser build of the surface they take: the
# editor fence renderer (engraver underneath), the facade, and
# keyflow-ui's WebGL2 path. Never keyflow-ui's `web` feature — it turns
# on dioxus/desktop, which cannot build for wasm32.
#
# Not `pdf`: it pulls lopdf -> rand -> getrandom 0.3, which compiles for
# the browser only once the final binary takes getrandom's `wasm_js`
# feature as a direct dependency. A library cannot do that for its
# consumer, so a wasm app that wants PDF export supplies it (the old
# site did).
wasm-check:
    cargo check --target wasm32-unknown-unknown \
        -p editor-keyflow -p keyflow -p keyflow-ui --no-default-features \
        --features editor-keyflow/render,keyflow/text,keyflow/svg,keyflow/highlighting,keyflow-ui/wasm-graphics

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
