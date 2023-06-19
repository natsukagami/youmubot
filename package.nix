{ naersk
, lib
, stdenv
, pkg-config
, openssl

, enableCodeforces ? true
, enableOsu ? true
, ...
}:
let
  customizeFeatures = !(enableCodeforces && enableOsu);
  featureFlags = lib.optionals customizeFeatures (
    [ "--no-default-features" "--features=core" ]
    ++ lib.optional enableCodeforces "--features=codeforces"
    ++ lib.optional enableOsu "--features=osu"
  );
in
naersk.buildPackage {
  name = "youmubot";
  version = "0.1.0";

  root = ./.;
  cargoBuildOptions = opts: opts ++ [ "--package youmubot" ] ++ featureFlags;

  buildInputs = [
    openssl
  ];

  nativeBuildInputs = lib.optionals stdenv.isLinux [
    pkg-config
  ];

  SQLX_OFFLINE = "true";
}

