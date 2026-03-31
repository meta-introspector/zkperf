from __future__ import annotations

import json
from pathlib import Path

from zkperf_stream.viz import (
    build_zkperf_feature_spectrogram_payload,
    project_zkperf_observation_metrics,
    render_zkperf_feature_spectrogram,
    render_zkperf_pca_spectrogram,
    render_zkperf_query_spectrogram,
)


def _fixture(tmp_path: Path) -> Path:
    fixture = {
        "streamId": "zkperf-stream-demo",
        "streamRevision": "rev-demo",
        "windows": [
            {
                "windowId": "window-0001",
                "sequence": 1,
                "payload": {
                    "observations": [
                        {
                            "zkperf_observation_id": "obs-1",
                            "metrics": [
                                {"metric": "summary.covered_count", "value": 1, "unit": "count"},
                                {"metric": "semantic_gap_score", "value": 7, "unit": "semantic_cost"}
                            ],
                            "registers": {"AX": "0x41", "BX": "0x10"},
                            "registerFingerprints": {"AX": "C", "BX": "H"},
                            "registerChanges": ["AX:=0x41", "BX:0x01→0x10", "ip:0x1000→0x1100"],
                            "flowTags": ["INPUT_READ", "ASCII_DATA"],
                            "regionId": "R0",
                            "fromRegion": "R0",
                            "toRegion": "R1"
                        }
                    ]
                }
            },
            {
                "windowId": "window-0002",
                "sequence": 2,
                "payload": {
                    "observations": [
                        {
                            "zkperf_observation_id": "obs-2",
                            "metrics": [
                                {"metric": "summary.covered_count", "value": 2, "unit": "count"},
                                {"metric": "semantic_gap_score", "value": 4, "unit": "semantic_cost"}
                            ],
                            "registers": {"AX": "0x42", "BX": "0x08"},
                            "fingerprint": "LM",
                            "registerOrder": ["AX", "BX"],
                            "registerChanges": [
                                {"register": "AX", "old": "0x41", "new": "0x42"},
                                {"register": "BX", "old": "0x10", "new": "0x08"}
                            ],
                            "flowTags": ["MIX_ROUND", "LOOP_TICK"],
                            "subRegion": "S4"
                        }
                    ]
                }
            }
        ]
    }
    path = tmp_path / "fixture.json"
    path.write_text(json.dumps(fixture), encoding="utf-8")
    return path


def test_build_feature_spectrogram_payload(tmp_path: Path) -> None:
    fixture = json.loads(_fixture(tmp_path).read_text(encoding="utf-8"))
    payload = build_zkperf_feature_spectrogram_payload(
        fixture,
        top_k_features=20,
        feature_prefixes=["summary", "semantic_gap_score", "reg", "flow"],
    )
    assert payload["streamId"] == "zkperf-stream-demo"
    assert "semantic_gap_score" in payload["featureNames"]
    assert "reg.AX.value" in payload["featureNames"]
    assert "flow.tag.MIX_ROUND" in payload["featureNames"]


def test_project_register_and_flow_metrics() -> None:
    payload = project_zkperf_observation_metrics(
        {
            "metrics": [{"metric": "summary.covered_count", "value": 3}],
            "registers": {"AX": "0x41"},
            "registerFingerprints": {"AX": "C"},
            "registerChanges": ["AX:0x40→0x41", "ip:0x1000→0x1010"],
            "flowTags": ["ASCII_DATA", "INPUT_READ"],
            "regionId": "R0",
            "fromRegion": "R0",
            "toRegion": "R1",
        }
    )
    assert payload["summary.covered_count"] == 3.0
    assert payload["reg.AX.value"] == 65.0
    assert payload["reg.AX.fingerprint_code"] == 0.0
    assert payload["reg.AX.changed"] == 1.0
    assert payload["reg.AX.delta"] == 1.0
    assert payload["flow.tag.ASCII_DATA"] == 1.0
    assert payload["flow.region.R0"] == 1.0
    assert payload["flow.transition.R0__R1"] == 1.0


def test_query_spectrogram_keeps_sparse_negative_features(tmp_path: Path) -> None:
    fixture = {
        "streamId": "zkperf-stream-demo",
        "streamRevision": "rev-demo",
        "windows": [
            {"windowId": "window-0001", "sequence": 1, "payload": {"observations": [{"zkperf_observation_id": "obs-1", "metrics": [{"metric": "neg_score", "value": -3.0}, {"metric": "shared", "value": 2.0}]}]}},
            {"windowId": "window-0002", "sequence": 2, "payload": {"observations": [{"zkperf_observation_id": "obs-2", "metrics": [{"metric": "shared", "value": 1.0}]}]}},
        ],
    }
    feature_payload = build_zkperf_feature_spectrogram_payload(
        fixture,
        top_k_features=4,
        feature_prefixes=["neg_score", "shared"],
    )
    assert "neg_score" in feature_payload["featureNames"]
    neg_col = feature_payload["featureNames"].index("neg_score")
    assert feature_payload["matrix"][0][neg_col] < 0

    payload = render_zkperf_query_spectrogram(
        fixture,
        output_path=tmp_path / "query_sparse.png",
        query_metrics={"neg_score": -3.0, "shared": 2.0},
    )
    assert "neg_score" in payload["queryFeatureNames"]
    assert len(payload["scores"]) == 2


def test_render_feature_pca_and_query_spectrograms(tmp_path: Path) -> None:
    fixture = json.loads(_fixture(tmp_path).read_text(encoding="utf-8"))
    feature_payload = render_zkperf_feature_spectrogram(
        fixture,
        output_path=tmp_path / "feature.png",
        metadata_path=tmp_path / "feature.json",
        top_k_features=20,
        cluster_k=2,
    )
    pca_payload = render_zkperf_pca_spectrogram(
        fixture,
        output_path=tmp_path / "pca.png",
        metadata_path=tmp_path / "pca.json",
        top_k_features=20,
        components=2,
        cluster_k=2,
    )
    query_payload = render_zkperf_query_spectrogram(
        fixture,
        output_path=tmp_path / "query.png",
        metadata_path=tmp_path / "query.json",
        query_metrics={"summary.covered_count": 1.0, "reg.AX.value": 65.0, "flow.tag.INPUT_READ": 1.0},
    )
    assert feature_payload["featureCount"] >= 1
    assert feature_payload["clusterCounts"]
    assert pca_payload["componentLabels"] == ["PC1", "PC2"]
    assert pca_payload["clusterCounts"]
    assert query_payload["queryFeatureNames"]
    assert len(query_payload["scores"]) == len(query_payload["rowLabels"])
