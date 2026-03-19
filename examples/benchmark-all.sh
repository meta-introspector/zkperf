#!/usr/bin/env bash
# Example: Record perf for all languages and generate a report
set -euo pipefail
cd "$(dirname "$0")/.."

echo "=== zkPerf: Full Language Benchmark ==="

for lang in rust python lua ruby; do
  ./scripts/record-language.sh "$lang"
done

./scripts/generate-report.sh recordings/ > proofs/benchmark_report.json
echo "=== Report ==="
cat proofs/benchmark_report.json
