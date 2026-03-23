.PHONY: keygen encrypt decrypt show client-conf clean

SOPS := sops
WG := nix-shell -p wireguard-tools --run

SECRETS_DIR := secrets
SECRETS_FILE := $(SECRETS_DIR)/wireguard.yaml

# VPN subnet: 10.100.0.0/24
# Server: 10.100.0.1 (mdupont-G470, 192.168.68.62)
# Client: 10.100.0.2 (GentsPC, moltis :57553)
# Listen port: 51820
# Secrets: age key at ~/.config/sops/age/keys.txt

keygen: $(SECRETS_DIR)
	@$(WG) "SERVER_KEY=\$$(wg genkey); \
	SERVER_PUB=\$$(echo \$$SERVER_KEY | wg pubkey); \
	CLIENT_KEY=\$$(wg genkey); \
	CLIENT_PUB=\$$(echo \$$CLIENT_KEY | wg pubkey); \
	PSK=\$$(wg genpsk); \
	printf 'server_private_key: %s\nserver_public_key: %s\nclient_private_key: %s\nclient_public_key: %s\npreshared_key: %s\n' \
	  \$$SERVER_KEY \$$SERVER_PUB \$$CLIENT_KEY \$$CLIENT_PUB \$$PSK > $(SECRETS_FILE)"
	$(SOPS) --encrypt --in-place $(SECRETS_FILE)
	@echo "Encrypted to $(SECRETS_FILE)"

$(SECRETS_DIR):
	mkdir -p $(SECRETS_DIR)

decrypt:
	$(SOPS) --decrypt $(SECRETS_FILE)

show:
	@$(SOPS) --decrypt $(SECRETS_FILE) | grep "_public_"

client-conf:
	@$(SOPS) --decrypt $(SECRETS_FILE) | nix-shell -p python3 --run "python3 -c \"\
	import sys; d=dict(l.strip().split(': ',1) for l in sys.stdin if ': ' in l); \
	print('[Interface]'); \
	print('PrivateKey =',d['client_private_key']); \
	print('Address = 10.100.0.2/24'); \
	print('DNS = 1.1.1.1'); \
	print(); \
	print('[Peer]'); \
	print('PublicKey =',d['server_public_key']); \
	print('PresharedKey =',d['preshared_key']); \
	print('Endpoint = 192.168.68.62:51820'); \
	print('AllowedIPs = 10.100.0.0/24'); \
	print('PersistentKeepalive = 25')\""

clean:
	rm -rf $(SECRETS_DIR)
