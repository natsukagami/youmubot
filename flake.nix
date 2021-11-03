{
  description = "A discord bot for Dự Tuyển Tổng Hợp server";
  inputs = {
    naersk.url = "github:nix-community/naersk";
    flake-utils.url = "github:numtide/flake-utils";
  };
  outputs = { self, nixpkgs, naersk, flake-utils }: flake-utils.lib.eachDefaultSystem (system:
    let
      pkgs = nixpkgs.legacyPackages."${system}";
      naersk-lib = naersk.lib."${system}";
    in
    rec {
      packages.youmubot = naersk-lib.buildPackage {
        name = "youmubot";
        version = "0.1.0";

        root = ./.;
        cargoBuildOptions = opts: opts ++ [ "--package youmubot" ];

        nativeBuildInputs = nixpkgs.lib.optionals (nixpkgs.lib.strings.hasSuffix "linux" system) (with pkgs; [
          pkg-config
          openssl
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
      devShell = pkgs.mkShell {
        nativeBuildInputs = with pkgs; [ rustc cargo ];
      };

      # module
      nixosModule = import ./module.nix defaultPackage;
    });
}
