{
  rustPlatform,
  lib,
  stdenv,
  pkg-config,
  openssl,

  enableCodeforces ? false,
  enableOsu ? true,
  ...
}:
let
  customizeFeatures = !(enableCodeforces && enableOsu);
in
rustPlatform.buildRustPackage {
  pname = "youmubot";
  version = "0.1.0";

  src = lib.cleanSourceWith {
    filter = name: type: !(type == "directory" && (baseNameOf (toString name)) == ".github");
    src = lib.cleanSource ./.;
  };
  cargoLock = {
    lockFile = ./Cargo.lock;
    outputHashes = {
      "rosu-v2-0.11.0" = "sha256-q4zacirIDyHPx8trp0HvfITBR02Of7usFjLooIczAOI=";
    };
  };

  buildNoDefaultFeatures = customizeFeatures;
  buildFeatures = lib.optionals customizeFeatures (
    [ "core" ] ++ lib.optional enableCodeforces "codeforces" ++ lib.optional enableOsu "osu"
  );

  cargoBuildFlags = [
    "--locked"
    "--package"
    "youmubot"
  ];

  buildInputs = [
    openssl
  ];

  nativeBuildInputs = lib.optionals stdenv.isLinux [
    pkg-config
  ];

  SQLX_OFFLINE = "true";
}
