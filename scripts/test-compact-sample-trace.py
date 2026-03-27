#!/usr/bin/env python3
"""Regression checks for compact-sample-trace.py."""

from __future__ import annotations

import importlib.util
import json
import tempfile
import unittest
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
SCRIPT_PATH = ROOT / "scripts" / "compact-sample-trace.py"


def _load_module():
    spec = importlib.util.spec_from_file_location("compact_sample_trace", SCRIPT_PATH)
    module = importlib.util.module_from_spec(spec)
    assert spec is not None and spec.loader is not None
    spec.loader.exec_module(module)
    return module


FIXTURE_COMPACT = {
    "trace_kind": "sample_trace_compact/v1",
    "source_trace_kind": "zkperf_sample_trace",
    "source_dir": "/tmp/sample",
    "artifact": "/tmp/sample",
    "template_set": "zkperf_samples:test",
    "shard_family_counts": {"sample": 3, "mmap": 0, "other": 0, "schema": 0},
    "events": ["cycles:P"],
    "rows": [
        {
            "step": 1,
            "event_idx": 0,
            "timestamp": 1000,
            "period": 999999,
            "pid": 27,
            "tid": 27,
            "cpu_mode": "User",
            "cid": "cid-a",
        },
        {
            "step": 2,
            "event_idx": 0,
            "timestamp": 1999,
            "period": 1584892,
            "pid": 27,
            "tid": 27,
            "cpu_mode": "User",
            "cid": "cid-b",
        },
        {
            "step": 3,
            "event_idx": 0,
            "timestamp": 2098,
            "period": 794327,
            "pid": 27,
            "tid": 27,
            "cpu_mode": "Kernel",
            "cid": "cid-c",
        },
    ],
}


class CompactSampleTraceTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls) -> None:
        cls.codec = _load_module()
        cls.fixture = cls.codec.decode_trace(FIXTURE_COMPACT)

    def test_roundtrip_exact(self) -> None:
        compact = self.codec.encode_trace(self.fixture)
        rebuilt = self.codec.decode_trace(compact)
        self.assertEqual(compact, FIXTURE_COMPACT)
        self.assertEqual(rebuilt, self.fixture)

    def test_compact_payload_is_smaller(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            source_path = Path(temp_dir) / "source.json"
            compact_path = Path(temp_dir) / "compact.json"
            roundtrip_path = Path(temp_dir) / "roundtrip.json"
            source_path.write_text(json.dumps(self.fixture, indent=2) + "\n", encoding="utf-8")
            stats = self.codec.compression_stats(source_path, compact_path, roundtrip_path)
            self.assertTrue(stats["roundtrip_equal"])
            self.assertLess(stats["compact_bytes"], stats["source_bytes"])


if __name__ == "__main__":
    unittest.main()
