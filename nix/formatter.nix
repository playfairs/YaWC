{
  treefmt-nix,
  pkgs,
  ...
}:
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
})).config.build.wrapper
