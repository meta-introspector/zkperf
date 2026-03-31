# zkPerf: Zero-Knowledge Performance Monitoring

**Tagline:** Witness the performance, prove the truth

## Overview

zkPerf is a decentralized performance monitoring system that uses:
- **perf** traces to extract system behavior
- **Zero-knowledge proofs** to verify observations
- **Side-channel analysis** to reveal hidden state
- **zkELF signatures** to prove code complexity
- **zkStego** to hide proofs in HTTP traffic
- **P2P witness network** for distributed consensus

## Components

### 1. zkELF - ELF Binary Signatures
Wrap each `.text` section with ZK proofs of computational complexity.

### 2. mod_zkrs - Kernel Module
Rust kernel module for deep performance observation and witness extraction.

### 3. zkStego - Steganographic Protocol
Hide ZK proofs in HTTP headers, timing, and whitespace (HTTPZ protocol).

### 4. Witness Network
Distributed nodes submit performance attestations to Solana blockchain.

## Key Insight

**Performance records reveal more than HTTP status codes.**

Running `perf` on `curl` exposes:
- CPU cycle patterns
- Cache timing (reveals server load)
- TLS handshake signatures
- Memory allocation patterns
- **Loop iteration counts** (covert channel!)
- Branch prediction patterns

These become **unforgeable proofs** of system state.

## Use Cases

1. **Distributed Monitoring** - DAO-operated sentinel nodes
2. **Side-Channel Key Extraction** - Witness crypto operations
3. **Code Complexity Proofs** - Verify O(n) claims
4. **Censorship-Resistant Communication** - zkStego over HTTP
5. **Performance Contracts** - Guarantee execution bounds

## Related Projects

- [SOLFUNMEME](https://github.com/meta-introspector/solfunmeme) - Main project
- [Introspector LLC](https://github.com/meta-introspector/introspector-llc) - First zkML NFT DAO LLC

## Quick Start

```bash
nix develop                                    # Enter dev shell
make build                                     # Build zkperf
./scripts/import-perf-actual.sh                # Import 7 language benchmarks
./scripts/record-http.sh https://example.com   # Record HTTP witness
./examples/stage-comparison.sh                 # Compare perf stages
make report                                    # Generate JSON report
```

## Examples

| Example | Pattern | Language |
|---|---|---|
| `examples/zkperf-chain.py` | Build full 5-layer provenance chain | Python |
| `examples/zkperf-verify.js` | Verify chain commitment + layers | JavaScript |
| `examples/zkperf-full-chain.sh` | Record complete witness chain | Bash |
| `examples/analyze-recordings.sh` | Analyze all imported recordings and proofs | Bash |
| `examples/benchmark-all.sh` | Record perf for multiple languages | Bash |
| `examples/component-proofs.sh` | Pipelight-style component proofs | Bash |
| `examples/conformal-witness.sh` | Cross-language conformal mapping witness | Bash |
| `examples/game-state-witness.sh` | Consensus game state with perf commitment | Bash |
| `examples/http-witness.sh` | HTTP request witness with perf + strace | Bash |
| `examples/import-existing.sh` | Import perf_actual data from nix cache | Bash |
| `examples/stage-comparison.sh` | Compare two perf stages (stage0 vs stage42) | Bash |

## Imported Data

- **7 language benchmarks** — coq, haskell, lua, ocaml, python, ruby, rust
- **state-4-zkperf** — Consensus game state (1.9M cycles, 2.7M instructions)
- **harbot proofs** — Conformal witness, no-old-code manifest
- **pipelight proofs** — 10 component proofs (parser → profiler)
- **[erdfa-publish](erdfa-publish/)** — Semantic CBOR shard publisher with CFT decomposition

## Documentation

- [The Need for Introspection](docs/NEED_FOR_INTROSPECTION.md) ← **start here**
- [Nix Derivation Archaeology](docs/NIX_DERIVATIONS.md)
- [Existing Data Catalog](docs/EXISTING_DATA.md)
- [Recording Tools Reference](docs/RECORDING_TOOLS.md)
- [zkperf Stream v1](docs/ZKPERF_STREAM_V1.md)
- [eRDFa Integration](docs/ERDFA_INTEGRATION.md)
- [Sample Trace Compression](docs/SAMPLE_TRACE_COMPRESSION.md)
- [Proof Artifacts](proofs/README.md)
- [CRQ-002: zkPerf Specification](../CRQ-002-introspector.md)
- [zkELF: ELF Signatures](../ZKELF.md)
- [zkStego: Steganographic Protocol](../ZKSTEGO.md)
- [Witness System](../WITNESS_SYSTEM.md)

## Sample Trace Utility

For normalized sample-trace JSON, `zkperf` now has a small standalone
projection/reconstruction codec:

```bash
python3 scripts/compact-sample-trace.py stats input.json compact.json \
  --roundtrip-output roundtrip.json
```

This utility keeps only the raw generating fields needed to reconstruct the
derived trace surface exactly. It is meant for normalized downstream
sample-trace JSON, not as a replacement for raw `perf.data` or DA51 shards.

The important property is exact round-trip:

- compact payload keeps the canonical generating fields
- derived matrix/annotation fields are dropped
- decode reconstructs the normalized trace contract exactly

## Python Stream Tooling

`zkperf` now also carries a bounded Python lane under `python/zkperf_stream` for:

- building `zkperf-stream/v1` fixtures and tar bundles
- maintaining latest/index contracts with retain-latest-n retention
- publishing stream artifacts to HF and resolving them back from HF or IPFS
- rendering register-aware and flow-aware spectrograms

Focused validation runs with:

```bash
PYTHONPATH=python pytest python/tests/test_zkperf_stream.py python/tests/test_zkperf_viz.py
```

## License

AGPL-3.0

For commercial Apache 2.0 licensing, contact: https://github.com/meta-introspector/introspector-llc
