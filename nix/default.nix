{
  lib,
  callPackage,
  naersk,
  makeWrapper,
  pkg-config,
  pipewire,
  seatd,
  libdisplay-info,
  alsa-lib,
  jack2,
  udev,
  pixman,
  libxkbcommon,
  libinput,
  libgbm,
  libx11,
  rust-bin,
  libxcursor,
  libxi,
  libxrandr,
  wayland,
  libGL,
  expat,
  fontconfig,
  freetype,
  vulkan-loader,
  alsa-plugins,
  ...
}:
let
  rustPkg = rust-bin.nightly.latest.default;
in
(callPackage naersk {
  cargo = rustPkg;
  rustc = rustPkg;
}).buildPackage
  rec {
    pname = "yawc";
    src = ../.;

    nativeBuildInputs = [
      makeWrapper
    ];

    buildInputs = [
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
      rustPkg
    ];

    libraryBuildInputs = [
      expat
      fontconfig
      freetype
      vulkan-loader
      alsa-plugins
      # udev
      libGL
      libx11
      libxcursor
      libxi
      libxrandr
      wayland
    ];

    postInstall = ''
      mkdir -p $out/share/wayland-sessions
      cp ${../yawc.desktop} $out/share/wayland-sessions/yawc.desktop

      wrapProgram $out/bin/yawc \
        --prefix LD_LIBRARY_PATH : "${lib.makeLibraryPath libraryBuildInputs}"
    '';
  }
