# zkperf WireGuard VPN — kagenti service mesh
#
# Subnet: 10.100.0.0/24
# Server: 10.100.0.1 (mdupont-G470)
# Client: 10.100.0.2 (GentsPC, moltis :57553)
#
# Secrets managed by sops-nix (PGP key A76A0CF9079EC60D)
{ config, lib, pkgs, ... }:

let
  cfg = config.services.zkperf-wireguard;
in {
  options.services.zkperf-wireguard = {
    enable = lib.mkEnableOption "zkperf WireGuard VPN for kagenti mesh";

    listenPort = lib.mkOption {
      type = lib.types.port;
      default = 51820;
    };

    serverAddress = lib.mkOption {
      type = lib.types.str;
      default = "10.100.0.1/24";
    };

    privateKeyFile = lib.mkOption {
      type = lib.types.path;
      description = "Path to sops-decrypted server private key";
    };

    clientPublicKey = lib.mkOption {
      type = lib.types.str;
      description = "Public key of GentsPC client";
    };

    presharedKeyFile = lib.mkOption {
      type = lib.types.path;
      description = "Path to sops-decrypted preshared key";
    };
  };

  config = lib.mkIf cfg.enable {
    networking.wireguard.interfaces.wg-kagenti = {
      ips = [ cfg.serverAddress ];
      listenPort = cfg.listenPort;
      privateKeyFile = cfg.privateKeyFile;

      peers = [{
        # GentsPC — Windows client running moltis v0.9.10 on :57553
        publicKey = cfg.clientPublicKey;
        presharedKeyFile = cfg.presharedKeyFile;
        allowedIPs = [ "10.100.0.2/32" ];
        persistentKeepalive = 25;
      }];
    };

    networking.firewall.allowedUDPPorts = [ cfg.listenPort ];
  };
}
