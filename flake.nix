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
  outputs = { self, nixpkgs, naersk, flake-utils, ... }: flake-utils.lib.eachDefaultSystem (system:
    let
      pkgs = import nixpkgs { inherit system; };

      naersk' = pkgs.callPackage naersk { };
    in
    rec {
      packages.youmubot = naersk'.buildPackage {
        name = "youmubot";
        version = "0.1.0";

        root = ./.;
        cargoBuildOptions = opts: opts ++ [ "--package youmubot" ];

        buildInputs = with pkgs; [
          openssl
        ];

        nativeBuildInputs = nixpkgs.lib.optionals pkgs.stdenv.isLinux (with pkgs; [
          pkg-config
        ]);

        SQLX_OFFLINE = "true";
      };

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

