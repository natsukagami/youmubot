{
  description = "A discord bot for Dự Tuyển Tổng Hợp server";
  inputs = {
    nixpkgs.url = "github:nixos/nixpkgs/nixos-22.05";
    nixpkgs-unstable.url = "github:nixos/nixpkgs";
    naersk.url = "github:nix-community/naersk";
    naersk.inputs.nixpkgs.follows = "nixpkgs";
    nixpkgs-mozilla = {
      url = github:mozilla/nixpkgs-mozilla;
      flake = false;
    };
    flake-utils.url = "github:numtide/flake-utils";
  };
  outputs = { self, nixpkgs, nixpkgs-unstable, naersk, flake-utils, nixpkgs-mozilla }: flake-utils.lib.eachDefaultSystem (system:
    let
      pkgs = import nixpkgs { inherit system; overlays = [ (import nixpkgs-mozilla) ]; };
      pkgs-unstable = import nixpkgs-unstable { inherit system; };

      rust-toolchain = (pkgs.rustChannelOf {
        channel = "1.65.0";
        sha256 = "sha256-DzNEaW724O8/B8844tt5AVHmSjSQ3cmzlU4BP90oRlY=";
      });

      naersk' = pkgs.callPackage naersk {
        cargo = rust-toolchain.rust;
        rustc = rust-toolchain.rust;
      };
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
            with rust-toolchain; [ rust ]
              ++ (with pkgs-unstable; [ rustfmt ])
              ++ nixpkgs.lib.optionals pkgs.stdenv.isDarwin (with pkgs; [
              libiconv
              darwin.apple_sdk.frameworks.Security
            ])
              ++ (with pkgs; [
              openssl
            ]);

          nativeBuildInputs = nixpkgs.lib.optionals pkgs.stdenv.isLinux (with pkgs; [
            pkg-config
          ]);

          shellHook = ''
            export RUST_SRC_PATH="${rust-toolchain.rust-src}/lib/rustlib/src/rust/library";
          '';
        };
      # module
      nixosModule = import ./module.nix defaultPackage;
    });
}

