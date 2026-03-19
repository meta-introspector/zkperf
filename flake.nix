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

        # Perf recording derivations
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
      });
}
