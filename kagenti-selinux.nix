{
  description = "kagenti-selinux — SELinux policies, DNS, IPv6, systemd units for all kagenti agents";

  inputs.nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";

  outputs = { self, nixpkgs }:
    let
      system = "x86_64-linux";
      pkgs = nixpkgs.legacyPackages.${system};

      # Read generated agent data
      agentsJson = builtins.fromJSON (builtins.readFile ./data/kagenti-generated/agents.json);

      # SELinux policy build
      selinuxPolicy = pkgs.runCommand "kagenti-selinux-policy" {
        buildInputs = [ pkgs.checkpolicy pkgs.policycoreutils ];
      } ''
        mkdir -p $out
        cp ${./data/kagenti-generated/kagenti_agents.te} $out/kagenti_agents.te
        cp ${./data/selinux-merged/policy.te} $out/zkperf_monster.te
        # Compile both modules
        for te in $out/*.te; do
          mod="''${te%.te}.mod"
          pp="''${te%.te}.pp"
          checkmodule -M -m -o "$mod" "$te" || true
          semodule_package -o "$pp" -m "$mod" || true
        done
      '';

      # DNS zone file
      dnsZone = pkgs.writeText "kagenti.zone" (builtins.readFile ./data/kagenti-generated/kagenti.zone);

      # IPv6 netplan
      ipv6Netplan = pkgs.writeText "99-kagenti.yaml" (builtins.readFile ./data/kagenti-generated/99-kagenti.yaml);

      # All systemd units
      serviceFiles = builtins.listToAttrs (map (agent: {
        name = agent.name;
        value = pkgs.writeText "${agent.name}.service"
          (builtins.readFile (./data/kagenti-generated/services + "/${agent.name}.service"));
      }) agentsJson);

      # Hardened versions of existing services
      hardenedFiles = builtins.attrNames (builtins.readDir ./data/selinux-hardened);
      hardenedServices = map (f:
        pkgs.writeText f (builtins.readFile (./data/selinux-hardened + "/${f}"))
      ) hardenedFiles;

      # Lean4 proof
      leanProof = pkgs.writeText "MonsterPolicy.lean"
        (builtins.readFile ./data/selinux-merged/MonsterPolicy.lean);

      # ZKP witness
      zkpWitness = pkgs.writeText "zkp-witness.json"
        (builtins.readFile ./data/selinux-merged/zkp-witness.json);

      # Install script
      installScript = pkgs.writeShellScriptBin "kagenti-selinux-install" ''
        set -euo pipefail
        echo "🛡️  kagenti-selinux installer"
        echo ""

        # SELinux
        if command -v semodule &>/dev/null; then
          echo "📦 Installing SELinux policies..."
          for pp in ${selinuxPolicy}/*.pp; do
            sudo semodule -i "$pp" && echo "   ✅ $(basename $pp)"
          done
        else
          echo "⏭️  SELinux not available, skipping"
        fi

        # Systemd units (new agents)
        echo "📦 Installing systemd units..."
        for svc in ${self.packages.${system}.services}/*.service; do
          name=$(basename "$svc")
          sudo cp "$svc" "/etc/systemd/system/$name"
          echo "   ✅ $name"
        done

        # Hardened units (existing agents)
        echo "📦 Installing hardened units..."
        for svc in ${self.packages.${system}.hardened}/*.service; do
          name=$(basename "$svc")
          sudo cp "$svc" "/etc/systemd/system/$name"
          echo "   🛡️  $name"
        done

        sudo systemctl daemon-reload

        # DNS
        echo "📡 Installing DNS zone..."
        sudo cp ${dnsZone} /etc/bind/kagenti.zone 2>/dev/null || \
          echo "   ⏭️  bind9 not found, zone file at ${dnsZone}"

        # IPv6
        echo "🌐 Installing IPv6 netplan..."
        sudo cp ${ipv6Netplan} /etc/netplan/99-kagenti.yaml 2>/dev/null || \
          echo "   ⏭️  netplan not found, file at ${ipv6Netplan}"

        echo ""
        echo "✅ Done. Commitment: $(cat ${zkpWitness} | ${pkgs.jq}/bin/jq -r .commitment)"
        echo "📄 Lean4 proof: ${leanProof}"
      '';

    in {
      packages.${system} = {
        default = installScript;

        # Individual components
        selinux = selinuxPolicy;
        dns = dnsZone;
        ipv6 = ipv6Netplan;
        lean = leanProof;
        witness = zkpWitness;

        services = pkgs.runCommand "kagenti-services" {} ''
          mkdir -p $out
          ${builtins.concatStringsSep "\n" (map (agent:
            "cp ${serviceFiles.${agent.name}} $out/${agent.name}.service"
          ) agentsJson)}
        '';

        hardened = pkgs.runCommand "kagenti-hardened" {} ''
          mkdir -p $out
          ${builtins.concatStringsSep "\n" (map (f:
            "cp ${./data/selinux-hardened}/${f} $out/${f}"
          ) hardenedFiles)}
        '';
      };

      # NixOS module
      nixosModules.default = { config, lib, pkgs, ... }: {
        # SELinux policies
        environment.etc = builtins.listToAttrs (map (agent: {
          name = "systemd/system/${agent.name}.service";
          value.source = serviceFiles.${agent.name};
        }) agentsJson);

        # DNS
        services.bind = lib.mkIf config.services.bind.enable {
          extraConfig = builtins.readFile dnsZone;
        };

        # IPv6 addresses
        networking.interfaces.eth0.ipv6.addresses = map (agent: {
          address = agent.ipv6;
          prefixLength = 128;
        }) agentsJson;
      };

      # Apps
      apps.${system}.default = {
        type = "app";
        program = "${installScript}/bin/kagenti-selinux-install";
      };
    };
}
