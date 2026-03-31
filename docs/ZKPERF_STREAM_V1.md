# zkperf Stream v1

This document defines the bounded Python lane added for zkperf stream fixtures, remote transport, and register-aware visualization.

## Scope

The Python surface lives under `python/zkperf_stream` and is intentionally separate from the Rust crates. It covers:

- stream fixture loading and bundle construction
- stream latest and index contracts
- HF publish and resolve
- IPFS resolve
- register-aware and flow-aware spectrogram rendering

It does not include SL or ITIR-specific producers.

## Layout

- `python/zkperf_stream/core.py`
- `python/zkperf_stream/index.py`
- `python/zkperf_stream/transport.py`
- `python/zkperf_stream/viz.py`
- `python/zkperf_stream/providers/hf.py`
- `python/zkperf_stream/providers/ipfs.py`

## Contracts

- `zkperf-stream/v1` groups observations into ordered windows and emits a tar bundle with per-window JSON members.
- `zkperf-stream-latest/v1` records the acknowledged upstream revision and latest window.
- `zkperf-stream-index/v1` tracks revision history with a retain-latest-n policy.

## Visualization Surface

The renderer projects observations into generic numeric features. Current prefixes include:

- `reg.<REGISTER>.value`
- `reg.<REGISTER>.fingerprint_code`
- `reg.<REGISTER>.changed`
- `reg.<REGISTER>.delta`
- `flow.tag.<TAG>`
- `flow.region.<REGION>`
- `flow.transition.<SRC>__<DST>`

That surface is intended to absorb newer upstream register work without binding the renderer to any one producer.

## Test Invocation

Run the focused Python suite from repo root:

`PYTHONPATH=python pytest python/tests/test_zkperf_stream.py python/tests/test_zkperf_viz.py`
