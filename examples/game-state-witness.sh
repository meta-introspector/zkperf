#!/usr/bin/env bash
# Example: Reproduce the state-4-zkperf derivation pattern
# Records perf stat on a game state transition and generates a commitment
set -euo pipefail
cd "$(dirname "$0")/.."
mkdir -p proofs

STATE='{"round":5,"action":"warp","from":71,"to":99}'
COMMITMENT=$(echo "$STATE" | sha256sum | cut -d' ' -f1)

echo "=== zkPerf: Game State Witness ==="
echo "State: $STATE"
echo "Commitment: $COMMITMENT"

# Record perf stat on the state echo (like the original derivation)
perf stat -e cycles,instructions,cache-misses \
  -o proofs/example-state.perf.txt \
  -- echo "$STATE" > proofs/example-state.json 2>/dev/null || \
  echo "$STATE" > proofs/example-state.json

echo "$COMMITMENT" > proofs/example-state.commitment

echo "=== Output ==="
cat proofs/example-state.json
echo "Commitment: $(cat proofs/example-state.commitment)"
[ -f proofs/example-state.perf.txt ] && cat proofs/example-state.perf.txt
