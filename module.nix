youmubot: { config, pkgs, lib, ... }:

with lib;
let
  cfg = config.services.youmubot;
in
{
  options.services.youmubot = {
    enable = mkEnableOption "Enable youmubot, the discord bot made with Rust.";
    package = mkOption {
      type = types.package;
      default = youmubot;
    };

    envFile = mkOption {
      type = types.path;
      description = "Path to the environment variable file, for secrets like TOKEN and OSU_API_KEY.";
    };

    prefixes = mkOption {
      type = types.listOf types.str;
      default = [ "y!" "y2!" ];
      description = "The prefixes that the bot will listen on";
    };

    databasePath = mkOption {
      type = types.str;
      default = "/var/lib/youmubot";
      description = "The path to the database directory";
    };
  };

  config = mkIf cfg.enable {
    # systemd unit
    systemd.services.youmubot = {
      after = [ "network.target" ];
      wantedBy = [ "multi-user.target" ];

      description = "the discord bot made with Rust";
      documentation = [ "https://github.com/natsukagami/youmubot" ];

      script = "${cfg.package}/bin/youmubot";

      environment = {
        DBPATH = cfg.databasePath;
        SQLPATH = cfg.databasePath + "/youmubot.db";
        PREFIX = lib.strings.concatStringsSep "," cfg.prefixes;
      };

      serviceConfig = {
        DynamicUser = true;

        WorkingDirectory = "/var/lib/youmubot";

        StateDirectory = "youmubot";

        EnvironmentFile = cfg.envFile;

        # Strict sandboxing. You have no reason to trust code written by strangers from GitHub.
        PrivateTmp = true;
        ProtectHome = true;
        ProtectSystem = "strict";
        ProtectKernelTunables = true;
        ProtectHostname = true;
        ProtectClock = true;
        ProtectControlGroups = true;
        RestrictAddressFamilies = "AF_UNIX AF_INET AF_INET6";

        # Additional sandboxing. You need to disable all of these options
        # for privileged helper binaries (for system auth) to work correctly.
        NoNewPrivileges = true;
        PrivateDevices = true;
        DeviceAllow = "/dev/syslog";
        RestrictSUIDSGID = true;
        ProtectKernelModules = true;
        MemoryDenyWriteExecute = true;
        RestrictNamespaces = true;
        RestrictRealtime = true;
        LockPersonality = true;

        Restart = "always";
      };
    };
  };
}
