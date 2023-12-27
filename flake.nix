{
  description = "A discord bot for Dự Tuyển Tổng Hợp server";
  inputs = {
    nixpkgs.url = "github:nixos/nixpkgs/nixos-23.11";
    naersk.url = "github:nix-community/naersk";
    naersk.inputs.nixpkgs.follows = "nixpkgs";
    flake-utils.url = "github:numtide/flake-utils";
  };
  nixConfig = {
    extra-substituters = [ "https://natsukagami.cachix.org" ];
    trusted-public-keys = [ "natsukagami.cachix.org-1:3U6GV8i8gWEaXRUuXd2S4ASfYgdl2QFPWg4BKPbmYiQ=" ];
  };
  outputs = { self, nixpkgs, flake-utils, ... }@inputs: flake-utils.lib.eachDefaultSystem
    (system:
      let
        pkgs = import nixpkgs { inherit system; };

        naersk = pkgs.callPackage inputs.naersk { };
      in
      rec {
        packages.youmubot = pkgs.callPackage ./package.nix { inherit naersk; };

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

            buildInputs = with pkgs; [ rustc rustfmt clippy ];

            nativeBuildInputs = nixpkgs.lib.optionals pkgs.stdenv.isLinux (with pkgs; [
              pkg-config
            ]);

            RUST_SRC_PATH = pkgs.rustPlatform.rustLibSrc;
          };
      }) // {
    overlays.default = final: prev: {
      youmubot = final.callPackage ./package.nix {
        naersk = final.callPackage inputs.naersk { };
      };
    };
    # module
    nixosModules.default = import ./module.nix;
  };
}

