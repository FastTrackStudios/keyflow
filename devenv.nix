# Keyflow's dev environment. The toolchain (rust 1.99.1 with wasm32, the
# rust-toolchain.toml pin), mold, cargo-hakari, cargo-rail, prek and just come
# from devenv-fleet's `rust` module, and dx and the wasm tools from `dioxus`
# (devenv.yaml). This file adds what this workspace links against and runs.
#
# No installers run on shell entry: graphify (a uv tool) and tracey (a cargo
# install) used to, and stalled shells on cache misses and flaky egress. Both
# are user tools: install them once by hand (see the tasks below), they land in
# ~/.local/bin and ~/.cargo/bin, which are appended to PATH.
{ pkgs, lib, config, inputs, ... }:
let
  toolchain = config.fleet.rust.toolchain;
  pkgsDx = import inputs.nixpkgs-dx { inherit (pkgs.stdenv) system; };

  # A pinned nightly rustc used ONLY to compile phon-jit's copy-and-patch
  # stencils, which chain via `become` (explicit tail calls). The rest of the
  # workspace stays on the stable pin; the stencils meet stable-compiled code
  # only as extracted machine code, so there is no ABI mixing. Darwin-only:
  # elsewhere phon-jit uses the interpreter and never invokes rustc for them.
  rustNightly = pkgs.rust-bin.selectLatestNightlyWith (t: t.minimal);
  # A nightly WITH rust-src and wasm, for the one build stable cannot do: the
  # browser keys rig's shared-memory worklet (wasm threads need
  # `-C target-feature=+atomics` and a std rebuilt with it, `-Z build-std`).
  rustNightlyWasm = pkgs.rust-bin.selectLatestNightlyWith (t:
    t.minimal.override {
      extensions = [ "rust-src" ];
      targets = [ "wasm32-unknown-unknown" ];
    });
  # An explicit door to that toolchain: there is no rustup here, so
  # `cargo +nightly` cannot work, and it must not be first on PATH as `cargo`.
  # `just keys-worklet-wasm-threads` calls it by name.
  cargoNightly = pkgs.writeShellScriptBin "cargo-nightly" ''
    export RUSTC="${rustNightlyWasm}/bin/rustc"
    exec ${rustNightlyWasm}/bin/cargo "$@"
  '';

  common = with pkgs; [
    openssl
    openssl.dev
    libiconv
    fontconfig
    freetype
    cmake
    python3
    rustPlatform.bindgenHook
  ];
  linux = with pkgs; lib.optionals stdenv.isLinux [
    alsa-lib
    alsa-lib.dev
    # hidapi's build script pkg-configs libudev (kontrol's raw USB HID).
    udev
    # `ort` is load-dynamic: it dlopens this libonnxruntime.so at run time
    # (ORT_DYLIB_PATH below), never a downloaded binary.
    onnxruntime
    # cpal's `jack` feature; run under `pw-jack` to route through PipeWire.
    libjack2
    # cpal's native `pipewire` feature; `.dev` has the .pc file and headers.
    pipewire
    pipewire.dev
    glib
    gtk3
    gdk-pixbuf
    pango
    cairo
    atk
    harfbuzz
    libsoup_3
    webkitgtk_4_1
    xdotool
    libx11
    libxcursor
    libxrandr
    libxi
    libxcb
    libxkbcommon
    wayland
    libGL
    vulkan-loader
  ];
  runtimeLibs = with pkgs; [ stdenv.cc.cc.lib fontconfig freetype openssl ] ++ linux;
in
{
  packages = common ++ linux ++ (with pkgs; [
    cargo-watch
    cargo-nextest
    bacon
    flac
    uv
    git
    cargoNightly
  ]) ++ lib.optionals pkgs.stdenv.isLinux (with pkgs; [
    # cargo-sweep reclaims stale target/ artifacts: `just sweep`, `just sweep-all`.
    cargo-sweep
    # pw-jack runs the live rigs through PipeWire's JACK shim; jack_iodelay and
    # jack_lsp measure round-trip latency; Xvfb + xvfb-run are the virtual
    # display of `just reaper-integration-test`.
    pipewire.jack
    jack-example-tools
    xorg.xorgserver
    xvfb-run
  ]);

  env = {
    LIBCLANG_PATH = "${pkgs.llvmPackages.libclang.lib}/lib";
    OPENSSL_DIR = "${pkgs.openssl.dev}";
    OPENSSL_LIB_DIR = "${pkgs.openssl.out}/lib";
    # Playwright's browsers for the editor web suite (libs/editor/tests), from
    # nixpkgs-dx: 1.59.1 matches the @playwright/test pin in tests/package.json,
    # so keep them in lockstep. Playwright's own chromium can't load shared
    # libs on NixOS (libnspr4.so).
    PLAYWRIGHT_BROWSERS_PATH = "${pkgsDx.playwright-driver.browsers}";
    PLAYWRIGHT_SKIP_VALIDATE_HOST_REQUIREMENTS = "true";
    # rust-analyzer's stdlib sources (the fleet rust module sets the same).
    RUST_SRC_PATH = "${toolchain}/lib/rustlib/src/rust/library";
  } // lib.optionalAttrs pkgs.stdenv.isLinux {
    LD_LIBRARY_PATH = lib.makeLibraryPath runtimeLibs;
    ORT_DYLIB_PATH = "${pkgs.onnxruntime}/lib/libonnxruntime.so";
    XDG_DATA_DIRS = "${pkgs.gsettings-desktop-schemas}/share/gsettings-schemas/${pkgs.gsettings-desktop-schemas.name}:${pkgs.gtk3}/share/gsettings-schemas/${pkgs.gtk3.name}";
    # WebKitGTK accelerated compositing fails on NixOS (GBM buffer error, white
    # window). Force software rendering: nixpkgs#32580.
    WEBKIT_DISABLE_COMPOSITING_MODE = "1";
  } // lib.optionalAttrs pkgs.stdenv.isDarwin {
    DYLD_LIBRARY_PATH = lib.makeLibraryPath runtimeLibs;
    # phon-jit's build script compiles its `become` stencils with this nightly.
    PHON_JIT_NIGHTLY_RUSTC = "${rustNightly}/bin/rustc";
  };

  # Store-provided tools win; the user tools (graphify, tracey) only need to
  # be reachable.
  enterShell = ''
    export PATH="$PATH:$HOME/.cargo/bin:$HOME/.local/bin"
  '';

  # User tools, installed on demand with `devenv tasks run keyflow:graphify`
  # and `keyflow:tracey`. Not before enterShell: they need the network.
  tasks."keyflow:graphify" = {
    # graphify, the whole-repo knowledge-graph tool (`just graph`); PyPI only.
    exec = ''
      UV_PYTHON_DOWNLOADS=never uv tool install --force \
        --python ${pkgs.python3}/bin/python3 "graphifyy[mcp]==0.9.15"
    '';
    status = ''uv tool list 2>/dev/null | grep -q "graphifyy v0.9.15"'';
  };
  tasks."keyflow:tracey" = {
    # tracey, the spec-coverage CLI (docs/spec/**, .config/tracey/config.styx).
    exec = ''cargo install tracey --locked --version "=1.3.0"'';
    status = ''tracey --version 2>/dev/null | grep -q "1\.3\.0"'';
  };
}
