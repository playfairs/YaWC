{
  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    flake-utils.url = "github:numtide/flake-utils";
    treefmt-nix.url = "github:numtide/treefmt-nix";
    rust-overlay.url = "github:oxalica/rust-overlay";
    naersk.url = "github:nix-community/naersk";
  };
  outputs =
    {
      nixpkgs,
      naersk,
      flake-utils,
      treefmt-nix,
      rust-overlay,
      ...
    }:
    let
      runtimeLibs = pkgs: with pkgs; [
        expat
        fontconfig
        freetype
        freetype.dev
        vulkan-loader
        alsa-plugins
        udev
        libGL
        pkg-config
        libx11
        libxcursor
        libxi
        libxrandr
        wayland
        libxkbcommon
      ];
      overlays = {
        default = final: prev: {
          yawc =
            (final.callPackage naersk {
              cargo = final.rust-bin.nightly.latest.default;
              rustc = final.rust-bin.nightly.latest.default;
            }).buildPackage {
              pname = "yawc";
              src = ./.;

              nativeBuildInputs = with final; [
                makeWrapper
              ];

              buildInputs = with final; [
                pkg-config
                pipewire
                seatd
                libdisplay-info
                alsa-lib
                jack2
                udev
                pixman
                libxkbcommon
                libinput
                libgbm
              ];

              postInstall = ''
                mkdir -p $out/share/wayland-sessions
                cp ${./yawc.desktop} $out/share/wayland-sessions/yawc.desktop

                wrapProgram $out/bin/yawc \
                  --prefix LD_LIBRARY_PATH : "${final.lib.makeLibraryPath (runtimeLibs final)}"
              '';
            };
        };
      };
    in
    flake-utils.lib.eachDefaultSystem (
      system:
      let
        pkgs = import nixpkgs {
          inherit system;
          overlays = [
            (import rust-overlay)
            overlays.default
          ];
        };
      in
      {
        devShells.default = pkgs.mkShell {
          buildInputs =
            with pkgs;
            [
              typos
              clippy
              rustfmt
              # INFO: Until configuration exists, alacritty
              # is a hardcoded keybind, but doesn't mean it's
              # a build dep.
              alacritty
              pkg-config
              rust-analyzer
              rust-bin.nightly.latest.default
            ]
            ++ lib.optionals stdenv.isLinux [
              pipewire
              seatd
              libdisplay-info
              alsa-lib
              jack2
              udev
              pixman
              libxkbcommon
              libinput
              libgbm
            ];


          LD_LIBRARY_PATH = pkgs.lib.makeLibraryPath runtimeLibs;
        };

        packages.default = pkgs.yawc;
        formatter =
          (treefmt-nix.lib.evalModule pkgs (_: {
            projectRootFile = "flake.nix";
            programs = {
              nixfmt.enable = true;
              nixf-diagnose.enable = true;
              rustfmt.enable = true;
              toml-sort.enable = true;
            };
            settings.formatter.rustfmt = {
              unstable-features = true;
              tab_spaces = 2;
              trailing_semicolon = false;
              style_edition = "2024";
              use_try_shorthand = true;
              wrap_comments = true;
            };
          })).config.build.wrapper;
      }
    )
    // {
      inherit overlays;
    };
}
