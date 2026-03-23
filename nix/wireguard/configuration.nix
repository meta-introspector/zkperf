# Example NixOS configuration importing the WireGuard module with sops-nix
#
# Add to your system flake inputs:
#   sops-nix.url = "github:Mic92/sops-nix";
#
# Then in nixosConfigurations:
{ config, pkgs, ... }:

{
  imports = [
    ./module.nix
    # sops-nix module (from flake input)
  ];

  sops.secrets."wireguard/server_private_key" = {
    sopsFile = ./secrets/wireguard.yaml;
    key = "server_private_key";
    owner = "root";
    mode = "0400";
  };

  sops.secrets."wireguard/preshared_key" = {
    sopsFile = ./secrets/wireguard.yaml;
    key = "preshared_key";
    owner = "root";
    mode = "0400";
  };

  services.zkperf-wireguard = {
    enable = true;
    privateKeyFile = config.sops.secrets."wireguard/server_private_key".path;
    presharedKeyFile = config.sops.secrets."wireguard/preshared_key".path;
    # Set after running: make show
    clientPublicKey = "TyS7Yg7rsSDwmNnFJ+xD23k5jLSGRXuM81R1xs4PRQ4=";
  };
}
