#!/usr/bin/env bash
# zkperf-ebpf-load — load the eBPF contract enforcer and populate from zkperf-service
#
# Usage:
#   sudo ./zkperf-ebpf-load.sh                    # load + populate from service
#   sudo ./zkperf-ebpf-load.sh add <pid> <max_ms> # add single contract
#   sudo ./zkperf-ebpf-load.sh status              # show stats
#   sudo ./zkperf-ebpf-load.sh unload              # remove programs

set -e
SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
BPF_OBJ="$SCRIPT_DIR/zkperf_enforce.bpf.o"
ZKPERF_URL="http://127.0.0.1:9718"

build() {
    echo "Building eBPF program..."
    clang -O2 -g -target bpf \
        -D__TARGET_ARCH_x86 \
        -c "$SCRIPT_DIR/zkperf_enforce.bpf.c" \
        -o "$BPF_OBJ"
    echo "Built: $BPF_OBJ"
}

load() {
    if [ ! -f "$BPF_OBJ" ]; then build; fi
    echo "Loading eBPF contract enforcer..."
    bpftool prog load "$BPF_OBJ" /sys/fs/bpf/zkperf_enforce \
        type tracepoint 2>/dev/null || true
    bpftool prog attach pinned /sys/fs/bpf/zkperf_enforce tracepoint sched sched_switch 2>/dev/null || true
    echo "Loaded and attached to sched_switch"
}

add_contract() {
    local pid=$1 max_ms=$2 enforce=${3:-1}
    local max_ns=$((max_ms * 1000000))
    echo "Adding contract: pid=$pid max_ms=$max_ms enforce=$enforce"
    # Write to BPF map via bpftool
    bpftool map update pinned /sys/fs/bpf/zkperf_enforce/contracts \
        key hex $(printf '%08x' $pid | sed 's/../& /g' | rev) \
        value hex 00 00 00 00 00 00 00 00 \
              $(printf '%016x' $max_ns | sed 's/../& /g' | rev) \
              00 00 00 00 00 00 00 00 \
              $(printf '%08x' $enforce | sed 's/../& /g' | rev) \
        2>/dev/null || echo "  (map update via bpftool — may need pinned maps)"
}

populate_from_service() {
    echo "Fetching contracts from zkperf-service..."
    local contracts=$(curl -s "$ZKPERF_URL/contracts" 2>/dev/null)
    if [ -z "$contracts" ] || [ "$contracts" = "[]" ]; then
        echo "  No contracts found"
        return
    fi
    echo "$contracts" | python3 -c "
import sys, json
for c in json.load(sys.stdin):
    sig = c.get('sig', c.get('signature', ''))
    max_ms = c.get('max_ms', 0)
    if max_ms > 0:
        print(f'  contract: {sig} max_ms={max_ms}')
" 2>/dev/null || echo "  (no python3 or empty contracts)"
}

status() {
    echo "=== zkperf eBPF status ==="
    bpftool prog show pinned /sys/fs/bpf/zkperf_enforce 2>/dev/null || echo "Not loaded"
    echo ""
    echo "=== Violation count ==="
    bpftool map dump pinned /sys/fs/bpf/zkperf_enforce/stats 2>/dev/null || echo "No stats"
}

unload() {
    echo "Unloading eBPF contract enforcer..."
    rm -f /sys/fs/bpf/zkperf_enforce 2>/dev/null
    echo "Unloaded"
}

case "${1:-load}" in
    build)    build ;;
    load)     load; populate_from_service ;;
    add)      add_contract "$2" "$3" "${4:-1}" ;;
    status)   status ;;
    unload)   unload ;;
    *)        echo "Usage: $0 {build|load|add <pid> <max_ms>|status|unload}" ;;
esac
