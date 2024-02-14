{ craneLib
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
craneLib.buildPackage {
  pname = "youmubot";
  version = "0.1.0";

  src = ./.;
  cargoExtraArgs = builtins.concatStringsSep " " ([ "--locked" "--package youmubot" ] ++ featureFlags);

  buildInputs = [
    openssl
  ];

  nativeBuildInputs = lib.optionals stdenv.isLinux [
    pkg-config
  ];

  SQLX_OFFLINE = "true";
}

