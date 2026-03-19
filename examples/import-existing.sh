#!/usr/bin/env bash
# Example: Import existing data and compare stages
set -euo pipefail
cd "$(dirname "$0")/.."

echo "=== zkPerf: Import & Compare ==="
./scripts/import-perf-actual.sh
echo ""
echo "Imported recordings:"
ls -la recordings/*.perf.data 2>/dev/null || echo "No perf data found"
