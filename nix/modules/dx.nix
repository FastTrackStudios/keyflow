# The nixpkgs-dx package set (fills fts.pkgsDx).
#
# A dedicated, current-unstable nixpkgs (the `nixpkgs-dx` input). It was
# pinned to source `dx` for the standalone web and iOS apps; those are
# gone (Keyflow ships inside Studio), and what still reads it is
# cargo-rail's build (nix/modules/cargo-rail.nix) and the dev shell's
# Playwright browsers.
{ inputs, ... }:
{
  perSystem = { system, ... }: {
    fts.pkgsDx = import inputs.nixpkgs-dx { inherit system; };
  };
}
