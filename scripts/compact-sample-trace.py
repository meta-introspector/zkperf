#!/usr/bin/env python3
"""Compact normalized sample-trace JSON by removing deterministic derived fields."""

from __future__ import annotations

import argparse
import json
import math
from pathlib import Path
from typing import Any


REGISTER_LABELS = [
    "idx",
    "log10(period+1)",
    "log10(ts_gap+1)",
    "pid",
    "tid",
    "cpu_mode",
]

CPU_MODE_SIGNAL = {
    "Kernel": -1.0,
    "User": 1.0,
}


def load_json(path: Path) -> dict[str, Any]:
    with path.open("r", encoding="utf-8") as handle:
        return json.load(handle)


def write_json(path: Path, payload: dict[str, Any]) -> None:
    path.write_text(json.dumps(payload, indent=2) + "\n", encoding="utf-8")


def _as_int(value: Any) -> int:
    if isinstance(value, bool):
        raise TypeError(f"unexpected bool where int was required: {value!r}")
    if isinstance(value, int):
        return value
    if isinstance(value, float) and value.is_integer():
        return int(value)
    return int(value)


def _matrix_row(step: int, period: int, ts_gap: int, pid: int, tid: int, cpu_mode: str) -> list[float]:
    return [
        float(step),
        math.log10(period + 1.0) if period else 0.0,
        math.log10(abs(ts_gap) + 1.0) if ts_gap else 0.0,
        float(pid),
        float(tid),
        CPU_MODE_SIGNAL.get(cpu_mode, 0.0),
    ]


def encode_trace(trace: dict[str, Any]) -> dict[str, Any]:
    metadata = trace.get("metadata", {})
    rows: list[dict[str, Any]] = []
    events: list[str] = []
    event_to_index: dict[str, int] = {}

    for annotation in trace.get("step_annotations", []):
        transition = str(annotation.get("transition", ""))
        event_index = event_to_index.setdefault(transition, len(events))
        if event_index == len(events):
            events.append(transition)
        rows.append(
            {
                "step": _as_int(annotation["step"]),
                "event_idx": event_index,
                "timestamp": _as_int(annotation["timestamp"]),
                "period": _as_int(annotation["period"]),
                "pid": _as_int(annotation["next_state"][3]),
                "tid": _as_int(annotation["next_state"][4]),
                "cpu_mode": str(annotation["cpu_mode"]),
                "cid": annotation["cid"],
            }
        )

    return {
        "trace_kind": "sample_trace_compact/v1",
        "source_trace_kind": trace.get("trace_kind"),
        "source_dir": trace.get("source_dir"),
        "artifact": metadata.get("artifact"),
        "template_set": metadata.get("template_set"),
        "shard_family_counts": metadata.get("shard_family_counts", {}),
        "events": events,
        "rows": rows,
    }


def decode_trace(compact: dict[str, Any]) -> dict[str, Any]:
    if compact.get("trace_kind") != "sample_trace_compact/v1":
        raise ValueError(f"unsupported trace_kind: {compact.get('trace_kind')!r}")

    events = list(compact.get("events", []))
    rows = compact.get("rows", [])

    matrix: list[list[float]] = []
    annotations: list[dict[str, Any]] = []
    prev_timestamp: int | None = None

    for row in rows:
        step = _as_int(row["step"])
        timestamp = _as_int(row["timestamp"])
        period = _as_int(row["period"])
        pid = _as_int(row["pid"])
        tid = _as_int(row["tid"])
        cpu_mode = str(row["cpu_mode"])
        ts_gap = 0 if prev_timestamp is None else timestamp - prev_timestamp
        prev_timestamp = timestamp
        transition = events[_as_int(row["event_idx"])]
        matrix_row = _matrix_row(step, period, ts_gap, pid, tid, cpu_mode)
        matrix.append(matrix_row)
        annotations.append(
            {
                "step": step,
                "transition": transition,
                "changed_register_count": 1,
                "changed_registers": ["sample"],
                "changed_register_mask": [True, True, True, True, True, True],
                "delta": matrix_row,
                "l1_step_delta": float(sum(abs(value) for value in matrix_row)),
                "state": None,
                "next_state": matrix_row,
                "cid": row["cid"],
                "cpu_mode": cpu_mode,
                "timestamp": timestamp,
                "period": period,
            }
        )

    return {
        "trace_kind": compact.get("source_trace_kind") or "sample_trace",
        "source_dir": compact.get("source_dir"),
        "register_labels": REGISTER_LABELS,
        "matrix": matrix,
        "metadata": {
            "template_set": compact.get("template_set"),
            "artifact": compact.get("artifact"),
            "register_count": 6,
            "walk_status": "sample_trace",
            "steps": len(matrix),
            "cycle_start": None,
            "final_state": matrix[-1] if matrix else [],
            "best_candidate": None,
            "regime_usage_by_slice": None,
            "shard_family_counts": compact.get("shard_family_counts", {}),
        },
        "step_annotations": annotations,
    }


def compression_stats(source_path: Path, compact_path: Path, roundtrip_path: Path | None = None) -> dict[str, Any]:
    source = load_json(source_path)
    compact = encode_trace(source)
    write_json(compact_path, compact)
    roundtrip = decode_trace(compact)
    if roundtrip_path is not None:
        write_json(roundtrip_path, roundtrip)
    source_bytes = source_path.stat().st_size
    compact_bytes = compact_path.stat().st_size
    return {
        "source": str(source_path),
        "compact": str(compact_path),
        "roundtrip": str(roundtrip_path) if roundtrip_path is not None else None,
        "source_bytes": source_bytes,
        "compact_bytes": compact_bytes,
        "saved_bytes": source_bytes - compact_bytes,
        "reduction_ratio": 0.0 if source_bytes == 0 else (source_bytes - compact_bytes) / source_bytes,
        "row_count": len(compact.get("rows", [])),
        "event_count": len(compact.get("events", [])),
        "roundtrip_equal": roundtrip == source,
    }


def main() -> None:
    parser = argparse.ArgumentParser(description="Compact normalized sample-trace JSON.")
    subparsers = parser.add_subparsers(dest="command", required=True)

    encode_parser = subparsers.add_parser("encode", help="Encode normalized sample trace into compact JSON")
    encode_parser.add_argument("input", type=Path)
    encode_parser.add_argument("output", type=Path)

    decode_parser = subparsers.add_parser("decode", help="Decode compact JSON back into normalized sample trace JSON")
    decode_parser.add_argument("input", type=Path)
    decode_parser.add_argument("output", type=Path)

    stats_parser = subparsers.add_parser("stats", help="Encode a trace and report size + round-trip stats")
    stats_parser.add_argument("input", type=Path)
    stats_parser.add_argument("compact_output", type=Path)
    stats_parser.add_argument("--roundtrip-output", type=Path)

    args = parser.parse_args()

    if args.command == "encode":
        write_json(args.output, encode_trace(load_json(args.input)))
        return

    if args.command == "decode":
        write_json(args.output, decode_trace(load_json(args.input)))
        return

    if args.command == "stats":
        print(json.dumps(compression_stats(args.input, args.compact_output, args.roundtrip_output), indent=2))
        return

    raise AssertionError(f"unhandled command: {args.command}")


if __name__ == "__main__":
    main()
