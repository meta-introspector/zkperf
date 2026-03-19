#!/usr/bin/env bash
# Example: Analyze imported perf recordings and generate a summary
set -euo pipefail
cd "$(dirname "$0")/.."

echo "=== zkPerf: Analyze Imported Recordings ==="

echo "Imported perf_actual summary:"
cat recordings/perf_actual_summary.txt 2>/dev/null || echo "(no summary)"
echo ""

echo "Recording sizes:"
for f in recordings/*.perf.data; do
  [ -f "$f" ] || continue
  NAME=$(basename "$f" .perf.data)
  SIZE=$(stat -c%s "$f")
  HASH=$(sha256sum "$f" | cut -c1-16)
  echo "  $NAME: ${SIZE} bytes, hash: $HASH"
done

echo ""
echo "Proof artifacts:"
for f in proofs/*.json; do
  [ -f "$f" ] || continue
  NAME=$(basename "$f")
  echo "  $NAME: $(jq -r '.claim // .statement // .stage0 // "proof"' "$f" 2>/dev/null)"
done
