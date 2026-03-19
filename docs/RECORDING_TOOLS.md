# Recording Tools

## Scripts

| Script | Purpose | Usage |
|---|---|---|
| `record-language.sh` | Record perf for language benchmarks | `./scripts/record-language.sh rust` |
| `record-http.sh` | Record perf for HTTP requests | `./scripts/record-http.sh https://example.com` |
| `record-nix-build.sh` | Record perf for nix builds | `./scripts/record-nix-build.sh .#default` |
| `compare-stages.sh` | Compare two perf recordings | `./scripts/compare-stages.sh a.perf.data b.perf.data` |
| `generate-report.sh` | Generate JSON report | `./scripts/generate-report.sh recordings/` |
| `import-perf-actual.sh` | Import existing perf_actual data | `./scripts/import-perf-actual.sh` |

## Makefile Targets

```bash
make record CMD="curl https://example.com"   # Record single command
make record-full CMD="./my-binary"            # Record perf + strace
make record-stat CMD="cargo build"            # Record stat counters
make record-all                               # Record all language benchmarks
make witness                                  # Generate ZK witness
make compare STAGE0=a.perf.data STAGE1=b.perf.data
make report                                   # Generate JSON report
```

## Nix Packages

```bash
nix run .#perf-record -- rust          # Record language benchmark
nix run .#perf-compare -- a.dat b.dat  # Compare stages
nix run .#perf-report -- recordings/   # Generate report
nix develop                            # Enter dev shell with all tools
```
