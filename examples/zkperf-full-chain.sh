#!/usr/bin/env bash
# zkperf-full-chain.sh — Record a full reproducible witness chain
#
# From the need-for-introspection manifesto:
#   1. nix package of binaries
#   2. source + debug symbols for those binaries
#   3. traces produced with the binaries
#   4. models created from the traces
#   5. events leading up to binary creation
#
# This script captures ALL FIVE LAYERS for a given nix store path or command.
set -euo pipefail
cd "$(dirname "$0")/.."

TARGET="${1:?Usage: zkperf-full-chain.sh <nix-store-path-or-command> [source-dir]}"
SOURCE="${2:-.}"
OUTDIR="proofs/chain-$(date +%Y%m%d_%H%M%S)"
mkdir -p "$OUTDIR"

echo "=== zkPerf Full Chain Witness ==="
echo "Target: $TARGET"
echo "Source: $SOURCE"
echo "Output: $OUTDIR"

# --- Layer 1: Binary package (nix closure) ---
echo ""
echo "--- Layer 1: Binaries ---"
if [[ "$TARGET" == /nix/store/* ]]; then
  nix-store --query --requisites "$TARGET" > "$OUTDIR/1_closure.txt" 2>/dev/null || echo "$TARGET" > "$OUTDIR/1_closure.txt"
  CLOSURE_SIZE=$(wc -l < "$OUTDIR/1_closure.txt")
  CLOSURE_HASH=$(sha256sum "$OUTDIR/1_closure.txt" | cut -d' ' -f1)
  echo "  Closure: $CLOSURE_SIZE paths, hash: ${CLOSURE_HASH:0:16}"
else
  BINARY=$(which "$TARGET" 2>/dev/null || echo "$TARGET")
  echo "$BINARY" > "$OUTDIR/1_closure.txt"
  CLOSURE_HASH=$(sha256sum "$OUTDIR/1_closure.txt" | cut -d' ' -f1)
  echo "  Binary: $BINARY, hash: ${CLOSURE_HASH:0:16}"
fi

# --- Layer 2: Source + debug symbols ---
echo ""
echo "--- Layer 2: Source + Debug ---"
if [ -d "$SOURCE" ]; then
  find "$SOURCE" -name '*.rs' -o -name '*.py' -o -name '*.js' -o -name '*.sh' -o -name '*.nix' 2>/dev/null | \
    head -50 | while read -r f; do sha256sum "$f"; done > "$OUTDIR/2_source_hashes.txt" 2>/dev/null || true
  echo "  Source files: $(wc -l < "$OUTDIR/2_source_hashes.txt")"
fi

# Debug symbols
DEBUG_DIR="$HOME/.debug${TARGET}"
if [ -d "$DEBUG_DIR" ]; then
  find "$DEBUG_DIR" -type f | head -20 > "$OUTDIR/2_debug_symbols.txt"
  echo "  Debug symbols: $(wc -l < "$OUTDIR/2_debug_symbols.txt") files"
else
  echo "  Debug symbols: not found at $DEBUG_DIR"
  echo "none" > "$OUTDIR/2_debug_symbols.txt"
fi

# --- Layer 3: Traces ---
echo ""
echo "--- Layer 3: Traces ---"
if [[ "$TARGET" == /nix/store/* ]]; then
  # Trace the store path itself
  perf stat -e cycles,instructions,cache-misses \
    -o "$OUTDIR/3_perf_stat.txt" -- ls "$TARGET" 2>/dev/null || echo "perf unavailable" > "$OUTDIR/3_perf_stat.txt"
else
  # Trace the command
  perf stat -e cycles,instructions,cache-misses \
    -o "$OUTDIR/3_perf_stat.txt" -- $TARGET 2>/dev/null || echo "perf unavailable" > "$OUTDIR/3_perf_stat.txt"
fi
echo "  Perf stat: $OUTDIR/3_perf_stat.txt"

# Also capture strace summary
if [[ "$TARGET" != /nix/store/* ]]; then
  strace -c -o "$OUTDIR/3_strace_summary.txt" -- $TARGET 2>/dev/null || echo "strace unavailable" > "$OUTDIR/3_strace_summary.txt"
  echo "  Strace: $OUTDIR/3_strace_summary.txt"
fi

# --- Layer 4: Model (chain hash) ---
echo ""
echo "--- Layer 4: Model ---"
CHAIN_INPUT=$(cat "$OUTDIR"/1_*.txt "$OUTDIR"/2_*.txt "$OUTDIR"/3_*.txt 2>/dev/null)
CHAIN_HASH=$(echo "$CHAIN_INPUT" | sha256sum | cut -d' ' -f1)
cat > "$OUTDIR/4_model.json" <<EOF
{
  "chain_hash": "$CHAIN_HASH",
  "layers_hashed": ["1_closure", "2_source_hashes", "2_debug_symbols", "3_perf_stat"],
  "timestamp": "$(date -Iseconds)"
}
EOF
echo "  Chain hash: ${CHAIN_HASH:0:16}"

# --- Layer 5: Events / provenance ---
echo ""
echo "--- Layer 5: Events ---"
cat > "$OUTDIR/5_events.json" <<EOF
{
  "builder": "$USER",
  "hostname": "$(hostname)",
  "kernel": "$(uname -r)",
  "nix_version": "$(nix --version 2>/dev/null || echo unknown)",
  "perf_version": "$(perf version 2>/dev/null || echo unknown)",
  "git_rev": "$(git rev-parse HEAD 2>/dev/null || echo unknown)",
  "timestamp": "$(date -Iseconds)",
  "cwd": "$(pwd)"
}
EOF
echo "  Builder: $USER@$(hostname)"

# --- Final commitment ---
echo ""
COMMITMENT=$(cat "$OUTDIR"/*.json "$OUTDIR"/*.txt 2>/dev/null | sha256sum | cut -d' ' -f1)
echo "$COMMITMENT" > "$OUTDIR/COMMITMENT"

cat > "$OUTDIR/witness.json" <<EOF
{
  "target": "$TARGET",
  "source": "$SOURCE",
  "commitment": "$COMMITMENT",
  "chain_hash": "$CHAIN_HASH",
  "timestamp": "$(date -Iseconds)",
  "layers": {
    "1_binaries": "1_closure.txt",
    "2_source": "2_source_hashes.txt",
    "2_debug": "2_debug_symbols.txt",
    "3_traces": "3_perf_stat.txt",
    "4_model": "4_model.json",
    "5_events": "5_events.json"
  }
}
EOF

echo "=== Full Chain Witness Complete ==="
echo "  Commitment: $COMMITMENT"
echo "  Output: $OUTDIR/"
ls -la "$OUTDIR/"
