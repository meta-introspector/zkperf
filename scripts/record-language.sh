#!/usr/bin/env bash
# Record perf data for a language benchmark
set -euo pipefail

LANG="${1:?Usage: record-language.sh <language>}"
OUTDIR="${2:-recordings}"
mkdir -p "$OUTDIR"

PERF_OUT="$OUTDIR/${LANG}_actual.perf.data"
STRACE_OUT="$OUTDIR/${LANG}_actual.strace.log"
STAT_OUT="$OUTDIR/${LANG}_actual.stat.txt"

# Map language to benchmark command
case "$LANG" in
  rust)    CMD="cargo build --release" ;;
  python)  CMD="python3 -c 'sum(range(10**7))'" ;;
  haskell) CMD="ghc --version" ;;
  ocaml)   CMD="ocaml -version" ;;
  coq)     CMD="coqc --version" ;;
  lua)     CMD="lua -e 'for i=1,10^7 do end'" ;;
  ruby)    CMD="ruby -e '(1..10**7).sum'" ;;
  *)       CMD="$LANG" ;;
esac

echo "=== Recording $LANG: $CMD ==="

# perf stat counters
perf stat -e cycles,instructions,cache-references,cache-misses,branch-misses \
  -o "$STAT_OUT" -- sh -c "$CMD" 2>/dev/null || true

# perf record call graph
perf record -g -o "$PERF_OUT" -- sh -c "$CMD" 2>/dev/null || true

# strace with timing
strace -T -tt -o "$STRACE_OUT" -- sh -c "$CMD" 2>/dev/null || true

echo "=== $LANG recorded ==="
echo "  perf.data: $PERF_OUT"
echo "  strace:    $STRACE_OUT"
echo "  stat:      $STAT_OUT"
