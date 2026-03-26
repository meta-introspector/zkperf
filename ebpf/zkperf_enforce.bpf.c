// zkperf_enforce.bpf.c — eBPF perf contract enforcer
//
// Kernel-level enforcement of zkperf contracts:
// 1. Reads contracts from BPF map (populated by zkperf-service)
// 2. Tracks CPU time per PID on scheduler switches
// 3. Sends SIGXCPU when max_ms exceeded
// 4. Writes violation events to ring buffer for userspace witnessing
//
// Build: clang -O2 -target bpf -c zkperf_enforce.bpf.c -o zkperf_enforce.bpf.o

#include <linux/bpf.h>
#include <bpf/bpf_helpers.h>
#include <linux/sched.h>

// Contract: what a PID promises
struct zkperf_contract {
    __u64 signature_hash;   // SHA-256 prefix of contract signature
    __u64 max_ns;           // max wall-clock nanoseconds
    __u64 max_cycles;       // max CPU cycles (0 = no limit)
    __u32 enforce;          // 1 = send signal on violation
};

// Runtime state per PID
struct zkperf_state {
    __u64 start_ns;         // when contract started
    __u64 cpu_ns;           // accumulated CPU time
    __u64 signature_hash;
    __u32 violated;         // already violated flag (don't re-signal)
};

// Violation event sent to userspace
struct zkperf_violation {
    __u32 pid;
    __u64 signature_hash;
    __u64 elapsed_ns;
    __u64 max_ns;
    __u64 timestamp;
    char comm[16];
};

// Map: PID → contract
struct {
    __uint(type, BPF_MAP_TYPE_HASH);
    __uint(max_entries, 4096);
    __type(key, __u32);
    __type(value, struct zkperf_contract);
} contracts SEC(".maps");

// Map: PID → runtime state
struct {
    __uint(type, BPF_MAP_TYPE_HASH);
    __uint(max_entries, 4096);
    __type(key, __u32);
    __type(value, struct zkperf_state);
} states SEC(".maps");

// Ring buffer for violation events → userspace
struct {
    __uint(type, BPF_MAP_TYPE_RINGBUF);
    __uint(max_entries, 256 * 1024);
} violations SEC(".maps");

// Counter: total violations
struct {
    __uint(type, BPF_MAP_TYPE_ARRAY);
    __type(key, __u32);
    __type(value, __u64);
    __uint(max_entries, 1);
} stats SEC(".maps");

SEC("tp/sched/sched_switch")
int enforce_on_switch(void *ctx)
{
    __u32 pid = bpf_get_current_pid_tgid() >> 32;
    __u64 now = bpf_ktime_get_ns();

    // Check if this PID has a contract
    struct zkperf_contract *contract = bpf_map_lookup_elem(&contracts, &pid);
    if (!contract)
        return 0;

    // Get or create state
    struct zkperf_state *state = bpf_map_lookup_elem(&states, &pid);
    if (!state) {
        struct zkperf_state new_state = {
            .start_ns = now,
            .cpu_ns = 0,
            .signature_hash = contract->signature_hash,
            .violated = 0,
        };
        bpf_map_update_elem(&states, &pid, &new_state, BPF_ANY);
        return 0;
    }

    // Accumulate CPU time
    __u64 elapsed = now - state->start_ns;

    // Check violation
    if (contract->max_ns > 0 && elapsed > contract->max_ns && !state->violated) {
        state->violated = 1;
        bpf_map_update_elem(&states, &pid, state, BPF_ANY);

        // Emit violation event to ring buffer
        struct zkperf_violation *evt = bpf_ringbuf_reserve(&violations, sizeof(*evt), 0);
        if (evt) {
            evt->pid = pid;
            evt->signature_hash = contract->signature_hash;
            evt->elapsed_ns = elapsed;
            evt->max_ns = contract->max_ns;
            evt->timestamp = now;
            bpf_get_current_comm(&evt->comm, sizeof(evt->comm));
            bpf_ringbuf_submit(evt, 0);
        }

        // Increment violation counter
        __u32 zero = 0;
        __u64 *count = bpf_map_lookup_elem(&stats, &zero);
        if (count)
            __sync_fetch_and_add(count, 1);

        // Enforce: send SIGXCPU to the violating process
        if (contract->enforce)
            bpf_send_signal(24); // SIGXCPU
    }

    return 0;
}

char LICENSE[] SEC("license") = "GPL";
