from __future__ import annotations

import io
import json
import tarfile
from pathlib import Path

from zkperf_stream import (
    apply_zkperf_stream_retention_policy,
    build_zkperf_stream_bundle,
    build_zkperf_stream_index,
    get_zkperf_stream_index_record,
    load_remote_zkperf_stream_index_ipfs,
    load_zkperf_stream_fixture,
    publish_zkperf_stream_index_to_hf,
    publish_zkperf_stream_to_hf,
    resolve_remote_zkperf_stream_window,
    resolve_remote_zkperf_stream_window_ipfs,
    resolve_remote_zkperf_stream_windows,
    resolve_remote_zkperf_stream_windows_ipfs,
    resolve_zkperf_stream_from_index_hf,
    resolve_zkperf_stream_from_index_ipfs,
    update_zkperf_stream_index,
)

FIXTURE = Path(__file__).parent / "fixtures" / "zkperf_stream_v1.example.json"


def test_build_zkperf_stream_bundle() -> None:
    fixture = load_zkperf_stream_fixture(FIXTURE)
    bundle = build_zkperf_stream_bundle(fixture)
    assert bundle["streamManifest"]["streamId"] == "zkperf-stream-demo"
    assert len(bundle["streamManifest"]["windows"]) == 2
    assert bundle["streamManifest"]["latestWindowId"] == "window-0002"
    assert bundle["streamManifest"]["observationCount"] == 2
    assert bundle["tarDigest"]
    with tarfile.open(fileobj=io.BytesIO(bundle["tarBytes"]), mode="r:*") as handle:
        payload = json.loads(handle.extractfile("windows/window-0001.json").read().decode("utf-8"))
    metric_keys = {item.get("metric") or item.get("name") for item in payload["observations"][0]["metrics"]}
    assert "stream_window_count" in metric_keys
    assert "stream_observation_count" in metric_keys
    assert "window_observation_count" in metric_keys
    assert "window_sequence" in metric_keys


def test_publish_zkperf_stream_to_hf(monkeypatch) -> None:
    monkeypatch.setattr(
        "zkperf_stream.transport.upload_hf_file_with_ack",
        lambda **kwargs: {
            "acknowledgedRevision": "rev-demo",
            "localSha256": build_zkperf_stream_bundle(load_zkperf_stream_fixture(FIXTURE))["tarDigest"],
            "localSizeBytes": len(build_zkperf_stream_bundle(load_zkperf_stream_fixture(FIXTURE))["tarBytes"]),
            "hfUri": kwargs["hf_uri"],
            "fetch": {"statusCode": 200},
            "verified": True,
        },
    )
    output = publish_zkperf_stream_to_hf(
        fixture_path=FIXTURE,
        hf_uri="hf://datasets/chbwa/itir-zos-ack-probe/zkperf-stream/zkperf-stream-demo.tar",
        commit_message="demo",
    )
    assert output["hfReceipt"]["verified"] is True
    assert output["streamManifest"]["containerObjectRef"]["uri"].startswith("hf://datasets/chbwa/")
    assert output["streamLatest"]["latestWindowId"] == "window-0002"
    assert output["timings"]["hfPublishMs"] >= 0


def test_update_zkperf_stream_index_and_retention() -> None:
    fixture = load_zkperf_stream_fixture(FIXTURE)
    bundle = build_zkperf_stream_bundle(fixture)
    index = update_zkperf_stream_index(
        existing_index=build_zkperf_stream_index(
            stream_id="zkperf-stream-demo",
            index_hf_uri="hf://datasets/chbwa/itir-zos-ack-probe/zkperf-stream/zkperf-stream-demo.index.json",
            created_at="2026-03-30T10:00:00Z",
        ),
        stream_manifest=bundle["streamManifest"],
        hf_receipt={"acknowledgedRevision": "rev-demo", "verified": True},
        index_hf_uri="hf://datasets/chbwa/itir-zos-ack-probe/zkperf-stream/zkperf-stream-demo.index.json",
    )
    assert index["latestRevision"] == "rev-20260330-a"
    assert index["revisions"][0]["latestWindowId"] == "window-0002"
    kept = apply_zkperf_stream_retention_policy(
        [{"streamRevision": "rev-a"}, {"streamRevision": "rev-b"}, {"streamRevision": "rev-c"}],
        {"policyVersion": "zkperf-retention/v1", "mode": "retain-latest-n", "maxRevisionCount": 2},
    )
    assert [item["streamRevision"] for item in kept] == ["rev-b", "rev-c"]


def test_publish_zkperf_stream_index_to_hf(monkeypatch) -> None:
    monkeypatch.setattr(
        "zkperf_stream.transport.upload_hf_file_with_ack",
        lambda **kwargs: {
            "acknowledgedRevision": "rev-index",
            "localSha256": "abc",
            "localSizeBytes": 123,
            "hfUri": kwargs["hf_uri"],
            "fetch": {"statusCode": 200},
            "verified": True,
        },
    )
    receipt = publish_zkperf_stream_index_to_hf(
        stream_index={"streamId": "zkperf-stream-demo", "latestRevision": "rev-20260330-a"},
        index_hf_uri="hf://datasets/chbwa/itir-zos-ack-probe/zkperf-stream/zkperf-stream-demo.index.json",
    )
    assert receipt["verified"] is True


