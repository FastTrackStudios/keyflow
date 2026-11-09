# Keyflow's dev environment. devenv-fleet's `rust` module (devenv.yaml) gives
# the toolchain, hakari, cargo-rail, mold, prek and kache's wrapper; its
# `dioxus` module gives dx and the wasm C toolchain. This file adds what the
# native and wasm builds link against and run, the Playwright browsers, and
# the nightly door the worklet build needs.
{ pkgs, lib, inputs, ... }:
let
  # Playwright's browsers come from the nixpkgs-dx pin: its playwright-driver
  # (1.59.1) matches the @playwright/test pin in tests/package.json. Without
  # it Playwright falls back to a downloaded chromium that cannot load its
  # shared libs on NixOS (libnspr4.so).
  pkgsDx = import inputs.nixpkgs-dx { inherit (pkgs.stdenv) system; };

  # `cargo-nightly`: the one door to a nightly with rust-src and the wasm
  # target, for the browser keys worklet's shared-memory build (wasm threads
  # need `-Z build-std`). Reached by name only; the 1.99.1 pin stays on PATH.
  rustNightly = pkgs.rust-bin.selectLatestNightlyWith (t: t.minimal);
  rustNightlyWasm = pkgs.rust-bin.selectLatestNightlyWith (t:
    t.minimal.override {
      extensions = [ "rust-src" ];
      targets = [ "wasm32-unknown-unknown" ];
    });
  cargoNightly = pkgs.writeShellScriptBin "cargo-nightly" ''
    export RUSTC="${rustNightlyWasm}/bin/rustc"
    exec ${rustNightlyWasm}/bin/cargo "$@"
  '';

  # The libraries the linker finds, also on the loader path so a desktop test
  # binary runs from this shell. The C++ runtime too: Luau is C++.
  libPath = lib.makeLibraryPath ([ pkgs.stdenv.cc.cc.lib pkgs.fontconfig pkgs.freetype pkgs.openssl ]
    ++ lib.optionals pkgs.stdenv.isLinux (with pkgs; [
      alsa-lib libjack2 pipewire
      libGL vulkan-loader gtk3 glib
      gdk-pixbuf pango cairo atk
      libx11 libxcb libxkbcommon wayland
      webkitgtk_4_1 libsoup_3 xdotool
    ]));
