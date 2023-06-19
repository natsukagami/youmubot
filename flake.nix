{
  description = "A discord bot for Dự Tuyển Tổng Hợp server";
  inputs = {
    nixpkgs.url = "github:nixos/nixpkgs/nixos-23.05";
    naersk.url = "github:nix-community/naersk";
    naersk.inputs.nixpkgs.follows = "nixpkgs";
    flake-utils.url = "github:numtide/flake-utils";
  };
  nixConfig = {
    extra-substituters = [ "https://natsukagami.cachix.org" ];
    trusted-public-keys = [ "natsukagami.cachix.org-1:3U6GV8i8gWEaXRUuXd2S4ASfYgdl2QFPWg4BKPbmYiQ=" ];
  };
  outputs = { self, nixpkgs, flake-utils, ... }@inputs: flake-utils.lib.eachDefaultSystem (system:
    let
      pkgs = import nixpkgs { inherit system; };

      naersk = pkgs.callPackage inputs.naersk { };
    in
    rec {
      packages.youmubot = pkgs.callPackage ./package.nix { inherit naersk; };

      defaultPackage = packages.youmubot;

      overlays.default = final: prev: {
        youmubot = final.callPackage ./package.nix {
          naersk = final.callPackage inputs.naersk { };
        };
      };

      # `nix run`
      apps.youmubot = flake-utils.lib.mkApp {
        drv = packages.youmubot;
        exePath = "/bin/youmubot";
      };
      defaultApp = apps.youmubot;

      # `nix develop`
      devShell = pkgs.mkShell
        {
          buildInputs =
            nixpkgs.lib.optionals pkgs.stdenv.isDarwin
              (with pkgs; [
                libiconv
                darwin.apple_sdk.frameworks.Security
              ])
            ++ (with pkgs; [
              openssl
              cargo
              rustfmt
            ]);

          nativeBuildInputs = nixpkgs.lib.optionals pkgs.stdenv.isLinux (with pkgs; [
            pkg-config
          ]);
        };
      # module
      nixosModule = import ./module.nix defaultPackage;
    });
}

