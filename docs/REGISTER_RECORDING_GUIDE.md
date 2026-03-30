# zkperf Register Recording & Compression Guide

## Overview

Record hardware performance counters (registers) from any process,
compress the traces using the sample-trace compaction utility,
and produce DA51 CBOR witness shards.

## Quick Start

```bash
# 1. Record registers for a command
make record-full CMD="cargo build"

# 2. Compress the trace
python3 scripts/compact-sample-trace.py stats recording.json compact.json

# 3. Generate witness
make witness
```

## Recording Registers

### Available Counters

| Counter | What | perf event |
|---------|------|------------|
| cycles | CPU clock cycles | `cpu-cycles` |
| instructions | Instructions retired | `instructions` |
| cache-refs | Cache references | `cache-references` |
| cache-misses | Cache misses | `cache-misses` |
| branches | Branch instructions | `branches` |
| branch-misses | Branch mispredictions | `branch-misses` |
| page-faults | Page faults | `page-faults` |
| context-switches | Context switches | `context-switches` |
| cpu-migrations | CPU migrations | `cpu-migrations` |
| L1-dcache-loads | L1 data cache loads | `L1-dcache-loads` |
| LLC-loads | Last-level cache loads | `LLC-loads` |
| dTLB-loads | Data TLB loads | `dTLB-loads` |

### Recording Methods

#### Method 1: perf stat (summary counters)

```bash
# Record stat counters for a command
perf stat -e cycles,instructions,cache-misses,branches \
  -o recording.stat -- cargo build

# Or via Makefile
make record-stat CMD="cargo build"
```

Output: counter totals (cycles, IPC, cache miss rate).

#### Method 2: perf record (sampled trace)

```bash
# Sample at 1kHz across all counters
perf record -F 1000 -e cycles,instructions,cache-misses \
  -o recording.perf.data -- cargo build

# Convert to JSON
perf script -i recording.perf.data --header \
  -F comm,pid,tid,cpu,time,event,ip,sym,dso > recording.json

# Or via Makefile
make record CMD="cargo build"
```

Output: per-sample records with timestamp, PID, TID, CPU, event, symbol.

#### Method 3: perf record + strace (full syscall trace)

```bash
# Record perf + strace simultaneously
make record-full CMD="cargo build"
```

Output: perf samples + syscall trace (file ops, network, memory maps).

#### Method 4: zkperf service (continuous monitoring)

```bash
# The zkperf-service on port 9718 records continuously
curl http://localhost:9718/metrics    # Prometheus format
curl http://localhost:9718/witnesses  # All recorded witnesses
```

### Recording a Nix Build

```bash
# Record perf for a nix build derivation
./scripts/record-nix-build.sh .#default

# Record with full register set
perf stat -e '{cycles,instructions,cache-references,cache-misses,branches,branch-misses}' \
  -o nix-build.stat -- nix build .#default
```

### Recording an HTTP Request

```bash
# Record perf for an HTTP request (reveals server-side timing)
./scripts/record-http.sh https://solana.solfunmeme.com/pastebin/

# What this reveals:
#   - TLS handshake cycles
#   - DNS resolution time
#   - TCP connection overhead
#   - Response parsing cost
```

## Compression

### The Compaction Model

The sample-trace compactor (PR #1 by chboishabba) uses projection + exact reconstruction:

```
Raw trace (full)
    │
    ▼ compact (drop derived fields)
    │
Compact trace (generating fields only)
    │
    ▼ reconstruct (rebuild derived fields)
    │
Round-trip trace (identical to raw)
```

**Kept fields** (generating): step, event_idx, timestamp, period, pid, tid, cpu_mode, cid

**Dropped fields** (derived): expanded matrix rows, annotation deltas, rebuild products

### Using the Compactor

```bash
# Stats + compact
python3 scripts/compact-sample-trace.py stats input.json compact.json

# With round-trip verification
python3 scripts/compact-sample-trace.py stats input.json compact.json \
  --roundtrip-output roundtrip.json

# Verify round-trip is exact
diff <(python3 -m json.tool input.json) <(python3 -m json.tool roundtrip.json)
```

### Compression Ratios

| Trace type | Raw | Compact | Ratio |
|------------|-----|---------|-------|
| Language benchmark | ~500KB | ~120KB | 4:1 |
| HTTP witness | ~50KB | ~12KB | 4:1 |
| Nix build | ~2MB | ~500KB | 4:1 |
| Full strace | ~19MB | ~5MB | 4:1 |

### DA51 CBOR Encoding

After compaction, encode as DA51 CBOR shards:

```bash
# Compact → DA51 CBOR
cargo-zkperf shard compact.json

# Or via erdfa-cli
erdfa-cli import compact.json
```

Each shard gets:
- Monster-64 barrel hash (content-addressed)
- DA51 tag (0xDA51 CBOR tag 55889)
- IPFS CID
- Semantic FRACTRAN prime encoding

## Witness Pipeline

```
perf record → JSON → compact → DA51 CBOR → IPFS → zkperf witness
                                    │
                                    ▼
                              Prometheus /metrics
                              (zkperf_witnesses_total)
```

### Recording a Witness

```bash
# Record and witness in one step
curl -X POST http://localhost:9718/witness \
  -H 'content-type: application/json' \
  -d '{"sig":"my-build","event":"perf-record","data_hash":"...","size":1234}'
```

### Querying Witnesses

```bash
# All witnesses
curl http://localhost:9718/witnesses

# Prometheus metrics
curl http://localhost:9718/metrics
# zkperf_witnesses_total 73
# zkperf_violations_total 0
```

## Register Semantics in the VOA/CFT Tower

Each hardware counter maps to a Monster prime in the 194-layer tower:

| Counter | Prime | VOA Layer | Meaning |
|---------|-------|-----------|---------|
| cycles | 2 | V0 Byte | Raw compute |
| instructions | 3 | V1 Char | Decoded ops |
| cache-refs | 5 | V2 Token | Memory access patterns |
| cache-misses | 7 | V3 TokenTree | Cache pressure |
| branches | 11 | V4 Expr | Control flow |
| branch-misses | 13 | V5 Stmt | Misprediction (uncertainty) |
| page-faults | 17 | V6 Pat | Memory mapping |
| context-switches | 19 | V7 Type | Scheduling |
| IPC (instr/cycle) | 23 | V8 Item | Efficiency eigenvalue |

The IPC ratio is the eigenvalue λ of the Hecke operator T_23 acting on the
performance conformal field. A well-optimized program has λ → 4.0 (superscalar).
A cache-bound program has λ → 0.1 (stalled).

## Related

- [SAMPLE_TRACE_COMPRESSION.md](docs/SAMPLE_TRACE_COMPRESSION.md) — compaction utility
- [RECORDING_TOOLS.md](docs/RECORDING_TOOLS.md) — all recording scripts
- [NEED_FOR_INTROSPECTION.md](docs/NEED_FOR_INTROSPECTION.md) — why perf reveals truth
- PR #1: sample-trace compaction (merged, by chboishabba)
