# Existing Perf Data & Nix Derivations

## Discovered Resources

### perf_actual recordings
Location: `~/.local/share/nix-builder/cache/perf_actual/`

Language benchmark recordings:
- `coq_actual.perf.data`
- `haskell_actual.perf.data`
- `lua_actual.perf.data`
- `ocaml_actual.perf.data`
- `python_actual.perf.data`
- `ruby_actual.perf.data`
- `rust_actual.perf.data`
- `summary.txt`

Import with: `./scripts/import-perf-actual.sh`

### Nix Derivations

| Derivation | Purpose |
|---|---|
| `state-4-zkperf.drv` | zkPerf state derivation |
| `perf-stage0.drv` | Baseline perf recording |
| `perf-stage42.drv` | Optimized perf recording |
| `compare-perf-stage0-vs-stage42.drv` | Stage comparison |
| `perf-linux-6.18.{5,6,8}` | Linux perf tool versions |
| `perf-linux-6.19` | Latest perf tool |
| `gperftools-2.17.2` | Google perf tools |

### zkperf_proofs in Nix Store

**harbot-proof-system:**
- `conformal_witness.json` — Python ≅ Rust via Monster group conformal mapping
- `NO_OLD_CODE_MANIFEST.json` — Proof that no old code was read (iteration 4)

**onlyskills_integration:**
- `pipelight-dev_compiler_1.json`
- `pipelight-dev_optimizer_2.json`
- `pipelight-dev_analyzer_3.json`
- `pipelight-dev_transformer_4.json`
- `pipelight-dev_validator_5.json`
- `pipelight-dev_generator_6.json`
- `pipelight-dev_executor_7.json`
- `pipelight-dev_tracer_8.json`
- `pipelight-dev_profiler_9.json`

### Solana perf-libs
Location: `~/.local/share/nix-builder/cache/solflake/bin/perf-libs/`
- `libpoh-simd.so`, `libsigning.so`, `signing.so`
- CUDA 10.0/10.1/10.2 variants
