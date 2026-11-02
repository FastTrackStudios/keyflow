# Shared perSystem option surface — every other module either fills or
# consumes `fts.*`. This is the ONLY cross-module contract; modules never
# reach into each other directly (dendritic rule: files are self-contained
# except for this schema).
{ lib, flake-parts-lib, ... }:
{
  options.perSystem = flake-parts-lib.mkPerSystemOption (_: {
    options.fts = {
      rustToolchain = lib.mkOption {
        type = lib.types.package;
        description = "FTS-wide pinned Rust toolchain (stable 1.99.1 + wasm32).";
      };
      buildInputs = lib.mkOption {
        type = lib.types.listOf lib.types.package;
        description = "Native libraries every cargo build in the tree links against.";
      };
      nativeBuildInputs = lib.mkOption {
        type = lib.types.listOf lib.types.package;
        description = "Host tools for interactive builds (pkg-config, bindgen, git…).";
      };
      shellEnv = lib.mkOption {
        type = lib.types.attrsOf lib.types.str;
        description = "Env every dev/CI shell exports (bindgen, wasm CC, LD paths…).";
      };
      pkgsDx = lib.mkOption {
        type = lib.types.raw;
        description = "The nixpkgs-dx package set (current-unstable; cargo-rail builds through it).";
      };
      cargoRail = lib.mkOption {
        type = lib.types.nullOr lib.types.package;
        description = "cargo-rail release binary (null where upstream ships no asset).";
      };
    };
  });
}