def test_get_zkperf_stream_index_record_latest() -> None:
    record = get_zkperf_stream_index_record(
        {"latestRevision": "rev-b", "revisions": [{"streamRevision": "rev-a"}, {"streamRevision": "rev-b"}]},
        latest=True,
    )
    assert record["streamRevision"] == "rev-b"


def test_remote_window_resolution(monkeypatch) -> None:
    fixture = load_zkperf_stream_fixture(FIXTURE)
    bundle = build_zkperf_stream_bundle(fixture)
    monkeypatch.setattr(
        "zkperf_stream.transport.download_hf_object_bytes",
        lambda **kwargs: {"bytes": bundle["tarBytes"], "metadata": {"statusCode": 200, "revision": kwargs["revision"]}},
    )
    payload = resolve_remote_zkperf_stream_window(
        stream_manifest=bundle["streamManifest"],
        hf_revision="rev-demo",
        window_id="window-0001",
    )
    assert payload["window"]["windowId"] == "window-0001"
    windows_payload = resolve_remote_zkperf_stream_windows(
        stream_manifest=bundle["streamManifest"],
        hf_revision="rev-demo",
        latest=True,
    )
    assert windows_payload["selection"]["selectedWindowIds"] == ["window-0002"]


def test_remote_window_resolution_ipfs(monkeypatch) -> None:
    fixture = load_zkperf_stream_fixture(FIXTURE)
    bundle = build_zkperf_stream_bundle(fixture)
    monkeypatch.setattr(
        "zkperf_stream.transport.download_ipfs_object_bytes",
        lambda **kwargs: {"bytes": bundle["tarBytes"], "metadata": {"statusCode": 200}},
    )
    payload = resolve_remote_zkperf_stream_window_ipfs(
        stream_manifest=bundle["streamManifest"],
        window_id="window-0001",
    )
    assert payload["fetch"]["sink"] == "ipfs"
    windows_payload = resolve_remote_zkperf_stream_windows_ipfs(
        stream_manifest=bundle["streamManifest"],
        latest=True,
    )
    assert windows_payload["selection"]["selectedWindowIds"] == ["window-0002"]


def test_resolve_from_hf_and_ipfs_index(monkeypatch, tmp_path: Path) -> None:
    fixture = load_zkperf_stream_fixture(FIXTURE)
    bundle = build_zkperf_stream_bundle(fixture)
    index_payload = {
        "latestRevision": "rev-20260330-a",
        "revisions": [
            {
                "streamRevision": "rev-20260330-a",
                "acknowledgedRevision": "ack-demo",
                "windowCount": bundle["streamManifest"]["windowCount"],
                "latestWindowId": bundle["streamManifest"]["latestWindowId"],
                "sequenceRange": bundle["streamManifest"]["sequenceRange"],
                "windows": bundle["streamManifest"]["windows"],
                "containerObjectRef": {"sink": "hf", "uri": "hf://datasets/chbwa/demo/zkperf-stream-demo.tar"},
            }
        ],
    }
    fixture_copy = tmp_path / "fixture.json"
    fixture_copy.write_text(json.dumps(fixture), encoding="utf-8")
    monkeypatch.setattr("zkperf_stream.transport.load_remote_zkperf_stream_index", lambda *args, **kwargs: index_payload)
    monkeypatch.setattr("zkperf_stream.transport.load_remote_zkperf_stream_index_ipfs", lambda *args, **kwargs: index_payload)
    monkeypatch.setattr(
        "zkperf_stream.transport.download_hf_object_bytes",
        lambda **kwargs: {"bytes": bundle["tarBytes"], "metadata": {"statusCode": 200, "revision": kwargs["revision"]}},
    )
    monkeypatch.setattr(
        "zkperf_stream.transport.download_ipfs_object_bytes",
        lambda **kwargs: {"bytes": bundle["tarBytes"], "metadata": {"statusCode": 200}},
    )
    resolved_hf = resolve_zkperf_stream_from_index_hf(
        fixture_path=fixture_copy,
        index_hf_uri="hf://datasets/chbwa/demo/zkperf-stream-demo.index.json",
        latest=True,
    )
    assert resolved_hf["streamIndex"]["resolvedStreamRevision"] == "rev-20260330-a"
    resolved_ipfs = resolve_zkperf_stream_from_index_ipfs(
        fixture_path=fixture_copy,
        index_ipfs_uri="ipfs://bafy-demo/zkperf-stream-demo.index.json",
        latest=True,
    )
    assert resolved_ipfs["streamIndex"]["resolvedStreamRevision"] == "rev-20260330-a"


def test_load_remote_zkperf_stream_index_ipfs(monkeypatch) -> None:
    monkeypatch.setattr(
        "zkperf_stream.transport.fetch_ipfs_object",
        lambda **kwargs: {"text": json.dumps({"latestRevision": "rev-demo"})},
    )
    payload = load_remote_zkperf_stream_index_ipfs("ipfs://bafy-demo/index.json")
    assert payload["latestRevision"] == "rev-demo"
