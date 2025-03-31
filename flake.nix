{
  description = "A discord bot for Dự Tuyển Tổng Hợp server";
  inputs = {
    nixpkgs.url = "github:nixos/nixpkgs/nixos-unstable";
    flake-utils.url = "github:numtide/flake-utils";
    rust-overlay = {
      url = "github:oxalica/rust-overlay";
      inputs.nixpkgs.follows = "nixpkgs";
    };
  };
  nixConfig = {
    extra-substituters = [ "https://natsukagami.cachix.org" ];
    extra-trusted-public-keys = [ "natsukagami.cachix.org-1:3U6GV8i8gWEaXRUuXd2S4ASfYgdl2QFPWg4BKPbmYiQ=" ];
  };
  outputs = { self, nixpkgs, flake-utils, ... }@inputs:
    flake-utils.lib.eachDefaultSystem
      (system:
        let
          pkgs = import nixpkgs
            {
              inherit system; overlays = [ (import inputs.rust-overlay) ];
            };
        in
        rec {
          packages.youmubot = pkgs.callPackage ./package.nix { };

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
        youmubot = final.callPackage ./package.nix { };
      };
      # module
      nixosModules.default = import ./module.nix;
    };
}

