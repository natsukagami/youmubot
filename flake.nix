{
  description = "A discord bot for Dự Tuyển Tổng Hợp server";
  inputs = {
    nixpkgs.url = "github:nixos/nixpkgs/nixos-23.11";
    crane = {
      url = "github:ipetkov/crane";
      inputs.nixpkgs.follows = "nixpkgs";
    };
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
        craneLib = inputs.crane.lib.${system};
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

            buildInputs = with pkgs; [ rustc rustfmt clippy sqlx-cli ];

            nativeBuildInputs = nixpkgs.lib.optionals pkgs.stdenv.isLinux (with pkgs; [
              pkg-config
            ]);

            RUST_SRC_PATH = pkgs.rustPlatform.rustLibSrc;
          };
      }) // {
    overlays.default = final: prev: {
      youmubot = final.callPackage ./package.nix {
        craneLib = inputs.crane.lib.${final.system};
      };
    };
    # module
    nixosModules.default = import ./module.nix;
  };
}

