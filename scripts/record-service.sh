#!/usr/bin/env bash
# record-service.sh — Dynamic profiling of a systemd service
# Usage: ./record-service.sh <service-name> [duration-seconds]
# Output: data/bench-<service>-<timestamp>/
set -euo pipefail

SERVICE=$1
DURATION=${2:-30}
TS=$(date +%s)
OUT="data/bench-${SERVICE}-${TS}"
mkdir -p "$OUT"

echo "🔬 Profiling $SERVICE for ${DURATION}s → $OUT"

# Get PID
PID=$(systemctl show -p MainPID "$SERVICE" | cut -d= -f2)
if [ "$PID" = "0" ]; then
  echo "❌ $SERVICE not running (PID=0). Start it first."
  exit 1
fi
echo "📌 PID: $PID"

# 1. strace — file + network syscalls
echo "📝 strace (file+network)..."
sudo strace -p "$PID" -f -e trace=file,network \
  -o "$OUT/strace.log" &
STRACE_PID=$!

# 2. perf record — CPU cycles
echo "📊 perf record..."
sudo perf record -p "$PID" -g -o "$OUT/perf.data" -- sleep "$DURATION" &
PERF_PID=$!

# 3. SELinux audit (if enabled)
if command -v ausearch &>/dev/null; then
  echo "🛡️  SELinux audit..."
  AUDIT_START=$(date +%H:%M:%S)
fi

# Wait for perf to finish (it sleeps $DURATION)
wait $PERF_PID 2>/dev/null || true

# Stop strace
sudo kill $STRACE_PID 2>/dev/null || true
wait $STRACE_PID 2>/dev/null || true

# Collect SELinux denials
if [ -n "${AUDIT_START:-}" ]; then
  sudo ausearch -m avc -ts "$AUDIT_START" 2>/dev/null > "$OUT/avc.log" || true
fi

# 4. Parse strace → access.json
echo "🔍 Parsing traces..."
python3 - "$OUT/strace.log" "$OUT/access.json" "$SERVICE" << 'PYEOF'
import re, json, sys
from collections import defaultdict

strace_file, out_file, svc_name = sys.argv[1], sys.argv[2], sys.argv[3]
files_read, files_write, sockets = set(), set(), set()

with open(strace_file) as f:
    for line in f:
        # open/openat read
        m = re.search(r'open(?:at)?\(.*?"([^"]+)".*?O_RDONLY', line)
        if m: files_read.add(m.group(1))
        # open/openat write
        m = re.search(r'open(?:at)?\(.*?"([^"]+)".*?O_(?:WRONLY|RDWR|CREAT)', line)
        if m: files_write.add(m.group(1))
        # connect
        m = re.search(r'connect\(.*?sin_port=htons\((\d+)\)', line)
        if m: sockets.add(int(m.group(1)))
        # stat/access
        m = re.search(r'(?:stat|access)\("([^"]+)"', line)
        if m: files_read.add(m.group(1))

result = {
    "service": svc_name,
    "files_read": sorted(files_read),
    "files_write": sorted(files_write),
    "network_ports": sorted(sockets),
    "network": bool(sockets),
}
with open(out_file, "w") as f:
    json.dump(result, f, indent=2)
print(f"   {len(files_read)} reads, {len(files_write)} writes, {len(sockets)} ports")
PYEOF

# 5. perf report summary
echo "📈 perf summary..."
sudo perf report -i "$OUT/perf.data" --stdio --no-children 2>/dev/null \
  | head -30 > "$OUT/perf-summary.txt"

# 6. Commitment
COMMIT=$(sha256sum "$OUT/access.json" | cut -c1-16)
echo "$COMMIT" > "$OUT/commitment.txt"

echo ""
echo "✅ Done: $OUT"
echo "🔗 Commitment: $COMMIT"
echo ""
echo "Files:"
ls -la "$OUT/"
