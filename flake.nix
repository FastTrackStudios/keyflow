{
  description = "Keyflow — the chart language and the Engraver layout engine";

  # Dendritic layout (den): every .nix under nix/modules/ is a
  # flake-parts module, auto-loaded by import-tree — one file per
  # concern, no central wiring. Shared values flow through the typed
  # `fts.*` perSystem options (nix/modules/options.nix); den's aspect
  # system (nix/modules/den.nix) is the sharing surface with the
  # system flake.
  outputs = inputs: inputs.flake-parts.lib.mkFlake { inherit inputs; }
    (inputs.import-tree ./nix/modules);

  inputs = {
    den.url = "github:denful/den";
    import-tree.url = "github:vic/import-tree";

    # Shared Dioxus toolchain hub — nixpkgs follows it.
    dioxus-flake.url = "github:FastTrackStudios/Dioxus-Flake";
    nixpkgs.follows = "dioxus-flake/nixpkgs";
    # rust-overlay is ours, not the hub's (task made the same move): the
    # hub's sat at 2026-04-05, which capped the toolchain at 1.94, and
    # architect v0.10.0 needs 1.99.1 (its Dioxus rev and dioxus-primitives
    # extend from `Box<[T; N]>`, which 1.94 cannot iterate). `dx` has its
    # own nixpkgs-dx pin, so nothing needed the coupling.
    rust-overlay.url = "github:oxalica/rust-overlay";
    rust-overlay.inputs.nixpkgs.follows = "nixpkgs";
    flake-parts.url = "github:hercules-ci/flake-parts";

    # Dedicated, current-unstable nixpkgs: cargo-rail builds through it
    # and the dev shell takes Playwright's browsers from it — see
    # nix/modules/dx.nix.
    nixpkgs-dx.url = "github:NixOS/nixpkgs/d99b013d5d1931ad77fe3912ed218170dec5d9a4";
  };

  nixConfig = {
    extra-trusted-public-keys = [
      "fasttrackstudio.cachix.org-1:r7v7WXBeSZ7m5meL6w0wttnvsOltRvTpXeVNItcy9f4="
    ];
    extra-substituters = [
      "https://fasttrackstudio.cachix.org"
    ];
  };
}
