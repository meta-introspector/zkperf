# erdfa-publish Integration

## Overview

[erdfa-publish](erdfa-publish/) is linked as part of the zkPerf output. It provides:

- **Semantic UI components as CBOR shards** — content-addressed, renderable anywhere
- **Conformal Field Tower (CFT)** — multi-scale text decomposition (post → paragraph → line → token → emoji → bytes)
- **DA51 CBOR shards** — every node and edge is content-addressed

## Connection to zkPerf

zkPerf proofs and witness data can be published as erdfa CBOR shards:

1. **Perf recordings** → CFT decomposition → CBOR shards with content-addressed IDs
2. **Proof artifacts** → Semantic components (tables, trees) → accessible rendering
3. **Witness commitments** → Shard IDs derived from sha256 content hashes

## Usage

```bash
# Build erdfa-publish
cd erdfa-publish && cargo build --release

# CLI: encode a shard
./target/release/erdfa-cli encode --input proofs/state-4-zkperf.json --output shards/

# From zkperf: publish proof as CBOR shard
cargo run --manifest-path erdfa-publish/Cargo.toml --bin erdfa-cli -- encode \
  --input proofs/conformal_witness.json \
  --output erdfa-publish/shards/
```

## Structure

```
erdfa-publish/
├── src/
│   ├── lib.rs          # Component types, Shard, CID generation
│   ├── render.rs       # HTML/tar/CLI renderers
│   ├── cft.rs          # Conformal Field Tower decomposition
│   └── bin/erdfa-cli.rs
├── examples/
│   ├── demo.rs         # Basic component demo
│   ├── cft_demo.rs     # CFT text decomposition
│   └── render_tar.rs   # Render shards to tar archive
├── shards/             # Pre-built CBOR shards
├── flake.nix
└── Cargo.toml
```
