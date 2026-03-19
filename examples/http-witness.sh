#!/usr/bin/env bash
# Example: Record HTTP witness and compare with baseline
set -euo pipefail
cd "$(dirname "$0")/.."

URL="${1:-https://example.com}"

echo "=== zkPerf: HTTP Witness Example ==="
./scripts/record-http.sh "$URL"
echo ""
echo "Witness data in recordings/"
ls -la recordings/http_*
