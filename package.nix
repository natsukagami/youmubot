{ rustPlatform
, lib
, stdenv
, pkg-config
, openssl

, enableCodeforces ? false
, enableOsu ? true
, ...
}:
let
  customizeFeatures = !(enableCodeforces && enableOsu);
in
rustPlatform.buildRustPackage {
  pname = "youmubot";
  version = "0.1.0";

  src = ./.;
  cargoLock = {
    lockFile = ./Cargo.lock;
    outputHashes = {
      "poise-0.6.1" = "sha256-iXyp9sR/vzPsexGPdRjfuKyFcGqvDdqiBAXnuw/HFo8=";
      "rosu-map-0.2.0" = "sha256-muCaRn/9qBxr2SJJ8F2HL9se4SLCL9ttI06tCoFfaNg=";
      "rosu-v2-0.10.0" = "sha256-44J/gnlsADel8+P3qjIIvA6Rdlt3D4F5hPwW7HEV2js=";
      "rosu-pp-2.0.0" = "sha256-1xQR6b7CFLaBECdbZcF5vflESCQjCtEiJNH7mmTkLl4=";
    };
  };

  buildNoDefaultFeatures = customizeFeatures;
  buildFeatures = lib.optionals customizeFeatures (
    [ "core" ]
    ++ lib.optional enableCodeforces "codeforces"
    ++ lib.optional enableOsu "osu"
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
