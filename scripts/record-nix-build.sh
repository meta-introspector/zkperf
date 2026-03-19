#!/usr/bin/env bash
# Record perf for a nix build and extract witness data
set -euo pipefail

DRV="${1:?Usage: record-nix-build.sh <derivation-or-flake-ref>}"
OUTDIR="${2:-recordings}"
mkdir -p "$OUTDIR"

SLUG=$(echo "$DRV" | sed 's|[/:#.]|_|g' | tail -c 60)
PERF_OUT="$OUTDIR/nix_${SLUG}.perf.data"
STAT_OUT="$OUTDIR/nix_${SLUG}.stat.txt"
LOG_OUT="$OUTDIR/nix_${SLUG}.build.log"

echo "=== Recording nix build: $DRV ==="

# perf stat on nix build
perf stat -e cycles,instructions,cache-misses,branch-misses \
  -o "$STAT_OUT" -- nix build "$DRV" --log-format raw 2>"$LOG_OUT" || true

# perf record on nix build (rebuild)
nix build "$DRV" --rebuild 2>/dev/null &
BUILD_PID=$!
perf record -g -o "$PERF_OUT" -p "$BUILD_PID" 2>/dev/null || true
wait "$BUILD_PID" 2>/dev/null || true

echo "=== Nix build recorded ==="
echo "  stat: $STAT_OUT"
echo "  perf: $PERF_OUT"
echo "  log:  $LOG_OUT"
