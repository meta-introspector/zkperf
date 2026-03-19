#!/usr/bin/env python3
"""zkperf-chain.py — Build a full provenance chain for a perf witness.

A reproducible perf witness requires:
  1. nix package of binaries
  2. source + debug symbols for those binaries
  3. traces produced with the binaries
  4. models created from the traces
  5. events leading up to binary creation

This script assembles and verifies the chain.
"""
import hashlib, json, subprocess, sys, os
from datetime import datetime, timezone

def sha256(data):
    return hashlib.sha256(data if isinstance(data, bytes) else data.encode()).hexdigest()

def nix_closure(drv_or_path):
    """Get the full nix closure (all dependencies) for a store path."""
    try:
        out = subprocess.check_output(
            ["nix-store", "--query", "--requisites", drv_or_path],
            stderr=subprocess.DEVNULL, text=True
        )
        return [p.strip() for p in out.strip().splitlines() if p.strip()]
    except Exception:
        return [drv_or_path]

def debug_info(store_path):
    """Find debug symbols for a store path."""
    debug_dir = os.path.expanduser(f"~/.debug{store_path}")
    if os.path.isdir(debug_dir):
        return {"path": debug_dir, "available": True}
    return {"path": debug_dir, "available": False}

def perf_stat(cmd):
    """Run perf stat and capture counters."""
    stat_file = "/tmp/zkperf_stat.txt"
    try:
        subprocess.run(
            ["perf", "stat", "-e", "cycles,instructions,cache-misses",
             "-o", stat_file, "--"] + cmd,
            capture_output=True, timeout=30
        )
        with open(stat_file) as f:
            return f.read()
    except Exception as e:
        return f"perf unavailable: {e}"

def build_chain(binary_path, source_path=None, perf_data=None):
    """Assemble the full witness chain."""
    chain = {
        "timestamp": datetime.now(timezone.utc).isoformat(),
        "version": "0.1.0",
        "layers": {}
    }

    # Layer 1: Binary package
    closure = nix_closure(binary_path)
    chain["layers"]["1_binaries"] = {
        "path": binary_path,
        "closure_size": len(closure),
        "closure_hash": sha256("\n".join(sorted(closure))),
        "paths": closure[:10],  # first 10 for brevity
    }

    # Layer 2: Source + debug symbols
    dbg = debug_info(binary_path)
    chain["layers"]["2_source_debug"] = {
        "source": source_path or "unknown",
        "debug_symbols": dbg,
        "source_hash": sha256(open(source_path, "rb").read()) if source_path and os.path.isfile(source_path) else None,
    }

    # Layer 3: Traces
    if perf_data and os.path.isfile(perf_data):
        with open(perf_data, "rb") as f:
            data = f.read()
        chain["layers"]["3_traces"] = {
            "perf_data": perf_data,
            "size": len(data),
            "hash": sha256(data),
        }
    else:
        chain["layers"]["3_traces"] = {"perf_data": None, "note": "no recording provided"}

    # Layer 4: Model (hash of all previous layers)
    model_input = json.dumps(chain["layers"], sort_keys=True)
    chain["layers"]["4_model"] = {
        "chain_hash": sha256(model_input),
        "layer_count": len(chain["layers"]),
    }

    # Layer 5: Events / provenance
    chain["layers"]["5_events"] = {
        "builder": os.environ.get("USER", "unknown"),
        "hostname": os.uname().nodename,
        "cwd": os.getcwd(),
    }

    # Final commitment
    chain["commitment"] = sha256(json.dumps(chain["layers"], sort_keys=True))
    return chain

if __name__ == "__main__":
    binary = sys.argv[1] if len(sys.argv) > 1 else "/nix/store/slnid5pk8zci6xvszn4y306wpzhbvpyy-state-4-zkperf"
    source = sys.argv[2] if len(sys.argv) > 2 else "src/witness.rs"
    perf = sys.argv[3] if len(sys.argv) > 3 else "recordings/rust_actual.perf.data"

    chain = build_chain(binary, source, perf)
    print(json.dumps(chain, indent=2, default=str))

    # Save
    os.makedirs("proofs", exist_ok=True)
    with open("proofs/witness-chain.json", "w") as f:
        json.dump(chain, f, indent=2, default=str)
    print(f"\n✅ Chain commitment: {chain['commitment']}", file=sys.stderr)