in
{
  packages = with pkgs; [
    cargo-watch
    cargo-nextest
    bacon
    flac
    # uv — the graphify bootstrap in enterShell (graphify is a PyPI tool).
    uv
    cargoNightly
    # Native headers and build tools for the cargo builds.
    openssl
    openssl.dev
    libiconv
    fontconfig
    freetype
    cmake
    python3
    # bindgen's clang paths; git — cargo shells out to it for every git dep.
    rustPlatform.bindgenHook
    git
  ]
  ++ lib.optionals pkgs.stdenv.isLinux (with pkgs; [
    alsa-lib alsa-lib.dev
    # libudev — hidapi's build script pkg-configs it (kontrol's raw USB HID).
    udev
    # ONNX Runtime for Chatterbox TTS (session-guide/tts): `ort` dlopens it.
    onnxruntime
    # JACK headers for cpal's `jack` feature (signal-sampler).
    libjack2
    # libpipewire headers for cpal's native `pipewire` feature.
    pipewire pipewire.dev
    glib gtk3 gdk-pixbuf pango cairo atk harfbuzz
    libsoup_3 webkitgtk_4_1 xdotool
    libx11 libxcursor libxrandr libxi libxcb
    libxkbcommon wayland libGL vulkan-loader
    # pw-jack runs the live rigs through PipeWire's JACK shim; jack_iodelay
    # and jack_lsp measure real round-trip latency.
    pipewire.jack
    jack-example-tools
    # Xvfb + xvfb-run — virtual display for the REAPER integration-test
    # harness (`just reaper-integration-test`).
    xorg.xorgserver
    xvfb-run
    # cargo-sweep — reclaims stale target/ artifacts (`just sweep`).
    cargo-sweep
  ]);

  env = {
    LIBCLANG_PATH = "${pkgs.llvmPackages.libclang.lib}/lib";
    OPENSSL_DIR = "${pkgs.openssl.dev}";
    OPENSSL_LIB_DIR = "${pkgs.openssl.out}/lib";
    PLAYWRIGHT_BROWSERS_PATH = "${pkgsDx.playwright-driver.browsers}";
    PLAYWRIGHT_SKIP_VALIDATE_HOST_REQUIREMENTS = "true";
  }
  // lib.optionalAttrs pkgs.stdenv.isLinux {
    LD_LIBRARY_PATH = libPath;
    # `ort` (load-dynamic) dlopens this exact .so. Unset, synthesis fails and
    # section cues fall back to the synth chime; the app still runs.
    ORT_DYLIB_PATH = "${pkgs.onnxruntime}/lib/libonnxruntime.so";
    XDG_DATA_DIRS = "${pkgs.gsettings-desktop-schemas}/share/gsettings-schemas/${pkgs.gsettings-desktop-schemas.name}:${pkgs.gtk3}/share/gsettings-schemas/${pkgs.gtk3.name}";
    # WebKitGTK's accelerated compositing fails on NixOS (GBM buffer error,
    # white window). https://github.com/NixOS/nixpkgs/issues/32580
    WEBKIT_DISABLE_COMPOSITING_MODE = "1";
  }
  // lib.optionalAttrs pkgs.stdenv.isDarwin (
    let
      # ring's wasm build uses clang-18, but DYLD_LIBRARY_PATH makes it load the
      # stdenv's clang-21 libclang-cpp (SIGABRT). Run it with that unset.
      wasmCC = pkgs.writeShellScript "wasm-clang-18" ''
        exec /usr/bin/env -u DYLD_LIBRARY_PATH ${pkgs.llvmPackages_18.clang-unwrapped}/bin/clang "$@"
      '';
    in
    {
      DYLD_LIBRARY_PATH = libPath;
      CC_wasm32_unknown_unknown = "${wasmCC}";
      # phon-jit's build script compiles its `become` stencils with this nightly.
      PHON_JIT_NIGHTLY_RUSTC = "${rustNightly}/bin/rustc";
    }
  );

  enterShell = ''
    # Appended, not prepended: store-provided tools must win over stale
    # cargo-installed copies; cargo and uv bins only need to be reachable.
    export PATH="$PATH:$HOME/.cargo/bin:$HOME/.local/bin"

    # Dev-convenience installers are for an interactive shell only. In CI, or
    # under direnv, `cargo install tracey` would compile from source each time.
    if [ -t 1 ] && [ -z "''${CI:-}" ]; then
      # graphify — whole-repo knowledge graph (safishamsi/graphify), pinned.
      # Code extraction is local (tree-sitter); the [mcp] extra serves it.
      GRAPHIFY_VERSION="0.9.15"
      if ! uv tool list 2>/dev/null | grep -q "graphifyy v$GRAPHIFY_VERSION"; then
        echo "  Installing graphify $GRAPHIFY_VERSION..."
        UV_PYTHON_DOWNLOADS=never uv tool install --force \
          --python "${pkgs.python3}/bin/python3" \
          "graphifyy[mcp]==$GRAPHIFY_VERSION" >/dev/null 2>&1 || \
          echo "  (graphify install failed — run: uv tool install 'graphifyy[mcp]==$GRAPHIFY_VERSION')"
      fi

      # tracey — spec-coverage CLI (crates.io), pinned. Config: .config/tracey/config.styx.
      TRACEY_VERSION=$(tracey --version 2>/dev/null | grep -oE '[0-9]+\.[0-9]+\.[0-9]+' | head -1 || echo "0")
      if [ "$TRACEY_VERSION" != "1.3.0" ]; then
        echo "  Installing tracey 1.3.0..."
        cargo install tracey --locked --version "=1.3.0" 2>/dev/null || true
      fi
    fi
  '';

  fleet.banner = ''
    Keyflow: just check / just test / just ci, just kf --help (the keyflow CLI)
    just graph (re)builds the repo knowledge graph
  '';
}
