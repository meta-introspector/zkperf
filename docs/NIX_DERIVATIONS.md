# Nix Derivation Archaeology

## state-4-zkperf

A game state derivation that uses `perf stat` to witness a consensus round.

**Derivation:** `/nix/store/qq2rrpndzda0nvm6wc2mafmk1vfhrgqh-state-4-zkperf.drv`
**Output:** `/nix/store/slnid5pk8zci6xvszn4y306wpzhbvpyy-state-4-zkperf`
**Uses:** `perf-linux-6.19`

### Perf Counters (actual)
| Counter | Value |
|---|---|
| cycles | 1,903,710 |
| instructions | 2,759,085 |
| cache-misses | 17,079 |
| wall time | 0.050s |

### State
- Round 4 consensus with 3 ships (alpha, beta, gamma)
- Quorum: 2 votes, winner: "2a"
- Last move: alpha warped from sector 42 → 71
- Commitment: `fab3ece438dd78f2cdcd645b984885ebd24ed4c3f8c4f2bfca87f56fd63a59d7`
- zkperf commitment: `2c36552088ccdd6ddf4195ae4d775c20de194b58693b1e2f2bd423ccf9b2b09c`

## perf-stage0

Deterministic perf trace from `trinity-stage0` derivation hash.

**Derivation:** `/nix/store/wr7lkqzym252vac0q010hxbns1w4mdal-perf-stage0.drv`
**Output:** `/nix/store/k4rdljk1ads93dirjsq6l0ii2fmwhfin-perf-stage0`
**Traces:** `/nix/store/sk10fl1gb7n3msvkshw4cv10qad1722d-trinity-stage0`

Method: sha256 hash of derivation → deterministic cycles/instructions/cache-misses.

## perf-stage42

Same method for `trinity-stage42`.

**Derivation:** `/nix/store/isxy5z5cccw7pz6d6h6ljzdk7b95kz25-perf-stage42.drv`
**Output:** `/nix/store/klzq1iy7fvxk8jamw7px6j3p8g79ky4a-perf-stage42`
**Traces:** `/nix/store/zx1p2zvi75yhlghfixvgpz1yv79s1rp3-trinity-stage42`

## compare-perf-stage0-vs-stage42

Compares stage0 and stage42 traces using `jq` and `bc`.

**Derivation:** `/nix/store/0jkcc9zfhbjc8bb1szihc2xwcxfhg56m-compare-perf-stage0-vs-stage42.drv`
**Output:** `/nix/store/swhm94py7n5wxg3dyx3avhm2199lxnvz-compare-perf-stage0-vs-stage42`

Computes:
- Cycle/instruction/cache-miss differences
- Percentage difference
- Equivalence within 1% tolerance

## perf_actual (7 language benchmarks)

**Source:** `/nix/store/i5wapivd5l39vmjsqlnzihhk07ss6zyl-all-language-perf/`
**Collected:** Jan 21, 2026

| Language | Size |
|---|---|
| coq | 23K |
| haskell | 78K |
| lua | 21K |
| ocaml | 35K |
| python | 23K |
| ruby | 27K |
| rust | 37K |

## Solana perf-libs

**Location:** `~/.local/share/nix-builder/cache/solflake/bin/perf-libs/`
- `libpoh-simd.so` — Proof of History SIMD
- `libsigning.so` / `signing.so` — Transaction signing
- CUDA 10.0/10.1/10.2 GPU variants
