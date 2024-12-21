{
  description = "A discord bot for Dự Tuyển Tổng Hợp server";
  inputs = {
    nixpkgs.url = "github:nixos/nixpkgs/nixos-unstable";
    crane.url = "github:ipetkov/crane";
    flake-utils.url = "github:numtide/flake-utils";
    rust-overlay = {
      url = "github:oxalica/rust-overlay";
      inputs.nixpkgs.follows = "nixpkgs";
    };
  };
  nixConfig = {
    extra-substituters = [ "https://natsukagami.cachix.org" ];
    trusted-public-keys = [ "cache.nixos.org-1:6NCHdD59X431o0gWypbMrAURkbJ16ZPMQFGspcDShjY=" "natsukagami.cachix.org-1:3U6GV8i8gWEaXRUuXd2S4ASfYgdl2QFPWg4BKPbmYiQ=" ];
  };
  outputs = { self, nixpkgs, flake-utils, ... }@inputs: flake-utils.lib.eachDefaultSystem
    (system:
      let
        pkgs = import nixpkgs
          {
            inherit system; overlays = [ (import inputs.rust-overlay) ];
          };
        craneLib = (inputs.crane.mkLib pkgs).overrideToolchain (p: p.rust-bin.stable."1.83.0".default);
        # craneLib = inputs.crane.mkLib pkgs;
      in
      rec {
        packages.youmubot = pkgs.callPackage ./package.nix { inherit craneLib; };

        defaultPackage = packages.youmubot;

        # `nix run`
        apps.youmubot = flake-utils.lib.mkApp {
          drv = packages.youmubot;
          exePath = "/bin/youmubot";
        };
        defaultApp = apps.youmubot;

        # `nix develop`
        devShell = pkgs.mkShell
          {
            inputsFrom = [ packages.youmubot ];

            buildInputs = with pkgs; [ rustfmt clippy sqlx-cli rust-analyzer ];

            nativeBuildInputs = nixpkgs.lib.optionals pkgs.stdenv.isLinux (with pkgs; [
              pkg-config
            ]);

            RUST_SRC_PATH = pkgs.rustPlatform.rustLibSrc;
          };
      }) // {
    overlays.default = final: prev: {
      youmubot = final.callPackage ./package.nix {
        craneLib = (inputs.crane.mkLib final).overrideToolchain (p: p.rust-bin.stable."1.79.0".default);
      };
    };
    # module
    nixosModules.default = import ./module.nix;
  };
}

