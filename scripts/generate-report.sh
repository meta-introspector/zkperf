#!/usr/bin/env bash
# Generate JSON report from all recordings in a directory
set -euo pipefail

DIR="${1:?Usage: generate-report.sh <recordings-dir>}"

echo '{"zkperf_report": {'
echo "  \"timestamp\": \"$(date -Iseconds)\","
echo '  "recordings": ['

FIRST=true
for f in "$DIR"/*.stat.txt; do
  [ -f "$f" ] || continue
  NAME=$(basename "$f" .stat.txt)
  CYCLES=$(grep -oP '[\d,]+\s+cycles' "$f" 2>/dev/null | grep -oP '[\d,]+' | tr -d ',' || echo "0")
  INSNS=$(grep -oP '[\d,]+\s+instructions' "$f" 2>/dev/null | grep -oP '[\d,]+' | tr -d ',' || echo "0")
  CMISS=$(grep -oP '[\d,]+\s+cache-misses' "$f" 2>/dev/null | grep -oP '[\d,]+' | tr -d ',' || echo "0")
  BMISS=$(grep -oP '[\d,]+\s+branch-misses' "$f" 2>/dev/null | grep -oP '[\d,]+' | tr -d ',' || echo "0")

  $FIRST || echo ","
  FIRST=false
  echo "    {\"name\": \"$NAME\", \"cycles\": $CYCLES, \"instructions\": $INSNS, \"cache_misses\": $CMISS, \"branch_misses\": $BMISS}"
done

echo '  ]'
echo '}}'
