{
  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    flake-parts.url = "github:hercules-ci/flake-parts";
    treefmt-nix.url = "github:numtide/treefmt-nix";
    rust-overlay.url = "github:oxalica/rust-overlay";
    naersk.url = "github:nix-community/naersk";
  };
  outputs =
    { self, ... }@inputs:
    inputs.flake-parts.lib.mkFlake { inherit inputs; } {
      imports = [
        inputs.flake-parts.flakeModules.easyOverlay
      ];
      systems = [
        "aarch64-linux"
        "x86_64-linux"
      ];
      flake = {
        nixosModules.yawc = import ./nix/nixos-modules.nix self;
      };
      perSystem =
        {
          system,
          pkgs,
          ...
        }:
        let
          pkgsWithRustOverlay = pkgs // {
            overlays = pkgs.overlays ++ [
              (import inputs.rust-overlay)
            ];
          };

          yawc = pkgsWithRustOverlay.callPackage ./nix {
            inherit (inputs) naersk;
          };

          shellOverride = old: {
            nativeBuildInputs = old.nativeBuildInputs ++ [ ];
            LD_LIBRARY_PATH = pkgs.lib.makeLibraryPath old.libraryBuildInputs;
            buildInputs = old.buildInputs ++ [
              pkgs.alacritty
            ];
          };
        in
        {
          _module.args.pkgs = import inputs.nixpkgs {
            inherit system;
            overlays = [
              (import inputs.rust-overlay)
            ];
            config = { };
          };
          packages = {
            default = yawc;
          };
          devShells.default = yawc.overrideAttrs shellOverride;
          formatter = pkgs.callPackage ./nix/formatter.nix {
            inherit (inputs) treefmt-nix;
            inherit pkgs; # Formatter args need directly pkgs
          };
        };
    };
}
