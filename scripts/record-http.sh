#!/usr/bin/env bash
# Record perf for an HTTP request (curl) with witness extraction
set -euo pipefail

URL="${1:?Usage: record-http.sh <url>}"
OUTDIR="${2:-recordings}"
mkdir -p "$OUTDIR"

SLUG=$(echo "$URL" | sed 's|https\?://||;s|[/:]|_|g' | head -c 60)
PERF_OUT="$OUTDIR/http_${SLUG}.perf.data"
STRACE_OUT="$OUTDIR/http_${SLUG}.strace.log"
STAT_OUT="$OUTDIR/http_${SLUG}.stat.txt"
RESULT_OUT="$OUTDIR/http_${SLUG}.result.json"

echo "=== Recording HTTP witness for $URL ==="

# perf stat
perf stat -e cycles,instructions,cache-misses,branch-misses \
  -o "$STAT_OUT" -- curl -sS -o /dev/null -w '%{http_code}|%{time_total}|%{remote_ip}' "$URL" > "$OUTDIR/.curl_result" 2>/dev/null || true

# perf record
perf record -g -o "$PERF_OUT" -- curl -sS -o /dev/null "$URL" 2>/dev/null || true

# strace
strace -T -tt -e trace=network -o "$STRACE_OUT" -- curl -sS -o /dev/null "$URL" 2>/dev/null || true

# Parse curl result
IFS='|' read -r HTTP_CODE TIME_TOTAL REMOTE_IP < "$OUTDIR/.curl_result" 2>/dev/null || true

cat > "$RESULT_OUT" <<EOF
{
  "url": "$URL",
  "http_status": ${HTTP_CODE:-0},
  "response_time": ${TIME_TOTAL:-0},
  "remote_ip": "${REMOTE_IP:-unknown}",
  "perf_data": "$PERF_OUT",
  "strace_log": "$STRACE_OUT",
  "stat_file": "$STAT_OUT",
  "timestamp": "$(date -Iseconds)"
}
EOF

rm -f "$OUTDIR/.curl_result"
echo "=== HTTP witness recorded ==="
cat "$RESULT_OUT"
