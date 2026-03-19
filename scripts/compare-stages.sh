#!/usr/bin/env bash
# Compare two perf recording stages
set -euo pipefail

STAGE0="${1:?Usage: compare-stages.sh <stage0.perf.data> <stage1.perf.data>}"
STAGE1="${2:?}"
OUTDIR="${3:-proofs}"
mkdir -p "$OUTDIR"

echo '{"comparison": {' > "$OUTDIR/comparison.json"

for STAGE in "$STAGE0" "$STAGE1"; do
  NAME=$(basename "$STAGE" .perf.data)
  echo "=== Analyzing $NAME ==="
  perf report -i "$STAGE" --stdio --header 2>/dev/null | head -30
  perf report -i "$STAGE" --stdio 2>/dev/null | \
    grep -E '^\s+[0-9]+\.[0-9]+%' | head -10 > "$OUTDIR/${NAME}_hotspots.txt"
done

# Extract cycle counts for comparison
C0=$(perf report -i "$STAGE0" --stdio --header 2>/dev/null | grep -oP 'sample_freq\s*:\s*\K[0-9]+' || echo "0")
C1=$(perf report -i "$STAGE1" --stdio --header 2>/dev/null | grep -oP 'sample_freq\s*:\s*\K[0-9]+' || echo "0")

cat > "$OUTDIR/comparison.json" <<EOF
{
  "stage0": "$(basename "$STAGE0")",
  "stage1": "$(basename "$STAGE1")",
  "stage0_freq": $C0,
  "stage1_freq": $C1,
  "timestamp": "$(date -Iseconds)"
}
EOF

echo "=== Comparison saved to $OUTDIR/comparison.json ==="
