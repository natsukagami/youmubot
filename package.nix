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
      "poise-0.6.1" = "sha256-iXyp9sR/vzPsexGPdRjfuKyFcGqvDdqiBAXnuw/HFo8=";
      "rosu-v2-0.11.0" = "sha256-DF/UDb7bnwn5ESBEfZ0iDOyf4c/9NsEDWL45BObeve4=";
      "rosu-pp-3.1.0" = "sha256-06nCeJreMVqDj89x31yHV5zz2kfJ9zXccafUbGuKDho=";
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
