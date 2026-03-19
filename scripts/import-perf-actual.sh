#!/usr/bin/env bash
# Import existing perf_actual data into zkperf recordings
set -euo pipefail

SRC="${1:-$HOME/.local/share/nix-builder/cache/perf_actual}"
DEST="${2:-recordings}"
mkdir -p "$DEST"

if [ ! -d "$SRC" ]; then
  echo "Source not found: $SRC"
  exit 1
fi

echo "=== Importing perf_actual data from $SRC ==="

for f in "$SRC"/*.perf.data; do
  [ -f "$f" ] || continue
  NAME=$(basename "$f")
  cp -v "$f" "$DEST/$NAME"
done

[ -f "$SRC/summary.txt" ] && cp -v "$SRC/summary.txt" "$DEST/perf_actual_summary.txt"

echo "=== Import complete ==="
ls -la "$DEST"
