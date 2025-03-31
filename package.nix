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
      "poise-0.6.1" = "sha256-44pPe02JJ97GEpzAXdQmDq/9bb4KS9G7ZFVlBRC6EYs=";
      "rosu-v2-0.9.0" = "sha256-dx0EwqqgkLaHwCPHyn5vMkhZ2NZcahH5SACFcsJKP1E=";
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
