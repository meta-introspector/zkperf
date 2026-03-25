{
  description = "zkPerf - Zero-Knowledge Performance Monitoring";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    flake-utils.url = "github:numtide/flake-utils";
    rust-overlay = {
      url = "github:oxalica/rust-overlay";
      inputs.nixpkgs.follows = "nixpkgs";
    };
  };

  outputs = { self, nixpkgs, flake-utils, rust-overlay }:
    flake-utils.lib.eachDefaultSystem (system:
      let
        overlays = [ (import rust-overlay) ];
        pkgs = import nixpkgs { inherit system overlays; };
        rustToolchain = pkgs.rust-bin.stable.latest.default;
      in {
        packages.default = pkgs.rustPlatform.buildRustPackage {
          pname = "zkperf";
          version = "0.1.0";
          src = ./.;
          cargoLock.lockFile = ./Cargo.lock;
        };

        # Perf witness: perf stat on the built binary inside nix build
        packages.zkperf-witness = pkgs.runCommand "zkperf-witness" {
          buildInputs = [ self.packages.${system}.default pkgs.linuxPackages.perf ];
        } ''
          mkdir -p $out
          perf stat -e cycles,instructions,cache-misses,branch-misses \
            -o $out/perf.txt -- zkperf-witness > $out/stdout.txt 2>&1 || true
          sha256sum ${self.packages.${system}.default}/bin/zkperf-witness > $out/binary_hash.txt
          cat > $out/witness.json <<EOF
          {
            "binary": "${self.packages.${system}.default}",
            "perf": "$(cat $out/perf.txt | tr '\n' '|')",
            "binary_hash": "$(cat $out/binary_hash.txt | cut -d' ' -f1)",
            "timestamp": "$(date -Iseconds)"
          }
          EOF
          echo "$(sha256sum $out/witness.json | cut -d' ' -f1)" > $out/commitment
        '';

        # Perf trace: hash-derived deterministic trace (like perf-stage0/stage42)
        packages.zkperf-trace = pkgs.runCommand "zkperf-trace" {
          buildInputs = [ pkgs.coreutils ];
        } ''
          mkdir -p $out
          HASH=$(sha256sum ${self.packages.${system}.default}/bin/zkperf-witness | cut -d' ' -f1)
          CYCLES=$((16#''${HASH:0:6} % 1000000))
          INST=$((16#''${HASH:6:6} % 2000000))
          MISS=$((16#''${HASH:12:6} % 10000))
          cat > $out/trace.json <<EOF
          {
            "name": "zkperf",
            "drv": "${self.packages.${system}.default}",
            "cycles": $CYCLES,
            "instructions": $INST,
            "cache_misses": $MISS,
            "hash": "$HASH"
          }
          EOF
          cat > $out/trace.txt <<EOF
          Perf trace for: zkperf
          Binary: ${self.packages.${system}.default}/bin/zkperf-witness
          Cycles: $CYCLES
          Instructions: $INST
          Cache-misses: $MISS
          Hash: $HASH
          EOF
        '';

        # Perf record: full call-graph recording inside nix build
        # Pattern from: perf-lib.nix, bash-perf-build.nix, mes-perf-recorder
        packages.zkperf-record = pkgs.runCommand "zkperf-record" {
          buildInputs = [
            self.packages.${system}.default
            pkgs.linuxPackages.perf
            pkgs.coreutils
          ];
        } ''
          mkdir -p $out

          # perf record: count-based sampling, call-graph dwarf, full registers, detailed CPU
          perf record \
            -o $out/zkperf.perf.data \
            -g \
            --call-graph dwarf,65528 \
            --user-regs=AX,BX,CX,DX,SI,DI,BP,SP,IP,FLAGS,R8,R9,R10,R11,R12,R13,R14,R15 \
            -e cycles:u,instructions:u,cache-misses:u,branch-misses:u \
            -c 100 \
            -- zkperf-witness 2>$out/record_stderr.txt || true

          # perf report --stdio
          perf report -i $out/zkperf.perf.data --stdio \
            > $out/perf-report.txt 2>&1 || true

          # perf script for full trace
          perf script -i $out/zkperf.perf.data -F comm,pid,tid,cpu,time,event,ip,sym,dso,symoff,srcline,iregs \
            > $out/perf-script.txt 2>&1 || true

          # perf annotate hottest symbols
          perf annotate -i $out/zkperf.perf.data --stdio \
            > $out/perf-annotate.txt 2>&1 || true

          # witness hashes
          sha256sum $out/zkperf.perf.data > $out/witness-hash.txt 2>/dev/null || true
          sha256sum ${self.packages.${system}.default}/bin/zkperf-witness >> $out/witness-hash.txt

          echo "zkperf-record complete" > $out/status.txt
          ls -lh $out/ >> $out/status.txt
        '';

        # 3rd build: analyze the perf recording
        packages.zkperf-analyze = pkgs.runCommand "zkperf-analyze" {
          buildInputs = [
            pkgs.linuxPackages.perf
            pkgs.jq
            pkgs.coreutils
          ];
        } ''
          mkdir -p $out
          RECORD=${self.packages.${system}.zkperf-record}

          # Full report (all events)
          perf report -f -i $RECORD/zkperf.perf.data --stdio --no-children \
            > $out/full-report.txt 2>&1 || true

          # Split per event using awk
          for evt in cycles instructions cache-misses branch-misses; do
            awk "/Event '$evt:u'/,/^$/" $out/full-report.txt > $out/top-$evt.txt 2>/dev/null
            [ ! -s $out/top-$evt.txt ] && grep -A 50 "$evt" $out/full-report.txt > $out/top-$evt.txt 2>/dev/null || true
          done

          # Extract header info
          perf report -f -i $RECORD/zkperf.perf.data --header-only \
            > $out/header.txt 2>&1 || true

          # Build JSON analysis
          cat > $out/analysis.json <<ENDJSON
          {
            "source": {
              "binary": "${self.packages.${system}.default}",
              "record": "$RECORD",
              "record_drv": "${self.packages.${system}.zkperf-record.drvPath}"
            },
            "events": ["cycles:u", "instructions:u", "cache-misses:u", "branch-misses:u"],
            "perf_data_hash": "$(sha256sum $RECORD/zkperf.perf.data | cut -d' ' -f1)",
            "binary_hash": "$(sha256sum ${self.packages.${system}.default}/bin/zkperf-witness | cut -d' ' -f1)"
          }
          ENDJSON

          # Hash the full analysis
          sha256sum $out/analysis.json > $out/commitment.txt
          for f in $out/top-*.txt $out/header.txt; do
            sha256sum "$f" >> $out/commitment.txt
          done
        '';

        # Export full chain as NAR + eRDFa witness
        packages.zkperf-export = pkgs.runCommand "zkperf-export" {
          buildInputs = [
            pkgs.gnutar
            pkgs.jq
            pkgs.coreutils
          ];
        } ''
          mkdir -p $out/nar $out/erdfa

          BINARY=${self.packages.${system}.default}
          RECORD=${self.packages.${system}.zkperf-record}
          ANALYZE=${self.packages.${system}.zkperf-analyze}
          WITNESS=${self.packages.${system}.zkperf-witness}

          # Export each artifact as tar archive (NAR requires nix daemon)
          tar cf $out/nar/zkperf-binary.nar -C $BINARY .
          tar cf $out/nar/zkperf-record.nar -C $RECORD .
          tar cf $out/nar/zkperf-analyze.nar -C $ANALYZE .
          tar cf $out/nar/zkperf-witness.nar -C $WITNESS .

          # Hash all archives
          for f in $out/nar/*.nar; do
            sha256sum "$f"
          done > $out/nar/hashes.txt

          # List closure contents
          ls -la $BINARY/bin/  > $out/nar/binary-closure.txt
          ls -la $RECORD/     > $out/nar/record-closure.txt
          ls -la $ANALYZE/    > $out/nar/analyze-closure.txt

          # eRDFa witness: each artifact as a sheaf section
          BINARY_HASH=$(sha256sum $out/nar/zkperf-binary.nar | cut -d' ' -f1)
          RECORD_HASH=$(sha256sum $out/nar/zkperf-record.nar | cut -d' ' -f1)
          ANALYZE_HASH=$(sha256sum $out/nar/zkperf-analyze.nar | cut -d' ' -f1)
          CHAIN_HASH=$(cat $out/nar/hashes.txt | sha256sum | cut -d' ' -f1)

          cat > $out/erdfa/witness.html <<ENDHTML
          <!DOCTYPE html>
          <html prefix="erdfa: https://erdfa.org/ns# dasl: https://dasl.org/ns#">
          <head><title>zkPerf Witness Chain</title></head>
          <body>
          <h1>zkPerf Full Chain Witness</h1>

          <div typeof="erdfa:SheafSection" about="#binary">
            <meta property="erdfa:layer" content="1_binaries" />
            <meta property="erdfa:store_path" content="$BINARY" />
            <meta property="erdfa:nar_hash" content="$BINARY_HASH" />
            <meta property="erdfa:encoding" content="nar" />
            <link rel="erdfa:artifact" href="nar/zkperf-binary.nar" />
          </div>

          <div typeof="erdfa:SheafSection" about="#record">
            <meta property="erdfa:layer" content="3_traces" />
            <meta property="erdfa:store_path" content="$RECORD" />
            <meta property="erdfa:nar_hash" content="$RECORD_HASH" />
            <meta property="erdfa:encoding" content="nar" />
            <meta property="erdfa:events" content="cycles:u,instructions:u,cache-misses:u,branch-misses:u" />
            <link rel="erdfa:artifact" href="nar/zkperf-record.nar" />
            <link rel="erdfa:source" href="#binary" />
          </div>

          <div typeof="erdfa:SheafSection" about="#analyze">
            <meta property="erdfa:layer" content="4_model" />
            <meta property="erdfa:store_path" content="$ANALYZE" />
            <meta property="erdfa:nar_hash" content="$ANALYZE_HASH" />
            <meta property="erdfa:encoding" content="nar" />
            <link rel="erdfa:artifact" href="nar/zkperf-analyze.nar" />
            <link rel="erdfa:source" href="#record" />
          </div>

          <div typeof="erdfa:SheafSection dasl:Type3" about="#chain">
            <meta property="erdfa:layer" content="5_commitment" />
            <meta property="erdfa:chain_hash" content="$CHAIN_HASH" />
            <meta property="dasl:addr" content="0x''${CHAIN_HASH:0:16}" />
            <link rel="erdfa:contains" href="#binary" />
            <link rel="erdfa:contains" href="#record" />
            <link rel="erdfa:contains" href="#analyze" />
          </div>

          </body></html>
          ENDHTML

          # JSON witness
          cat > $out/erdfa/witness.json <<ENDJSON
          {
            "chain_hash": "$CHAIN_HASH",
            "layers": {
              "1_binaries": {"store_path": "$BINARY", "nar_hash": "$BINARY_HASH"},
              "3_traces":   {"store_path": "$RECORD", "nar_hash": "$RECORD_HASH"},
              "4_model":    {"store_path": "$ANALYZE", "nar_hash": "$ANALYZE_HASH"}
            },
            "closures": {
              "binary":  $(wc -l < $out/nar/binary-closure.txt),
              "record":  $(wc -l < $out/nar/record-closure.txt),
              "analyze": $(wc -l < $out/nar/analyze-closure.txt)
            }
          }
          ENDJSON

          echo "$CHAIN_HASH" > $out/COMMITMENT
        '';

        devShells.default = pkgs.mkShell {
          buildInputs = [
            rustToolchain
            pkgs.linuxPackages.perf
            pkgs.strace
            pkgs.gnumake
            pkgs.jq
            pkgs.gperftools
          ];
          shellHook = ''
            echo "🎵 zkPerf dev shell"
            echo "  perf: $(perf version 2>/dev/null || echo 'needs root')"
            echo "  rust: $(rustc --version)"
          '';
        };

        # Nix-packaged scripts
        packages.perf-record = pkgs.writeShellApplication {
          name = "zkperf-record";
          runtimeInputs = [ pkgs.linuxPackages.perf pkgs.strace pkgs.jq ];
          text = builtins.readFile ./scripts/record-language.sh;
        };

        packages.perf-compare = pkgs.writeShellApplication {
          name = "zkperf-compare";
          runtimeInputs = [ pkgs.linuxPackages.perf pkgs.jq ];
          text = builtins.readFile ./scripts/compare-stages.sh;
        };

        packages.perf-report = pkgs.writeShellApplication {
          name = "zkperf-report";
          runtimeInputs = [ pkgs.jq ];
          text = builtins.readFile ./scripts/generate-report.sh;
        };

        # === kagenti-selinux: SELinux + DNS + IPv6 + systemd for all agents ===

        packages.kagenti-selinux = pkgs.runCommand "kagenti-selinux" {} ''
          mkdir -p $out/{services,hardened,selinux,dns,ipv6,lean,zkp}

          # New agent .service files
          cp ${./data/kagenti-generated/services}/*.service $out/services/

          # Hardened existing .service files
          cp ${./data/selinux-hardened}/*.service $out/hardened/

          # SELinux policies
          cp ${./data/kagenti-generated/kagenti_agents.te} $out/selinux/
          cp ${./data/selinux-merged/policy.te} $out/selinux/zkperf_monster.te

          # DNS zone
          cp ${./data/kagenti-generated/kagenti.zone} $out/dns/

          # IPv6 netplan
          cp ${./data/kagenti-generated/99-kagenti.yaml} $out/ipv6/

          # Lean4 proof
          cp ${./data/selinux-merged/MonsterPolicy.lean} $out/lean/

          # ZKP witness
          cp ${./data/selinux-merged/zkp-witness.json} $out/zkp/

          # Agent manifest
          cp ${./data/kagenti-generated/agents.json} $out/

          # Install script
          cat > $out/install.sh << 'INSTALL'
          #!/usr/bin/env bash
          set -euo pipefail
          DIR="$(cd "$(dirname "$0")" && pwd)"
          echo "🛡️  kagenti-selinux install from $DIR"

          echo "📦 systemd units (new)..."
          for f in "$DIR"/services/*.service; do
            sudo cp "$f" /etc/systemd/system/
            echo "   ✅ $(basename $f)"
          done

          echo "🛡️  systemd units (hardened)..."
          for f in "$DIR"/hardened/*.service; do
            sudo cp "$f" /etc/systemd/system/
            echo "   🛡️  $(basename $f)"
          done

          sudo systemctl daemon-reload

          echo "📡 DNS zone → /etc/bind/kagenti.zone"
          sudo cp "$DIR"/dns/kagenti.zone /etc/bind/ 2>/dev/null || echo "   ⏭️  bind9 not found"

          echo "🌐 IPv6 netplan → /etc/netplan/99-kagenti.yaml"
          sudo cp "$DIR"/ipv6/99-kagenti.yaml /etc/netplan/ 2>/dev/null || echo "   ⏭️  netplan not found"

          if command -v checkmodule &>/dev/null; then
            echo "🔒 SELinux policies..."
            for te in "$DIR"/selinux/*.te; do
              mod="''${te%.te}.mod"
              pp="''${te%.te}.pp"
              checkmodule -M -m -o "$mod" "$te"
              semodule_package -o "$pp" -m "$mod"
              sudo semodule -i "$pp"
              echo "   ✅ $(basename $te)"
            done
          fi

          echo ""
          echo "✅ Installed. ZKP witness:"
          cat "$DIR"/zkp/zkp-witness.json | head -5
          INSTALL
          chmod +x $out/install.sh
        '';

        packages.kagenti-selinux-install = pkgs.writeShellApplication {
          name = "kagenti-selinux-install";
          runtimeInputs = [ pkgs.jq ];
          text = ''
            exec ${self.packages.${system}.kagenti-selinux}/install.sh "$@"
          '';
        };
      });
}
