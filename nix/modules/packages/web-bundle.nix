# The dx web bundle (keyflow-web) + its static-site OCI image.
#
# `dx bundle` → $out/www, brotli pre-compressed, wrapped by
# `fts.mkStaticSite` into a static-web-server image the cluster runs.
# Mirrors task's web-bundles.nix — same dx, same crane, same serving
# shape — because the two sites are deployed by the same pipeline and a
# second way of doing it is a second thing to debug at 3am.
{ ... }:
{
  perSystem =
    {
      pkgs,
      lib,
      config,
      ...
    }:
    let
      inherit (config.fts) craneLib commonArgs mkStaticSite;

      # Crates that compile C to wasm via cc::Build (arborium's
      # tree-sitter runtime and grammars, ring) need an UNWRAPPED clang
      # targeting wasm32 plus llvm-ar: the cc-wrapper injects host-only
      # hardening flags clang rejects for wasm. Without these the C
      # symbols stay as unresolved `(import "env" …)` entries and the
      # shipped bundle white-screens. Mirrors devShells.default.
      dxWebEnv = {
        CC_wasm32_unknown_unknown = "${pkgs.llvmPackages_18.clang-unwrapped}/bin/clang";
        AR_wasm32_unknown_unknown = "${pkgs.llvmPackages_18.bintools-unwrapped}/bin/llvm-ar";
        CFLAGS_wasm32_unknown_unknown = "-isystem ${pkgs.llvmPackages_18.clang}/resource-root/include";
        # Hermetic dx: the sandbox has no network, so `NO_DOWNLOADS`
        # makes dx resolve wasm-opt / wasm-bindgen from PATH rather than
        # fetching them from GitHub.
        NO_DOWNLOADS = "1";
        # getrandom 0.3 needs BOTH the cfg and the feature; the cfg
        # lives in .cargo/config.toml and travels with the source.
      };

      dxWebNativeInputs =
        commonArgs.nativeBuildInputs
        ++ [
          config.fts.dx.cli
          config.fts.dx.wasmBindgen
          config.fts.dx.binaryen
        ]
        ++ (with pkgs; [
          tailwindcss_4
          jq
          brotli
          llvmPackages_18.clang-unwrapped
          llvmPackages_18.bintools-unwrapped
        ]);

      keyflow-webapp = craneLib.buildPackage (
        commonArgs
        // dxWebEnv
        // {
          pname = "keyflow-webapp";
          version = "0.1.0";
          cargoArtifacts = null;
          cargoExtraArgs = "--manifest-path apps/web/Cargo.toml";
          nativeBuildInputs = dxWebNativeInputs;
          doNotPostBuildInstallCargoBinaries = true;

          buildPhaseCargoCommand = ''
            export HOME="$TMPDIR/dx-home"
            mkdir -p "$HOME"

            # The sheet is gitignored build output that `asset!()` demands
            # at compile time, and it @sources two crates that arrive as
            # GIT DEPS — a git dep has no stable path, so the globs point
            # at symlinks that have to be made first. Same resolution the
            # Justfile's `_tw-link` does, via cargo metadata, because that
            # is the only thing that knows where cargo put them.
            #
            # A @source matching nothing is SILENT: no error, just missing
            # classes and an unstyled component. So this fails loudly if a
            # crate cannot be resolved rather than building a short sheet.
            mkdir -p apps/web/.tailwind-src
            for crate in architect-ui view-knowledge-graph; do
              dir="$(cargo metadata --format-version 1 --offline \
                | jq -er --arg c "$crate" \
                    '.packages[] | select(.name == $c) | .manifest_path' \
                | head -1)"
              dir="''${dir%/Cargo.toml}"
              if [ -z "$dir" ] || [ ! -d "$dir" ]; then
                echo "cannot resolve $crate for the tailwind @source globs" >&2
                exit 1
              fi
              ln -sfn "$dir" "apps/web/.tailwind-src/$crate"
            done
            (cd apps/web && tailwindcss -i ./tailwind.css -o ./assets/tailwind.css --minify)

            cd apps/web
            # --debug-symbols false: drops DWARF for a smaller bundle and
            # sidesteps DWARF-version mismatches in wasm-opt.
            dx build --release --platform web --debug-symbols false
          '';

          # The build phase ends inside apps/web, but dx writes to the
          # WORKSPACE-ROOT target/dx/keyflow-web/…; anchor the copy there.
          installPhaseCommand = ''
            mkdir -p $out/www
            srcdir="$(pwd)"
            case "$srcdir" in */apps/web) srcdir="''${srcdir%/apps/web}";; esac
            cp -R "$srcdir/target/dx/keyflow-web/release/web/public/." $out/www/
            # Pre-compress so static-web-server's `compression-static`
            # can serve .br — the wasm is multi-MB uncompressed.
            find $out/www -type f \( -name '*.wasm' -o -name '*.js' \
              -o -name '*.css' -o -name '*.html' -o -name '*.json' \
              -o -name '*.svg' \) -exec brotli --keep --quality=9 {} +
          '';

          doCheck = false;
        }
      );
    in
    {
      packages =
        { inherit keyflow-webapp; }
        // lib.optionalAttrs pkgs.stdenv.isLinux {
          keyflow-web-image = mkStaticSite {
            name = "keyflow-web";
            siteRoot = "${keyflow-webapp}/www";
          };
        };
    };
}
