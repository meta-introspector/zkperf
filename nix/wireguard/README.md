# zkperf WireGuard VPN — kagenti service mesh
#
# Secrets managed by sops with age key at ~/.config/sops/age/keys.txt
# Age public key: age1qauw7gyzue9psh4rkyedg567e9qywtd6l7hmpgcqs3mml4mmyalq4292rj
#
# Usage:
#   make keygen       — generate new WireGuard keys, encrypt with sops
#   make show         — display public keys (safe)
#   make client-conf  — print full Windows client config (secrets!)
#   make decrypt      — dump all secrets (careful)
#
# Network:
#   Subnet:  10.100.0.0/24
#   Server:  10.100.0.1 (mdupont-G470, 192.168.68.62:51820)
#   Client:  10.100.0.2 (GentsPC, moltis :57553)
#
# Files:
#   .sops.yaml              — sops creation rules (age key)
#   secrets/wireguard.yaml  — encrypted keys (safe to commit)
#   module.nix              — NixOS WireGuard module
#   configuration.nix       — example NixOS config with sops-nix
#   client-gentspc.conf.template — Windows client template (no secrets)
#   Makefile                — key management commands
